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

impl View {
    pub(super) fn focus_chart(&mut self, point: egui::Vec2) {
        self.centre = point;
        self.follow = false;
    }
    pub(super) fn focus(&mut self, pos: IVec3) {
        self.centre = egui::Vec2::new(pos.x as f32, pos.z as f32);
        self.follow = false;
    }
}

/// Zoom by `factor` while the column under `cursor` stays put.
pub fn zoom_about(centre: egui::Vec2, zoom: f32, cursor: egui::Vec2, factor: f32) -> (egui::Vec2, f32) {
    let next = (zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
    let anchor = centre + cursor / zoom;
    (anchor - cursor / next, next)
}

fn canvas(ui: &mut egui::Ui, s: &mut MapState, rect: Rect) {
    let Some((pos, mut yaw)) = s.player else { return };
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let strict = !s.knowledge.legacy;
    let player = if strict { s.knowledge.project(pos * s.voxel_m, false) } else { egui::Vec2::new(pos.x, pos.z) };
    if strict { yaw = s.knowledge.yaw; }
    if s.view.follow {
        s.view.centre = if strict && s.knowledge.selected.is_some() && s.knowledge.selected != s.knowledge.active {
            egui::Vec2::ZERO
        } else { player };
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
    terrain(&p, s, mid, centre, zoom, Rot2::IDENTITY, rect.size().length() * 0.5 / zoom, false);

    if let Some(freight) = &s.freight {
        use crate::world::farming::planet_freight::Clearance;
        use engine_worldgen::planet_freight::{BRIDGE, DETOUR_Z, EAST_SETTLEMENT, HOLDINGS, LANES, WEST_SETTLEMENT};
        let screen = |x: f32, z: f32| to_screen((freight.site.origin.x + x) / s.voxel_m, (freight.site.origin.z + z) / s.voxel_m);
        let road = Stroke::new(2.0, style::AMBER);
        let (near, far) = (BRIDGE.start as f32, BRIDGE.end as f32);
        p.line_segment([screen(0.0, 0.0), screen(near - 2.0, 0.0)], road);
        p.line_segment([screen(far + 2.0, 0.0), screen(HOLDINGS[1].0 as f32, 0.0)], road);
        for (lane, status) in LANES.into_iter().zip(freight.lanes) {
            let color = match status {
                Clearance::Open => style::AMBER,
                Clearance::Blocked => egui::Color32::LIGHT_RED,
                Clearance::Unobserved => style::MUTED,
            };
            p.add(Shape::line(vec![
                screen(near - 2.0, 0.0),
                screen(near - 1.0, lane),
                screen(far + 1.0, lane),
                screen(far + 2.0, 0.0),
            ], Stroke::new(1.5, color)));
        }
        if let Some(status) = freight.detour {
            let color = match status {
                Clearance::Open => style::JADE,
                Clearance::Blocked => egui::Color32::LIGHT_RED,
                Clearance::Unobserved => style::MUTED,
            };
            p.add(Shape::line(vec![screen(near - 2.0, 0.0), screen(near - 2.0, DETOUR_Z),
                screen(far + 2.0, DETOUR_Z), screen(far + 2.0, 0.0)], Stroke::new(1.5, color)));
        }
        let labels = HOLDINGS.into_iter().zip([EAST_SETTLEMENT, "South holding"]);
        for ((x, z), label) in std::iter::once(((0, 8), WEST_SETTLEMENT)).chain(labels) {
            let (x, z) = (x as f32, z as f32);
            let at = screen(x, z);
            p.line_segment([screen(x, 0.0), at], road);
            p.circle_filled(at, 3.0, style::AMBER);
            p.text(at + vec2(5.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(12.0), style::PAPER);
        }
        if let Some(cart) = freight.cart {
            p.circle_filled(to_screen(cart.x / s.voxel_m, cart.z / s.voxel_m), 3.5, style::PAPER);
        }
    }

    for w in &s.waypoints.list {
        if strict && !s.knowledge.marker_on_layer(w.pos, s.voxel_m, false) { continue; }
        let target = if strict { s.knowledge.project(w.pos.as_vec3() * s.voxel_m, false) } else { egui::vec2(w.pos.x as f32 + 0.5, w.pos.z as f32 + 0.5) };
        let at = to_screen(target.x, target.y);
        diamond(&p, at, 6.5, w.color(), &w.glyph());
        p.text(at + vec2(10.0, 0.0), Align2::LEFT_CENTER, &w.name, FontId::proportional(12.0), style::PAPER);
    }
    for w in s.objective.iter().chain(s.delivery.iter()).chain(s.workshop_marker.iter()).filter(|w| !strict || s.knowledge.marker_on_layer(w.pos, s.voxel_m, false)).cloned().collect::<Vec<_>>() {
        let target = if strict { s.knowledge.project(w.pos.as_vec3() * s.voxel_m, false) } else { egui::vec2(w.pos.x as f32 + 0.5, w.pos.z as f32 + 0.5) };
        let at = to_screen(target.x, target.y);
        diamond(&p, at, 8.0, style::AMBER, "!");
        p.text(at + vec2(12.0, 0.0), Align2::LEFT_CENTER, &w.name, FontId::proportional(12.0), style::AMBER);
        let hit = Rect::from_center_size(at, vec2(22.0, 22.0)).intersect(rect);
        if hit.is_positive() && ui.interact(hit, egui::Id::new(("map_destination_pin", w.pos, &w.name)), Sense::click())
            .on_hover_text("Pin Waypoint").clicked() {
            s.pin(w);
        }
    }
    let at = to_screen(player.x, player.y);
    let arrow = [vec2(0.0, -8.0), vec2(6.0, 6.0), vec2(0.0, 3.0), vec2(-6.0, 6.0)];
    let rot = Rot2::from_angle(yaw);
    if !strict || s.knowledge.selected.is_none() || s.knowledge.selected == s.knowledge.active {
        p.add(Shape::convex_polygon(arrow.iter().map(|v| at + rot * *v).collect(), style::AMBER, Stroke::new(1.0, style::INK)));
    }

    if let Some(hover) = response.hover_pos() {
        let column = centre + (hover - mid) / zoom;
        let text = format!("{} · {}", column.x.floor() as i32, column.y.floor() as i32);
        p.text(rect.left_bottom() + vec2(10.0, -10.0), Align2::LEFT_BOTTOM, text, FontId::monospace(12.0), style::PAPER);
        let waypoint = if strict { s.knowledge.unproject(column, s.voxel_m) } else { Some(IVec3::new(column.x.floor() as i32, pos.y.floor() as i32, column.y.floor() as i32)) };
        if response.secondary_clicked() && let Some(cell) = waypoint
            && s.waypoints.add(cell).is_some()
        {
            if strict { s.knowledge.mark(cell, false); }
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
    let before = (s.knowledge.selected, s.knowledge.legacy);
    if s.config.is_some_and(|c|c.world_type==crate::world::WorldType::Manyfold) {
        s.knowledge.picker(ui);
    } else {ui.strong("Surface");}
    if before != (s.knowledge.selected, s.knowledge.legacy) { s.view.follow = true; }
    if let Some(freight) = &s.freight {
        ui.separator();
        ui.strong("Market Freight");
        ui.label(&freight.text);
        ui.weak("Use the market counter to collect stock.");
        let pos = (freight.site.market() / s.voxel_m).floor().as_ivec3();
        ui.horizontal(|ui| {
            if ui.button("Locate").clicked() {
                s.view.focus(pos);
            }
            if ui.button("Pin Market").clicked() {
                s.pin(super::waypoints::Waypoint {
                    name: "Market".into(),
                    pos,
                    colour: 0,
                    kind: super::waypoints::Kind::Marker,
                });
            }
        });
    }
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
            if !s.knowledge.legacy { s.knowledge.mark(cell, true); }
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
    for w in s.objective.iter().chain(s.delivery.iter()).chain(s.workshop_marker.iter()).filter(|w| s.knowledge.legacy || s.knowledge.marker_on_layer(w.pos, s.voxel_m, false)).cloned().collect::<Vec<_>>() {
        ui.label(egui::RichText::new(&w.name).color(style::AMBER));
        if let Some(p) = here.filter(|_| s.knowledge.legacy || s.knowledge.can_measure_from_player()) {
            let delta = (w.pos.as_vec3() + glam::Vec3::splat(0.5) - p) * s.voxel_m;
            ui.label(format!("{:.0} m · {:+.0} m elevation", delta.x.hypot(delta.z), delta.y));
        }
        ui.push_id(("destination", w.pos, &w.name), |ui| { ui.horizontal(|ui| {
            if ui.button("Locate").clicked() {
                if s.knowledge.legacy { s.view.focus(w.pos); }
                else { s.view.focus_chart(s.knowledge.project(w.pos.as_vec3() * s.voxel_m, false)); }
            }
            if ui.button("Pin").clicked() { s.pin(w.clone()); }
        }); });
        ui.separator();
    }
    let mut remove = None;
    let mut focus = None;
    let mut changed = false;
    let voxel_m = s.voxel_m;
    let knowledge = &s.knowledge;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (i, w) in s.waypoints.list.iter_mut().enumerate() {
            let located = knowledge.legacy || knowledge.marker_on_layer(w.pos, voxel_m, false);
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
                if let Some(p) = here.filter(|_| located && (knowledge.legacy || knowledge.can_measure_from_player())) {
                    let delta = (w.pos.as_vec3() + glam::Vec3::splat(0.5) - p) * voxel_m;
                    let metres = delta.x.hypot(delta.z);
                    ui.label(egui::RichText::new(format!("{metres:.0} m")).color(style::MUTED).monospace())
                        .on_hover_text(format!("Elevation: {:+.0} m", delta.y));
                }
                let death = w.kind == super::waypoints::Kind::Death;
                if ui.add_enabled(located, egui::Button::new("◎").small())
                    .on_hover_text(if death { "Center on last death · Walk over dropped gear" } else { "Center on waypoint" })
                    .clicked() {
                    focus = Some(if knowledge.legacy { egui::Vec2::new(w.pos.x as f32, w.pos.z as f32) }
                        else { knowledge.project(w.pos.as_vec3() * voxel_m, false) });
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

pub(super) fn draw(ctx: &egui::Context, s: &mut MapState, open: &mut bool, workshop: bool) -> Option<u32> {
    if !*open {
        return None;
    }
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new("world_map"))
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
            let page = if workshop {
                use crate::egui_ui::workshop::{Destination, navigation};
                match navigation(&mut child, None) {
                    Some(Destination::Page(page)) => Some(page),
                    _ => None,
                }
            } else {
                None
            };
            list(&mut child, s, open);
            page
        }).inner
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
