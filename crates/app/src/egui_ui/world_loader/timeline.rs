//! Per-stage clocks and remaining-time estimates from measured progress rates.
use std::collections::HashMap;
use std::sync::Mutex;

use super::{StageRow, StageState};

/// Last measured duration per stage, in seconds; seeds estimates for stages
/// that report no work count (a far-field build) on the next load.
static HISTORY: Mutex<Option<HashMap<&'static str, f32>>> = Mutex::new(None);

#[derive(Clone, Copy, Default)]
struct Track {
    started: Option<f64>,
    ended: Option<f64>,
    /// Fraction and time of the oldest sample in the current rate window.
    anchor: Option<(f32, f64)>,
    /// Fraction per second, smoothed.
    rate: f32,
}

#[derive(Clone, Default)]
pub(super) struct Timeline {
    key: u64,
    began: f64,
    tracks: HashMap<&'static str, Track>,
}

/// Times for one row, all in seconds.
#[derive(Clone, Copy, Default)]
pub(super) struct Timing {
    pub elapsed: Option<f32>,
    pub remaining: Option<f32>,
    /// True when `remaining` comes from a previous load, not this one.
    pub from_history: bool,
}

fn history_get(key: &str) -> Option<f32> {
    HISTORY.lock().ok()?.as_ref()?.get(key).copied()
}

fn history_set(key: &'static str, seconds: f32) {
    if let Ok(mut h) = HISTORY.lock() {
        h.get_or_insert_with(HashMap::new).insert(key, seconds);
    }
}

impl Timeline {
    /// Advances every clock to `now` and returns one timing per row.
    pub fn update(&mut self, key: u64, now: f64, rows: &[StageRow], details: bool) -> Vec<Timing> {
        if self.key != key || self.tracks.is_empty() && self.began == 0.0 {
            *self = Timeline { key, began: now, tracks: HashMap::new() };
        }
        if details {
            rows.iter().map(|row| self.step(now, row)).collect()
        } else {
            for row in rows {
                self.step(now, row);
            }
            Vec::new()
        }
    }

    pub fn elapsed(&self, now: f64) -> f32 {
        (now - self.began).max(0.0) as f32
    }

    fn step(&mut self, now: f64, row: &StageRow) -> Timing {
        let t = self.tracks.entry(row.key).or_default();
        match row.state {
            StageState::Queued => {}
            StageState::Running => {
                t.started.get_or_insert(now);
                t.ended = None;
            }
            StageState::Done => {
                let start = *t.started.get_or_insert(now);
                if t.ended.is_none() {
                    t.ended = Some(now);
                    // A stage already done on the first frame was cached; its
                    // zero duration says nothing about the next cold load.
                    if now - start > 0.05 {
                        history_set(row.key, (now - start) as f32);
                    }
                }
            }
        }
        let elapsed = t.started.map(|s| (t.ended.unwrap_or(now) - s).max(0.0) as f32);
        let mut timing = Timing { elapsed, ..Timing::default() };
        match row.state {
            StageState::Done => timing.remaining = Some(0.0),
            StageState::Running => {
                if let Some(f) = row.fraction.filter(|f| *f > 0.0) {
                    match t.anchor {
                        None => t.anchor = Some((f, now)),
                        Some((f0, t0)) if now - t0 >= 0.5 => {
                            let sample = (f - f0).max(0.0) / (now - t0) as f32;
                            t.rate = if t.rate > 0.0 { t.rate * 0.7 + sample * 0.3 } else { sample };
                            t.anchor = Some((f, now));
                        }
                        Some(_) => {}
                    }
                    // Whole-stage average until the windowed rate has a sample.
                    let average = elapsed.filter(|e| *e > 0.25).map(|e| f / e).unwrap_or(0.0);
                    let rate = if t.rate > 0.0 { t.rate } else { average };
                    if rate > 0.0 && f < 1.0 {
                        timing.remaining = Some((1.0 - f) / rate);
                    }
                }
                if timing.remaining.is_none()
                    && let Some(total) = history_get(row.key)
                {
                    timing.remaining = Some((total - elapsed.unwrap_or(0.0)).max(0.0));
                    timing.from_history = true;
                }
            }
            StageState::Queued => {
                timing.remaining = history_get(row.key);
                timing.from_history = timing.remaining.is_some();
            }
        }
        timing
    }
}

/// Whole-load remaining time and whether some stage has no estimate yet.
pub(super) fn overall_remaining(rows: &[StageRow], timings: &[Timing]) -> (f32, bool) {
    let mut sum = 0.0;
    let mut unknown = false;
    for (row, t) in rows.iter().zip(timings) {
        match (row.state, t.remaining) {
            (StageState::Done, _) => {}
            (_, Some(r)) => sum += r,
            (_, None) => unknown = true,
        }
    }
    (sum, unknown)
}

