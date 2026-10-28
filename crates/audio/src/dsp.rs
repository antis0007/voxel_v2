//! Small real-time DSP parts shared by the ambience, music and footstep layers.

/// Deterministic xorshift noise; seeded per layer so beds stay decorrelated.
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

    #[test]
    fn the_parabolic_sine_tracks_the_library_sine() {
        for i in 0..4096 {
            let turns = i as f32 / 4096.0 * 3.0 - 1.0;
            let error = (sine(turns) - (turns * std::f32::consts::TAU).sin()).abs();
            assert!(error < 2.0e-3, "turns {turns}: error {error}");
        }
    }

    #[test]
    fn hashes_spread_neighbouring_inputs() {
        assert_ne!(hash(1, 2), hash(2, 1));
        let ones: u32 = (0..64).map(|i| hash(7, i).count_ones()).sum();
        assert!((64 * 12..64 * 20).contains(&ones), "bit balance {ones}");
    }
}
