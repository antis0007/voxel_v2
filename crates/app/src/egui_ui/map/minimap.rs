//! Corner minimap: rotates with the view, so the arrow always points up.
use super::super::{grimoire, presentation as style};
use super::tiles::{REGION_PX, TILE};
use super::MapState;
use egui::{Align2, Color32, FontId, Mesh, Pos2, Rect, Shape, Stroke, emath::Rot2, pos2, vec2};
use glam::IVec2;

const SIZE: f32 = 136.0;
/// Screen points per map column.
const ZOOM: f32 = 1.0;

/// Rotation that turns the view heading (yaw 0 = north = -z) to screen up.
pub fn view_rotation(yaw: f32) -> Rot2 {
    Rot2::from_angle(-yaw)
}

/// Offset in columns (x east, y = +z south) to screen points around the arrow.
pub fn to_screen(delta: egui::Vec2, yaw: f32) -> egui::Vec2 {
    view_rotation(yaw) * (delta * ZOOM)
}

/// Pull a point outside `half` back onto the square's edge.
fn clamp_to_edge(v: egui::Vec2, half: f32) -> (egui::Vec2, bool) {
    let m = v.x.abs().max(v.y.abs());
    if m <= half { (v, true) } else { (v * (half / m), false) }
}

pub(super) fn diamond(p: &egui::Painter, at: Pos2, r: f32, fill: Color32, glyph: &str) {
    p.add(Shape::convex_polygon(
        vec![at + vec2(0.0, -r), at + vec2(r, 0.0), at + vec2(0.0, r), at + vec2(-r, 0.0)],
        fill,
        Stroke::new(1.0, style::INK),
    ));
    if !glyph.is_empty() {
        p.text(at, Align2::CENTER_CENTER, glyph, FontId::proportional(r * 1.3), style::INK);
    }
}

/// Region textures as one rotated mesh per texture, clipped by the caller.
pub(super) fn terrain(p: &egui::Painter, s: &MapState, centre: Pos2, origin: egui::Vec2, scale: f32, rot: Rot2, reach: f32) {
    let lo = ((origin - egui::Vec2::splat(reach)) / TILE as f32).floor();
    let hi = ((origin + egui::Vec2::splat(reach)) / TILE as f32).floor();
    let (lo, hi) = (IVec2::new(lo.x as i32, lo.y as i32), IVec2::new(hi.x as i32, hi.y as i32));
    for (region, texture) in s.tiles.textures_in(lo, hi) {
        let corner = egui::Vec2::new(region.x as f32, region.y as f32) * REGION_PX as f32;
        let rect = Rect::from_min_size(centre + (corner - origin) * scale, egui::Vec2::splat(REGION_PX as f32 * scale));
        let mut mesh = Mesh::with_texture(texture);
        mesh.add_rect_with_uv(rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        mesh.rotate(rot, centre);
        p.add(Shape::mesh(mesh));
    }
}

pub(super) fn draw(ctx: &egui::Context, s: &MapState, top: f32) {
    let Some((pos, yaw)) = s.player else { return };
    let screen = ctx.content_rect();
    let frame = Rect::from_min_size(pos2(screen.right() - 12.0 - SIZE, screen.top() + top), vec2(SIZE, SIZE));
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, "minimap".into()));
    p.rect_filled(frame, 0, grimoire::RECESS);
    let inner = frame.shrink(4.0);
    let centre = inner.center();
    let origin = egui::Vec2::new(pos.x, pos.z);
    let rot = view_rotation(yaw);
    let clip = p.with_clip_rect(inner);
    terrain(&clip, s, centre, origin, ZOOM, rot, inner.width() * 0.75 / ZOOM);

    let half = inner.width() * 0.5 - 6.0;
    for w in &s.waypoints.list {
        let delta = egui::Vec2::new(w.pos.x as f32 + 0.5, w.pos.z as f32 + 0.5) - origin;
        let (v, inside) = clamp_to_edge(to_screen(delta, yaw), half);
        let glyph = if inside { w.glyph() } else { String::new() };
        diamond(&clip, centre + v, if inside { 5.5 } else { 4.0 }, w.color(), &glyph);
    }
    let (north, _) = clamp_to_edge(to_screen(egui::Vec2::new(0.0, -1.0e6), yaw), half - 2.0);
    clip.circle_filled(centre + north, 6.5, Color32::from_black_alpha(170));
    clip.text(centre + north, Align2::CENTER_CENTER, "N", FontId::proportional(10.0), style::AMBER);

    clip.add(Shape::convex_polygon(
        vec![centre + vec2(0.0, -6.0), centre + vec2(4.5, 5.0), centre + vec2(0.0, 2.5), centre + vec2(-4.5, 5.0)],
        style::AMBER,
        Stroke::new(1.0, style::INK),
    ));
    grimoire::binding(&p, frame, false);

    if s.prefs.coordinates {
        let cell = pos.floor().as_ivec3();
        let text = format!("{} · {} · {}", cell.x, cell.y, cell.z);
        let at = pos2(frame.center().x, frame.bottom() + 3.0);
        p.text(at + vec2(1.0, 1.0), Align2::CENTER_TOP, &text, FontId::monospace(10.5), style::INK);
        p.text(at, Align2::CENTER_TOP, &text, FontId::monospace(10.5), style::PAPER);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: egui::Vec2, b: egui::Vec2) -> bool {
        (a - b).length() < 1e-3
    }

    #[test]
    fn map_minimap_heading_points_up() {
        for yaw in [0.0f32, 0.7, 1.9, -2.4, 3.1] {
            let forward = egui::Vec2::new(yaw.sin(), -yaw.cos());
            assert!(near(to_screen(forward, yaw), egui::Vec2::new(0.0, -ZOOM)), "yaw {yaw}");
        }
        // Facing east (yaw 90°), north sits on the left.
        let north = to_screen(egui::Vec2::new(0.0, -1.0), std::f32::consts::FRAC_PI_2);
        assert!(near(north, egui::Vec2::new(-ZOOM, 0.0)));
    }

    #[test]
    fn map_minimap_edge_clamp_keeps_direction() {
        let (v, inside) = clamp_to_edge(egui::Vec2::new(300.0, -150.0), 60.0);
        assert!(!inside);
        assert!(near(v, egui::Vec2::new(60.0, -30.0)));
        assert!(clamp_to_edge(egui::Vec2::new(10.0, 5.0), 60.0).1);
    }
}
