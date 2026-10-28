//! Procedural music: seeded pieces from `assets/music.ron` and the silence between them.
use crate::dsp::{hash, unit};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Mood {
    Day,
    Night,
    Underground,
    Combat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Timbre {
    /// Struck, inharmonic, long decay.
    Bell,
    /// Slow swell, held.
    Pad,
    /// Plucked string, short and bright.
    Pluck,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PieceDef {
    pub name: String,
    pub moods: Vec<Mood>,
    pub root_hz: f32,
    /// Scale degrees as semitones above the root, ascending, within one octave.
    pub scale: Vec<u8>,
    pub tempo_bpm: f32,
    pub beats_per_bar: u32,
    pub bars: u32,
    /// Chance that a melody step sounds rather than rests.
    pub density: f32,
    /// Melody range in scale degrees above the root.
    pub span: u32,
    pub melody: Timbre,
    pub bass: Option<Timbre>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MusicTable {
    /// Silence before the first piece of a session, seconds (min, max).
    pub first_gap_s: (f32, f32),
    /// Silence after each piece, seconds (min, max).
    pub gap_s: (f32, f32),
    /// Combat may cut a silence short, but not before this long after a piece.
    pub combat_gap_s: f32,
    pub pieces: Vec<PieceDef>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    pub start_s: f32,
    pub length_s: f32,
    pub hz: f32,
    pub gain: f32,
    pub pan: f32,
    pub timbre: Timbre,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub name: String,
    pub notes: Vec<Note>,
    /// Until the last note's release ends, seconds.
    pub length_s: f32,
}

fn degree_hz(def: &PieceDef, degree: i32, octave: i32) -> f32 {
    let len = def.scale.len().max(1) as i32;
    let step = def.scale.get(degree.rem_euclid(len) as usize).copied().unwrap_or(0) as i32;
    let semitones = step + 12 * (degree.div_euclid(len) + octave);
    def.root_hz * 2f32.powf(semitones as f32 / 12.0)
}

/// Writes one piece. Phrases follow A A' B A so a motif returns.
pub fn compose(def: &PieceDef, seed: u32) -> Piece {
    let beat = 60.0 / def.tempo_bpm.clamp(30.0, 200.0);
    let per_bar = def.beats_per_bar.clamp(2, 8);
    let bars = def.bars.clamp(2, 64);
    let span = def.span.clamp(2, 24) as i32;
    let phrase_bars = 2;
    let mut notes = Vec::new();
    let mut degree = span / 3;
    for bar in 0..bars {
        let phrase = (bar / phrase_bars) % 4;
        let letter = match phrase {
            2 => 1,
            _ => 0,
        };
        let in_phrase = bar % phrase_bars;
        // A' changes only its last bar.
        let variant = u32::from(phrase == 1 && in_phrase == phrase_bars - 1);
        let bar_start = bar as f32 * per_bar as f32 * beat;
        if let Some(timbre) = def.bass {
            let fifth = if hash(seed ^ 0xba55, bar).is_multiple_of(3) { 4 } else { 0 };
            notes.push(Note {
                start_s: bar_start,
                length_s: per_bar as f32 * beat * 0.95,
                hz: degree_hz(def, fifth, -1),
                gain: 0.42,
                pan: 0.0,
                timbre,
            });
        }
        let mut beat_index = 0;
        while beat_index < per_bar {
            let key = hash(seed ^ (letter * 0x51ed) ^ (variant * 0x2545), in_phrase * 16 + beat_index);
            let held = 1 + (key >> 20) % 3;
            if unit(key, 1) < def.density.clamp(0.0, 1.0) {
                let moves = [-2, -1, -1, 0, 1, 1, 2];
                degree = (degree + moves[(key >> 4) as usize % moves.len()]).clamp(0, span - 1);
                // Phrase ends lean on the root so each phrase sounds closed.
                if in_phrase == phrase_bars - 1 && beat_index + held >= per_bar {
                    degree = if degree > span / 2 { def.scale.len() as i32 } else { 0 };
                }
                notes.push(Note {
                    start_s: bar_start + beat_index as f32 * beat,
                    length_s: held as f32 * beat * 0.9,
                    hz: degree_hz(def, degree, 0),
                    gain: 0.5 + unit(key, 2) * 0.3,
                    pan: (unit(key, 3) - 0.5) * 0.6,
                    timbre: def.melody,
                });
            }
            beat_index += held;
        }
    }
    let end = notes.iter().map(|n| n.start_s + n.length_s).fold(0.0, f32::max);
    Piece { name: def.name.clone(), notes, length_s: end + crate::perform::RELEASE_S }
}

/// Decides when a piece plays; long silences separate pieces.
#[derive(Clone, Debug, PartialEq)]
pub struct Director {
    seed: u32,
    plays: u32,
    wait_s: f32,
    playing_s: f32,
    since_end_s: f32,
}

fn between(range: (f32, f32), key: u32) -> f32 {
    let (lo, hi) = (range.0.min(range.1).max(0.0), range.0.max(range.1).max(0.0));
    lo + (hi - lo) * unit(key, 0x6a09)
}

impl Director {
    pub fn new(table: &MusicTable, seed: u32) -> Self {
        Self {
            seed,
            plays: 0,
            wait_s: between(table.first_gap_s, hash(seed, 0xf1257)),
            playing_s: 0.0,
            since_end_s: f32::INFINITY,
        }
    }

    pub fn plays(&self) -> u32 {
        self.plays
    }

    pub fn playing(&self) -> bool {
        self.playing_s > 0.0
    }

    /// Advances by `dt` seconds; returns a piece to start now.
    pub fn update(&mut self, table: &MusicTable, dt: f32, mood: Mood) -> Option<Piece> {
        if !dt.is_finite() || dt <= 0.0 {
            return None;
        }
        if self.playing_s > 0.0 {
            self.playing_s -= dt;
            if self.playing_s <= 0.0 {
                self.wait_s = between(table.gap_s, hash(self.seed, self.plays));
                self.since_end_s = 0.0;
            }
            return None;
        }
        self.since_end_s += dt;
        self.wait_s -= dt;
        let combat_ready = mood == Mood::Combat && self.since_end_s >= table.combat_gap_s;
        if self.wait_s > 0.0 && !combat_ready {
            return None;
        }
        let candidates: Vec<&PieceDef> =
            table.pieces.iter().filter(|p| p.moods.contains(&mood)).collect();
        if candidates.is_empty() {
            return None;
        }
        let def = candidates[hash(self.seed ^ 0x9e37, self.plays) as usize % candidates.len()];
        let piece = compose(def, hash(self.seed, self.plays.wrapping_add(1)));
        self.plays = self.plays.wrapping_add(1);
        self.playing_s = piece.length_s;
        Some(piece)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn table() -> MusicTable {
        let def = |name: &str, moods: Vec<Mood>| PieceDef {
            name: name.into(),
            moods,
            root_hz: 220.0,
            scale: vec![0, 2, 4, 7, 9],
            tempo_bpm: 72.0,
            beats_per_bar: 4,
            bars: 8,
            density: 0.6,
            span: 8,
            melody: Timbre::Bell,
            bass: Some(Timbre::Pad),
        };
        MusicTable {
            first_gap_s: (30.0, 90.0),
            gap_s: (300.0, 600.0),
            combat_gap_s: 60.0,
            pieces: vec![
                def("day", vec![Mood::Day]),
                def("night", vec![Mood::Night, Mood::Underground]),
                def("fight", vec![Mood::Combat]),
            ],
        }
    }

    /// Start times and pieces over `seconds` of a fixed mood.
    fn run(seed: u32, mood: Mood, seconds: f32) -> Vec<(f32, Piece)> {
        let table = table();
        let mut director = Director::new(&table, seed);
        let mut out = Vec::new();
        let dt = 0.05;
        for step in 0..(seconds / dt) as u32 {
            if let Some(piece) = director.update(&table, dt, mood) {
                out.push((step as f32 * dt, piece));
            }
        }
        out
    }

    #[test]
    fn scheduling_is_deterministic_for_a_seed() {
        let a = run(42, Mood::Day, 3600.0);
        assert!(a.len() >= 2, "an hour holds several pieces");
        assert_eq!(a, run(42, Mood::Day, 3600.0));
        assert_ne!(a, run(43, Mood::Day, 3600.0));
    }

    #[test]
    fn pieces_are_separated_by_the_silence_gap() {
        let table = table();
        let starts = run(7, Mood::Night, 4.0 * 3600.0);
        assert!(starts[0].0 >= table.first_gap_s.0 && starts[0].0 <= table.first_gap_s.1 + 0.1);
        for pair in starts.windows(2) {
            let silence = pair[1].0 - (pair[0].0 + pair[0].1.length_s);
            assert!(silence >= table.gap_s.0 - 0.2, "silence {silence}");
            assert!(silence <= table.gap_s.1 + 0.2, "silence {silence}");
            assert_eq!(pair[1].1.name, "night");
        }
    }

    #[test]
    fn combat_shortens_only_a_silence_that_is_already_long_enough() {
        let table = table();
        let mut director = Director::new(&table, 3);
        assert!(director.update(&table, 1.0, Mood::Combat).is_some(), "no piece before: combat starts at once");
        let length = director.playing_s;
        let mut t = 0.0;
        let mut next = None;
        while t < length + 200.0 {
            t += 0.5;
            if let Some(p) = director.update(&table, 0.5, Mood::Combat) {
                next = Some((t, p));
                break;
            }
        }
        let (t, piece) = next.expect("a second combat piece");
        assert_eq!(piece.name, "fight");
        assert!(t - length >= table.combat_gap_s - 0.6, "combat waited {}", t - length);
    }

    #[test]
    fn composed_notes_stay_in_scale_and_inside_the_piece() {
        let table = table();
        let def = &table.pieces[0];
        let piece = compose(def, 99);
        assert!(piece.notes.len() > 8);
        let allowed: Vec<f32> = (-12..36)
            .filter(|s: &i32| def.scale.contains(&(s.rem_euclid(12) as u8)))
            .map(|s| def.root_hz * 2f32.powf(s as f32 / 12.0))
            .collect();
        for note in &piece.notes {
            assert!(allowed.iter().any(|hz| (hz - note.hz).abs() < 0.01), "{} Hz off scale", note.hz);
            assert!(note.start_s + note.length_s <= piece.length_s);
        }
        assert_ne!(compose(def, 99), compose(def, 100));
    }
}
