//! Inside-the-helmet bed: life-support blower, electrical hum, breathing
//! through a demand valve.
//!
//! Every noise source here is steady or shaped like a breath (fast attack,
//! short hold, quick release). The previous bed swelled one wide noise band
//! with a 1.8 s sine-squared envelope every 4.5 s, which is the envelope and
//! period of surf on a beach.
use crate::dsp::{Noise, Svf, advance, sine};

/// Breath cycle split, in turns: inhale, hold, exhale, then rest.
const INHALE: (f32, f32) = (0.0, 0.28);
const EXHALE: (f32, f32) = (0.34, 0.68);

pub(crate) struct Suit {
    rate: f32,
    noise: [Noise; 2],
    /// Breath cycle, hum and blower phases in turns.
    breath: f32,
    hum: f32,
    blade: [f32; 2],
    /// Inhale resonance, exhale first and second resonances.
    inhale: Svf,
    exhale: [Svf; 2],
    /// Steady blower air per ear and its thin duct whistle.
    air: [Svf; 2],
    whistle: Svf,
    /// Demand-valve snaps: inhale opening, exhale flap.
    valve: [Svf; 2],
    /// Slow random roughness on the exhale (lip and valve flutter).
    flutter: f32,
}

impl Suit {
    pub(crate) fn new(rate: f32) -> Self {
        Self {
            rate,
            noise: [Noise::new(0x7a31_c0d5), Noise::new(0x19e4_2b67)],
            breath: 0.55,
            hum: 0.0,
            blade: [0.0, 0.37],
            inhale: Svf::new(2100.0, 2.6, rate),
            exhale: [Svf::new(720.0, 2.2, rate), Svf::new(1450.0, 3.0, rate)],
            air: [Svf::new(950.0, 0.6, rate), Svf::new(1010.0, 0.6, rate)],
            whistle: Svf::new(2350.0, 16.0, rate),
            valve: [Svf::new(3200.0, 7.0, rate), Svf::new(1700.0, 5.0, rate)],
            flutter: 0.0,
        }
    }

    /// Breath cycle phase in turns, for tests.
    #[cfg(test)]
    pub(crate) fn breath_phase(&self) -> f32 {
        self.breath
    }

    /// One stereo frame. `strain` 0 calm to 1 near death.
    pub(crate) fn frame(&mut self, strain: f32) -> [f32; 2] {
        let strain = strain.clamp(0.0, 1.0);
        let before = self.breath;
        // About 13 breaths a minute at rest, 33 when badly hurt.
        advance(&mut self.breath, 0.22 + 0.33 * strain, self.rate);
        let p = self.breath;
        let crossed = |at: f32| (before < at && p >= at) || (at == 0.0 && p < before);

        let n0 = self.noise[0].next();
        let n1 = self.noise[1].next();
        let mono = (n0 + n1) * 0.5;

        // Valve snaps: a struck resonance, a few milliseconds long.
        let open = if crossed(INHALE.0) { 1.0 } else { 0.0 };
        let flap = if crossed(EXHALE.0) { 1.0 } else { 0.0 };
        let snap = self.valve[0].ring(open) * 0.060 + self.valve[1].ring(flap) * 0.036;

        let inhale_env = breath_envelope(p, INHALE);
        let exhale_env = breath_envelope(p, EXHALE);
        self.flutter += (n0.abs() - self.flutter) * 0.002;
        let rough = 1.0 + (self.flutter - 0.5) * (0.6 + 0.8 * strain);
        let inhale = self.inhale.band(mono) * inhale_env * (0.26 + 0.12 * strain);
        let exhale = (self.exhale[0].band(mono) * 0.7 + self.exhale[1].band(mono) * 0.3)
            * exhale_env * rough * (0.32 + 0.14 * strain);
        let breath = inhale + exhale + snap;

        // Blower: blade tone with two harmonics and a slightly detuned second
        // rotor, steady air and a thin duct whistle. Nothing here swells.
        advance(&mut self.blade[0], 184.0, self.rate);
        advance(&mut self.blade[1], 186.1, self.rate);
        advance(&mut self.hum, 120.0, self.rate);
        let tone = |ph: f32| sine(ph) * 0.6 + sine(ph * 2.0) * 0.25 + sine(ph * 3.0) * 0.1;
        let blower = (tone(self.blade[0]) + tone(self.blade[1])) * 0.018;
        let hum = sine(self.hum) * 0.022;
        let whistle = self.whistle.band(mono) * 0.016;
        let air = [self.air[0].band(n0) * 0.020, self.air[1].band(n1) * 0.020];
        let bed = blower + hum + whistle;
        [bed + air[0] + breath, bed + air[1] + breath]
    }
}

/// Breath loudness through one segment: about a fifth of a second to open,
/// a slowly falling hold, then a quick close. Zero outside the segment.
fn breath_envelope(p: f32, (from, to): (f32, f32)) -> f32 {
    if p <= from || p >= to {
        return 0.0;
    }
    let u = (p - from) / (to - from);
    let open = smooth(u / 0.16);
    let close = 1.0 - smooth((u - 0.70) / 0.30);
    open * close * (1.0 - 0.3 * u)
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breath_envelope_opens_fast_and_stays_inside_its_segment() {
        assert_eq!(breath_envelope(0.30, INHALE), 0.0);
        assert_eq!(breath_envelope(0.80, EXHALE), 0.0);
        // A fifth of the way into the inhale it is already near full: a
        // breath, not a slow swell.
        assert!(breath_envelope(INHALE.0 + 0.2 * (INHALE.1 - INHALE.0), INHALE) > 0.8);
    }

    #[test]
    fn suit_bed_has_no_slow_noise_swell() {
        // Surf is a broadband level that rises and falls over seconds. Measure
        // the low-frequency envelope of the 300-600 Hz band: the blower and air
        // there must stay steady through a whole breath cycle.
        let rate = 48_000.0;
        let mut suit = Suit::new(rate);
        let mut band = Svf::new(420.0, 1.2, rate);
        let window = 4800;
        let mut levels = Vec::new();
        let mut acc = 0.0f32;
        for i in 0..(rate as usize * 10) {
            let [l, _] = suit.frame(0.0);
            let v = band.band(l);
            acc += v * v;
            if (i + 1) % window == 0 {
                levels.push((acc / window as f32).sqrt());
                acc = 0.0;
            }
        }
        let max = levels.iter().cloned().fold(0.0, f32::max);
        let min = levels.iter().cloned().fold(f32::MAX, f32::min);
        assert!(max < min * 2.0, "low-mid level swings {min:.5}..{max:.5}");
    }
}
