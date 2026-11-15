//! Sound subtitles: recent audible cues listed in the lower right with their side.

use crate::audio::{Cue, Event};
use egui::{Align2, RichText};
use glam::Vec3;
use std::sync::{
    LazyLock, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};

const TABLE: &str = include_str!("../../../../assets/subtitles.ron");
/// Matches the mixer's audible radius, so a line never names a sound that was culled.
const AUDIBLE_M: f32 = 42.0;
const SHOWN_S: f32 = 3.0;
const FADE_S: f32 = 1.0;
const MAX_LINES: usize = 8;
/// Pan beyond this marks a subtitle with its side.
const SIDE_PAN: f32 = 0.3;

static ENABLED: AtomicBool = AtomicBool::new(false);
static LINES: Mutex<Vec<Line>> = Mutex::new(Vec::new());

#[derive(serde::Deserialize)]
struct Row {
    cue: String,
    text: String,
}

static ROWS: LazyLock<Vec<Row>> = LazyLock::new(|| ron::from_str(TABLE).unwrap_or_default());

struct Line {
    text: &'static str,
    pan: f32,
    age: f32,
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
    if !on {
        LINES.lock().unwrap_or_else(PoisonError::into_inner).clear();
    }
}

fn caption(cue: Cue) -> Option<&'static str> {
    let name = format!("{cue:?}");
    ROWS.iter().find(|row| row.cue == name).map(|row| row.text.as_str())
}

/// Side of a sound in the listener's view: -1 left, 1 right, 0 ahead or unplaced.
fn pan(pos: Option<Vec3>, eye: Vec3, forward: Vec3) -> Option<f32> {
    let Some(pos) = pos else { return Some(0.0) };
    let delta = pos - eye;
    if !delta.is_finite() || delta.length() > AUDIBLE_M {
        return None;
    }
    Some(delta.normalize_or_zero().dot(forward.cross(Vec3::Y).normalize_or_zero()))
}

/// Records a played event; positions are in metres, as passed to the mixer.
pub fn note(event: &Event, eye: Vec3, forward: Vec3) {
    if !enabled() {
        return;
    }
    let (Some(text), Some(pan)) = (caption(event.cue), pan(event.pos, eye, forward)) else { return };
    let mut lines = LINES.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(line) = lines.iter_mut().find(|l| l.text == text) {
        line.pan = pan;
        line.age = 0.0;
        return;
    }
    if lines.len() >= MAX_LINES {
        lines.remove(0);
    }
    lines.push(Line { text, pan, age: 0.0 });
}

fn label(text: &str, pan: f32) -> String {
    if pan < -SIDE_PAN {
        format!("‹ {text}")
    } else if pan > SIDE_PAN {
        format!("{text} ›")
    } else {
        text.to_owned()
    }
}

pub(super) fn draw(ctx: &egui::Context, dt: f32) {
    let mut lines = LINES.lock().unwrap_or_else(PoisonError::into_inner);
    for line in lines.iter_mut() {
        line.age += dt.max(0.0);
    }
    lines.retain(|l| l.age < SHOWN_S + FADE_S);
    if lines.is_empty() || !enabled() {
        return;
    }
    let shown = egui::Area::new(egui::Id::new("subtitles"))
        .anchor(Align2::RIGHT_BOTTOM, [-16.0, -60.0])
        .interactable(false)
        .show(ctx, |ui| {
            super::presentation::hud_frame(ui.ctx()).show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                    for line in lines.iter() {
                        let fade = ((SHOWN_S + FADE_S - line.age) / FADE_S).clamp(0.0, 1.0);
                        let colour = ui.visuals().text_color().gamma_multiply(fade);
                        ui.label(RichText::new(label(line.text, line.pan)).color(colour));
                    }
                });
            });
        });
    // The pickup feed stacks above this column.
    super::hud_layout::claim(ctx, "subtitles", shown.response.rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_names_a_cue_once_and_has_text() {
        let mut names = std::collections::HashSet::new();
        for row in ROWS.iter() {
            assert!(ron::from_str::<Cue>(&row.cue).is_ok(), "{} is not a cue", row.cue);
            assert!(names.insert(row.cue.as_str()), "{} listed twice", row.cue);
            assert!(!row.text.is_empty() && row.text.len() <= 24, "{}", row.cue);
        }
        assert_eq!(caption(Cue::Mine), Some("Block Broken"));
        assert_eq!(caption(Cue::Splash), Some("Splash"));
    }

    #[test]
    fn side_follows_the_listener_and_range_culls() {
        let eye = Vec3::ZERO;
        let forward = Vec3::NEG_Z;
        assert!(pan(Some(Vec3::new(5.0, 0.0, 0.0)), eye, forward).is_some_and(|p| p > SIDE_PAN));
        assert!(pan(Some(Vec3::new(-5.0, 0.0, 0.0)), eye, forward).is_some_and(|p| p < -SIDE_PAN));
        assert!(pan(Some(Vec3::new(0.0, 0.0, -5.0)), eye, forward).is_some_and(|p| p.abs() < SIDE_PAN));
        assert_eq!(pan(Some(Vec3::new(0.0, 0.0, -50.0)), eye, forward), None);
        assert_eq!(pan(None, eye, forward), Some(0.0));
        assert_eq!(label("Splash", 0.9), "Splash ›");
        assert_eq!(label("Splash", -0.9), "‹ Splash");
    }
}
