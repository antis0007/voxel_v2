//! Bounded, asynchronous combat audio. Original synthesized Foley lives in memory;
//! no network assets, file decoding, or device calls run on the rendering thread.
#[cfg(windows)]
mod output;
mod synth;
use glam::Vec3;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering},
    mpsc,
};

static VOLUME: AtomicU32 = AtomicU32::new(0.65_f32.to_bits());
static STATUS: AtomicU8 = AtomicU8::new(0);
pub fn volume() -> f32 {
    f32::from_bits(VOLUME.load(Ordering::Relaxed))
}
pub fn set_volume(value: f32) {
    VOLUME.store(
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.65
        }
        .to_bits(),
        Ordering::Relaxed,
    );
}
pub fn status() -> &'static str {
    match STATUS.load(Ordering::Relaxed) {
        1 => "Combat audio ready",
        2 => "Audio device unavailable · retrying",
        3 => "Audio unavailable on this platform",
        _ => "Starting audio",
    }
}

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Cue {
    Swing,
    Impact,
    Clang,
    Spark,
    Frost,
    Burst,
    Mend,
    Ward,
    Bloom,
    Hurt,
    Warning,
    Defeat,
    Treasure,
    Shatter,
    ForgeWindup,
    ForgeSlam,
    ArchiveWindup,
    ArchiveRelease,
    PrismWindup,
    PrismRelease,
    CinderStrike,
    Rift,
    Echo,
}
impl Cue {
    const COUNT: usize = 23;
}
#[derive(Clone, Copy)]
pub struct Event {
    pub cue: Cue,
    pub pos: Option<Vec3>,
    pub gain: f32,
}
pub(super) struct Play {
    cue: Cue,
    left: f32,
    right: f32,
    rate: f32,
}
pub struct Audio {
    tx: mpsc::SyncSender<Play>,
    running: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    serial: u32,
}
impl Audio {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(64);
        let running = Arc::new(AtomicBool::new(true));
        let active = Arc::new(AtomicBool::new(false));
        let run = running.clone();
        let enabled = active.clone();
        let worker = std::thread::Builder::new()
            .name("combat-audio".into())
            .spawn(move || {
                #[cfg(windows)]
                output::run(rx, run, enabled);
                #[cfg(not(windows))]
                {
                    let _ = (rx, run, enabled);
                    STATUS.store(3, Ordering::Relaxed);
                }
            })
            .ok();
        if worker.is_none() {
            STATUS.store(3, Ordering::Relaxed);
        }
        Self {
            tx,
            running,
            active,
            worker,
            serial: 0,
        }
    }
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }
    pub fn play(&mut self, event: Event, eye: Vec3, forward: Vec3) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }
        let (gain, pan) = if let Some(pos) = event.pos {
            let delta = pos - eye;
            let distance = delta.length();
            if distance > 42.0 {
                return;
            }
            let pan = delta
                .normalize_or_zero()
                .dot(forward.cross(Vec3::Y).normalize_or_zero())
                .clamp(-0.9, 0.9);
            (
                (1.0 - distance / 42.0).max(0.0) / (1.0 + distance * distance * 0.025),
                pan,
            )
        } else {
            (1.0, 0.0)
        };
        self.serial = self.serial.wrapping_mul(1664525).wrapping_add(1013904223);
        let gain = gain * event.gain.clamp(0.0, 1.0);
        let _ = self.tx.try_send(Play {
            cue: event.cue,
            left: gain * ((1.0 - pan) * 0.5).sqrt(),
            right: gain * ((1.0 + pan) * 0.5).sqrt(),
            rate: 0.96 + (self.serial >> 24) as f32 / 255.0 * 0.08,
        });
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
