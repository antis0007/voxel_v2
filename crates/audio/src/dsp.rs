//! Small real-time DSP parts shared by the ambience, music and footstep layers.

/// Deterministic xorshift noise; seeded per layer so beds stay decorrelated.
#[derive(Clone, Copy, Default)]
pub(crate) struct Noise(u32);

impl Noise {
    pub(crate) fn new(seed: u32) -> Self {
        Self(seed | 1)
    }
    /// White noise in -1..1.
    pub(crate) fn next(&mut self) -> f32 {
        let mut s = self.0;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.0 = s;
        s as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
    /// Uniform in 0..1.
    pub(crate) fn unit(&mut self) -> f32 {
        self.next() * 0.5 + 0.5
    }
}

/// Sine of a phase in turns, from a parabola with one refinement step.
///
/// Peak error is about 0.1 percent; `f32::sin` costs several times more per
/// voice and the mixer evaluates a few dozen of these per frame.
pub(crate) fn sine(turns: f32) -> f32 {
    let p = turns - turns.floor();
    let t = if p < 0.5 { 2.0 * p } else { 2.0 * p - 2.0 };
    let y = 4.0 * t - 4.0 * t * t.abs();
    0.225 * (y * y.abs() - y) + y
}

/// Advances a phase in turns by `hz` at `rate`, wrapped to 0..1.
pub(crate) fn advance(phase: &mut f32, hz: f32, rate: f32) {
    *phase += hz / rate;
    if *phase >= 1.0 {
        *phase -= phase.floor();
    }
}

/// One-pole smoothing coefficient for a cutoff in hertz.
pub(crate) fn coefficient(hz: f32, rate: f32) -> f32 {
    1.0 - (-std::f32::consts::TAU * hz / rate).exp()
}

#[derive(Clone, Copy, Default)]
pub(crate) struct OnePole(f32);

impl OnePole {
    pub(crate) fn low(&mut self, x: f32, a: f32) -> f32 {
        self.0 += (x - self.0) * a;
        self.0
    }
}

/// Thins a stereo signal as the air around the listener thins.
///
/// In vacuum only what carries through the suit and body is left: a low thud
/// under 250 Hz at about a third of the level.
pub(crate) struct Muffle {
    air: f32,
    a: f32,
    state: [OnePole; 2],
}

impl Muffle {
    const VACUUM_HZ: f32 = 250.0;
    const OPEN_HZ: f32 = 18_000.0;
    const VACUUM_GAIN: f32 = 0.35;

    pub(crate) fn new() -> Self {
        Self { air: 1.0, a: 1.0, state: [OnePole::default(); 2] }
    }

    /// Glides toward `air` (0 vacuum, 1 sea level); called once per output buffer.
    pub(crate) fn approach(&mut self, air: f32, step: f32, rate: f32) {
        self.air += (air.clamp(0.0, 1.0) - self.air) * step;
        // Exponential sweep so the cutoff falls evenly by ear.
        let hz = Self::VACUUM_HZ * (Self::OPEN_HZ / Self::VACUUM_HZ).powf(self.air);
        self.a = coefficient(hz, rate).min(1.0);
    }

    pub(crate) fn air(&self) -> f32 {
        self.air
    }

    pub(crate) fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        if self.air > 0.999 {
            // Keep the state current so leaving the open air does not click.
            self.state = [OnePole(x[0]), OnePole(x[1])];
            return x;
        }
        let gain = Self::VACUUM_GAIN + (1.0 - Self::VACUUM_GAIN) * self.air;
        [self.state[0].low(x[0], self.a) * gain, self.state[1].low(x[1], self.a) * gain]
    }
}

/// Two-pole state-variable filter (topology-preserving form).
///
/// One-pole differences roll off at 6 dB per octave and leak raw noise up to
/// Nyquist, which reads as fizz; this gives 12 dB slopes and a real band-pass.
#[derive(Clone, Copy, Default)]
pub(crate) struct Svf {
    ic1: f32,
    ic2: f32,
    g: f32,
    k: f32,
}

