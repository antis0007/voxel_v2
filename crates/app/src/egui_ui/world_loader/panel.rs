//! Progress panel beside the globe: headline and percent, a bar segmented by
//! stage group, the live stages with their counts, then clocks.
//!
//! Every value comes from the report and the timeline; motion (sheen, head
//! glow) is decoration on top of measured fills and stops under reduced motion.
use egui::epaint::{Mesh, Shadow};
use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, pos2, vec2};

use super::super::themes::Palette;
use super::groups::{self, Group};
use super::timeline::{self, Timing};
use super::{StageRow, StageState, stages};

const PAD: f32 = 18.0;
const BAR_Y: f32 = 84.0;
const BAR_H: f32 = 6.0;
const LIVE_Y: f32 = 122.0;
const LIVE_H: f32 = 21.0;

pub(super) struct Input<'a> {
    pub title: &'static str,
    pub rows: &'a [StageRow],
    pub times: &'a [Timing],
    pub progress: f32,
    pub elapsed: f32,
    /// Seconds left when every remaining stage has a real estimate; `None` shows no countdown.
    pub left: Option<f32>,
    pub ready: bool,
    pub seed: u32,
    pub detail: bool,
    pub can_show_detail: bool,
    pub seconds: f32,
    pub still: bool,
}

fn running(rows: &[StageRow]) -> impl Iterator<Item = &StageRow> {
    rows.iter().filter(|s| s.state == StageState::Running)
}

/// Height the panel wants for this report.
pub(super) fn height(rows: &[StageRow], detail: bool) -> f32 {
    // Stage rows are developer detail: shown only behind the details toggle.
    let body = if detail { stages::height(rows.len()) + 8.0 } else { 0.0 };
    LIVE_Y + body + 36.0
}

fn count_text(s: &StageRow) -> String {
    match s.count {
        Some((d, n)) if n > 0 => format!("{d}/{n} {}", s.unit),
        Some((d, _)) if d > 0 => format!("{d} {}", s.unit),
        _ => String::new(),
    }
}

/// Horizontal gradient strip; `colours` are left, middle and right.
fn sheen(p: &Painter, r: Rect, colours: [Color32; 3]) {
    let mut m = Mesh::default();
    let xs = [r.left(), r.center().x, r.right()];
    for (x, c) in xs.into_iter().zip(colours) {
        m.colored_vertex(pos2(x, r.top()), c);
        m.colored_vertex(pos2(x, r.bottom()), c);
    }
    for k in 0..2u32 {
        let i = k * 2;
        m.add_triangle(i, i + 1, i + 2);
        m.add_triangle(i + 1, i + 3, i + 2);
    }
    p.add(m);
}

/// The overall bar: one segment per stage group, filled by its measured work.
fn segmented(p: &Painter, track: Rect, groups: &[Group], theme: Palette, seconds: f32, still: bool) {
    let n = groups.len().max(1) as f32;
    let gap = 6.0;
    let w = (track.width() - gap * (n - 1.0)) / n;
    for (i, g) in groups.iter().enumerate() {
        let seg = Rect::from_min_size(pos2(track.left() + i as f32 * (w + gap), track.top()), vec2(w, track.height()));
        p.rect_filled(seg, 2.0, theme.edge.gamma_multiply(0.75));
        let f = match g.state {
            StageState::Done => 1.0,
            StageState::Queued => 0.0,
            StageState::Running => g.fraction.clamp(0.0, 1.0),
        };
        if f > 0.0 {
            let fill = Rect::from_min_size(seg.min, vec2((seg.width() * f).max(2.0), seg.height()));
            let colour = if g.state == StageState::Done { theme.accent.gamma_multiply(0.8) } else { theme.accent };
            p.rect_filled(fill, 2.0, colour);
            // Top highlight: a lit edge reads as a raised, glassy fill.
            p.rect_filled(Rect::from_min_size(fill.min, vec2(fill.width(), 1.5)), 1.0, Color32::from_white_alpha(46));
        }
        if g.state == StageState::Running {
            let head = pos2(seg.left() + seg.width() * f, seg.center().y);
            if !still {
                // A light band sweeps the filled part, then a pulse at the head.
                let fill_w = seg.width() * f;
                if fill_w > 12.0 {
                    let band = 36.0f32.min(fill_w);
                    let x = seg.left() - band + ((seconds * 0.45).fract()) * (fill_w + band);
                    let r = Rect::from_min_max(pos2(x.max(seg.left()), seg.top()), pos2((x + band).min(seg.left() + fill_w), seg.bottom()));
                    if r.width() > 0.5 {
                        let clear = Color32::TRANSPARENT;
                        sheen(p, r, [clear, Color32::from_white_alpha(70), clear]);
                    }
                }
                let pulse = 0.5 + 0.5 * (seconds * 3.2).sin();
                for (k, a) in [(9.0, 0.10), (5.5, 0.22), (3.0, 0.6)] {
                    p.circle_filled(head, k * (0.85 + 0.15 * pulse), theme.accent.gamma_multiply(a * (0.7 + 0.3 * pulse)));
                }
            } else {
                p.circle_filled(head, 3.0, theme.accent);
            }
        }
    }
}

