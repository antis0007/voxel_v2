//! Footstep sounds: a few synthesized renders per walking surface.
use crate::dsp::{advance, coefficient, sine, Noise, OnePole};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Surface {
    Stone,
    /// Dirt, mud, clay and gravel.
    Soil,
    /// Grass, moss and foliage.
    Grass,
    Sand,
    Wood,
    Snow,
    Metal,
    /// Wading through shallow water.
    Water,
}

impl Surface {
    pub const COUNT: usize = 8;
    pub const ALL: [Surface; Surface::COUNT] = [
        Surface::Stone,
        Surface::Soil,
        Surface::Grass,
        Surface::Sand,
        Surface::Wood,
        Surface::Snow,
        Surface::Metal,
        Surface::Water,
    ];
}

pub(crate) const VARIANTS: usize = 4;
const MAX_VOICES: usize = 6;

/// Every surface's renders, indexed `surface * VARIANTS + variant`.
pub(crate) fn bank(rate: u32) -> Vec<Vec<f32>> {
    Surface::ALL
        .iter()
        .flat_map(|&surface| (0..VARIANTS).map(move |variant| render(surface, variant, rate)))
        .collect()
}

fn render(surface: Surface, variant: usize, rate: u32) -> Vec<f32> {
    let rate_f = rate as f32;
    let length = match surface {
        Surface::Metal => 0.32,
        Surface::Water => 0.30,
        Surface::Snow | Surface::Sand => 0.18,
        Surface::Grass => 0.16,
        _ => 0.13,
    };
    let mut noise = Noise::new(0x5eed_0000 ^ ((surface as u32) << 8) ^ (variant as u32 * 0x9e37));
    let shift = 1.0 + (variant as f32 - 1.5) * 0.04;
    let frames = (length * rate_f) as usize;
    let mut low = OnePole::default();
    let mut mid = OnePole::default();
    let mut grain = 0.0f32;
    let mut phase = [0.0f32; 3];
    let (a_low, a_mid) = (coefficient(260.0 * shift, rate_f), coefficient(1800.0 * shift, rate_f));
    let mut data = Vec::with_capacity(frames);
    for i in 0..frames {
        let t = i as f32 / rate_f;
        let u = t / length;
        let white = noise.next();
        let soft = low.low(white, a_low);
        let mid_v = mid.low(white, a_mid);
        let band = mid_v - soft;
        let high = white - mid_v;
        let attack = (t / 0.003).min(1.0);
        let tail = (1.0 - u).max(0.0).powi(2);
        let sample = match surface {
            Surface::Stone => {
                advance(&mut phase[0], 140.0 * shift * (1.0 - u * 0.3), rate_f);
                high * 0.35 * (-t * 120.0).exp() + sine(phase[0]) * 0.5 * (-t * 45.0).exp()
                    + band * 0.5 * (-t * 60.0).exp()
            }
            Surface::Soil => soft * 4.0 * (-t * 30.0).exp() + band * 0.6 * (-t * 50.0).exp(),
            Surface::Grass => {
                let rustle = 0.6 + 0.4 * sine(t * 70.0 * shift);
                band * 1.1 * rustle * (u * std::f32::consts::PI).sin() + soft * 1.5 * (-t * 40.0).exp()
            }
            Surface::Sand | Surface::Snow => {
                // Granular crunch: sparse noise grains, denser and duller for snow.
                let density = if surface == Surface::Snow { 0.03 } else { 0.012 };
                if noise.unit() < density {
                    grain = 0.7 + noise.unit() * 0.3;
                }
                grain *= if surface == Surface::Snow { 0.993 } else { 0.985 };
                let body = if surface == Surface::Snow { band * 0.9 + soft * 2.5 } else { high * 0.5 + band * 0.7 };
                body * grain * (u * std::f32::consts::PI).sin().max(0.0).sqrt()
            }
            Surface::Wood => {
                advance(&mut phase[0], 190.0 * shift, rate_f);
                advance(&mut phase[1], 433.0 * shift, rate_f);
                (sine(phase[0]) * 0.55 + sine(phase[1]) * 0.25) * (-t * 38.0).exp()
                    + high * 0.25 * (-t * 150.0).exp()
            }
            Surface::Metal => {
                advance(&mut phase[0], 520.0 * shift, rate_f);
                advance(&mut phase[1], 1371.0 * shift, rate_f);
                advance(&mut phase[2], 2236.0 * shift, rate_f);
                (sine(phase[0]) * 0.30 + sine(phase[1]) * 0.20 * (-t * 14.0).exp() + sine(phase[2]) * 0.10 * (-t * 22.0).exp())
                    * (-t * 9.0).exp()
                    + soft * 2.5 * (-t * 50.0).exp()
                    + high * 0.2 * (-t * 160.0).exp()
            }
            Surface::Water => {
                // Slosh plus two bubbles whose pitch rises as they close.
                let bubble = |p: &mut f32, start: f32, hz: f32| {
                    let local = t - start;
                    if local < 0.0 {
                        return 0.0;
                    }
                    advance(p, hz * shift * (1.0 + local * 12.0), rate_f);
                    sine(*p) * (-local * 30.0).exp()
                };
                soft * 3.5 * (u * std::f32::consts::PI).sin()
                    + band * 0.5 * (-t * 12.0).exp()
                    + bubble(&mut phase[0], 0.03, 620.0) * 0.18
                    + bubble(&mut phase[1], 0.11, 880.0) * 0.12
            }
        };
        data.push(sample * attack * tail);
    }
    let peak = data.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1.0e-6);
    for x in &mut data {
        *x *= 0.8 / peak;
    }
    data
}

