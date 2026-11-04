//! Birdsong and frog chorus: sparse synthesized calls whose voice follows the biome.
mod families;

use crate::dsp::{advance, sine, Noise};
use families::Voice;
use std::f32::consts::TAU;

/// Compass sectors in the probe's order: +x, +x+z, +z, -x+z, -x, -x-z, -z, +x-z.
const SECTORS: [(f32, f32); 8] = [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, 0.0), (-1.0, -1.0), (0.0, -1.0), (1.0, -1.0)];

/// Bearing of a sector in turns clockwise from -z, the same frame as the listener yaw.
fn bearing(sector: usize) -> f32 {
    let (dx, dz) = SECTORS[sector];
    dx.atan2(-dz) / TAU
}

/// Stereo pan of a bearing heard by a listener facing `yaw`, -1 left to 1 right.
fn pan_of(bearing: f32, yaw: f32) -> f32 {
    (((bearing - yaw) * TAU).sin()).clamp(-0.9, 0.9)
}

/// Picks a sector by weight; `None` when every weight is zero.
fn draw_sector(weights: &[f32; 8], roll: f32) -> Option<usize> {
    let total: f32 = weights.iter().map(|w| w.max(0.0)).sum();
    if total <= 0.0 {
        return None;
    }
    let mut acc = 0.0;
    for (i, w) in weights.iter().enumerate() {
        acc += w.max(0.0);
        if roll * total < acc {
            return Some(i);
        }
    }
    Some(7)
}

/// Shape of a biome's bird call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Family {
    /// A falling clean note.
    #[default]
    Whistle,
    /// Fast warble over short notes.
    Trill,
    /// Rapid two-formant chirps with pitch steps.
    Chatter,
    /// One drawn-out noisy note.
    Caw,
}

/// Where calls come from around the listener.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placement {
    /// Bird emitter weight per compass sector; all zero means no birds.
    pub canopy: [f32; 8],
    /// Frog chorus weight per sector, from still water.
    pub water: [f32; 8],
    /// Listener yaw in turns, 0 facing -z, so sector bearings pan correctly.
    pub yaw: f32,
}

/// One biome's animal voice. Levels are 0 to 1; the game scales them by time of day.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Fauna {
    pub birds: f32,
    /// Centre pitch of a bird note, Hz.
    pub bird_hz: f32,
    /// Bird calls started per second at full level.
    pub calls_per_s: f32,
    /// 0 a clean whistle, 1 a fast warble.
    pub trill: f32,
    pub frogs: f32,
    /// Fundamental of a croak, Hz.
    pub frog_hz: f32,
    #[serde(default)]
    pub family: Family,
}

impl Fauna {
    /// Weighted blend; used to cross a biome border without a jump.
    pub fn mix(self, other: Fauna, t: f32) -> Fauna {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Fauna {
            birds: l(self.birds, other.birds),
            bird_hz: l(self.bird_hz, other.bird_hz),
            calls_per_s: l(self.calls_per_s, other.calls_per_s),
            trill: l(self.trill, other.trill),
            frogs: l(self.frogs, other.frogs),
            frog_hz: l(self.frog_hz, other.frog_hz),
            family: if t < 0.5 { self.family } else { other.family },
        }
    }

    fn clean(self) -> Fauna {
        let f = |v: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { lo };
        Fauna {
            birds: f(self.birds, 0.0, 1.0),
            bird_hz: f(self.bird_hz, 800.0, 8000.0),
            calls_per_s: f(self.calls_per_s, 0.0, 6.0),
            trill: f(self.trill, 0.0, 1.0),
            frogs: f(self.frogs, 0.0, 1.0),
            frog_hz: f(self.frog_hz, 80.0, 1200.0),
            family: self.family,
        }
    }
}

/// A bird call: a phrase of falling notes with an optional warble.
#[derive(Clone, Copy, Default)]
struct Call {
    notes: u32,
    age: u32,
    len: u32,
    gap: u32,
    voice: Voice,
    hz: f32,
    sweep: f32,
    gain: f32,
    /// Direction of the bird in turns clockwise from -z.
    bearing: f32,
}

/// A croak: a pulsed low tone.
#[derive(Clone, Copy, Default)]
struct Croak {
    age: u32,
    len: u32,
    phase: f32,
    pulse: f32,
    hz: f32,
    gain: f32,
    bearing: f32,
}

