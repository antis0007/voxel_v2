//! Small semantic emblems drawn as native pixel stairs, never a full-screen layer.
use egui::{Color32, Painter, Pos2, Rect, Vec2};

pub fn draw(p: &Painter, center: Pos2, radius: f32, sides: usize, color: Color32) {
    let pixel = 1.0 / p.ctx().pixels_per_point();
    let snap = |v: Pos2| Pos2::new((v.x / pixel).round() * pixel, (v.y / pixel).round() * pixel);
    for i in 0..sides {
        let point = |j: usize| {
            let a = j as f32 * std::f32::consts::TAU / sides as f32 - std::f32::consts::FRAC_PI_2;
            snap(center + Vec2::new(a.cos(), a.sin()) * radius)
        };
        let a = point(i);
        let b = point(i + 1);
        let count = ((b.x - a.x).abs().max((b.y - a.y).abs()) / pixel)
            .ceil()
            .max(1.0) as usize;
        for j in 0..=count {
            let q = snap(a.lerp(b, j as f32 / count as f32));
            p.rect_filled(Rect::from_min_size(q, Vec2::splat(pixel)), 0.0, color);
        }
    }
}
