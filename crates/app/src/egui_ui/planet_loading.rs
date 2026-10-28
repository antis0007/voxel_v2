//! Planet solve on the loading screen: the real face grid as it erodes.
//!
//! Every pixel is a solve cell and the bar counts finished erosion steps, so
//! the screen shows exactly how far the solve has got.
use std::sync::Arc;

use super::{grimoire, pixel_art};
use egui::{Align2, Color32, ColorImage, FontId, Id, LayerId, Order, Rect, TextureHandle, TextureOptions, pos2, vec2};
use engine_worldgen::geo::{Entry, Preview, Stage};

const STAGES: [(Stage, &str); 5] = [
    (Stage::Plates, "Plates"),
    (Stage::Uplift, "Uplift"),
    (Stage::Climate, "Climate"),
    (Stage::Carving, "Carving"),
    (Stage::Rivers, "Rivers"),
];

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn image(p: &Preview) -> ColorImage {
    let n = p.n;
    let mut rgb = Vec::with_capacity(n * n * 3);
    for i in 0..n * n {
        let h = p.height[i];
        let c = if h < 0.0 {
            [22.0, 44.0, 74.0]
        } else if p.discharge_km2.as_ref().is_some_and(|d| d[i] > 2.5) {
            let d = p.discharge_km2.as_ref().map_or(0.0, |d| d[i]);
            mix([70.0, 120.0, 170.0], [150.0, 205.0, 240.0], (d / 60.0).min(1.0))
        } else {
            let (x, z) = (i % n, i / n);
            let up = p.height[z.saturating_sub(1) * n + x.saturating_sub(1)].max(0.0);
            let shade = (0.8 + (up - h) * 9.0).clamp(0.35, 1.35);
            let low = match &p.wet {
                Some(w) => mix([168.0, 150.0, 102.0], [78.0, 120.0, 70.0], w[i]),
                None => [120.0, 116.0, 98.0],
            };
            let ground = mix(low, [196.0, 190.0, 178.0], (h * 1.4 - 0.25).clamp(0.0, 1.0));
            ground.map(|v| v * shade)
        };
        rgb.extend(c.map(|v| v.clamp(0.0, 255.0) as u8));
    }
    ColorImage::from_rgb([n, n], &rgb)
}

/// Uploads the newest preview once; later frames reuse the texture.
fn texture(ctx: &egui::Context, preview: &Arc<Preview>) -> TextureHandle {
    let id = Id::new("planet_loading_map");
    let key = Arc::as_ptr(preview) as usize;
    if let Some((k, tex)) = ctx.data(|d| d.get_temp::<(usize, TextureHandle)>(id))
        && k == key
    {
        return tex;
    }
    let tex = ctx.load_texture("planet_loading_map", image(preview), TextureOptions::NEAREST);
    ctx.data_mut(|d| d.insert_temp(id, (key, tex.clone())));
    tex
}

pub(super) fn draw(ctx: &egui::Context, entry: &Entry) {
    let r = ctx.content_rect();
    if !r.is_positive() {
        return;
    }
    let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("planet_loading")));
    p.rect_filled(r, 0, Color32::from_rgb(20, 24, 27));
    let side = (r.width().min(r.height()) * 0.62).max(64.0);
    let map = Rect::from_center_size(r.center() - vec2(0.0, 40.0), vec2(side, side));
    if let Some(preview) = entry.progress.preview() {
        let tex = texture(ctx, &preview);
        p.image(tex.id(), map, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    p.rect_stroke(map, 0, (1.0, grimoire::SILVER_DARK), egui::StrokeKind::Outside);

    let stage = entry.progress.stage();
    let mut x = map.left();
    let y = map.bottom() + 18.0;
    for (s, name) in STAGES {
        let colour = if s == stage {
            pixel_art::GOLD
        } else if (s as u8) < (stage as u8) {
            pixel_art::PAPER
        } else {
            grimoire::SILVER_DARK
        };
        let g = p.layout_no_wrap(name.to_string(), FontId::proportional(15.0), colour);
        let w = g.size().x;
        p.galley(pos2(x, y), g, colour);
        x += w + 18.0;
    }
    let (done, total) = entry.progress.steps();
    p.text(
        pos2(map.right(), y),
        Align2::RIGHT_TOP,
        format!("{done}/{total} steps"),
        FontId::monospace(13.0),
        pixel_art::PAPER,
    );
    let track = Rect::from_min_size(pos2(map.left(), y + 28.0), vec2(side, 3.0));
    p.rect_filled(track, 0, grimoire::SILVER_DARK);
    let fraction = if total == 0 { 0.0 } else { done as f32 / total as f32 };
    p.rect_filled(Rect::from_min_size(track.min, vec2(side * fraction, 3.0)), 0, pixel_art::GOLD);
    ctx.request_repaint_after(std::time::Duration::from_millis(66));
}
