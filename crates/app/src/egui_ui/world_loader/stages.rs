//! Stage table: marker, name, work count, percent and the two clocks.
use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, pos2};

use super::super::{grimoire, pixel_art};
use super::timeline::{Timing, duration};
use super::{StageRow, StageState};

pub const ROW_H: f32 = 44.0;
const MUTED: Color32 = Color32::from_rgb(120, 130, 132);

fn marker(p: &Painter, at: egui::Pos2, state: StageState) {
    match state {
        StageState::Done => {
            p.circle_filled(at, 5.0, pixel_art::GOLD);
        }
        StageState::Queued => {
            p.circle_stroke(at, 4.5, Stroke::new(1.0, grimoire::SILVER_DARK));
        }
        StageState::Running => {
            p.circle_filled(at, 5.0, pixel_art::TEAL);
        }
    }
}

fn state_label(s: StageState) -> &'static str {
    match s {
        StageState::Queued => "Queued",
        StageState::Running => "Running",
        StageState::Done => "Done",
    }
}

/// Paints the header and one row per stage inside `area`.
pub fn table(p: &Painter, area: Rect, rows: &[StageRow], times: &[Timing]) {
    let body = FontId::proportional(15.0);
    let mono = FontId::monospace(12.5);
    let right = area.right();
    let cols = [right - 92.0, right];
    let y0 = area.top();
    for (x, text) in [(cols[0] - 4.0, "Elapsed"), (cols[1], "Remaining")] {
        p.text(pos2(x, y0), Align2::RIGHT_TOP, text, FontId::proportional(12.0), MUTED);
    }
    p.text(pos2(area.left() + 22.0, y0), Align2::LEFT_TOP, "Stage", FontId::proportional(12.0), MUTED);
    p.hline(area.x_range(), y0 + 18.0, Stroke::new(1.0, grimoire::SILVER_DARK.gamma_multiply(0.7)));
    for (i, (row, t)) in rows.iter().zip(times).enumerate() {
        let top = y0 + 26.0 + i as f32 * ROW_H;
        let mid = top + 9.0;
        if row.state == StageState::Running {
            let band = Rect::from_min_max(pos2(area.left() - 6.0, top - 5.0), pos2(right + 6.0, top + ROW_H - 3.0));
            p.rect_filled(band, 3.0, pixel_art::TEAL.gamma_multiply(0.06));
        }
        marker(p, pos2(area.left() + 6.0, mid), row.state);
        let name_colour = match row.state {
            StageState::Done => pixel_art::PAPER,
            StageState::Running => pixel_art::HIGHLIGHT,
            StageState::Queued => MUTED,
        };
        p.text(pos2(area.left() + 22.0, mid), Align2::LEFT_CENTER, row.name, body.clone(), name_colour);
        let detail = match (row.count, row.fraction) {
            (Some((d, 0)), _) => format!("{d} {}", row.unit),
            (Some((d, n)), _) => format!("{d}/{n} {}", row.unit),
            (None, Some(f)) => format!("{:.0}%", f * 100.0),
            (None, None) => state_label(row.state).to_string(),
        };
        p.text(pos2(area.left() + 22.0, mid + 17.0), Align2::LEFT_CENTER, detail, mono.clone(), name_colour.gamma_multiply(0.85));
        let elapsed = t.elapsed.map_or_else(|| "–".to_string(), duration);
        p.text(pos2(cols[0] - 4.0, mid), Align2::RIGHT_CENTER, elapsed, mono.clone(), name_colour);
        let remaining = match (row.state, t.remaining) {
            (StageState::Done, _) => "–".to_string(),
            (_, Some(r)) if t.from_history => format!("≈{}", duration(r)),
            (_, Some(r)) => duration(r),
            (_, None) => "–".to_string(),
        };
        p.text(pos2(cols[1], mid), Align2::RIGHT_CENTER, remaining, mono.clone(), name_colour);
    }
}

/// Height the table needs for `n` rows.
pub fn height(n: usize) -> f32 {
    26.0 + n as f32 * ROW_H
}
