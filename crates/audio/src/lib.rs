//! Bounded, asynchronous combat audio. Original synthesized Foley lives in memory;
//! no network assets, file decoding, or device calls run on the rendering thread.
mod beds;
mod binaural;
mod dsp;
mod fauna;
mod layers;
mod music;
#[cfg(windows)]
mod output;
mod perform;
mod reverb;
#[cfg(test)]
mod review;
mod steps;
mod suit;
mod synth;
mod tap;
pub use beds::{Bed, BED_COUNT};
pub use dsp::hash;
pub use fauna::{Family, Fauna, Placement};
pub use layers::{ambience_volume, music_volume, set_ambience_volume, set_music_volume};
pub use music::{compose, Director, Mood, MusicTable, Note, Piece, PieceDef, Timbre};
pub use steps::{Contact, Surface};
use glam::Vec3;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
        mpsc, Arc,
    },
    time::Instant,
};

static VOLUME: AtomicU32 = AtomicU32::new(0.65_f32.to_bits());
static STATUS: AtomicU8 = AtomicU8::new(0);
static UNDERRUNS: AtomicU32 = AtomicU32::new(0);
/// Longest sleep-to-wake gap and longest mixing pass of the output thread,
/// in microseconds; they tell a starved thread from a slow mix.
static LONGEST_GAP_US: AtomicU32 = AtomicU32::new(0);
static LONGEST_MIX_US: AtomicU32 = AtomicU32::new(0);
/// Times the output queue ran dry since start; each is an audible gap.
pub fn underruns() -> u32 {
    UNDERRUNS.load(Ordering::Relaxed)
}
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

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    Splash,
    Mine,
    Smelt,
    /// Movement kit: an airy rising whoosh with a short thruster chirp.
    Dash,
    /// Gritty low friction hiss under a slide start.
    Slide,
    /// Soft thruster puff and a rising tone for the mid-air jump.
    AirJump,
    /// Heavy-landing thump after a long fall.
    Land,
    /// Grazing animal call; pitch scales with body size.
    Bleat,
    /// Slime gel rebound: a wet slap under a wobbling rising tone.
    Boing,
}
impl Cue {
    const COUNT: usize = 32;
}
#[derive(Clone, Copy)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub cue: Cue,
    pub pos: Option<Vec3>,
    pub gain: f32,
    /// Per-event playback pitch. 1.0 is neutral; spell families and variants
    /// use this to avoid every projectile sharing one identical chirp.
    pub pitch: f32,
}
pub(crate) struct Play {
    cue: Cue,
    gain: f32,
    ears: binaural::Ears,
    /// Samples of flight time before the sound arrives.
    wait: f32,
    rate: f32,
    /// Wet send into the short spatial delay/reverb bus.
    reverb: f32,
    /// One of the three decorrelated renders for this cue.
    variant: usize,
}

/// Reverb sends are deliberately cue-specific: impacts and rifts get space,
/// sparks stay tight so rapid fire does not smear into a wash.
pub(crate) fn reverb_send(cue: Cue) -> f32 {
    match cue {
        Cue::Rift | Cue::Echo | Cue::Shatter => 0.32,
        Cue::Burst | Cue::Splash => 0.22,
        Cue::Bloom
        | Cue::ForgeSlam
        | Cue::ForgeWindup
        | Cue::ArchiveRelease
        | Cue::PrismRelease
        | Cue::CinderStrike => 0.22,
        Cue::Spark | Cue::Frost | Cue::Swing | Cue::Ward => 0.08,
        // Movement is constant and close to the ear: keep it dry.
        Cue::Dash | Cue::Slide | Cue::AirJump | Cue::Boing => 0.06,
        Cue::Land => 0.12,
        _ => 0.24,
    }
}

/// Spell casts get slightly different space by material; repeatable bolts stay
/// dry enough to remain clear when fired in quick succession.
fn rapid_window_ms(cue: Cue) -> u64 {
    match cue {
        Cue::Spark | Cue::Frost | Cue::Ward | Cue::Swing => 70,
        Cue::Burst | Cue::Bloom | Cue::Splash => 110,
        _ => 55,
    }
}

fn rapid_repeat(last_ms: u64, now_ms: u64, window_ms: u64) -> bool {
    last_ms != u64::MAX && now_ms.saturating_sub(last_ms) < window_ms
}
pub struct Audio {
    tx: mpsc::SyncSender<Play>,
    layer_tx: mpsc::SyncSender<layers::Command>,
    running: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    serial: u32,
    started: Instant,
    /// Last trigger time per cue. Rapid identical cues receive a ducked gain
    /// instead of stacking at full amplitude and turning into a harsh buzz.
    last_cue_ms: [u64; Cue::COUNT],
    spell_last_ms: [u64; 3],
}
impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}

