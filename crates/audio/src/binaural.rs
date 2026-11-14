//! Parametric binaural stage: interaural time and level, head shadow, pinna colour, air loss.
//!
//! No measured HRTF set ships with the engine, so the cues come from a spherical-head model:
//! Woodworth delay, a far-ear low-pass that closes with the lateral angle, a rear low-pass for
//! the pinna shadow, an elevation-dependent notch and an air-absorption low-pass with range.
use crate::dsp::{coefficient, OnePole, Svf};
use glam::Vec3;
use std::f32::consts::TAU;

const HEAD_RADIUS_M: f32 = 0.0875;
const SOUND_SPEED_MS: f32 = 343.0;
/// Longest interaural delay is 32 samples at 48 kHz; the line holds twice that.
const LINE: usize = 64;
/// Per-sample glide of the cue parameters, so a turning head does not click.
const GLIDE: f32 = 0.002;
const OPEN_HZ: f32 = 18_000.0;
const SHADOW_HZ: f32 = 2_000.0;
const PINNA_FRONT_HZ: f32 = 16_000.0;
const PINNA_REAR_HZ: f32 = 5_000.0;
const NOTCH_DEPTH: f32 = 0.5;
const NOTCH_Q: f32 = 2.5;
/// Air absorption corner at range: 1/e of the open cutoff every `AIR_RANGE_M`.
const AIR_RANGE_M: f32 = 45.0;
const AIR_FLOOR_HZ: f32 = 2_500.0;
/// Cutoff through a fully blocked path: walls pass little but the low thud.
const OCCLUDED_HZ: f32 = 400.0;
/// Level lost through a fully blocked path.
const OCCLUDED_LOSS: f32 = 0.55;

/// Where a sound sits in the listener's head frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cues {
    /// Sine of the angle off the median plane, -1 left to 1 right.
    pub lateral: f32,
    /// Cosine share along the facing direction, 1 ahead to -1 behind.
    pub front: f32,
    /// Sine of the elevation, -1 below to 1 above.
    pub up: f32,
    pub distance_m: f32,
    /// Share of the path blocked by solid cells, 0 clear to 1 sealed off.
    pub occlusion: f32,
    head_locked: bool,
}

impl Cues {
    /// Interface and music sounds: no cues, identical in both ears.
    pub(crate) const HEAD_LOCKED: Cues = Cues { lateral: 0.0, front: 1.0, up: 0.0, distance_m: 0.0, occlusion: 0.0, head_locked: true };

    /// Cues for a source `delta` metres from the head, for a head facing `forward`.
    pub(crate) fn toward(delta: Vec3, forward: Vec3) -> Self {
        let distance_m = delta.length();
        let forward = forward.normalize_or_zero();
        if !distance_m.is_finite() || distance_m < 1.0e-3 || forward == Vec3::ZERO {
            return Self::HEAD_LOCKED;
        }
        let mut right = forward.cross(Vec3::Y).normalize_or_zero();
        if right == Vec3::ZERO {
            right = Vec3::X;
        }
        let up = right.cross(forward);
        let d = delta / distance_m;
        Self {
            lateral: d.dot(right).clamp(-1.0, 1.0),
            front: d.dot(forward).clamp(-1.0, 1.0),
            up: d.dot(up).clamp(-1.0, 1.0),
            distance_m,
            occlusion: 0.0,
            head_locked: false,
        }
    }

    /// The same cues with the path `occlusion` blocked; no effect on head-locked sounds.
    pub(crate) fn occluded(self, occlusion: f32) -> Self {
        Self { occlusion: if occlusion.is_finite() { occlusion.clamp(0.0, 1.0) } else { 0.0 }, ..self }
    }

    /// Cues from a bearing in turns clockwise from straight ahead and an elevation in radians.
    pub(crate) fn polar(bearing_turns: f32, elevation: f32, distance_m: f32) -> Self {
        let (az, el) = (bearing_turns * TAU, elevation);
        Self {
            lateral: el.cos() * az.sin(),
            front: el.cos() * az.cos(),
            up: el.sin(),
            distance_m: distance_m.max(0.0),
            occlusion: 0.0,
            head_locked: false,
        }
    }

