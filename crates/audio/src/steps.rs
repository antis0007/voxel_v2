//! Footstep and block contact sounds: a few synthesized renders per surface.
use crate::dsp::{advance, sine, Noise, Svf};
use recipe::{recipe, shaped};

mod recipe;

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

/// How a body meets a surface; each reshapes the surface's step recipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Contact {
    Step,
    /// One strike while a block is being mined.
    Hit,
    /// A block coming apart: a longer crumble.
    Break,
    /// A block set down: a short, dull seat.
    Place,
}

impl Contact {
    pub const COUNT: usize = 4;
    pub const ALL: [Contact; Contact::COUNT] = [Contact::Step, Contact::Hit, Contact::Break, Contact::Place];
}

pub(crate) const VARIANTS: usize = 4;
const MAX_VOICES: usize = 8;

/// Bank position of one render.
pub(crate) fn index(contact: Contact, surface: Surface, variant: usize) -> usize {
    (contact as usize * Surface::COUNT + surface as usize) * VARIANTS + variant % VARIANTS
}

/// Every contact's and surface's renders, in `index` order.
pub(crate) fn bank(rate: u32) -> Vec<Vec<f32>> {
    Contact::ALL
        .iter()
        .flat_map(|&contact| {
            Surface::ALL.iter().flat_map(move |&surface| {
                (0..VARIANTS).map(move |variant| render(contact, surface, variant, rate))
            })
        })
        .collect()
}

fn render(contact: Contact, surface: Surface, variant: usize, rate: u32) -> Vec<f32> {
    let rate_f = rate as f32;
    let r = shaped(recipe(surface), contact);
    let mut noise = Noise::new(
        0x5eed_0000 ^ ((surface as u32) << 8) ^ (variant as u32 * 0x9e37) ^ ((contact as u32) << 20),
    );
    let shift = 1.0 + (variant as f32 - 1.5) * 0.05;
    // Variants also differ in how fast they decay, so repeats do not share one envelope.
    let speed = 1.0 + (variant as f32 - 1.5) * 0.10;
    let frames = (r.length * rate_f) as usize;
    let mut grain = Svf::new(r.band[0], r.q, rate_f);
    let mut grain_amp = 0.0f32;
    let grain_decay = (-1.0 / (r.grain_s * rate_f)).exp();
    let mut body = Svf::new(r.body_hz * shift, 0.6, rate_f);
    let mut modes: Vec<(Svf, f32)> =
        r.modes.iter().map(|&(hz, q, gain)| (Svf::new(hz * shift, q, rate_f), gain)).collect();
    let mut swish = Svf::new(400.0, 1.0, rate_f);
    let mut ceiling = [Svf::new(r.ceiling * shift, 0.707, rate_f); 2];
    let mut phase = [0.0f32; 2];
    let ratio = r.band[1] / r.band[0];
    let mut data = Vec::with_capacity(frames);
    for i in 0..frames {
        let t = i as f32 / rate_f;
        let u = t / r.length;
        // Decay runs on scaled time, so `speed` shortens every envelope at once.
        let s = t * speed;
        let white = noise.next();
        let heel = (-s / r.spread).exp();
        let toe = r.toe.map_or(0.0, |at| 0.55 * (-((s - at) / (0.6 * r.spread)).powi(2)).exp());
        // The first grain lands on the first frame so every step has a contact.
        if i == 0 || noise.unit() < r.grains * (heel + toe) / rate_f {
            let level = if i == 0 { 1.0 } else { 0.35 + 0.65 * noise.unit() };
            grain_amp = grain_amp.max(level);
            grain.tune(r.band[0] * ratio.powf(noise.unit()) * shift, r.q, rate_f);
        }
        grain_amp *= grain_decay;
        let crunch = grain.band(white * grain_amp) * r.crunch;
        let weight = body.low(white) * r.body * (-s / r.body_s).exp();
        let strike = white * (-s * 900.0).exp();
        let ring: f32 = modes.iter_mut().map(|(mode, gain)| mode.ring(strike) * *gain).sum();
        let mut wet = 0.0;
        if r.swish > 0.0 {
            // Water rushes up through the band and settles, with two bubbles closing.
            if i % 32 == 0 {
                swish.tune((350.0 + 800.0 * (u * std::f32::consts::PI).sin()) * shift, 1.0, rate_f);
            }
            let bubble = |p: &mut f32, start: f32, hz: f32| {
                let local = t - start;
                if local < 0.0 {
                    return 0.0;
                }
                advance(p, hz * shift * (1.0 + local * 10.0), rate_f);
                sine(*p) * (local / 0.002).min(1.0) * (-local * 45.0).exp()
            };
            wet = swish.band(white) * r.swish * (u * std::f32::consts::PI).sin().powf(1.5)
                + bubble(&mut phase[0], 0.04, 520.0) * 0.08
                + bubble(&mut phase[1], 0.12, 740.0) * 0.06;
        }
        let attack = (t / 0.0012).min(1.0);
        let tail = (1.0 - u).max(0.0).powi(2);
        let dry = (crunch + weight + ring + wet) * attack * tail;
        let once = ceiling[0].low(dry);
        data.push(ceiling[1].low(once));
    }
    let peak = data.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1.0e-6);
    for x in &mut data {
        *x *= 0.8 / peak;
    }
    data
}

pub(crate) struct StepPlay {
    pub(crate) contact: Contact,
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
            let sample = &self.bank[index(play.contact, play.surface, play.variant)];
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
        assert_eq!(bank.len(), Contact::COUNT * Surface::COUNT * VARIANTS);
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
    fn firm_surfaces_open_with_a_tap() {
        let bank = bank(48_000);
        for surface in [Surface::Stone, Surface::Soil, Surface::Wood, Surface::Metal] {
            for variant in 0..VARIANTS {
                let render = &bank[index(Contact::Step, surface, variant)];
                let onset = render[..48].iter().fold(0.0f32, |m, x| m.max(x.abs()));
                assert!(onset > 0.1, "{surface:?} variant {variant} onset {onset}");
            }
        }
    }

    #[test]
    fn breaks_crumble_longer_and_places_seat_shorter_than_steps() {
        let bank = bank(48_000);
        for surface in Surface::ALL {
            let len = |contact| bank[index(contact, surface, 0)].len();
            assert!(len(Contact::Break) > len(Contact::Step), "{surface:?} break");
            assert!(len(Contact::Place) < len(Contact::Step), "{surface:?} place");
            assert!(len(Contact::Hit) < len(Contact::Place), "{surface:?} hit");
            assert_ne!(bank[index(Contact::Break, surface, 0)], bank[index(Contact::Step, surface, 0)]);
        }
    }

    #[test]
    fn step_voices_finish_and_are_bounded() {
        let mut steps = Steps::new(48_000);
        for i in 0..10 {
            let contact = Contact::ALL[i % Contact::COUNT];
            steps.start(StepPlay { contact, surface: Surface::ALL[i % 8], variant: i, left: 0.5, right: 0.5, rate: 1.0 });
        }
        assert_eq!(steps.voices.len(), MAX_VOICES);
        for _ in 0..48_000 {
            steps.frame();
        }
        assert!(steps.voices.is_empty());
    }
}