impl Audio {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(64);
        let (layer_tx, layer_rx) = mpsc::sync_channel(32);
        let running = Arc::new(AtomicBool::new(true));
        let active = Arc::new(AtomicBool::new(false));
        let run = running.clone();
        let enabled = active.clone();
        let worker = std::thread::Builder::new()
            .name("combat-audio".into())
            .spawn(move || {
                #[cfg(windows)]
                output::run(rx, layer_rx, run, enabled);
                #[cfg(not(windows))]
                {
                    let _ = (rx, layer_rx, run, enabled);
                    STATUS.store(3, Ordering::Relaxed);
                }
            })
            .ok();
        if worker.is_none() {
            STATUS.store(3, Ordering::Relaxed);
        }
        Self {
            tx,
            layer_tx,
            running,
            active,
            worker,
            serial: 0,
            started: Instant::now(),
            last_cue_ms: [u64::MAX; Cue::COUNT],
            spell_last_ms: [u64::MAX; 3],
        }
    }
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }
    fn spell_family(cue: Cue) -> Option<usize> {
        match cue {
            Cue::Spark | Cue::Frost | Cue::Ward => Some(0),
            Cue::Burst | Cue::CinderStrike => Some(1),
            Cue::Bloom | Cue::Splash => Some(2),
            _ => None,
        }
    }
    pub fn play(&mut self, event: Event, eye: Vec3, forward: Vec3) {
        self.play_occluded(event, eye, forward, 0.0);
    }

    /// As `play`, with `occlusion` of the path to the source blocked, 0 clear to 1 sealed.
    pub fn play_occluded(&mut self, event: Event, eye: Vec3, forward: Vec3, occlusion: f32) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }
        let (gain, cues) = if let Some(pos) = event.pos {
            let delta = pos - eye;
            let distance = delta.length();
            if distance > 42.0 {
                return;
            }
            (
                (1.0 - distance / 42.0).max(0.0) / (1.0 + distance * distance * 0.025)
                    * layers::carry(distance),
                binaural::Cues::toward(delta, forward).occluded(occlusion),
            )
        } else {
            (1.0, binaural::Cues::HEAD_LOCKED)
        };
        self.serial = self.serial.wrapping_mul(1664525).wrapping_add(1013904223);
        let cue_index = event.cue as usize;
        let now = self.started.elapsed().as_millis() as u64;
        let last = self.last_cue_ms[cue_index];
        let rapid = rapid_repeat(last, now, rapid_window_ms(event.cue));
        self.last_cue_ms[cue_index] = now;
        let family_rapid = Self::spell_family(event.cue).is_some_and(|family| {
            let last = self.spell_last_ms[family];
            let rapid = rapid_repeat(last, now, 52);
            self.spell_last_ms[family] = now;
            rapid
        });
        let duck = if rapid || family_rapid { 0.36 } else { 1.0 };
        // Keep spells clear under rapid fire while letting isolated impacts
        // retain enough body to read as meaningful feedback.
        let gain = gain * event.gain.clamp(0.0, 1.0) * duck * 0.62 * binaural::CENTRE_GAIN;
        let _ = self.tx.try_send(Play {
            cue: event.cue,
            gain,
            ears: binaural::Ears::new(cues, synth::RATE as f32),
            wait: cues.flight_s() * synth::RATE as f32,
            rate: ((0.96 + (self.serial >> 24) as f32 / 255.0 * 0.08)
                * event.pitch.clamp(0.60, 1.70))
            .clamp(0.55, 1.80),
            reverb: reverb_send(event.cue),
            variant: (self.serial.wrapping_mul(1_664_525) >> 7) as usize % 3,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impact_cues_are_wetter_than_rapid_fire_cues() {
        assert!(reverb_send(Cue::Rift) > reverb_send(Cue::Spark));
        assert!(reverb_send(Cue::Burst) > reverb_send(Cue::Swing));
        assert!(reverb_send(Cue::Splash) > reverb_send(Cue::Frost));
        assert!((0.0..=0.6).contains(&reverb_send(Cue::Hurt)));
    }

    #[test]
    fn the_first_trigger_is_not_treated_as_a_rapid_repeat() {
        assert!(!rapid_repeat(u64::MAX, 0, 24));
        assert!(rapid_repeat(100, 110, 24));
        assert!(!rapid_repeat(100, 124, 24));
    }

    #[test]
    fn common_spell_families_share_repeat_ducking() {
        assert_eq!(Audio::spell_family(Cue::Spark), Audio::spell_family(Cue::Frost));
        assert_eq!(Audio::spell_family(Cue::Burst), Audio::spell_family(Cue::CinderStrike));
        assert_eq!(Audio::spell_family(Cue::Bloom), Audio::spell_family(Cue::Splash));
        assert_ne!(Audio::spell_family(Cue::Spark), Audio::spell_family(Cue::Burst));
    }
}