pub(crate) struct StepPlay {
    pub(crate) surface: Surface,
    pub(crate) variant: usize,
    pub(crate) left: f32,
    pub(crate) right: f32,
    pub(crate) rate: f32,
}

pub(crate) struct Steps {
    bank: Vec<Vec<f32>>,
    voices: Vec<(StepPlay, f32)>,
}

impl Steps {
    pub(crate) fn new(rate: u32) -> Self {
        Self { bank: bank(rate), voices: Vec::with_capacity(MAX_VOICES) }
    }

    pub(crate) fn start(&mut self, play: StepPlay) {
        if self.voices.len() >= MAX_VOICES {
            self.voices.remove(0);
        }
        self.voices.push((play, 0.0));
    }

    pub(crate) fn clear(&mut self) {
        self.voices.clear();
    }

    pub(crate) fn frame(&mut self) -> [f32; 2] {
        let mut out = [0.0f32; 2];
        for (play, cursor) in &mut self.voices {
            let sample = &self.bank[play.surface as usize * VARIANTS + play.variant % VARIANTS];
            let i = *cursor as usize;
            if i + 1 < sample.len() {
                let value = sample[i] + (sample[i + 1] - sample[i]) * cursor.fract();
                out[0] += value * play.left;
                out[1] += value * play.right;
                *cursor += play.rate;
            } else {
                *cursor = f32::MAX;
            }
        }
        self.voices.retain(|(_, cursor)| *cursor < f32::MAX);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_surface_has_distinct_finite_declicked_renders() {
        let bank = bank(48_000);
        assert_eq!(bank.len(), Surface::COUNT * VARIANTS);
        for (index, render) in bank.iter().enumerate() {
            assert!(render.iter().all(|x| x.is_finite() && x.abs() <= 0.81), "render {index}");
            assert!(render.last().is_some_and(|x| x.abs() < 0.02), "render {index} ends in a click");
            let energy = render.iter().map(|x| x * x).sum::<f32>() / render.len() as f32;
            assert!(energy > 1.0e-4, "render {index} is silent");
        }
        for a in 0..Surface::COUNT {
            for b in a + 1..Surface::COUNT {
                assert_ne!(bank[a * VARIANTS], bank[b * VARIANTS], "{a} and {b} sound the same");
            }
        }
    }

    #[test]
    fn step_voices_finish_and_are_bounded() {
        let mut steps = Steps::new(48_000);
        for i in 0..10 {
            steps.start(StepPlay { surface: Surface::ALL[i % 8], variant: i, left: 0.5, right: 0.5, rate: 1.0 });
        }
        assert_eq!(steps.voices.len(), MAX_VOICES);
        for _ in 0..48_000 {
            steps.frame();
        }
        assert!(steps.voices.is_empty());
    }
}
