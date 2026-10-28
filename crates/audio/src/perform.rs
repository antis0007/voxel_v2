//! Plays a composed piece on a few additive voices with a soft stereo echo.
use crate::dsp::{advance, sine, OnePole};
use crate::music::{Piece, Timbre};

/// Release after a note's length, seconds; `compose` adds it to the piece.
pub(crate) const RELEASE_S: f32 = 0.6;
const MAX_VOICES: usize = 10;
/// Fade applied when a piece is stopped early, seconds.
const STOP_S: f32 = 3.0;

struct Voice {
    hz: f32,
    gain: f32,
    left: f32,
    right: f32,
    timbre: Timbre,
    age: u32,
    hold: u32,
    attack: u32,
    decay: f32,
    decay_mult: f32,
    bright: f32,
    bright_mult: f32,
    phase: [f32; 3],
}

impl Voice {
    fn new(note: &crate::music::Note, rate: f32) -> Self {
        // (attack s, decay time constant s, overtone decay s)
        let (attack, decay, bright) = match note.timbre {
            Timbre::Bell => (0.004, 1.8, 0.25),
            Timbre::Pad => (0.7, 6.0, 4.0),
            Timbre::Pluck => (0.003, 0.45, 0.08),
        };
        let pan = note.pan.clamp(-1.0, 1.0);
        Self {
            hz: note.hz,
            gain: note.gain,
            left: ((1.0 - pan) * 0.5).sqrt(),
            right: ((1.0 + pan) * 0.5).sqrt(),
            timbre: note.timbre,
            age: 0,
            hold: (note.length_s.max(0.0) * rate) as u32,
            attack: ((attack * rate) as u32).max(1),
            decay: 1.0,
            decay_mult: (-1.0 / (decay * rate)).exp(),
            bright: 1.0,
            bright_mult: (-1.0 / (bright * rate)).exp(),
            phase: [0.0; 3],
        }
    }

    /// Next sample, or `None` once the release has finished.
    fn sample(&mut self, rate: f32) -> Option<f32> {
        let release = (RELEASE_S * rate) as u32;
        if self.age >= self.hold + release || self.decay < 1.0e-4 {
            return None;
        }
        let attack = (self.age as f32 / self.attack as f32).min(1.0);
        let releasing = if self.age > self.hold {
            1.0 - (self.age - self.hold) as f32 / release as f32
        } else {
            1.0
        };
        let (ratios, mix) = match self.timbre {
            Timbre::Bell => ([1.0, 2.76, 5.40], [1.0, 0.35, 0.12]),
            Timbre::Pad => ([1.0, 2.0, 1.004], [0.8, 0.18, 0.5]),
            Timbre::Pluck => ([1.0, 2.0, 3.0], [1.0, 0.5, 0.25]),
        };
        let mut value = 0.0;
        for i in 0..3 {
            advance(&mut self.phase[i], self.hz * ratios[i], rate);
            let weight = if i == 0 || self.timbre == Timbre::Pad { mix[i] } else { mix[i] * self.bright };
            value += sine(self.phase[i]) * weight;
        }
        self.decay *= self.decay_mult;
        self.bright *= self.bright_mult;
        self.age += 1;
        Some(value * self.gain * attack * releasing * self.decay)
    }
}

/// Two damped delay lines crossed left to right.
struct Echo {
    left: Vec<f32>,
    right: Vec<f32>,
    index: usize,
    damp: [OnePole; 2],
}

impl Echo {
    fn new(rate: f32) -> Self {
        Self {
            left: vec![0.0; (rate * 0.37) as usize],
            right: vec![0.0; (rate * 0.29) as usize],
            index: 0,
            damp: [OnePole::default(); 2],
        }
    }

