//! WinMM owns only stable boxed buffers on this worker thread. Headers are
//! unprepared before memory is accessed again; reset returns queued blocks on exit.
use super::*;
use std::{
    ffi::{c_char, c_void},
    mem::size_of,
    ptr, thread,
    time::Duration,
};
#[repr(C, packed(1))]
struct Format {
    tag: u16,
    channels: u16,
    rate: u32,
    bytes_sec: u32,
    align: u16,
    bits: u16,
    extra: u16,
}
#[repr(C)]
struct Header {
    data: *mut c_char,
    len: u32,
    recorded: u32,
    user: usize,
    flags: u32,
    loops: u32,
    next: *mut Header,
    reserved: usize,
}
#[link(name = "winmm")]
unsafe extern "system" {
    fn waveOutOpen(
        handle: *mut *mut c_void,
        id: u32,
        format: *const Format,
        callback: usize,
        instance: usize,
        flags: u32,
    ) -> u32;
    fn waveOutPrepareHeader(handle: *mut c_void, header: *mut Header, size: u32) -> u32;
    fn waveOutUnprepareHeader(handle: *mut c_void, header: *mut Header, size: u32) -> u32;
    fn waveOutWrite(handle: *mut c_void, header: *mut Header, size: u32) -> u32;
    fn waveOutReset(handle: *mut c_void) -> u32;
    fn waveOutClose(handle: *mut c_void) -> u32;
}
struct Buffer {
    data: Box<[i16; 1024]>,
    header: Box<Header>,
    prepared: bool,
}
impl Buffer {
    fn new() -> Self {
        let mut data = Box::new([0; 1024]);
        let header = Box::new(Header {
            data: data.as_mut_ptr().cast(),
            len: 2048,
            recorded: 0,
            user: 0,
            flags: 0,
            loops: 0,
            next: ptr::null_mut(),
            reserved: 0,
        });
        Self {
            data,
            header,
            prepared: false,
        }
    }
}
struct Voice {
    play: Play,
    cursor: f32,
}

/// Repeated voices share a gently normalized bus.
fn equal_cue_gain(count: u32) -> f32 {
    1.0 / count.max(1) as f32
}

/// A tiny ping-pong delay with a damped feedback path. It is not a full
/// convolution room; it gives rifts, impacts and blooms a believable short
/// space without adding a dependency or a large allocation.
struct Reverb {
    left: Vec<f32>,
    right: Vec<f32>,
    index: usize,
    low_l: f32,
    low_r: f32,
}

impl Reverb {
    fn new(rate: u32) -> Self {
        let len = ((rate as f32 * 0.071) as usize).max(256);
        Self {
            left: vec![0.0; len],
            right: vec![0.0; len],
            index: 0,
            low_l: 0.0,
            low_r: 0.0,
        }
    }