    /// Seconds the sound takes to travel from the source.
    pub(crate) fn flight_s(&self) -> f32 {
        if self.head_locked { 0.0 } else { self.distance_m / SOUND_SPEED_MS }
    }

    /// Woodworth interaural delay in seconds; positive when the right ear hears it first.
    fn itd_s(&self) -> f32 {
        let theta = self.lateral.clamp(-1.0, 1.0).asin();
        HEAD_RADIUS_M / SOUND_SPEED_MS * (theta + self.lateral.clamp(-1.0, 1.0))
    }
}

fn smoothstep(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Per-voice renderer from one mono sample to two ear signals.
#[derive(Clone, Copy)]
pub(crate) struct Ears {
    line: [f32; LINE],
    head: usize,
    rate: f32,
    locked: bool,
    delay: [f32; 2],
    gain: [f32; 2],
    shadow_a: [f32; 2],
    pinna_a: f32,
    air_a: f32,
    to: Targets,
    shadow: [OnePole; 2],
    pinna: OnePole,
    air: OnePole,
    notch: Svf,
}

#[derive(Clone, Copy, Default)]
struct Targets {
    delay: [f32; 2],
    gain: [f32; 2],
    shadow_a: [f32; 2],
    pinna_a: f32,
    air_a: f32,
}

impl Default for Ears {
    fn default() -> Self {
        Self::new(Cues::HEAD_LOCKED, 48_000.0)
    }
}

impl Ears {
    /// Starts settled on `cues`.
    pub(crate) fn new(cues: Cues, rate: f32) -> Self {
        let mut ears = Self {
            line: [0.0; LINE],
            head: 0,
            rate,
            locked: cues.head_locked,
            delay: [0.0; 2],
            gain: [1.0; 2],
            shadow_a: [1.0; 2],
            pinna_a: 1.0,
            air_a: 1.0,
            to: Targets::default(),
            shadow: [OnePole::default(); 2],
            pinna: OnePole::default(),
            air: OnePole::default(),
            notch: Svf::new(8_000.0, NOTCH_Q, rate),
        };
        ears.set(cues);
        ears.delay = ears.to.delay;
        ears.gain = ears.to.gain;
        ears.shadow_a = ears.to.shadow_a;
        ears.pinna_a = ears.to.pinna_a;
        ears.air_a = ears.to.air_a;
        ears
    }

    /// Moves the source; the ear parameters glide there over a few milliseconds.
    pub(crate) fn set(&mut self, cues: Cues) {
        self.locked = cues.head_locked;
        if self.locked {
            return;
        }
        let coef = |hz: f32| coefficient(hz.min(self.rate * 0.45), self.rate).min(1.0);
        let itd = cues.itd_s() * self.rate;
        let shadow = smoothstep(cues.lateral.abs());
        // The far ear sits in the head's shadow; the near one gains a little from the reflection.
        let far_hz = OPEN_HZ * (SHADOW_HZ / OPEN_HZ).powf(shadow);
        let open = 1.0 - OCCLUDED_LOSS * cues.occlusion;
        let (near, far) = (open * (1.0 + 0.2 * shadow), open * (1.0 - 0.15 * shadow));
        // Index 0 is the left ear; a source on the right reaches the right ear first.
        let right_first = cues.lateral >= 0.0;
        self.to = Targets {
            delay: if right_first { [itd.abs(), 0.0] } else { [0.0, itd.abs()] },
            gain: if right_first { [far, near] } else { [near, far] },
            shadow_a: if right_first { [coef(far_hz), coef(OPEN_HZ)] } else { [coef(OPEN_HZ), coef(far_hz)] },
            pinna_a: coef(PINNA_FRONT_HZ * (PINNA_REAR_HZ / PINNA_FRONT_HZ).powf((-cues.front).max(0.0))),
            air_a: coef(
                (OPEN_HZ * (-cues.distance_m / AIR_RANGE_M).exp()).max(AIR_FLOOR_HZ).min(OPEN_HZ * (OCCLUDED_HZ / OPEN_HZ).powf(cues.occlusion)),
            ),
        };
        self.notch.tune((8_000.0 + 3_000.0 * cues.up).clamp(4_000.0, 12_000.0), NOTCH_Q, self.rate);
    }

    pub(crate) fn process(&mut self, x: f32) -> [f32; 2] {
        if self.locked {
            return [x, x];
        }
        for ear in 0..2 {
            self.delay[ear] += (self.to.delay[ear] - self.delay[ear]) * GLIDE;
            self.gain[ear] += (self.to.gain[ear] - self.gain[ear]) * GLIDE;
            self.shadow_a[ear] += (self.to.shadow_a[ear] - self.shadow_a[ear]) * GLIDE;
        }
        self.pinna_a += (self.to.pinna_a - self.pinna_a) * GLIDE;
        self.air_a += (self.to.air_a - self.air_a) * GLIDE;
        let coloured = self.pinna.low(self.air.low(x, self.air_a), self.pinna_a);
        let coloured = coloured - NOTCH_DEPTH * self.notch.band(coloured);
        self.line[self.head] = coloured;
        let mut out = [0.0; 2];
        for ear in 0..2 {
            let whole = self.delay[ear].floor();
            let newer = (self.head + LINE - whole as usize % LINE) % LINE;
            let older = (newer + LINE - 1) % LINE;
            let tap = self.line[newer] + (self.line[older] - self.line[newer]) * (self.delay[ear] - whole);
            out[ear] = self.shadow[ear].low(tap, self.shadow_a[ear]) * self.gain[ear];
        }
        self.head = (self.head + 1) % LINE;
        out
    }
}

/// Unit-power gain for a source: the old pan law gave each ear 0.707 at the centre.
pub(crate) const CENTRE_GAIN: f32 = std::f32::consts::FRAC_1_SQRT_2;

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const RATE: f32 = 48_000.0;

    fn run(cues: Cues, signal: impl Fn(usize) -> f32, frames: usize) -> Vec<[f32; 2]> {
        let mut ears = Ears::new(cues, RATE);
        (0..frames).map(|i| ears.process(signal(i))).collect()
    }

    fn rms(frames: &[[f32; 2]], ear: usize) -> f32 {
        (frames.iter().map(|f| f[ear] * f[ear]).sum::<f32>() / frames.len() as f32).sqrt()
    }

    fn tone(hz: f32) -> impl Fn(usize) -> f32 {
        move |i| (TAU * hz * i as f32 / RATE).sin()
    }

    fn peak_at(frames: &[[f32; 2]], ear: usize) -> usize {
        frames.iter().enumerate().max_by(|a, b| a.1[ear].abs().total_cmp(&b.1[ear].abs())).map_or(0, |(i, _)| i)
    }

    fn side(lateral: f32) -> Cues {
        Cues::polar(lateral.asin() / TAU, 0.0, 2.0)
    }

    #[test]
    fn a_source_to_the_right_reaches_the_right_ear_first_by_the_woodworth_delay() {
        let click = |i: usize| if i == 100 { 1.0 } else { 0.0 };
        let frames = run(side(1.0), click, 400);
        let gap = peak_at(&frames, 0) as f32 - peak_at(&frames, 1) as f32;
        let expected = HEAD_RADIUS_M / SOUND_SPEED_MS * (PI / 2.0 + 1.0) * RATE;
        assert!((gap - expected).abs() < 3.0, "gap {gap} expected {expected}");
        let mirrored = run(side(-1.0), click, 400);
        assert!(peak_at(&mirrored, 1) > peak_at(&mirrored, 0));
    }

    #[test]
    fn the_head_shadows_highs_far_more_than_lows() {
        let high = run(side(1.0), tone(6_000.0), 4_800);
        let low = run(side(1.0), tone(200.0), 4_800);
        let (high_gap, low_gap) = (rms(&high[2_400..], 1) / rms(&high[2_400..], 0), rms(&low[2_400..], 1) / rms(&low[2_400..], 0));
        assert!(high_gap > 4.0, "6 kHz near/far {high_gap}");
        assert!(low_gap < 1.6, "200 Hz near/far {low_gap}");
    }

    #[test]
    fn a_centred_source_is_identical_in_both_ears() {
        for frame in run(Cues::polar(0.0, 0.0, 5.0), tone(1_000.0), 2_000) {
            assert!((frame[0] - frame[1]).abs() < 1.0e-6);
        }
    }

    #[test]
    fn behind_is_duller_than_ahead_at_the_same_lateral_angle() {
        let ahead = run(Cues::polar(0.0, 0.0, 3.0), tone(10_000.0), 4_800);
        let behind = run(Cues::polar(0.5, 0.0, 3.0), tone(10_000.0), 4_800);
        assert!(rms(&behind[2_400..], 0) < 0.6 * rms(&ahead[2_400..], 0));
    }

    #[test]
    fn elevation_moves_the_notch() {
        let level = |elevation: f32| rms(&run(Cues::polar(0.0, elevation, 3.0), tone(8_000.0), 4_800)[2_400..], 0);
        assert!(level(0.0) < 0.75 * level(1.0), "8 kHz sits in the level notch, not in the raised one");
    }

    #[test]
    fn distance_takes_the_top_off_and_adds_flight_time() {
        let near = rms(&run(Cues::polar(0.0, 0.0, 1.0), tone(12_000.0), 4_800)[2_400..], 0);
        let far = rms(&run(Cues::polar(0.0, 0.0, 40.0), tone(12_000.0), 4_800)[2_400..], 0);
        assert!(far < 0.7 * near);
        assert!((Cues::polar(0.0, 0.0, 34.3).flight_s() - 0.1).abs() < 1.0e-4);
    }

    #[test]
    fn a_wall_between_takes_the_highs_and_some_level() {
        let level = |occlusion: f32, hz: f32| rms(&run(Cues::polar(0.0, 0.0, 4.0).occluded(occlusion), tone(hz), 4_800)[2_400..], 0);
        assert!(level(1.0, 4_000.0) < 0.15 * level(0.0, 4_000.0));
        assert!(level(1.0, 100.0) > 0.35 * level(0.0, 100.0), "the thud still carries");
        assert_eq!(Cues::HEAD_LOCKED.occluded(1.0).occlusion, 1.0);
        assert_eq!(Cues::HEAD_LOCKED.occluded(f32::NAN).occlusion, 0.0);
    }

    #[test]
    fn head_locked_sounds_pass_through_and_degenerate_geometry_is_locked() {
        let mut ears = Ears::new(Cues::HEAD_LOCKED, RATE);
        assert_eq!(ears.process(0.37), [0.37, 0.37]);
        assert_eq!(Cues::toward(Vec3::ZERO, Vec3::NEG_Z), Cues::HEAD_LOCKED);
        assert_eq!(Cues::toward(Vec3::X, Vec3::ZERO), Cues::HEAD_LOCKED);
    }

    #[test]
    fn toward_uses_the_head_frame_including_looking_straight_up() {
        let c = Cues::toward(Vec3::new(3.0, 0.0, 0.0), Vec3::NEG_Z);
        assert!(c.lateral > 0.99 && c.front.abs() < 1.0e-5);
        let c = Cues::toward(Vec3::new(0.0, 0.0, -3.0), Vec3::NEG_Z);
        assert!(c.front > 0.99);
        let c = Cues::toward(Vec3::new(0.0, 2.0, 0.0), Vec3::Y);
        assert!(c.front > 0.99 && c.lateral.is_finite());
    }

    #[test]
    fn moving_the_source_glides_without_a_jump_or_nan() {
        let mut ears = Ears::new(side(-1.0), RATE);
        let mut last = [0.0f32; 2];
        let mut worst = 0.0f32;
        for i in 0..9_600 {
            if i == 2_000 {
                ears.set(side(1.0));
            }
            let out = ears.process((TAU * 500.0 * i as f32 / RATE).sin());
            assert!(out.iter().all(|v| v.is_finite()));
            worst = worst.max((out[0] - last[0]).abs()).max((out[1] - last[1]).abs());
            last = out;
        }
        assert!(worst < 0.5, "step {worst}");
    }
}