const CALLS: usize = 4;
const CROAKS: usize = 3;
/// Cross-fade time constant for levels, seconds.
const FADE_S: f32 = 3.0;
/// Yaw smoothing time constant, seconds; a snap would zipper every active call.
const YAW_S: f32 = 0.05;
/// Share of calls from a sector that start close, scaled by the sector's canopy.
const NEAR_SHARE: f32 = 0.3;

pub(crate) struct Chorus {
    rate: f32,
    pub(crate) target: Fauna,
    placement: Placement,
    /// Smoothed listener yaw in turns.
    yaw: f32,
    yaw_target: f32,
    yaw_fade: f32,
    level: [f32; 2],
    fade: f32,
    noise: Noise,
    calls: [Call; CALLS],
    croaks: [Croak; CROAKS],
}

impl Chorus {
    pub(crate) fn new(rate: u32) -> Self {
        let rate = rate as f32;
        Self {
            rate,
            target: Fauna::default(),
            placement: Placement::default(),
            yaw: 0.0,
            yaw_target: 0.0,
            yaw_fade: 1.0 - (-1.0 / (YAW_S * rate)).exp(),
            level: [0.0; 2],
            fade: 1.0 - (-1.0 / (FADE_S * rate)).exp(),
            noise: Noise::new(0x6a09_e667),
            calls: [Call::default(); CALLS],
            croaks: [Croak::default(); CROAKS],
        }
    }

