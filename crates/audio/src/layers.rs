//! Footsteps, ambience beds and music, mixed after the effects bus.
use crate::beds::{Beds, BED_COUNT};
use crate::music::Piece;
use crate::perform::Performer;
use crate::steps::{Contact, StepPlay, Steps, Surface, VARIANTS};
use crate::{dsp, Audio};
use glam::Vec3;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;

/// Music is off until the player turns it up.
static MUSIC_VOLUME: AtomicU32 = AtomicU32::new(0.0_f32.to_bits());
static AMBIENCE_VOLUME: AtomicU32 = AtomicU32::new(0.6_f32.to_bits());
/// Enclosure around the listener, 0 open air to 1 a closed cave.
static SPACE: AtomicU32 = AtomicU32::new(0.0_f32.to_bits());
/// Air density at the listener, 1 at sea level to 0 in vacuum.
static AIR: AtomicU32 = AtomicU32::new(1.0_f32.to_bits());
/// Listener strain, 0 calm to 1 near death; sets the suit breathing rate.
/// Ears under water, 0 dry to 1 submerged.
static SUBMERGED: AtomicU32 = AtomicU32::new(0.0_f32.to_bits());
static STRAIN: AtomicU32 = AtomicU32::new(0.0_f32.to_bits());
/// Listener yaw in turns clockwise from -z; animal calls pan against it.
static YAW: AtomicU32 = AtomicU32::new(0.0_f32.to_bits());
/// Lift drive speed 0..1 and signed load -1..1 (stored as 0..1 about 0.5).
static DRIVE_SPEED: AtomicU32 = AtomicU32::new(0.0_f32.to_bits());
static DRIVE_LOAD: AtomicU32 = AtomicU32::new(0.5_f32.to_bits());

fn store(slot: &AtomicU32, value: f32, fallback: f32) {
    let value = if value.is_finite() { value.clamp(0.0, 1.0) } else { fallback };
    slot.store(value.to_bits(), Ordering::Relaxed);
}
pub fn music_volume() -> f32 {
    f32::from_bits(MUSIC_VOLUME.load(Ordering::Relaxed))
}
pub fn set_music_volume(value: f32) {
    store(&MUSIC_VOLUME, value, 0.0);
}
pub fn ambience_volume() -> f32 {
    f32::from_bits(AMBIENCE_VOLUME.load(Ordering::Relaxed))
}
pub fn set_ambience_volume(value: f32) {
    store(&AMBIENCE_VOLUME, value, 0.6);
}

pub(crate) fn space() -> f32 {
    f32::from_bits(SPACE.load(Ordering::Relaxed))
}

pub(crate) fn air() -> f32 {
    f32::from_bits(AIR.load(Ordering::Relaxed))
}

/// How clearly the ear hears world sounds: thin air and water both muffle.
/// Water still carries sound far, so only `air` feeds `carry`.
pub(crate) fn clarity() -> f32 {
    let submerged = f32::from_bits(SUBMERGED.load(Ordering::Relaxed));
    air().min(1.0 - 0.8 * submerged)
}

/// Share of a sound `distance_m` away that reaches the listener through the air.
/// Within tool reach it carries through the hand or the ground, so vacuum keeps it.
pub(crate) fn carry(distance_m: f32) -> f32 {
    carry_through(air(), distance_m)
}

fn carry_through(air: f32, distance_m: f32) -> f32 {
    let t = ((distance_m - 2.0) / 4.0).clamp(0.0, 1.0);
    let touching = 1.0 - t * t * (3.0 - 2.0 * t);
    air + (1.0 - air) * touching
}

/// Footsteps and block contacts feed the room at this level; the room's own
/// wetness follows `space`, so outdoors they stay nearly dry.
const STEP_SEND: f32 = 0.5;

pub(crate) enum Command {
    Step(StepPlay),
    Beds([f32; BED_COUNT]),
    Fauna(crate::Fauna, crate::Placement),
    Music(Piece),
    StopMusic,
}

/// Matches the effects bus: footsteps past this distance are culled.
const AUDIBLE_M: f32 = 42.0;

pub(crate) struct Layers {
    steps: Steps,
    beds: Beds,
    fauna: crate::fauna::Chorus,
    music: Performer,
    muffle: dsp::Muffle,
    rate: f32,
    enabled: bool,
    fade: f32,
    gains: [f32; 3],
}

impl Layers {
    pub(crate) fn new(rate: u32) -> Self {
        Self {
            steps: Steps::new(rate),
            beds: Beds::new(rate),
            fauna: crate::fauna::Chorus::new(rate),
            music: Performer::new(rate),
            muffle: dsp::Muffle::new(),
            rate: rate as f32,
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
                Command::Fauna(voice, placement) => self.fauna.set(voice, placement),
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
        // Same glide as the effects bus in `output`, so steps and cues thin together.
        self.muffle.approach(clarity(), 0.012, self.rate);
        self.beds.strain = f32::from_bits(STRAIN.load(Ordering::Relaxed));
        self.fauna.yaw(f32::from_bits(YAW.load(Ordering::Relaxed)));
        self.beds.drive_speed = f32::from_bits(DRIVE_SPEED.load(Ordering::Relaxed));
        self.beds.drive_load = f32::from_bits(DRIVE_LOAD.load(Ordering::Relaxed)) * 2.0 - 1.0;
    }

    /// The dry stereo layers and the mono send they feed into the room.
    pub(crate) fn frame(&mut self) -> ([f32; 2], f32) {
        self.fade += (f32::from(u8::from(self.enabled)) - self.fade) * 0.0005;
        if !self.enabled && self.fade < 1.0e-4 {
            return ([0.0; 2], 0.0);
        }
        let [effects, ambience, music] = self.gains;
        // Music is not in the world, so only steps go through the thin air.
        let s = self.muffle.process(self.steps.frame());
        let b = self.beds.frame();
        let f = self.fauna.frame();
        let b = [b[0] + f[0], b[1] + f[1]];
        // A muted piece is not rendered; it resumes where it stood when raised.
        let m = if music > 0.0 { self.music.frame() } else { [0.0; 2] };
        let mix = |ch: usize| (s[ch] * effects + b[ch] * ambience + m[ch] * music) * self.fade;
        let room_send = (s[0] + s[1]) * 0.5 * effects * STEP_SEND * self.muffle.air()
            + self.beds.room_send * ambience * 0.18 * self.muffle.air();
        ([mix(0), mix(1)], room_send * self.fade)
    }
}

impl Audio {
    /// Queues one footstep. `seed` picks the render and pitch, so a stride count gives variety.
    pub fn footstep(&mut self, surface: Surface, pos: Option<Vec3>, gain: f32, seed: u32, eye: Vec3, forward: Vec3) {
        self.contact(Contact::Step, surface, pos, gain, seed, eye, forward);
    }

