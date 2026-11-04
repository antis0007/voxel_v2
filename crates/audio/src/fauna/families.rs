//! Per-family bird note synthesis: whistle, trill, chatter and caw.
use super::Family;
use crate::dsp::{advance, sine, Noise, Svf};

/// Breath-noise onset at the start of every note, seconds and share of the note's gain.
const BREATH_S: f32 = 0.008;
const BREATH_LEVEL: f32 = 0.1;
/// Caw band-pass Q; narrow enough to read as a throat, wide enough to stay noisy.
const CAW_Q: f32 = 6.0;
/// Band-passed noise at Q 6 sits well under a sine of the same gain; this levels it.
const CAW_MAKEUP: f32 = 4.0;

/// Phrase shape drawn at call start: note length, count, gap and pitch fall, in samples and ratios.
#[derive(Clone, Copy, Default)]
pub(super) struct Shape {
    pub(super) len: u32,
    pub(super) notes: u32,
    pub(super) gap: u32,
    pub(super) sweep: f32,
}

/// Samples for a duration drawn between `lo` and `hi` seconds.
fn span(noise: &mut Noise, rate: f32, lo: f32, hi: f32) -> u32 {
    ((lo + noise.unit() * (hi - lo)) * rate) as u32
}

pub(super) fn shape(family: Family, noise: &mut Noise, rate: f32) -> Shape {
    let mut s = |lo: f32, hi: f32| span(noise, rate, lo, hi);
    match family {
        Family::Whistle => {
            let (len, gap) = (s(0.12, 0.26), s(0.06, 0.14));
            Shape { len, gap, notes: 1 + span(noise, 3.0, 0.0, 1.0), sweep: 0.1 + noise.unit() * 0.2 }
        }
        Family::Trill => {
            let (len, gap) = (s(0.04, 0.12), s(0.03, 0.07));
            Shape { len, gap, notes: 3 + span(noise, 4.0, 0.0, 1.0), sweep: 0.15 + noise.unit() * 0.3 }
        }
        Family::Chatter => {
            let (len, gap) = (s(0.03, 0.07), s(0.02, 0.05));
            Shape { len, gap, notes: 3 + span(noise, 6.0, 0.0, 1.0), sweep: 0.1 + noise.unit() * 0.2 }
        }
        Family::Caw => {
            let len = s(0.25, 0.45);
            Shape { len, notes: 1, gap: 0, sweep: -0.08 - noise.unit() * 0.08 }
        }
    }
}

/// Pitch ratio applied between the notes of a phrase.
pub(super) fn next_note(family: Family, noise: &mut Noise) -> f32 {
    match family {
        // Chatter hops either way; the others step down a little.
        Family::Chatter => 0.85 + noise.unit() * 0.3,
        _ => 0.94,
    }
}

/// Oscillator state of one note.
#[derive(Clone, Copy, Default)]
pub(super) struct Voice {
    phase: f32,
    partial: f32,
    warble: f32,
    band: Svf,
    noise: Noise,
    age: u32,
}

impl Voice {
    pub(super) fn start(family: Family, hz: f32, noise: &mut Noise, rate: f32) -> Voice {
        let seed = (noise.unit() * u32::MAX as f32) as u32;
        let band = match family {
            Family::Caw => Svf::new(hz.clamp(1_200.0, 1_800.0), CAW_Q, rate),
            _ => Svf::default(),
        };
        Voice { phase: 0.0, partial: 0.0, warble: 0.0, band, noise: Noise::new(seed), age: 0 }
    }

    /// One mono sample of a note at `t` in 0..1 of its length, before gain and envelope.
    pub(super) fn sample(&mut self, family: Family, hz: f32, t: f32, trill: f32, rate: f32) -> f32 {
        let breath_len = BREATH_S * rate;
        let breath = if (self.age as f32) < breath_len { BREATH_LEVEL * (1.0 - self.age as f32 / breath_len) * self.noise.next() } else { 0.0 };
        self.age += 1;
        let tone = match family {
            Family::Whistle => {
                advance(&mut self.phase, hz, rate);
                sine(self.phase)
            }
            Family::Trill => {
                advance(&mut self.phase, hz, rate);
                advance(&mut self.warble, 40.0 + 20.0 * trill, rate);
                sine(self.phase) * (1.0 - 0.5 * (1.0 + sine(self.warble)) * (0.5 + 0.5 * trill))
            }
            Family::Chatter => {
                advance(&mut self.phase, hz, rate);
                advance(&mut self.partial, hz * 1.5, rate);
                let carrier = sine(self.phase);
                carrier * 0.6 + carrier * sine(self.partial) * 0.4
            }
            Family::Caw => {
                // The throat opens through the note, so the band centre rises a little then settles.
                let open = 1.0 + 0.15 * sine(t * 0.5);
                self.band.tune((hz * open).clamp(1_200.0, 1_800.0), CAW_Q, rate);
                self.band.band(self.noise.next()) * CAW_MAKEUP
            }
        };
        tone + breath
    }
}