    pub(crate) fn set(&mut self, voice: Fauna, placement: Placement) {
        self.target = voice.clean();
        let clean = |w: [f32; 8]| w.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 });
        // The snapshot yaw is for headless use; the live yaw arrives per buffer through `yaw`,
        // before placements drain, so it must not be overwritten here.
        self.placement = Placement { canopy: clean(placement.canopy), water: clean(placement.water), yaw: placement.yaw };
    }

    /// Listener yaw in turns, updated per output buffer; overrides the placement's snapshot.
    pub(crate) fn yaw(&mut self, turns: f32) {
        if turns.is_finite() {
            self.yaw_target = turns;
        }
    }

    fn mix(out: &mut [f32; 2], s: f32, pan: f32) {
        out[0] += s * (1.0 - pan) * 0.5;
        out[1] += s * (1.0 + pan) * 0.5;
    }

    pub(crate) fn frame(&mut self) -> [f32; 2] {
        // Shortest way round the circle, so a wrap at one turn does not spin the pan.
        let delta = (self.yaw_target - self.yaw + 0.5).rem_euclid(1.0) - 0.5;
        self.yaw += delta * self.yaw_fade;
        let v = self.target;
        self.level[0] += (v.birds - self.level[0]) * self.fade;
        self.level[1] += (v.frogs - self.level[1]) * self.fade;
        let [birds, frogs] = self.level;
        let mut out = [0.0f32; 2];
        if birds > 1.0e-4 || self.calls.iter().any(|c| c.notes > 0) {
            self.birds(v, birds, &mut out);
        }
        if frogs > 1.0e-4 || self.croaks.iter().any(|c| c.len > 0) {
            self.frogs(v, frogs, &mut out);
        }
        out
    }

    fn birds(&mut self, v: Fauna, level: f32, out: &mut [f32; 2]) {
        if self.noise.unit() < v.calls_per_s * level / self.rate {
            let roll = self.noise.unit();
            if let Some(sector) = draw_sector(&self.placement.canopy, roll)
                && let Some(slot) = self.calls.iter_mut().find(|c| c.notes == 0)
            {
                let n = &mut self.noise;
                let shape = families::shape(v.family, n, self.rate);
                // Most birds are far off in the crown; a few sit close when the canopy is dense.
                let near = n.unit() < self.placement.canopy[sector] * NEAR_SHARE;
                let gain = if near { 0.06 + n.unit() * 0.03 } else { 0.02 + n.unit() * 0.02 };
                let hz = v.bird_hz * (0.8 + n.unit() * 0.45);
                *slot = Call {
                    notes: shape.notes,
                    age: 0,
                    len: shape.len.max(1),
                    gap: shape.gap,
                    voice: Voice::start(v.family, hz, n, self.rate),
                    hz,
                    sweep: shape.sweep,
                    gain,
                    bearing: bearing(sector) + n.next() * 0.03,
                };
            }
        }
        for c in &mut self.calls {
            if c.notes == 0 {
                continue;
            }
            if c.age < c.len {
                let t = c.age as f32 / c.len as f32;
                // Each note falls from above its pitch, like most simple songs.
                let hz = c.hz * (1.0 + c.sweep * (1.0 - t));
                let env = sine(t * 0.5);
                let s = c.voice.sample(v.family, hz, t, v.trill, self.rate) * env * env * c.gain * level;
                Self::mix(out, s, pan_of(c.bearing, self.yaw));
            }
            c.age += 1;
            if c.age >= c.len + c.gap {
                c.notes -= 1;
                c.age = 0;
                c.hz *= families::next_note(v.family, &mut self.noise);
                c.voice = Voice::start(v.family, c.hz, &mut self.noise, self.rate);
            }
        }
    }

    fn frogs(&mut self, v: Fauna, level: f32, out: &mut [f32; 2]) {
        if self.noise.unit() < 1.6 * level / self.rate {
            let roll = self.noise.unit();
            if let Some(sector) = draw_sector(&self.placement.water, roll)
                && let Some(slot) = self.croaks.iter_mut().find(|c| c.len == 0)
            {
                let n = &mut self.noise;
                *slot = Croak {
                    age: 0,
                    len: ((0.12 + n.unit() * 0.25) * self.rate) as u32,
                    phase: 0.0,
                    pulse: 0.0,
                    hz: v.frog_hz * (0.85 + n.unit() * 0.3),
                    gain: 0.04 + n.unit() * 0.06,
                    bearing: bearing(sector) + n.next() * 0.04,
                };
            }
        }
        for c in &mut self.croaks {
            if c.len == 0 {
                continue;
            }
            let t = c.age as f32 / c.len as f32;
            advance(&mut c.phase, c.hz, self.rate);
            advance(&mut c.pulse, 24.0, self.rate);
            let gate = sine(c.pulse).max(0.0);
            let tone = sine(c.phase) * 0.7 + sine(c.phase * 2.0) * 0.3;
            let s = tone * gate * sine(t * 0.5) * c.gain * level;
            Self::mix(out, s, pan_of(c.bearing, self.yaw));
            c.age += 1;
            if c.age >= c.len {
                c.len = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(voice: Fauna) -> f32 {
        let mut chorus = Chorus::new(48_000);
        chorus.set(voice, Placement { canopy: [1.0; 8], water: [1.0; 8], yaw: 0.0 });
        chorus.level = [voice.birds, voice.frogs];
        let frames = 48_000 * 8;
        let mut sum = 0.0f64;
        for _ in 0..frames {
            let [l, r] = chorus.frame();
            assert!(l.is_finite() && r.is_finite());
            sum += (l * l + r * r) as f64 * 0.5;
        }
        (sum / frames as f64).sqrt() as f32
    }

    const MEADOW: Fauna = Fauna { birds: 1.0, bird_hz: 3200.0, calls_per_s: 2.0, trill: 0.3, frogs: 0.0, frog_hz: 300.0, family: Family::Trill };

    fn energy(chorus: &mut Chorus, frames: usize) -> (f64, f64) {
        let (mut l, mut r) = (0.0f64, 0.0f64);
        for _ in 0..frames {
            let [a, b] = chorus.frame();
            l += (a * a) as f64;
            r += (b * b) as f64;
        }
        (l, r)
    }

    #[test]
    fn calls_come_from_the_canopy_sector() {
        let mut p = Placement::default();
        p.canopy[0] = 1.0; // east
        let mut chorus = Chorus::new(48_000);
        chorus.set(MEADOW, p);
        chorus.level = [1.0, 0.0];
        chorus.yaw(0.0); // facing north: east is on the right
        let (l, r) = energy(&mut chorus, 48_000 * 6);
        assert!(r > l * 4.0, "left {l} right {r}");
        chorus.yaw(0.5); // facing south: east is on the left
        let (l, r) = energy(&mut chorus, 48_000 * 6);
        assert!(l > r * 4.0, "left {l} right {r}");
    }

    #[test]
    fn a_placement_snapshot_never_overrides_the_live_yaw() {
        // The output loop applies the live yaw before it drains placements; a stale
        // snapshot must not swing the pan back toward the old heading.
        let mut p = Placement::default();
        p.canopy[0] = 1.0; // east
        p.yaw = 0.5; // snapshot taken facing south
        let mut chorus = Chorus::new(48_000);
        chorus.yaw(0.0); // live: facing north, east on the right
        chorus.set(MEADOW, p);
        chorus.level = [1.0, 0.0];
        let (l, r) = energy(&mut chorus, 48_000 * 6);
        assert!(r > l * 4.0, "left {l} right {r}");
    }

    #[test]
    fn no_canopy_means_no_new_calls_and_a_phrase_still_ends() {
        let mut chorus = Chorus::new(48_000);
        chorus.set(MEADOW, Placement { canopy: [1.0; 8], ..Default::default() });
        chorus.level = [1.0, 0.0];
        for _ in 0..48_000 {
            chorus.frame();
        }
        chorus.set(MEADOW, Placement::default());
        let mut tail = 0.0f32;
        for i in 0..48_000 * 4 {
            let [a, _] = chorus.frame();
            if i > 48_000 * 2 {
                tail += a.abs();
            }
        }
        assert_eq!(tail, 0.0);
    }

    /// Largest sample-to-sample jump over 4 s, with the listener either still or spinning once a second.
    fn worst_step(spin: bool) -> f32 {
        let mut p = Placement::default();
        p.canopy[2] = 1.0;
        let mut chorus = Chorus::new(48_000);
        chorus.set(MEADOW, p);
        chorus.level = [1.0, 0.0];
        let mut last = [0.0f32; 2];
        let mut worst = 0.0f32;
        for i in 0..48_000 * 4 {
            // The still listener faces west, so the south sector is hard-panned like the spin's peaks.
            chorus.yaw(if spin { i as f32 / 48_000.0 } else { 0.75 });
            let s = chorus.frame();
            assert!(s[0].is_finite() && s[1].is_finite());
            worst = worst.max((s[0] - last[0]).abs()).max((s[1] - last[1]).abs());
            last = s;
        }
        worst
    }

    #[test]
    fn yaw_changes_pan_without_steps() {
        // The same noise seed starts the same calls; only the pan differs.
        let (still, spin) = (worst_step(false), worst_step(true));
        assert!(spin < still * 1.5 + 1e-4, "still {still} spinning {spin}");
    }

    /// Spread (standard deviation over mean) of the intervals between zero crossings over 8 s
    /// of one family at full canopy: a swept sine keeps them even, band-passed noise does not.
    fn crossing_spread(family: Family) -> f32 {
        let mut chorus = Chorus::new(48_000);
        chorus.set(Fauna { family, ..MEADOW }, Placement { canopy: [1.0; 8], ..Default::default() });
        chorus.level = [1.0, 0.0];
        let (mut last, mut since) = (0.0f32, 0u32);
        let mut intervals = Vec::new();
        for _ in 0..48_000 * 8 {
            let [l, _] = chorus.frame();
            since += 1;
            if l != 0.0 && (l > 0.0) != (last > 0.0) {
                if last != 0.0 && since < 200 {
                    intervals.push(since as f32);
                }
                since = 0;
            }
            last = l;
        }
        let n = intervals.len().max(1) as f32;
        let mean = intervals.iter().sum::<f32>() / n;
        let var = intervals.iter().map(|i| (i - mean) * (i - mean)).sum::<f32>() / n;
        var.sqrt() / mean.max(1e-6)
    }

    #[test]
    fn every_family_renders_in_the_chorus_band() {
        for family in [Family::Whistle, Family::Trill, Family::Chatter, Family::Caw] {
            let v = rms(Fauna { family, ..MEADOW });
            assert!((0.002..0.08).contains(&v), "{family:?} {v}");
        }
    }

    #[test]
    fn caw_is_noisier_than_whistle() {
        let (caw, whistle) = (crossing_spread(Family::Caw), crossing_spread(Family::Whistle));
        assert!(caw > whistle * 2.0, "caw {caw} whistle {whistle}");
    }

    #[test]
    fn mix_keeps_the_dominant_family() {
        let a = Fauna { family: Family::Caw, ..MEADOW };
        let b = Fauna { family: Family::Trill, ..MEADOW };
        assert_eq!(a.mix(b, 0.3).family, Family::Caw);
        assert_eq!(a.mix(b, 0.7).family, Family::Trill);
    }

    #[test]
    fn silent_voice_is_silent() {
        assert_eq!(rms(Fauna::default()), 0.0);
    }

    #[test]
    fn chorus_sits_below_the_beds() {
        let birds = rms(MEADOW);
        let frogs = rms(Fauna { birds: 0.0, frogs: 1.0, ..MEADOW });
        assert!((0.002..0.08).contains(&birds), "birds {birds}");
        assert!((0.002..0.08).contains(&frogs), "frogs {frogs}");
    }

    #[test]
    fn denser_biomes_are_louder() {
        assert!(rms(Fauna { calls_per_s: 5.0, ..MEADOW }) > rms(Fauna { calls_per_s: 0.5, ..MEADOW }));
    }
}
