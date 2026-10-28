//! Footsteps, ambience beds and music, mixed after the effects bus.
use crate::beds::{Beds, BED_COUNT};
use crate::music::Piece;
use crate::perform::Performer;
use crate::steps::{StepPlay, Steps, Surface, VARIANTS};
use crate::{dsp, Audio};
use glam::Vec3;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;

static MUSIC_VOLUME: AtomicU32 = AtomicU32::new(0.5_f32.to_bits());
static AMBIENCE_VOLUME: AtomicU32 = AtomicU32::new(0.6_f32.to_bits());

fn store(slot: &AtomicU32, value: f32, fallback: f32) {
    let value = if value.is_finite() { value.clamp(0.0, 1.0) } else { fallback };
    slot.store(value.to_bits(), Ordering::Relaxed);
}
pub fn music_volume() -> f32 {
    f32::from_bits(MUSIC_VOLUME.load(Ordering::Relaxed))
}
pub fn set_music_volume(value: f32) {
    store(&MUSIC_VOLUME, value, 0.5);
}
pub fn ambience_volume() -> f32 {
    f32::from_bits(AMBIENCE_VOLUME.load(Ordering::Relaxed))
}
pub fn set_ambience_volume(value: f32) {
    store(&AMBIENCE_VOLUME, value, 0.6);
}

pub(crate) enum Command {
    Step(StepPlay),
    Beds([f32; BED_COUNT]),
    Music(Piece),
    StopMusic,
}

/// Matches the effects bus: footsteps past this distance are culled.
const AUDIBLE_M: f32 = 42.0;

pub(crate) struct Layers {
    steps: Steps,
    beds: Beds,
    music: Performer,
    enabled: bool,
    fade: f32,
    gains: [f32; 3],
}

impl Layers {
    pub(crate) fn new(rate: u32) -> Self {
        Self {
            steps: Steps::new(rate),
            beds: Beds::new(rate),
            music: Performer::new(rate),
            enabled: false,
            fade: 0.0,
            gains: [0.0; 3],
        }
    }

    pub(crate) fn receive(&mut self, rx: &mpsc::Receiver<Command>) {
        while let Ok(command) = rx.try_recv() {
            match command {
                Command::Step(play) if self.enabled => self.steps.start(play),
                Command::Step(_) => {}
                Command::Beds(levels) => self.beds.target = levels,
                Command::Music(piece) => self.music.start(piece),
                Command::StopMusic => self.music.stop(),
            }
        }
    }

    /// Snapshots the volumes once per output buffer.
    pub(crate) fn begin(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.steps.clear();
        }
        self.gains = [crate::volume(), ambience_volume(), music_volume()];
    }

    pub(crate) fn frame(&mut self) -> [f32; 2] {
        self.fade += (f32::from(u8::from(self.enabled)) - self.fade) * 0.0005;
        if !self.enabled && self.fade < 1.0e-4 {
            return [0.0; 2];
        }
        let [effects, ambience, music] = self.gains;
        let s = self.steps.frame();
        let b = self.beds.frame();
        let m = self.music.frame();
        let mix = |ch: usize| (s[ch] * effects + b[ch] * ambience + m[ch] * music) * self.fade;
        [mix(0), mix(1)]
    }
}

impl Audio {
    /// Queues one footstep. `seed` picks the render and pitch, so a stride count gives variety.
    pub fn footstep(&mut self, surface: Surface, pos: Option<Vec3>, gain: f32, seed: u32, eye: Vec3, forward: Vec3) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }
        let (distance_gain, pan) = match pos {
            Some(pos) => {
                let delta = pos - eye;
                let distance = delta.length();
                if !distance.is_finite() || distance > AUDIBLE_M {
                    return;
                }
                let pan = delta.normalize_or_zero().dot(forward.cross(Vec3::Y).normalize_or_zero()).clamp(-0.9, 0.9);
                ((1.0 - distance / AUDIBLE_M) / (1.0 + distance * distance * 0.04), pan)
            }
            None => (1.0, 0.0),
        };
        let gain = distance_gain * gain.clamp(0.0, 1.0) * 0.5;
        let key = dsp::hash(seed, surface as u32);
        let _ = self.layer_tx.try_send(Command::Step(StepPlay {
            surface,
            variant: key as usize % VARIANTS,
            left: gain * ((1.0 - pan) * 0.5).sqrt(),
            right: gain * ((1.0 + pan) * 0.5).sqrt(),
            rate: 0.93 + dsp::unit(key, 9) * 0.14,
        }));
    }

    /// Sets each ambience bed's target level, 0 to 1; the mixer cross-fades.
    pub fn set_ambience(&self, levels: [f32; BED_COUNT]) {
        let levels = levels.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 });
        let _ = self.layer_tx.try_send(Command::Beds(levels));
    }

    pub fn start_music(&self, piece: Piece) {
        let _ = self.layer_tx.try_send(Command::Music(piece));
    }

    /// Fades the current piece out over a few seconds.
    pub fn stop_music(&self) {
        let _ = self.layer_tx.try_send(Command::StopMusic);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::{compose, tests::table};

    /// Real-time cost of the layers with every bed, a full music texture and
    /// steady footsteps, rendered for `seconds` of audio.
    fn busy_render(seconds: u32) -> std::time::Duration {
        let mut layers = Layers::new(48_000);
        layers.begin(true);
        layers.beds.target = [1.0; BED_COUNT];
        let table = table();
        let mut piece = compose(&table.pieces[0], 11);
        piece.notes.iter_mut().for_each(|n| n.timbre = crate::music::Timbre::Pad);
        layers.music.start(piece);
        let started = std::time::Instant::now();
        let mut sink = 0.0f32;
        for frame in 0..48_000 * seconds {
            if frame % 12_000 == 0 {
                layers.steps.start(StepPlay { surface: Surface::Metal, variant: 1, left: 0.4, right: 0.4, rate: 1.0 });
            }
            let [l, r] = layers.frame();
            sink += l + r;
        }
        assert!(sink.is_finite());
        started.elapsed()
    }

    #[test]
    fn layers_render_far_faster_than_real_time() {
        let seconds = 4;
        // Best of three: peer builds on the same machine skew a single run.
        let best = (0..3).map(|_| busy_render(seconds)).min().unwrap_or_default();
        let share = best.as_secs_f64() / f64::from(seconds);
        println!("layers: {:.3} ms per second of audio ({:.2}% of one core)", share * 1000.0, share * 100.0);
        // Debug builds are several times slower than the shipped profile.
        assert!(share < 0.25, "layers took {:.1}% of real time", share * 100.0);
    }

    #[test]
    fn volumes_clamp_and_reject_non_finite_values() {
        set_music_volume(2.0);
        assert_eq!(music_volume(), 1.0);
        set_ambience_volume(f32::NAN);
        assert_eq!(ambience_volume(), 0.6);
        set_music_volume(0.5);
    }

    #[test]
    fn a_disabled_mixer_fades_every_layer_to_silence() {
        let mut layers = Layers::new(48_000);
        layers.begin(true);
        layers.beds.target = [1.0; BED_COUNT];
        for _ in 0..48_000 {
            layers.frame();
        }
        layers.begin(false);
        for _ in 0..48_000 {
            layers.frame();
        }
        assert_eq!(layers.frame(), [0.0, 0.0]);
    }
}