    /// Sets how enclosed the listener is, 0 to 1; the room reverb follows it.
    pub fn set_space(&self, space: f32) {
        store(&SPACE, space, 0.0);
    }

    /// Sets air density at the listener, 1 sea level to 0 vacuum; world sounds thin with it.
    pub fn set_air(&self, air: f32) {
        store(&AIR, air, 1.0);
    }

    /// Sets whether the ears are under water, 0 to 1; world sounds go dull and low.
    pub fn set_submerged(&self, submerged: f32) {
        store(&SUBMERGED, submerged, 0.0);
    }

    /// Sets listener strain, 0 calm to 1 near death; the suit breathing follows it.
    pub fn set_strain(&self, strain: f32) {
        store(&STRAIN, strain, 0.0);
    }

    /// Lift drive cue: `speed` is the share of peak speed, `load` the signed
    /// share of peak drive (negative while braking).
    pub fn set_drive(&self, speed: f32, load: f32) {
        store(&DRIVE_SPEED, speed, 0.0);
        store(&DRIVE_LOAD, load * 0.5 + 0.5, 0.5);
    }

    /// Queues a step, mining strike, break or placement on `surface`.
    #[allow(clippy::too_many_arguments)]
    pub fn contact(
        &mut self,
        contact: Contact,
        surface: Surface,
        pos: Option<Vec3>,
        gain: f32,
        seed: u32,
        eye: Vec3,
        forward: Vec3,
    ) {
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
                ((1.0 - distance / AUDIBLE_M) / (1.0 + distance * distance * 0.04) * carry(distance), pan)
            }
            None => (1.0, 0.0),
        };
        // Steps carry their energy at 1 to 3 kHz, where hearing is most
        // sensitive, so they sit well under the effects bus.
        // A break is the loudest contact and a mining strike the quietest, as
        // in block games where steps sit far under digging.
        let level = match contact {
            Contact::Step => 1.0,
            Contact::Hit => 0.5,
            Contact::Break => 1.6,
            Contact::Place => 1.3,
        };
        let gain = distance_gain * gain.clamp(0.0, 1.0) * 0.35 * level;
        let key = dsp::hash(seed, surface as u32 ^ (contact as u32) << 8);
        let _ = self.layer_tx.try_send(Command::Step(StepPlay {
            contact,
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

    /// Sets the biome's animal voice and where it comes from; levels cross-fade over a few seconds.
    pub fn set_fauna(&self, voice: crate::Fauna, placement: crate::Placement) {
        let _ = self.layer_tx.try_send(Command::Fauna(voice, placement));
    }

    /// Sets the listener yaw in turns clockwise from -z, so placed calls keep their bearing as the view turns.
    pub fn set_listener_yaw(&self, turns: f32) {
        let value = if turns.is_finite() { turns.rem_euclid(1.0) } else { 0.0 };
        YAW.store(value.to_bits(), Ordering::Relaxed);
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

    #[test]
    fn vacuum_silences_distant_sounds_but_not_the_hand() {
        assert_eq!(carry_through(1.0, 30.0), 1.0);
        assert_eq!(carry_through(0.0, 1.0), 1.0);
        assert_eq!(carry_through(0.0, 8.0), 0.0);
        assert!((carry_through(0.5, 8.0) - 0.5).abs() < 1e-6);
    }
    use crate::music::{compose, tests::table};

    /// Real-time cost of the layers with every bed, a full music texture and
    /// steady footsteps, rendered for `seconds` of audio.
    fn busy_render(seconds: u32) -> std::time::Duration {
        let mut layers = Layers::new(48_000);
        layers.begin(true);
        layers.gains = [1.0; 3];
        layers.beds.target = [1.0; BED_COUNT];
        let table = table();
        let mut piece = compose(&table.pieces[0], 11);
        piece.notes.iter_mut().for_each(|n| n.timbre = crate::music::Timbre::Pad);
        layers.music.start(piece);
        let started = std::time::Instant::now();
        let mut sink = 0.0f32;
        for frame in 0..48_000 * seconds {
            if frame % 12_000 == 0 {
                layers.steps.start(StepPlay {
                    contact: Contact::Break,
                    surface: Surface::Metal,
                    variant: 1,
                    left: 0.4,
                    right: 0.4,
                    rate: 1.0,
                });
            }
            let ([l, r], _) = layers.frame();
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
        assert_eq!(layers.frame(), ([0.0, 0.0], 0.0));
    }
}
