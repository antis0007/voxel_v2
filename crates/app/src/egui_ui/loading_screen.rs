//! Loading stage and progress in the title column; the room continues without a cut.
use super::{LoadingUiState, grimoire, pixel_art};
use egui::{Align2, FontId, Id, LayerId, Order, Rect, pos2, vec2};

pub(super) fn draw(ctx: &egui::Context, state: &LoadingUiState) {
    let r = ctx.content_rect();
    if !r.is_positive() {
        return;
    }
    let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("loading_room")));
    p.rect_filled(r, 0, egui::Color32::from_rgb(20, 24, 27));
    let (left, width, quiet_edge) = super::menu_scene::column(r);
    super::menu_scene::paint(&p, r, quiet_edge, 0.0);
    let title = p.layout(
        state.title.clone(),
        FontId::proportional(20.0),
        pixel_art::PAPER,
        (width - 58.0).max(1.0),
    );
    let height = title.size().y;
    // Where the primary action sits on the title screen.
    let y = (r.center().y - height - 20.0).max(r.top() + 24.0);
    p.galley(pos2(left, y), title, pixel_art::PAPER);
    if let Some(value) = state.progress_01.filter(|v| v.is_finite()) {
        p.text(
            pos2(left + width, y + height),
            Align2::RIGHT_BOTTOM,
            format!("{:.0}%", value.clamp(0.0, 1.0) * 100.0),
            FontId::monospace(13.0),
            pixel_art::PAPER,
        );
    }
    let track = Rect::from_min_size(pos2(left, y + height + 14.0), vec2(width, 3.0));
    p.rect_filled(track, 0, grimoire::SILVER_DARK);
    let still = super::magic_fx::reduced(ctx);
    if let Some(progress) = state.progress_01.filter(|v| v.is_finite()) {
        p.rect_filled(
            Rect::from_min_size(track.min, vec2(width * progress.clamp(0.0, 1.0), 3.0)),
            0,
            pixel_art::GOLD,
        );
    } else if still {
        for i in 0..24 {
            p.rect_filled(
                Rect::from_min_size(
                    track.min + vec2(i as f32 * width / 24.0, 0.0),
                    vec2(width / 48.0, 3.0),
                ),
                0,
                pixel_art::GOLD,
            );
        }
    } else {
        // Stepped sweep: the bar advances in whole pixels like the rest of the UI.
        let phase = ctx.input(|i| (i.time as f32 * 0.25).fract());
        let start = (phase * (width + 80.0) - 80.0).floor();
        p.with_clip_rect(track).rect_filled(
            Rect::from_min_size(track.min + vec2(start, 0.0), vec2(80.0, 3.0)),
            0,
            pixel_art::GOLD,
        );
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(if still { 1000 } else { 33 }));
}
