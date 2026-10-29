//! Loading stage on a drafting sheet; the drawing completes with the stage.
use super::{LoadingUiState, pixel_art, schematics};
use egui::{Align2, FontId, Id, LayerId, Order, Rect, Stroke, StrokeKind, pos2, vec2};

pub(super) fn draw(ctx: &egui::Context, state: &LoadingUiState) {
    let r = ctx.content_rect();
    if !r.is_positive() {
        return;
    }
    let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("loading_sheet")));
    let sheet = schematics::ground(&p, r);
    let block = Rect::from_min_max(
        pos2((sheet.right() - 360.0).max(sheet.left() + 8.0), sheet.bottom() - 72.0),
        sheet.max - vec2(8.0, 8.0),
    );
    let still = super::magic_fx::reduced(ctx);
    let progress = state.progress_01.filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0));
    // Without a work count the sheet redraws on a loop so the screen never looks stalled.
    let reveal = match progress {
        Some(v) => v,
        None if still => 1.0,
        None => ctx.input(|i| (i.time as f32 / 12.0).fract()),
    };
    let area = Rect::from_min_max(sheet.min + vec2(16.0, 16.0), pos2(sheet.right() - 16.0, block.top() - 8.0));
    schematics::draw(&p, area, schematics::seed_of(&state.title), reveal);

    p.rect_filled(block, 0, schematics::GROUND);
    p.rect_stroke(block, 0, Stroke::new(1.0, pixel_art::PAPER), StrokeKind::Inside);
    let split = block.top() + 40.0;
    p.hline(block.x_range(), split, Stroke::new(1.0, pixel_art::PAPER));
    let title = p.layout(
        state.title.clone(),
        FontId::proportional(18.0),
        pixel_art::PAPER,
        (block.width() - 84.0).max(1.0),
    );
    p.galley(pos2(block.left() + 10.0, split - 10.0 - title.size().y), title, pixel_art::PAPER);
    if let Some(value) = progress {
        p.text(
            pos2(block.right() - 10.0, split - 10.0),
            Align2::RIGHT_BOTTOM,
            format!("{:.0}%", value * 100.0),
            FontId::monospace(13.0),
            pixel_art::PAPER,
        );
    }
    let track = Rect::from_min_max(pos2(block.left() + 10.0, split + 13.0), pos2(block.right() - 10.0, split + 16.0));
    p.rect_filled(track, 0, super::grimoire::SILVER_DARK);
    let fill = progress.unwrap_or(reveal);
    p.rect_filled(Rect::from_min_size(track.min, vec2((track.width() * fill).floor(), 3.0)), 0, pixel_art::GOLD);
    ctx.request_repaint_after(std::time::Duration::from_millis(if still { 1000 } else { 33 }));
}