/// Group names under their segments, coloured by state.
fn segment_labels(p: &Painter, track: Rect, groups: &[Group], theme: Palette) {
    let n = groups.len().max(1) as f32;
    let gap = 6.0;
    let w = (track.width() - gap * (n - 1.0)) / n;
    let font = FontId::proportional(12.0);
    for (i, g) in groups.iter().enumerate() {
        let x = track.left() + i as f32 * (w + gap);
        let colour = match g.state {
            StageState::Done => theme.text().gamma_multiply(0.72),
            StageState::Running => theme.accent,
            StageState::Queued => theme.muted().gamma_multiply(0.8),
        };
        // A name wider than its segment keeps only its first word.
        let mut galley = p.layout_no_wrap(g.name.to_owned(), font.clone(), colour);
        if galley.size().x > w - 4.0 {
            let short = g.name.split_whitespace().next().unwrap_or(g.name);
            galley = p.layout_no_wrap(short.to_owned(), font.clone(), colour);
        }
        let clip = p.with_clip_rect(Rect::from_min_size(pos2(x, track.bottom()), vec2(w, 22.0)));
        clip.galley(pos2(x, track.bottom() + 6.0), galley, colour);
    }
}

pub(super) fn draw(p: &Painter, panel: Rect, input: &Input, theme: Palette) {
    if !panel.is_positive() {
        return;
    }
    // Glass card: soft shadow, translucent fill, hairline edge, lit top rule.
    p.add(Shadow { offset: [0, 10], blur: 28, spread: 0, color: Color32::from_black_alpha(110) }.as_shape(panel, 6));
    p.rect_filled(panel, 6.0, theme.panel.gamma_multiply(0.9));
    p.rect_stroke(panel, 6.0, Stroke::new(1.0, theme.edge), egui::StrokeKind::Inside);
    let rule = Rect::from_min_size(panel.min + vec2(6.0, 0.0), vec2(panel.width() - 12.0, 2.0));
    let clear = Color32::TRANSPARENT;
    sheen(p, rule, [clear, theme.accent.gamma_multiply(0.85), clear]);

    let p = p.with_clip_rect(panel);
    let at = panel.min + vec2(PAD, PAD - 2.0);
    let right = panel.right() - PAD;
    let width = right - at.x;
    let groups = groups::collect(input.rows);
    let live: Vec<&StageRow> = running(input.rows).take(3).collect();

    // Headline and percent.
    let title = if input.ready { "World Ready" } else { input.title };
    p.text(at, Align2::LEFT_TOP, title, FontId::proportional(24.0), theme.text());
    let pct = format!("{:.0}%", (input.progress * 100.0).floor().clamp(0.0, 100.0));
    p.text(pos2(right, at.y - 3.0), Align2::RIGHT_TOP, pct, FontId::monospace(28.0), theme.accent);

    // Current stage on the left, time left on the right.
    let status_y = at.y + 38.0;
    if input.ready {
        p.text(pos2(at.x, status_y), Align2::LEFT_TOP, "[Any Key] Continue", FontId::proportional(15.0), theme.accent);
    } else {
        let status = live.first().map_or_else(
            || if input.rows.iter().all(|s| s.state == StageState::Done) { "Finalizing".to_owned() } else { "Preparing".to_owned() },
            // Stage names and work counts are developer detail; the summary names the broad phase.
            |s| if input.detail {
                let n = count_text(s);
                if n.is_empty() { s.name.to_owned() } else { format!("{}  ·  {n}", s.name) }
            } else {
                s.group.to_owned()
            },
        );
        p.text(pos2(at.x, status_y), Align2::LEFT_TOP, status, FontId::proportional(15.0), theme.text().gamma_multiply(0.92));
        if let Some(left) = input.left {
            p.text(pos2(right, status_y + 1.0), Align2::RIGHT_TOP, timeline::eta_text(left), FontId::monospace(13.0), theme.muted());
        }
    }

    let track = Rect::from_min_size(pos2(at.x, panel.top() + BAR_Y), vec2(width, BAR_H));
    if input.ready {
        p.rect_filled(track, 2.0, theme.accent);
    } else {
        segmented(&p, track, &groups, theme, input.seconds, input.still);
    }
    segment_labels(&p, track, &groups, theme);

    let body_top = panel.top() + LIVE_Y;
    if input.detail {
        let list = Rect::from_min_max(pos2(at.x, body_top), pos2(right, panel.bottom() - 36.0));
        stages::table(&p, list, input.rows, input.times);
    }

    // Footer clocks.
    let foot_y = panel.bottom() - 28.0;
    p.hline(at.x..=right, foot_y - 8.0, Stroke::new(1.0, theme.edge.gamma_multiply(0.7)));
    let done = input.rows.iter().filter(|s| s.state == StageState::Done).count();
    let footer = if input.ready {
        format!("Seed {}", input.seed)
    } else {
        format!("{done}/{} Stages  ·  {}  ·  Seed {}", input.rows.len(), timeline::duration(input.elapsed), input.seed)
    };
    p.text(pos2(at.x, foot_y), Align2::LEFT_TOP, footer, FontId::monospace(12.5), theme.muted());
    if input.can_show_detail {
        let hint = if input.detail { "[Shift] Summary" } else { "[Shift] Details" };
        p.text(pos2(right, foot_y), Align2::RIGHT_TOP, hint, FontId::proportional(12.5), theme.muted());
    }
}