    fn process(&mut self, mono: f32, send: f32) -> (f32, f32) {
        let i = self.index;
        let dl = self.left[i];
        let dr = self.right[i];
        // Damp the feedback path so the tail settles instead of ringing.
        self.low_l += (dl - self.low_l) * 0.32;
        self.low_r += (dr - self.low_r) * 0.32;
        let wet_l = self.low_l;
        let wet_r = self.low_r;
        let input = mono * send;
        self.left[i] = input + wet_r * 0.34;
        self.right[i] = input + wet_l * 0.34;
        self.index = (i + 1) % self.left.len();
        (wet_l * 0.22, wet_r * 0.22)
    }
}
pub(super) fn run(rx: mpsc::Receiver<Play>, running: Arc<AtomicBool>, active: Arc<AtomicBool>) {
    let bank = super::synth::variants();
    let mut gain = 0.0;
    while running.load(Ordering::Relaxed) {
        let format = Format {
            tag: 1,
            channels: 2,
            rate: synth::RATE,
            bytes_sec: synth::RATE * 4,
            align: 4,
            bits: 16,
            extra: 0,
        };
        let mut device = ptr::null_mut();
        // WAVE_MAPPER, CALLBACK_NULL: no foreign callback or cross-thread pointer.
        if unsafe { waveOutOpen(&mut device, u32::MAX, &format, 0, 0, 0) } != 0 {
            STATUS.store(2, Ordering::Relaxed);
            for _ in 0..100 {
                while rx.try_recv().is_ok() {}
                if !running.load(Ordering::Relaxed) {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            continue;
        }
        STATUS.store(1, Ordering::Relaxed);
        let mut buffers: Vec<_> = (0..3).map(|_| Buffer::new()).collect();
        let mut voices: Vec<Voice> = Vec::with_capacity(16);
        let mut reverb = Reverb::new(synth::RATE);
        let mut failed = false;
        while running.load(Ordering::Relaxed) && !failed {
            let enabled = active.load(Ordering::Relaxed);
            if !enabled {
                voices.clear();
            }
            while let Ok(play) = rx.try_recv() {
                if enabled {
                    // Bound both same-cue repeats and total voices so dense
                    // combat cannot turn into a crunchy wall of sound.
                    let cue = play.cue as usize;
                    let same = voices.iter().filter(|v| v.play.cue as usize == cue).count();
                    if same >= 4 {
                        // Replace the voice furthest into its sample; it is the
                        // closest to a natural release and least likely to
                        // click, while rapid fire remains audible.
                        if let Some(index) = voices
                            .iter()
                            .enumerate()
                            .filter(|(_, v)| v.play.cue as usize == cue)
                            .max_by(|(_, a), (_, b)| a.cursor.total_cmp(&b.cursor))
                            .map(|(index, _)| index)
                        {
                            voices.remove(index);
                        }
                    }
                    if voices.len() < 16 {
                        voices.push(Voice { play, cursor: 0.0 });
                    } else if let Some(index) = voices
                        .iter()
                        .enumerate()
                        .max_by(|(_, a), (_, b)| a.cursor.total_cmp(&b.cursor))
                        .map(|(index, _)| index)
                    {
                        // Under cross-cue overload, retire the most finished
                        // tail before admitting a new high-priority onset.
                        voices.remove(index);
                        voices.push(Voice { play, cursor: 0.0 });
                    }
                }
            }
            for buffer in &mut buffers {
                if buffer.prepared {
                    let result = unsafe {
                        waveOutUnprepareHeader(
                            device,
                            &mut *buffer.header,
                            size_of::<Header>() as u32,
                        )
                    };
                    if result == 33 {
                        continue;
                    } // WAVERR_STILLPLAYING; buffer remains device-owned.
                    if result != 0 {
                        failed = true;
                        break;
                    }
                    buffer.prepared = false;
                }
                let target = if enabled { volume() } else { 0.0 };
                let send = voices
                    .iter()
                    .map(|voice| voice.play.reverb)
                    .sum::<f32>()
                    .min(0.42);
                for frame in buffer.data.chunks_exact_mut(2) {
                    let mut stereo = [0.0f32; 2];
                    let mut cue_counts = [0u32; Cue::COUNT];
                    let mut active_count = 0u32;
                    gain += (target - gain) * 0.002;
                    for voice in &mut voices {
                        let cue = voice.play.cue as usize;
                        let sample = &bank[cue][voice.play.variant];
                        let i = voice.cursor as usize;
                        if i + 1 < sample.len() {
                            active_count += 1;
                            cue_counts[cue] += 1;
                            let value =
                                sample[i] + (sample[i + 1] - sample[i]) * voice.cursor.fract();
                            // Equal-cue voices share one bus so repeated
                            // Sparks do not grow linearly in loudness.
                            let norm = equal_cue_gain(cue_counts[cue]);
                            stereo[0] += value * voice.play.left * norm;
                            stereo[1] += value * voice.play.right * norm;
                            voice.cursor += voice.play.rate;
                        } else {
                            voice.cursor = sample.len() as f32;
                        }
                    }
                    let (wet_l, wet_r) = reverb.process((stereo[0] + stereo[1]) * 0.5, send);
                    stereo[0] += wet_l;
                    stereo[1] += wet_r;
                    // Recompute from voices that are actually sounding this
                    // frame so quiet tails don't suppress a new cast.
                    let bus_gain = (1.0 / (active_count.max(1) as f32).sqrt()).max(0.42);
                    for ch in 0..2 {
                        let limited = stereo[ch] * bus_gain;
                        // Cubic soft knee is nearly linear at ordinary levels
                        // and rolls off peaks without the saturated tanh buzz.
                        let abs = limited.abs();
                        let shaped = if abs <= 0.55 {
                            limited
                        } else {
                            limited.signum() * (0.55 + 0.40 * (1.0 - (-((abs - 0.55) / 0.40)).exp()))
                        };
                        frame[ch] = (shaped * gain * 24000.0) as i16;
                    }
                }
                voices.retain(|v| {
                    (v.cursor as usize) < bank[v.play.cue as usize][v.play.variant].len()
                });
                buffer.header.flags = 0;
                if unsafe {
                    waveOutPrepareHeader(device, &mut *buffer.header, size_of::<Header>() as u32)
                } != 0
                {
                    failed = true;
                    break;
                }
                buffer.prepared = true;
                if unsafe { waveOutWrite(device, &mut *buffer.header, size_of::<Header>() as u32) }
                    != 0
                {
                    failed = true;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(2));
        }
        // A driver failure must never free memory it might still own.
        if unsafe { waveOutReset(device) } == 0 {
            for mut buffer in buffers {
                if buffer.prepared
                    && unsafe {
                        waveOutUnprepareHeader(
                            device,
                            &mut *buffer.header,
                            size_of::<Header>() as u32,
                        )
                    } != 0
                {
                    std::mem::forget(buffer);
                }
            }
            unsafe {
                waveOutClose(device);
            }
        } else {
            std::mem::forget(buffers);
        }
        STATUS.store(2, Ordering::Relaxed);
        for _ in 0..50 {
            if !running.load(Ordering::Relaxed) {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_cue_voices_share_one_normalized_bus() {
        assert_eq!(equal_cue_gain(0), 1.0);
        assert_eq!(equal_cue_gain(1), 1.0);
        assert_eq!(equal_cue_gain(4), 0.5);
        assert!(equal_cue_gain(16) < equal_cue_gain(2));
    }

    #[test]
    fn reverb_keeps_a_damped_tail_after_the_input_stops() {
        let mut reverb = Reverb::new(synth::RATE);
        let delay = reverb.left.len();
        reverb.process(0.5, 1.0);
        let mut early_peak = 0.0f32;
        for _ in 0..delay * 4 {
            let (left, right) = reverb.process(0.0, 0.0);
            early_peak = early_peak.max(left.abs()).max(right.abs());
        }
        let mut late_peak = 0.0f32;
        for _ in 0..delay * 24 {
            let (left, right) = reverb.process(0.0, 0.0);
            late_peak = late_peak.max(left.abs()).max(right.abs());
        }
        assert!(early_peak > 0.0, "the impulse should produce a wet tail");
        assert!(
            late_peak < early_peak,
            "feedback must decay, not ring forever"
        );
    }
}
