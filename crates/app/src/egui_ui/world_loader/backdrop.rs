//! Night-sky ground, star field and the gold progress bar.
use egui::epaint::Mesh;
use egui::{Color32, Painter, Rect, pos2, vec2};

use super::super::{grimoire, pixel_art};

/// Orbital tracks stay outside the terrain silhouette. The segmented ring
/// follows measured loading progress; moving lights are decorative only.
pub fn orbits(p: &Painter, centre: egui::Pos2, radius: f32, fill: f32, seconds: f32, still: bool) {
    use egui::{Shape, Stroke};
    let tau = std::f32::consts::TAU;
    for ring in 0..2 {
        let tilt: f32 = if ring == 0 { -0.35 } else { 0.55 };
        let point = |angle: f32| {
            let x = angle.cos() * radius * (1.32 + ring as f32 * 0.13);
            let y = angle.sin() * radius * 0.48;
            centre + vec2(x * tilt.cos() - y * tilt.sin(), x * tilt.sin() + y * tilt.cos())
        };
        let points = (0..=96).map(|i| point(tau * i as f32 / 96.0)).collect();
        p.add(Shape::line(points, Stroke::new(1.0, pixel_art::TEAL.gamma_multiply(0.16))));
        let phase = if still { ring as f32 * 2.4 } else { seconds * (0.16 + ring as f32 * 0.06) + ring as f32 * 2.4 };
        for tail in (0..12).rev() {
            let at = point(phase - tail as f32 * 0.025);
            let alpha = (1.0 - tail as f32 / 12.0) * 0.65;
            p.circle_filled(at, if tail == 0 { 2.4 } else { 1.3 }, pixel_art::TEAL.gamma_multiply(alpha));
        }
    }
    for tick in 0..80 {
        let angle = tau * tick as f32 / 80.0 - std::f32::consts::FRAC_PI_2;
        let direction = vec2(angle.cos(), angle.sin());
        let done = (tick as f32 + 1.0) / 80.0 <= fill.clamp(0.0, 1.0);
        let colour = if done { pixel_art::GOLD.gamma_multiply(0.8) } else { grimoire::SILVER_DARK.gamma_multiply(0.45) };
        let length = if tick % 10 == 0 { 8.0 } else { 4.0 };
        p.line_segment([centre + direction * (radius * 1.14), centre + direction * (radius * 1.14 + length)], Stroke::new(1.2, colour));
    }
}

pub const TOP: Color32 = Color32::from_rgb(14, 20, 32);
pub const BOTTOM: Color32 = Color32::from_rgb(6, 8, 12);

/// Vertical gradient with a faint warm glow behind the globe.
pub fn sky(p: &Painter, r: Rect, glow_at: egui::Pos2, glow_r: f32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(r.left_top(), TOP);
    mesh.colored_vertex(r.right_top(), TOP);
    mesh.colored_vertex(r.right_bottom(), BOTTOM);
    mesh.colored_vertex(r.left_bottom(), BOTTOM);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(mesh);
    let mut glow = Mesh::default();
    let steps = 64;
    glow.colored_vertex(glow_at, Color32::from_rgba_premultiplied(26, 30, 44, 0));
    for s in 0..=steps {
        let t = std::f32::consts::TAU * s as f32 / steps as f32;
        glow.colored_vertex(glow_at + vec2(t.cos(), t.sin()) * glow_r, Color32::TRANSPARENT);
        if s > 0 {
            glow.add_triangle(0, s, s + 1);
        }
    }
    p.add(glow);
}

fn hash(i: u32, salt: u32) -> f32 {
    engine_core::hash::hash01(i.wrapping_mul(747_796_405).wrapping_add(salt))
}

/// Fixed hashed stars; each twinkles on its own smooth sine.
pub fn stars(p: &Painter, r: Rect, seconds: f32, still: bool) {
    for i in 0..170u32 {
        let pos = pos2(r.left() + hash(i, 1) * r.width(), r.top() + hash(i, 2) * r.height());
        let size = 0.5 + hash(i, 3).powi(3) * 1.3;
        let base = 0.25 + 0.55 * hash(i, 4);
        let twinkle = if still { 1.0 } else { 0.7 + 0.3 * (seconds * (0.4 + hash(i, 5) * 1.4) + hash(i, 6) * 6.3).sin() };
        let warm = hash(i, 7) > 0.8;
        let c = if warm { pixel_art::HIGHLIGHT } else { grimoire::SILVER };
        p.circle_filled(pos, size, c.gamma_multiply(base * twinkle));
    }
}

/// Track and gold fill; a soft highlight travels along the filled part.
pub fn bar(p: &Painter, track: Rect, fill: f32, seconds: f32, still: bool) {
    p.rect_filled(track, 1.0, grimoire::SILVER_DARK.gamma_multiply(0.6));
    let fill = fill.clamp(0.0, 1.0);
    if fill <= 0.0 {
        return;
    }
    let done = Rect::from_min_size(track.min, vec2(track.width() * fill, track.height()));
    p.rect_filled(done, 1.0, pixel_art::GOLD);
    if still {
        return;
    }
    let w = (done.width() * 0.25).clamp(12.0, 120.0);
    let x = done.left() + ((seconds * 0.35).fract() * (done.width() + w)) - w;
    let band = Rect::from_x_y_ranges(x.max(done.left())..=(x + w).min(done.right()), done.y_range());
    if band.width() <= 0.0 {
        return;
    }
    let mut m = Mesh::default();
    let mid = (band.left() + band.right()) * 0.5;
    let bright = pixel_art::HIGHLIGHT.gamma_multiply(0.75);
    for (xx, c) in [(band.left(), Color32::TRANSPARENT), (mid, bright), (band.right(), Color32::TRANSPARENT)] {
        m.colored_vertex(pos2(xx, band.top()), c);
        m.colored_vertex(pos2(xx, band.bottom()), c);
    }
    for q in 0..2u32 {
        let a = q * 2;
        m.add_triangle(a, a + 1, a + 3);
        m.add_triangle(a, a + 3, a + 2);
    }
    p.add(m);
}