/// Whole-load completion and whether it is weighted by time. With an estimate
/// for every unfinished stage, each stage counts by its measured or expected
/// seconds; otherwise each stage counts once.
pub(super) fn overall_fraction(rows: &[StageRow], timings: &[Timing]) -> (f32, bool) {
    let (mut spent, mut total, mut known) = (0.0, 0.0, true);
    for (row, t) in rows.iter().zip(timings) {
        let elapsed = t.elapsed.unwrap_or(0.0);
        match (row.state, t.remaining) {
            (StageState::Done, _) => {
                spent += elapsed;
                total += elapsed;
            }
            (_, Some(r)) => {
                spent += elapsed;
                total += elapsed + r;
            }
            (_, None) => known = false,
        }
    }
    if known && total > 1.0 {
        return ((spent / total).clamp(0.0, 1.0), true);
    }
    let n = rows.len().max(1) as f32;
    let work: f32 = rows
        .iter()
        .map(|r| match r.state {
            StageState::Done => 1.0,
            StageState::Running => r.fraction.unwrap_or(0.0),
            StageState::Queued => 0.0,
        })
        .sum();
    (work / n, false)
}

/// Expected seconds of a stage on a cold first load, before any rate or
/// history exists. Only the share of the whole bar depends on it.
fn prior_seconds(key: &str) -> f32 {
    match key {
        "solve-drift" => 100.0,
        "solve-plates" => 10.0,
        "solve-climate" => 1.0,
        "solve-carving" | "solve-rivers" => 8.0,
        "planet-cover" => 25.0,
        "spawn-chunks" => 15.0,
        _ => 8.0,
    }
}

/// The drainage solve runs its stages one after another; everything that
/// waits on it (spawn chunks, cover, globe tiles, survey) runs side by side.
fn concurrent(row: &StageRow) -> bool {
    !row.key.starts_with("solve-")
}

/// Wall-clock completion and seconds left. Serial solve stages add up; the
/// concurrent stages after it cost only their slowest member. A running stage
/// past its estimate keeps one second so the bar never reads full early.
///
/// The bar weights unmeasured stages by a prior, so it always has a fill. The
/// seconds are different: they come back only when every unfinished stage has
/// a measured rate or a time from an earlier load, and only once the load has
/// run long enough for those rates to settle. A guess presented as a countdown
/// was off by 4x on a cold planet load, so no figure beats a wrong one.
pub(super) fn wall_progress(rows: &[StageRow], timings: &[Timing], elapsed: f32) -> (f32, Option<f32>) {
    let (mut serial, mut parallel) = (0.0_f32, 0.0_f32);
    let mut known = true;
    for (row, t) in rows.iter().zip(timings) {
        let left = match row.state {
            StageState::Done => continue,
            StageState::Running => t.remaining.unwrap_or_else(|| {
                known = false;
                prior_seconds(row.key) - t.elapsed.unwrap_or(0.0)
            }).max(1.0),
            StageState::Queued => t.remaining.unwrap_or_else(|| {
                known = false;
                prior_seconds(row.key)
            }),
        };
        if concurrent(row) { parallel = parallel.max(left) } else { serial += left }
    }
    let left = serial + parallel;
    let fraction = if elapsed + left > 0.0 { elapsed / (elapsed + left) } else { 0.0 };
    let eta = (known && elapsed >= SETTLE_SECONDS).then_some(left);
    (fraction.clamp(0.0, 1.0), eta)
}

/// Seconds a load runs before its measured rates are trusted for a countdown.
const SETTLE_SECONDS: f32 = 10.0;

/// A countdown in words: coarse, since the estimate is not precise.
pub(super) fn eta_text(seconds: f32) -> String {
    if seconds < 10.0 {
        "Almost done".to_owned()
    } else if seconds < 90.0 {
        format!("About {} s left", (seconds / 10.0).round() as u32 * 10)
    } else {
        format!("About {} min left", (seconds / 60.0).round().max(2.0) as u32)
    }
}

/// Seconds as `12 s` or `3 min 05 s`.
pub(super) fn duration(seconds: f32) -> String {
    let s = seconds.max(0.0).round() as u32;
    if s < 60 { format!("{s} s") } else { format!("{} min {:02} s", s / 60, s % 60) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(state: StageState, fraction: Option<f32>) -> StageRow {
        StageRow { key: "test-stage", name: "Test", group: "Test", state, count: None, unit: "", fraction }
    }

    #[test]
    fn running_rate_predicts_remaining() {
        let mut tl = Timeline::default();
        tl.update(1, 10.0, &[row(StageState::Running, Some(0.1))], true);
        let t = tl.update(1, 12.0, &[row(StageState::Running, Some(0.3))], true);
        let r = t[0].remaining.unwrap_or(0.0);
        assert!((r - 7.0).abs() < 0.01, "remaining {r}");
        assert!(!t[0].from_history);
    }

    #[test]
    fn new_world_resets_clocks() {
        let mut tl = Timeline::default();
        tl.update(1, 10.0, &[row(StageState::Running, None)], true);
        tl.update(2, 50.0, &[row(StageState::Running, None)], true);
        assert_eq!(tl.elapsed(51.0), 1.0);
    }

    #[test]
    fn duration_formats_minutes() {
        assert_eq!(duration(4.4), "4 s");
        assert_eq!(duration(185.0), "3 min 05 s");
    }
}
