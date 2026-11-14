//! Arena HUD plates: chamfered pixel panels, stepped glow, sparks and sweeps.
//!
//! Every shape sits on the two-point art lattice snapped to whole physical
//! pixels, the same lattice `pixel_art::rainbow_selection` uses, so plates,
//! glows and particles never blur against the sprite art.
use super::presentation as style;
use egui::{Color32, Painter, Rect, Shape, pos2, vec2};

/// Two art points, rounded to whole physical pixels.
pub(super) fn cell(ctx: &egui::Context) -> f32 {
    let ppp = ctx.pixels_per_point().max(0.1);
    (2.0 * ppp).round().max(1.0) / ppp
}

fn snap(r: Rect, c: f32) -> Rect {
    Rect::from_min_max(
        pos2((r.left() / c).floor() * c, (r.top() / c).floor() * c),
        pos2((r.right() / c).ceil() * c, (r.bottom() / c).ceil() * c),
    )
}

fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

fn hash(i: u32) -> f32 {
    engine_core::hash::hash01(i)
}

/// Rows of a rectangle whose top-left and bottom-right corners are cut in
/// `steps` lattice stairs, as (top, bottom, left inset, right inset).
fn chamfer_rows(r: Rect, c: f32, steps: u32) -> Vec<(f32, f32, f32, f32)> {
    let rows = ((r.height() / c).round() as u32).max(1);
    let steps = steps.min(rows / 2);
    (0..rows)
        .map(|row| {
            let top = r.top() + row as f32 * c;
            let left = steps.saturating_sub(row) as f32 * c;
            let right = (row + steps + 1).saturating_sub(rows) as f32 * c;
            (top, top + c, left, right)
        })
        .collect()
}

fn add_rect(mesh: &mut egui::Mesh, r: Rect, color: Color32) {
    if r.width() > 0.0 && r.height() > 0.0 {
        mesh.add_colored_rect(r, color);
    }
}

/// Dark translucent plate with cut corners, a lit accent lip and a dark rim.
pub(super) fn plate(ctx: &egui::Context, r: Rect, accent: Color32) -> Shape {
    let c = cell(ctx);
    let r = snap(r, c);
    let mut mesh = egui::Mesh::default();
    let rows = chamfer_rows(r, c, 3);
    let last = rows.len().saturating_sub(1);
    for (i, &(top, bottom, li, ri)) in rows.iter().enumerate() {
        let (x0, x1) = (r.left() + li, r.right() - ri);
        // Body darkens toward the bottom: lit from above like the slot wells.
        let depth = i as f32 / rows.len().max(1) as f32;
        let body = Color32::from_rgba_unmultiplied(14, 16, 22, (176.0 + depth * 40.0) as u8);
        add_rect(&mut mesh, Rect::from_min_max(pos2(x0, top), pos2(x1, bottom)), body);
        // Rim: the cut stairs carry the accent so the chamfers read at a glance;
        // straight sides stay dark.
        let side = with_alpha(style::INK, 0.9);
        let left_rim = if li > 0.0 || i == 0 { accent } else { side };
        let right_rim = if ri > 0.0 { with_alpha(accent, 0.7) } else { side };
        add_rect(&mut mesh, Rect::from_min_max(pos2(x0, top), pos2(x0 + c * 0.5, bottom)), left_rim);
        add_rect(&mut mesh, Rect::from_min_max(pos2(x1 - c * 0.5, top), pos2(x1, bottom)), right_rim);
        if li > 0.0 && i > 0 {
            // Stair tread: the cell row above each step is lit too.
            add_rect(&mut mesh, Rect::from_min_max(pos2(x0, top), pos2(x0 + c, top + c * 0.5)), accent);
        }
        if i == 0 {
            add_rect(&mut mesh, Rect::from_min_max(pos2(x0, top), pos2(x1, top + c)), accent);
            // A bright leading segment marks the plate's "front".
            let lead = ((x1 - x0) * 0.22 / c).round() * c;
            add_rect(
                &mut mesh,
                Rect::from_min_max(pos2(x0, top), pos2(x0 + lead, top + c)),
                super::pixel_art::top_light(accent),
            );
        }
        if i == last {
            add_rect(&mut mesh, Rect::from_min_max(pos2(x0, bottom - c * 0.5), pos2(x1, bottom)), style::INK);
        }
    }
    Shape::mesh(mesh)
}

