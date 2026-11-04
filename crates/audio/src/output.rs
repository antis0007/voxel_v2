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
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThread() -> *mut c_void;
    fn SetThreadPriority(thread: *mut c_void, priority: i32) -> i32;
}
/// The mixer sleeps between short bursts, so the top priority cannot starve
/// other threads; at HIGHEST it still missed its deadline during the startup
/// burst of horizon and shader work.
const THREAD_PRIORITY_TIME_CRITICAL: i32 = 15;
/// Queued output blocks of 512 frames; five hold about 53 ms at 48 kHz.
const BUFFERS: usize = 5;
/// Overall mix level. It sits under the final knee, so ordinary stacks of
/// beds, steps and cues stay in its linear range instead of saturating.
const MASTER: f32 = 0.5;

/// Holds the 1 ms system timer for the mixer's lifetime. Without it the 2 ms
/// sleep rounds up to the 15.6 ms default tick, leaving the queued blocks a
/// few milliseconds from an underrun whenever the game loads every core;
/// underruns are heard as crackle.
struct TimerPeriod;
impl TimerPeriod {
    fn begin() -> Self {
        unsafe {
            timeBeginPeriod(1);
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
        }
        Self
    }
}
impl Drop for TimerPeriod {
    fn drop(&mut self) {
        unsafe {
            timeEndPeriod(1);
        }
    }
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

/// Repeated voices share one equal-power bus: n copies of a cue sum to
/// sqrt(n) loudness, not n, and not a flat 1.
fn equal_cue_gain(count: u32) -> f32 {
    1.0 / (count.max(1) as f32).sqrt()
}

/// Linear to 0.55, then rolls off toward 0.95. The slope is continuous at the
/// knee, so peaks bend instead of hitting the i16 cast.
fn soft_knee(value: f32) -> f32 {
    let abs = value.abs();
    if abs <= 0.55 {
        value
    } else {
        value.signum() * (0.55 + 0.40 * (1.0 - (-((abs - 0.55) / 0.40)).exp()))
    }
}

pub(super) fn run(
    rx: mpsc::Receiver<Play>,
    layer_rx: mpsc::Receiver<layers::Command>,
    running: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
) {
    let _timer = TimerPeriod::begin();
    let bank = super::synth::variants();
    let mut layers = layers::Layers::new(synth::RATE);
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
                layers.receive(&layer_rx);
                if !running.load(Ordering::Relaxed) {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            continue;
        }
        STATUS.store(1, Ordering::Relaxed);
        let mut buffers: Vec<_> = (0..BUFFERS).map(|_| Buffer::new()).collect();
        let mut voices: Vec<Voice> = Vec::with_capacity(16);
        let mut reverb = crate::reverb::Reverb::new(synth::RATE);
        let mut muffle = crate::dsp::Muffle::new();
        let mut tap = tap::Tap::from_env();
        let mut started = false;
        let mut slept = std::time::Instant::now();
        let mut failed = false;
        while running.load(Ordering::Relaxed) && !failed {
            let enabled = active.load(Ordering::Relaxed);
            if !enabled {
                voices.clear();
            }
            layers.begin(enabled);
            layers.receive(&layer_rx);
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
            let woke = std::time::Instant::now();
            let mut refilled = 0;
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
                // About one second to settle into a cave or back out to air.
                reverb.approach(layers::space(), 0.012);
                muffle.approach(layers::clarity(), 0.012, synth::RATE as f32);
                for frame in buffer.data.chunks_exact_mut(2) {
                    let mut stereo = [0.0f32; 2];
                    let mut cue_counts = [0u32; Cue::COUNT];
                    let mut active_count = 0u32;
                    let mut cue_send = 0.0f32;
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
                            cue_send += value
                                * (voice.play.left + voice.play.right)
                                * 0.5
                                * norm
                                * voice.play.reverb;
                            voice.cursor += voice.play.rate;
                        } else {
                            voice.cursor = sample.len() as f32;
                        }
                    }
                    let (layered, layer_send) = layers.frame();
                    // Recompute from voices that are actually sounding this
                    // frame so quiet tails don't suppress a new cast.
                    let bus_gain = (1.0 / (active_count.max(1) as f32).sqrt()).max(0.42);
                    // Vacuum has no room to ring; the layers already scale their own send.
                    let wet = reverb.process(cue_send * bus_gain * gain * muffle.air() + layer_send);
                    let stereo = muffle.process(stereo);
                    for ch in 0..2 {
                        let limited = stereo[ch] * bus_gain;
                        // Layers carry their own volumes, but they sum without a
                        // limit. The knee must run on the final sum: a loud bed
                        // plus footsteps past the i16 cast clips hard, which is
                        // the crunch.
                        let sum = (soft_knee(limited) * gain + layered[ch] + wet[ch]) * MASTER;
                        frame[ch] = (soft_knee(sum) * 24000.0) as i16;
                    }
                    if let Some(tap) = tap.as_mut() {
                        tap.push([frame[0], frame[1]]);
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
                refilled += 1;
                if unsafe { waveOutWrite(device, &mut *buffer.header, size_of::<Header>() as u32) }
                    != 0
                {
                    failed = true;
                    break;
                }
            }
            // Every block came back finished: the device drained the queue
            // before this pass, which is heard as a gap.
            if started && refilled == BUFFERS {
                UNDERRUNS.fetch_add(1, Ordering::Relaxed);
            }
            if started {
                let gap = woke.duration_since(slept).as_micros() as u32;
                let mix = woke.elapsed().as_micros() as u32;
                LONGEST_GAP_US.fetch_max(gap, Ordering::Relaxed);
                LONGEST_MIX_US.fetch_max(mix, Ordering::Relaxed);
            }
            started = true;
            slept = std::time::Instant::now();
            thread::sleep(Duration::from_millis(2));
        }
        if let Some(tap) = tap.as_mut() {
            tap.flush();
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
    fn soft_knee_is_linear_below_the_knee_and_bounded_above() {
        for step in 0..=55 {
            let x = step as f32 * 0.01;
            assert_eq!(soft_knee(x), x);
            assert_eq!(soft_knee(-x), -x);
        }
        let mut last = soft_knee(0.55);
        for step in 1..=400 {
            let x = 0.55 + step as f32 * 0.01;
            let y = soft_knee(x);
            assert!(y >= last, "soft knee must not fold back at {x}");
            assert!(y < 0.951, "soft knee output {y} escaped its ceiling");
            last = y;
        }
        assert!(soft_knee(4.0) < 0.951);
    }
}