    fn process(&mut self, dry: [f32; 2]) -> [f32; 2] {
        let li = self.index % self.left.len();
        let ri = self.index % self.right.len();
        let wet_l = self.damp[0].low(self.left[li], 0.25);
        let wet_r = self.damp[1].low(self.right[ri], 0.25);
        self.left[li] = dry[0] * 0.5 + wet_r * 0.38;
        self.right[ri] = dry[1] * 0.5 + wet_l * 0.38;
        self.index = (self.index + 1) % (self.left.len() * self.right.len());
        [dry[0] + wet_l * 0.45, dry[1] + wet_r * 0.45]
    }
}

pub(crate) struct Performer {
    rate: f32,
    piece: Option<Piece>,
    next: usize,
    clock: u32,
    voices: Vec<Voice>,
    echo: Echo,
    level: f32,
    stopping: bool,
}

impl Performer {
    pub(crate) fn new(rate: u32) -> Self {
        let rate = rate as f32;
        Self {
            rate,
            piece: None,
            next: 0,
            clock: 0,
            voices: Vec::with_capacity(MAX_VOICES),
            echo: Echo::new(rate),
            level: 1.0,
            stopping: false,
        }
    }

    pub(crate) fn start(&mut self, piece: Piece) {
        self.piece = Some(piece);
        self.next = 0;
        self.clock = 0;
        self.level = 1.0;
        self.stopping = false;
    }

    pub(crate) fn stop(&mut self) {
        self.stopping = true;
    }

    #[cfg(test)]
    pub(crate) fn playing(&self) -> bool {
        self.piece.is_some() || !self.voices.is_empty()
    }

    pub(crate) fn frame(&mut self) -> [f32; 2] {
        if let Some(piece) = &self.piece {
            let now = self.clock as f32 / self.rate;
            while let Some(note) = piece.notes.get(self.next).filter(|n| n.start_s <= now) {
                if self.voices.len() >= MAX_VOICES {
                    self.voices.remove(0);
                }
                self.voices.push(Voice::new(note, self.rate));
                self.next += 1;
            }
            self.clock += 1;
            if self.next >= piece.notes.len() && now >= piece.length_s {
                self.piece = None;
            }
        }
        if self.stopping {
            self.level -= 1.0 / (STOP_S * self.rate);
            if self.level <= 0.0 {
                self.piece = None;
                self.voices.clear();
                self.stopping = false;
                self.level = 1.0;
            }
        }
        let mut dry = [0.0f32; 2];
        let rate = self.rate;
        self.voices.retain_mut(|voice| match voice.sample(rate) {
            Some(value) => {
                dry[0] += value * voice.left;
                dry[1] += value * voice.right;
                true
            }
            None => false,
        });
        let wet = self.echo.process(dry);
        [wet[0] * 0.18 * self.level, wet[1] * 0.18 * self.level]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::{compose, tests::table};

    #[test]
    fn a_piece_plays_audibly_and_then_falls_silent() {
        let table = table();
        let piece = compose(&table.pieces[0], 5);
        let frames = ((piece.length_s + 2.0) * 48_000.0) as usize;
        let mut performer = Performer::new(48_000);
        performer.start(piece);
        let (mut peak, mut energy) = (0.0f32, 0.0f64);
        for _ in 0..frames {
            let [l, r] = performer.frame();
            assert!(l.is_finite() && r.is_finite());
            peak = peak.max(l.abs()).max(r.abs());
            energy += (l * l) as f64;
        }
        assert!(peak < 0.9, "peak {peak}");
        assert!(energy / frames as f64 > 1.0e-5, "music is audible");
        assert!(!performer.playing());
    }

    #[test]
    fn stopping_fades_out_within_the_stop_time() {
        let table = table();
        let mut performer = Performer::new(48_000);
        performer.start(compose(&table.pieces[0], 1));
        for _ in 0..48_000 {
            performer.frame();
        }
        performer.stop();
        for _ in 0..(STOP_S * 48_000.0) as usize + 10 {
            performer.frame();
        }
        assert!(!performer.playing());
    }
}
