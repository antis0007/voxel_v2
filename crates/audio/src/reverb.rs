//! Diffuse stereo room: damped feedback combs into series all-passes.
//!
//! One delay line with feedback repeats at a single period, heard as a
//! metallic flutter. Eight combs with unrelated lengths build echo density,
//! the all-passes smear it, and the damping in each comb lets highs die first
//! as stone and air absorb them.

/// Comb and all-pass lengths in samples at 44.1 kHz, scaled to the rate.
const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
/// Right channel lengths are offset, so the tail is wide instead of centred.
const SPREAD: usize = 23;

struct Comb {
    line: Vec<f32>,
    index: usize,
    store: f32,
}

impl Comb {
    fn process(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let out = self.line[self.index];
        self.store = out + (self.store - out) * damp;
        self.line[self.index] = x + self.store * feedback;
        self.index = (self.index + 1) % self.line.len();
        out
    }
}

struct AllPass {
    line: Vec<f32>,
    index: usize,
}

impl AllPass {
    fn process(&mut self, x: f32) -> f32 {
        let delayed = self.line[self.index];
        self.line[self.index] = x + delayed * 0.5;
        self.index = (self.index + 1) % self.line.len();
        delayed - x
    }
}

struct Channel {
    combs: Vec<Comb>,
    passes: Vec<AllPass>,
}

impl Channel {
    fn new(rate: u32, offset: usize) -> Self {
        let scale = |n: usize| ((n + offset) as f32 * rate as f32 / 44_100.0) as usize;
        Self {
            combs: COMBS.iter().map(|&n| Comb { line: vec![0.0; scale(n)], index: 0, store: 0.0 }).collect(),
            passes: ALLPASSES.iter().map(|&n| AllPass { line: vec![0.0; scale(n)], index: 0 }).collect(),
        }
    }

    fn process(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let mut sum = 0.0;
        for comb in &mut self.combs {
            sum += comb.process(x, feedback, damp);
        }
        self.passes.iter_mut().fold(sum, |y, pass| pass.process(y))
    }
}

pub(crate) struct Reverb {
    left: Channel,
    right: Channel,
    pre: Vec<f32>,
    pre_index: usize,
    /// Smoothed openness: 0 is open air, 1 an enclosed cave.
    space: f32,
}

impl Reverb {
    pub(crate) fn new(rate: u32) -> Self {
        Self {
            left: Channel::new(rate, 0),
            right: Channel::new(rate, SPREAD),
            // Early reflections arrive about 14 ms after the dry sound.
            pre: vec![0.0; (rate as f32 * 0.014) as usize],
            pre_index: 0,
            space: 0.0,
        }
    }

    /// Moves toward a new enclosure over about a second at the call rate.
    pub(crate) fn approach(&mut self, space: f32, step: f32) {
        self.space += (space.clamp(0.0, 1.0) - self.space) * step;
    }

    /// Returns the wet pair for one mono send sample.
    pub(crate) fn process(&mut self, send: f32) -> [f32; 2] {
        let delayed = self.pre[self.pre_index];
        self.pre[self.pre_index] = send;
        self.pre_index = (self.pre_index + 1) % self.pre.len();
        // Caves ring longer and keep more top end off hard stone.
        let feedback = 0.70 + 0.16 * self.space;
        let damp = 0.42 - 0.17 * self.space;
        // Open air keeps only a hint of space; Minecraft-style outdoor sound
        // is close to dry, and the wash belongs to enclosed rock.
        let wet = (0.15 + 1.35 * self.space) * 3.0;
        let input = delayed * 0.015;
        [
            self.left.process(input, feedback, damp) * wet,
            self.right.process(input, feedback, damp) * wet,
        ]
    }

    #[cfg(test)]
    pub(crate) fn space(&self) -> f32 {
        self.space
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn impulse_tail(space: f32) -> (f32, f32, Vec<f32>) {
        let mut reverb = Reverb::new(48_000);
        reverb.space = space;
        let mut out = Vec::new();
        out.push(reverb.process(1.0)[0]);
        for _ in 0..48_000 * 3 {
            out.push(reverb.process(0.0)[0]);
        }
        let energy = |range: std::ops::Range<usize>| out[range].iter().map(|x| x * x).sum::<f32>();
        (energy(0..24_000), energy(96_000..144_000), out)
    }

    #[test]
    fn the_tail_is_finite_decays_and_caves_ring_longer() {
        let (early_open, late_open, open) = impulse_tail(0.0);
        let (early_cave, late_cave, _) = impulse_tail(1.0);
        assert!(open.iter().all(|x| x.is_finite()));
        assert!(early_open > 0.0 && late_open < early_open * 1.0e-3, "open air {early_open} {late_open}");
        assert!(late_cave / early_cave > late_open / early_open, "a cave must decay slower");
    }

    #[test]
    fn the_tail_is_dense_not_a_single_repeating_echo() {
        let (_, _, out) = impulse_tail(0.5);
        // Count 10 ms windows between 50 and 500 ms that hold any energy; a
        // single delay line leaves most of them empty between its repeats.
        let window = 480;
        let busy = (5..50)
            .filter(|w| out[w * window..(w + 1) * window].iter().any(|x| x.abs() > 1.0e-5))
            .count();
        assert!(busy >= 44, "{busy} of 45 windows hold reverb");
    }

    #[test]
    fn space_follows_its_target_smoothly() {
        let mut reverb = Reverb::new(48_000);
        reverb.approach(1.0, 0.1);
        assert!((reverb.space() - 0.1).abs() < 1.0e-6);
        reverb.approach(f32::NAN.max(2.0), 1.0);
        assert_eq!(reverb.space(), 1.0);
    }
}
