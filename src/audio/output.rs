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
pub(super) fn run(rx: mpsc::Receiver<Play>, running: Arc<AtomicBool>, active: Arc<AtomicBool>) {
    let bank = super::synth::bank();
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
        let mut voices: Vec<Voice> = Vec::with_capacity(24);
        let mut failed = false;
        while running.load(Ordering::Relaxed) && !failed {
            let enabled = active.load(Ordering::Relaxed);
            if !enabled {
                voices.clear();
            }
            while let Ok(play) = rx.try_recv() {
                if enabled && voices.len() < 24 {
                    voices.push(Voice { play, cursor: 0.0 });
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
                for frame in buffer.data.chunks_exact_mut(2) {
                    let mut stereo = [0.0f32; 2];
                    gain += (target - gain) * 0.002;
                    for voice in &mut voices {
                        let sample = &bank[voice.play.cue as usize];
                        let i = voice.cursor as usize;
                        if i + 1 < sample.len() {
                            let value =
                                sample[i] + (sample[i + 1] - sample[i]) * voice.cursor.fract();
                            stereo[0] += value * voice.play.left;
                            stereo[1] += value * voice.play.right;
                            voice.cursor += voice.play.rate;
                        } else {
                            voice.cursor = sample.len() as f32;
                        }
                    }
                    for ch in 0..2 {
                        frame[ch] = (stereo[ch].tanh() * gain * 28000.0) as i16;
                    }
                }
                voices.retain(|v| (v.cursor as usize) < bank[v.play.cue as usize].len());
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
