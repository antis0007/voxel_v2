//! Full-screen world map: north-up, pan and zoom, with the waypoint list.
use super::super::{grimoire, presentation as style};
use super::minimap::{diamond, terrain};
use super::waypoints::{MAX_WAYPOINTS, MAX_NAME, clip_name};
use super::MapState;
use egui::{Align2, Color32, FontId, Rect, Sense, Shape, Stroke, emath::Rot2, pos2, vec2};
use glam::IVec3;

const LIST_WIDTH: f32 = 270.0;
const ZOOM_MIN: f32 = 0.125;
const ZOOM_MAX: f32 = 8.0;

pub(super) struct View {
    /// Map centre in columns (x, z).
    centre: egui::Vec2,
    /// Screen points per column.
    zoom: f32,
    pub(super) follow: bool,
}

impl Default for View {
    fn default() -> Self {
        Self { centre: egui::Vec2::ZERO, zoom: 2.0, follow: true }
    }
}

/// Zoom by `factor` while the column under `cursor` stays put.
pub fn zoom_about(centre: egui::Vec2, zoom: f32, cursor: egui::Vec2, factor: f32) -> (egui::Vec2, f32) {
    let next = (zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
    let anchor = centre + cursor / zoom;
    (anchor - cursor / next, next)
}

fn canvas(ui: &mut egui::Ui, s: &mut MapState, rect: Rect) {
    let Some((pos, yaw)) = s.player else { return };
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let player = egui::Vec2::new(pos.x, pos.z);
    if s.view.follow {
        s.view.centre = player;
    }
    if response.dragged() {
        s.view.centre -= response.drag_delta() / s.view.zoom;
        s.view.follow = false;
    }
    let mid = rect.center();
    if let Some(hover) = response.hover_pos() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let (centre, zoom) = zoom_about(s.view.centre, s.view.zoom, hover - mid, (scroll * 0.004).exp());
            s.view.centre = centre;
            s.view.zoom = zoom;
            s.view.follow = false;
        }
    }
    let (centre, zoom) = (s.view.centre, s.view.zoom);
    let to_screen = |x: f32, z: f32| mid + (egui::Vec2::new(x, z) - centre) * zoom;
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0, grimoire::RECESS);
    terrain(&p, s, mid, centre, zoom, Rot2::IDENTITY, rect.size().length() * 0.5 / zoom);

    for w in &s.waypoints.list {
        let at = to_screen(w.pos.x as f32 + 0.5, w.pos.z as f32 + 0.5);
        diamond(&p, at, 6.5, w.color(), &w.glyph());
        p.text(at + vec2(10.0, 0.0), Align2::LEFT_CENTER, &w.name, FontId::proportional(12.0), style::PAPER);
    }
    let at = to_screen(pos.x, pos.z);
    let arrow = [vec2(0.0, -8.0), vec2(6.0, 6.0), vec2(0.0, 3.0), vec2(-6.0, 6.0)];
    let rot = Rot2::from_angle(yaw);
    p.add(Shape::convex_polygon(arrow.iter().map(|v| at + rot * *v).collect(), style::AMBER, Stroke::new(1.0, style::INK)));

    if let Some(hover) = response.hover_pos() {
        let column = centre + (hover - mid) / zoom;
        let text = format!("{} · {}", column.x.floor() as i32, column.y.floor() as i32);
        p.text(rect.left_bottom() + vec2(10.0, -10.0), Align2::LEFT_BOTTOM, text, FontId::monospace(12.0), style::PAPER);
        if response.secondary_clicked()
            && s.waypoints.add(IVec3::new(column.x.floor() as i32, pos.y.floor() as i32, column.y.floor() as i32)).is_some()
        {
            s.waypoints_unsaved = true;
        }
    }
    p.text(rect.right_bottom() + vec2(-10.0, -10.0), Align2::RIGHT_BOTTOM, format!("{zoom:.2}x"), FontId::monospace(12.0), style::MUTED);
}

fn swatch(ui: &mut egui::Ui, colour: Color32) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::click());
    ui.painter().rect_filled(rect, 0, colour);
    ui.painter().rect_stroke(rect, 0, Stroke::new(1.0, style::INK), egui::StrokeKind::Inside);
    response.clicked()
}

fn list(ui: &mut egui::Ui, s: &mut MapState, open: &mut bool) {
    let here = s.player.map(|(p, _)| p);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Waypoints").size(15.0).color(style::AMBER));
        ui.label(egui::RichText::new(format!("{} / {MAX_WAYPOINTS}", s.waypoints.list.len())).color(style::MUTED));
    });
    ui.horizontal(|ui| {
        if ui.button("Add").clicked()
            && let Some(cell) = here.map(|p| p.floor().as_ivec3())
            && s.waypoints.add(cell).is_some()
        {
            s.waypoints_unsaved = true;
        }
        if ui.button("Centre").clicked() {
            s.view.follow = true;
        }
        if ui.button("Done").clicked() {
            *open = false;
        }
    });
    ui.separator();
    let mut remove = None;
    let mut focus = None;
    let mut changed = false;
    let voxel_m = s.voxel_m;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (i, w) in s.waypoints.list.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                if swatch(ui, w.color()) {
                    w.colour = (w.colour + 1) % super::waypoints::PALETTE.len() as u8;
                    changed = true;
                }
                let edit = egui::TextEdit::singleline(&mut w.name).char_limit(MAX_NAME).desired_width(96.0);
                if ui.add(edit).changed() {
                    w.name = clip_name(&w.name);
                    changed = true;
                }
                if let Some(p) = here {
                    let metres = (w.pos.as_vec3() + glam::Vec3::splat(0.5) - p).length() * voxel_m;
                    ui.label(egui::RichText::new(format!("{metres:.0} m")).color(style::MUTED).monospace());
                }
                if ui.small_button("◎").clicked() {
                    focus = Some(egui::Vec2::new(w.pos.x as f32, w.pos.z as f32));
                }
                if ui.small_button("✕").clicked() {
                    remove = Some(i);
                }
            });
        }
    });
    if let Some(i) = remove {
        changed |= s.waypoints.remove(i);
    }
    s.waypoints_unsaved |= changed;
    if let Some(centre) = focus {
        s.view.centre = centre;
        s.view.follow = false;
    }
}

pub(super) fn draw(ctx: &egui::Context, s: &mut MapState, open: &mut bool) {
    if !*open {
        return;
    }
    let screen = ctx.content_rect();
    egui::Area::new("world_map".into())
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            let side = Rect::from_min_max(pos2(screen.right() - LIST_WIDTH, screen.top()), screen.max);
            let map = Rect::from_min_max(screen.min, pos2(side.left(), screen.bottom()));
            canvas(ui, s, map);
            ui.painter().rect_filled(side, 0, grimoire::LEATHER);
            grimoire::binding(ui.painter(), side, false);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(side.shrink(12.0)));
            list(&mut child, s, open);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_panel_zoom_keeps_cursor_column() {
        let centre = egui::Vec2::new(100.0, -40.0);
        let cursor = egui::Vec2::new(80.0, -30.0);
        let before = centre + cursor / 2.0;
        let (next, zoom) = zoom_about(centre, 2.0, cursor, 1.5);
        assert!((zoom - 3.0).abs() < 1e-6);
        assert!(((next + cursor / zoom) - before).length() < 1e-3);
        assert_eq!(zoom_about(centre, ZOOM_MAX, cursor, 4.0).1, ZOOM_MAX);
    }
}