/// Stepped outer glow: `rings` chamfered outlines, one lattice cell apart,
/// each fainter than the last.
pub(super) fn glow(ctx: &egui::Context, r: Rect, color: Color32, rings: u32, strength: f32) -> Shape {
    let c = cell(ctx);
    let mut mesh = egui::Mesh::default();
    for ring in 1..=rings {
        let a = strength * (1.0 - (ring - 1) as f32 / rings as f32) * 0.38;
        let col = with_alpha(color, a);
        let o = snap(r.expand(ring as f32 * c), c);
        for &(top, bottom, li, ri) in &chamfer_rows(o, c, 3 + ring) {
            let (x0, x1) = (o.left() + li, o.right() - ri);
            if top < o.top() + c || bottom > o.bottom() - c {
                add_rect(&mut mesh, Rect::from_min_max(pos2(x0, top), pos2(x1, bottom)), col);
            } else {
                add_rect(&mut mesh, Rect::from_min_max(pos2(x0, top), pos2(x0 + c, bottom)), col);
                add_rect(&mut mesh, Rect::from_min_max(pos2(x1 - c, top), pos2(x1, bottom)), col);
            }
        }
    }
    Shape::mesh(mesh)
}

/// Analytic pixel sparks rising off the top edge of `r`; no state is kept.
/// `seed` separates emitters so neighbours do not move in step.
pub(super) fn sparks(p: &Painter, r: Rect, color: Color32, time: f32, count: u32, seed: u32) {
    if !p.is_visible() || super::magic_fx::reduced(p.ctx()) {
        return;
    }
    let c = cell(p.ctx());
    let time = (time * 12.0).floor() / 12.0;
    let rise = (r.height() * 0.9).max(12.0);
    for i in 0..count {
        let k = seed.wrapping_mul(0x9e37_79b9) ^ i.wrapping_mul(0x85eb_ca6b);
        let speed = 0.22 + hash(k) * 0.18;
        let age = (time * speed + hash(k ^ 0x51)).fract();
        let x = r.left() + hash(k ^ 0x77) * r.width() + (age * 6.0 + hash(k) * 6.0).sin() * c;
        let y = r.bottom() - age * rise;
        let head = pos2((x / c).floor() * c, (y / c).floor() * c);
        // Fade in over the first fifth so births do not pop at full alpha.
        let life = (age * 5.0).min(1.0) * (1.0 - age);
        for tail in 0..2 {
            let a = life * (0.8 - tail as f32 * 0.4);
            let col = if tail == 0 { super::pixel_art::top_light(color) } else { color };
            p.rect_filled(
                Rect::from_min_size(head + vec2(0.0, tail as f32 * c), vec2(c, c)),
                0,
                with_alpha(col, a),
            );
        }
    }
    p.ctx().request_repaint_after(std::time::Duration::from_millis(83));
}

/// Pixel radial cooldown: lattice cells clockwise from twelve o'clock up to
/// `left` (remaining share, 0..1) are shaded; the leading edge glints.
pub(super) fn sweep(p: &Painter, r: Rect, left: f32) {
    if left <= 0.0 {
        return;
    }
    let c = cell(p.ctx());
    let r = snap(r, c);
    let (nx, ny) = ((r.width() / c) as i32, (r.height() / c) as i32);
    let center = r.center();
    let done = 1.0 - left.clamp(0.0, 1.0);
    let mut mesh = egui::Mesh::default();
    for y in 0..ny {
        for x in 0..nx {
            let at = r.min + vec2((x as f32 + 0.5) * c, (y as f32 + 0.5) * c);
            let d = at - center;
            // Turns clockwise from straight up.
            let turn = (d.x.atan2(-d.y) / std::f32::consts::TAU).rem_euclid(1.0);
            if turn < done {
                continue;
            }
            let edge = turn - done < 0.04;
            let col = if edge { with_alpha(style::PAPER, 0.55) } else { Color32::from_black_alpha(150) };
            mesh.add_colored_rect(Rect::from_min_size(at - vec2(c, c) * 0.5, vec2(c, c)), col);
        }
    }
    p.add(Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chamfer_cuts_opposite_corners_only() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 20.0));
        let rows = chamfer_rows(r, 2.0, 3);
        assert_eq!(rows.len(), 10);
        assert_eq!(rows[0].2, 6.0);
        assert_eq!(rows[0].3, 0.0);
        assert_eq!(rows[9].2, 0.0);
        assert_eq!(rows[9].3, 6.0);
        assert!(rows[4].2 == 0.0 && rows[4].3 == 0.0);
    }

    #[test]
    fn chamfer_survives_tiny_plates() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(8.0, 2.0));
        let rows = chamfer_rows(r, 2.0, 3);
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].2, rows[0].3), (0.0, 0.0));
    }
}