impl Svf {
    pub(crate) fn new(hz: f32, q: f32, rate: f32) -> Self {
        let mut filter = Self::default();
        filter.tune(hz, q, rate);
        filter
    }
    /// Retunes without clearing state, so a sweep stays continuous.
    pub(crate) fn tune(&mut self, hz: f32, q: f32, rate: f32) {
        self.g = (std::f32::consts::PI * hz.clamp(10.0, rate * 0.45) / rate).tan();
        self.k = 1.0 / q.max(0.05);
    }
    /// Low-pass, band-pass and high-pass outputs for one sample.
    pub(crate) fn run(&mut self, x: f32) -> [f32; 3] {
        let a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        let a2 = self.g * a1;
        let a3 = self.g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        [v2, v1, x - self.k * v1 - v2]
    }
    pub(crate) fn low(&mut self, x: f32) -> f32 {
        self.run(x)[0]
    }
    /// Band-pass whose ring after a short strike is equally tall at any centre;
    /// Q then sets only the ring time, about Q / (pi * hz) seconds.
    pub(crate) fn ring(&mut self, x: f32) -> f32 {
        self.run(x)[1] / self.g
    }
    /// Band-pass scaled to unity gain at the centre, whatever the Q.
    pub(crate) fn band(&mut self, x: f32) -> f32 {
        self.run(x)[1] * self.k
    }
}

/// Seeded integer hash; music and footstep choices use it instead of a RNG.
pub fn hash(a: u32, b: u32) -> u32 {
    let mut h = a ^ b.wrapping_mul(0x9e37_79b9).rotate_left(5);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    h
}

/// Hash mapped to 0..1.
pub(crate) fn unit(a: u32, b: u32) -> f32 {
    (hash(a, b) >> 8) as f32 / (1u32 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(muffle: &mut Muffle, hz: f32) -> f32 {
        let mut phase = 0.0;
        (0..48_000).map(|i| {
            advance(&mut phase, hz, 48_000.0);
            let y = muffle.process([sine(phase); 2])[0].abs();
            if i > 24_000 { y } else { 0.0 }
        }).fold(0.0, f32::max)
    }

    #[test]
    fn vacuum_keeps_the_thud_and_drops_the_hiss() {
        let mut open = Muffle::new();
        open.approach(1.0, 1.0, 48_000.0);
        assert!(peak(&mut open, 4_000.0) > 0.99);
        let mut vacuum = Muffle::new();
        vacuum.approach(0.0, 1.0, 48_000.0);
        let (low, high) = (peak(&mut vacuum, 60.0), peak(&mut vacuum, 4_000.0));
        assert!(low > 0.3 && high < 0.03, "low {low} high {high}");
    }

    #[test]
    fn the_parabolic_sine_tracks_the_library_sine() {
        for i in 0..4096 {
            let turns = i as f32 / 4096.0 * 3.0 - 1.0;
            let error = (sine(turns) - (turns * std::f32::consts::TAU).sin()).abs();
            assert!(error < 2.0e-3, "turns {turns}: error {error}");
        }
    }

    #[test]
    fn the_state_variable_filter_passes_its_band_and_rejects_the_rest() {
        let rate = 48_000.0;
        let gain = |hz: f32, band: bool| {
            let mut filter = Svf::new(1000.0, if band { 1.0 } else { 0.707 }, rate);
            let mut peak = 0.0f32;
            for i in 0..48_000 {
                let x = (i as f32 * hz / rate * std::f32::consts::TAU).sin();
                let y = if band { filter.band(x) } else { filter.low(x) };
                if i > 24_000 {
                    peak = peak.max(y.abs());
                }
            }
            peak
        };
        assert!(gain(100.0, false) > 0.95 && gain(8000.0, false) < 0.03, "low-pass");
        assert!(gain(1000.0, true) > 0.97 && gain(100.0, true) < 0.12 && gain(10_000.0, true) < 0.12, "band-pass");
    }

    #[test]
    fn hashes_spread_neighbouring_inputs() {
        assert_ne!(hash(1, 2), hash(2, 1));
        let ones: u32 = (0..64).map(|i| hash(7, i).count_ones()).sum();
        assert!((64 * 12..64 * 20).contains(&ones), "bit balance {ones}");
    }
}
