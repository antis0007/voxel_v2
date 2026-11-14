//! Orrery canvas: orbits, bodies, the player, the target and the transfer arc.
use super::super::flight_hud::{body_color, orbit_paths};
use super::super::grimoire::RECESS;
use super::super::presentation::{AMBER, JADE, MUTED, PAPER};
use super::View;
use crate::space_nav::{self, Fix, TICKS_PER_SECOND, Transfer, text};
use egui::{Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, pos2, vec2};
use engine_space::{BodyId, BodyKind, Snapshot, System};
use glam::DVec3;
use std::f64::consts::{PI, TAU};

const ZOOM_MIN: f64 = 1.0e-10;
const ZOOM_MAX: f64 = 0.05;
/// Share of the shorter canvas side a fitted system spans from its centre.
const FIT: f64 = 0.42;
const PICK_POINTS: f32 = 14.0;
const ARC_POINTS: usize = 48;

pub(super) struct Scene<'a> {
    pub system: &'a System,
    pub fix: &'a Fix,
    /// Bodies at the shown time.
    pub shown: &'a Snapshot,
    pub shown_tick: u64,
    pub plan: Option<Transfer>,
    pub target: Option<BodyId>,
}

/// Radius that holds a body and everything orbiting it, metres.
fn extent(system: &System, body: BodyId) -> f64 {
    system
        .bodies()
        .filter(|b| system.parent(*b) == Some(body))
        .filter_map(|b| system.orbit(b))
        .map(|o| o.a * (1.0 + o.e))
        .fold(space_nav::surface_radius(system, body) * 6.0, f64::max)
}

/// Smallest drawn radius per kind, points.
fn floor_points(kind: &BodyKind) -> f32 {
    match kind {
        BodyKind::Star { .. } => 9.0,
        BodyKind::GasGiant => 6.5,
        BodyKind::Planet => 5.0,
        BodyKind::Moon => 3.5,
        BodyKind::Barycenter => 2.0,
    }
}

/// Polar angle about +Y from +X toward -Z, the way the rails turn.
fn longitude(v: DVec3) -> f64 {
    (-v.z).atan2(v.x)
}

/// Hohmann half-ellipse from radius `r1` at longitude `l0` to radius `r2`, in the system plane.
// Inclinations at home stay under a few degrees, so the arc ignores them.
fn transfer_arc(r1: f64, r2: f64, l0: f64) -> impl Iterator<Item = DVec3> {
    let a = 0.5 * (r1 + r2);
    let e = (r2 - r1).abs() / (r1 + r2);
    // Outward the burn is at periapsis; inward it is at apoapsis.
    let start = if r2 >= r1 { 0.0 } else { PI };
    (0..=ARC_POINTS).map(move |k| {
        let swept = PI * k as f64 / ARC_POINTS as f64;
        let nu = start + swept;
        let r = a * (1.0 - e * e) / (1.0 + e * nu.cos());
        let l = l0 + swept;
        DVec3::new(r * l.cos(), 0.0, -r * l.sin())
    })
}

/// A round scale length near `metres`: 1, 2 or 5 times a power of ten.
fn round_length(metres: f64) -> f64 {
    let power = 10f64.powf(metres.max(1e-9).log10().floor());
    let lead = metres / power;
    power * if lead >= 5.0 { 5.0 } else if lead >= 2.0 { 2.0 } else { 1.0 }
}

pub(super) fn draw(ui: &mut egui::Ui, rect: Rect, response: &egui::Response, view: &mut View, scene: &Scene) {
    let Scene { system, fix, shown, .. } = *scene;
    if !view.fitted {
        view.zoom = (FIT * f64::from(rect.width().min(rect.height())) / extent(system, view.focus)).clamp(ZOOM_MIN, ZOOM_MAX);
        view.fitted = true;
    }
    if response.dragged() {
        let d = response.drag_delta();
        if ui.input(|i| i.modifiers.shift) {
            view.pan += d;
        } else {
            view.yaw += d.x * 0.008;
            view.pitch = (view.pitch + d.y * 0.008).clamp(0.0, 1.5);
        }
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        view.zoom = (view.zoom * f64::from(scroll * 0.004).exp()).clamp(ZOOM_MIN, ZOOM_MAX);
    }
    let origin = shown.position(view.focus);
    let (sy, cy) = f64::from(view.yaw).sin_cos();
    let (sp, cp) = f64::from(view.pitch).sin_cos();
    let zoom = view.zoom;
    let mid = rect.center() + view.pan;
    let to_screen = |p: DVec3| -> Pos2 {
        let r = p - origin;
        let (x, z) = (r.x * cy + r.z * sy, -r.x * sy + r.z * cy);
        pos2(mid.x + (x * zoom) as f32, mid.y + ((z * cp - r.y * sp) * zoom) as f32)
    };
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0, RECESS);

    let mut bodies: Vec<BodyId> = system.bodies()
        .filter(|b| !matches!(system.kind(*b), BodyKind::Barycenter))
        .filter(|b| std::iter::successors(Some(*b), |body| system.parent(*body)).any(|body| body == view.focus))
        .collect();
    // Reserve label space for the selection and destination before other bodies.
    bodies.sort_by_key(|b| (if *b == view.selected { 0 } else if Some(*b) == scene.target { 1 } else { 2 }, b.0));
    let show_orbits = space_nav::orbit_lines();
    for &body in &bodies {
        if !show_orbits { break; }
        if body == view.focus { continue; }
        let Some(parent) = system.parent(body) else { continue };
        let centre = shown.position(parent);
        let line: Vec<Pos2> = orbit_paths(system)[body.0 as usize].iter().map(|q| to_screen(centre + *q)).collect();
        let chosen = body == view.selected || Some(body) == scene.target;
        let color = if Some(body) == scene.target { AMBER } else { body_color(system.kind(body)) };
        p.add(Shape::line(line, Stroke::new(if chosen { 1.5 } else { 1.0 }, color.gamma_multiply(if chosen { 0.8 } else { 0.4 }))));
    }
    if show_orbits && let Some(soi) = system.soi(view.selected) {
        let centre = shown.position(view.selected);
        let ring: Vec<Pos2> = (0..=64).map(|k| {
            let a = TAU * f64::from(k) / 64.0;
            to_screen(centre + DVec3::new(a.cos(), 0.0, a.sin()) * soi)
        }).collect();
        p.add(Shape::line(ring, Stroke::new(1.0, MUTED.gamma_multiply(0.35))));
    }
    if let Some(plan) = scene.plan {
        arc(&p, scene, view.selected, plan, &to_screen);
    }

    let font = FontId::proportional(12.0);
    let mut picked: Option<(f32, BodyId)> = None;
    let mut labels: Vec<Rect> = Vec::new();
    let click = response.interact_pointer_pos().filter(|_| response.clicked() || response.double_clicked());
    for &body in &bodies {
        let at = to_screen(shown.position(body));
        let kind = system.kind(body);
        let radius = ((space_nav::surface_radius(system, body) * zoom) as f32).max(floor_points(kind));
        if !rect.expand(radius).contains(at) {
            continue;
        }
        p.circle_filled(at, radius, body_color(kind));
        if body == view.selected {
            p.circle_stroke(at, radius + 3.0, Stroke::new(1.0, PAPER));
        }
        if Some(body) == scene.target {
            let half = radius + 7.0;
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = at + vec2(sx * half, sy * half);
                p.line_segment([corner, corner - vec2(sx * 5.0, 0.0)], Stroke::new(1.5, AMBER));
                p.line_segment([corner, corner - vec2(0.0, sy * 5.0)], Stroke::new(1.5, AMBER));
            }
        }
        let galley = p.layout_no_wrap(text::name(system, body), font.clone(), PAPER);
        let label_at = at + vec2(radius + 6.0, -galley.size().y * 0.5);
        for offset in [0.0, -18.0, 18.0, -36.0, 36.0] {
            let label = Rect::from_min_size(label_at + vec2(0.0, offset), galley.size());
            if rect.contains_rect(label) && labels.iter().all(|r| !r.intersects(label.expand(3.0))) {
                if offset != 0.0 { p.line_segment([at + vec2(radius, 0.0), label.left_center()], Stroke::new(0.5, MUTED)); }
                p.galley(label.min, galley.clone(), PAPER);
                labels.push(label);
                break;
            }
        }
        if let Some(click) = click {
            let d = (click.distance(at) - radius).max(0.0);
            if d < PICK_POINTS && picked.is_none_or(|(best, _)| d < best) {
                picked = Some((d, body));
            }
        }
    }
    // The player rides the reference body through the shown time.
    let here = to_screen(shown.position(fix.reference) + (fix.pos - fix.snapshot.position(fix.reference)));
    if scene.shown_tick == fix.tick && rect.contains(here) {
        let diamond = [vec2(0.0, -6.0), vec2(5.0, 0.0), vec2(0.0, 6.0), vec2(-5.0, 0.0)].map(|o| here + o).to_vec();
        p.add(Shape::convex_polygon(diamond, PAPER, Stroke::new(1.0, Color32::BLACK)));
        p.text(here + vec2(9.0, 12.0), Align2::LEFT_TOP, "Position", font.clone(), PAPER);
    }
    scale_bar(&p, rect, zoom, font);

    if let Some((_, body)) = picked {
        view.selected = body;
        if response.double_clicked() {
            view.focus_on(body);
        }
    }
}

/// Transfer arc, its arrival point and where the destination will be then.
fn arc(p: &egui::Painter, scene: &Scene, to: BodyId, plan: Transfer, to_screen: &impl Fn(DVec3) -> Pos2) {
    let Scene { system, shown, .. } = *scene;
    let (Some(to_orbit), Some(hub)) = (system.orbit(to), system.parent(to)) else { return };
    let mut arrival = Snapshot::default();
    system.evaluate(scene.shown_tick + (plan.coast_s * TICKS_PER_SECOND) as u64, &mut arrival);
    let centre = shown.position(hub);
    // The destination at arrival, drawn about the hub where it is shown now.
    let meet = arrival.position(to) - arrival.position(hub);
    let (r1, l0) = if plan.child {
        // Any time works from a parking orbit: leave opposite the meeting point.
        (space_nav::surface_radius(system, plan.from) * 1.25, longitude(meet) - PI)
    } else {
        let from = shown.position(plan.from) - centre;
        (from.length(), longitude(from))
    };
    let line: Vec<Pos2> = transfer_arc(r1, to_orbit.a, l0).map(|q| to_screen(centre + q)).collect();
    let end = line.last().copied();
    p.add(Shape::line(line, Stroke::new(1.5, JADE)));
    let ghost = to_screen(centre + meet);
    p.circle_stroke(ghost, floor_points(system.kind(to)) + 2.0, Stroke::new(1.0, body_color(system.kind(to))));
    if let Some(end) = end {
        p.line_segment([end - vec2(4.0, 4.0), end + vec2(4.0, 4.0)], Stroke::new(1.5, JADE));
        p.line_segment([end - vec2(4.0, -4.0), end + vec2(4.0, -4.0)], Stroke::new(1.5, JADE));
    }
}

fn scale_bar(p: &egui::Painter, rect: Rect, zoom: f64, font: FontId) {
    let metres = round_length(100.0 / zoom);
    let width = (metres * zoom) as f32;
    let left = rect.left_bottom() + vec2(14.0, -16.0);
    let stroke = Stroke::new(1.0, MUTED);
    p.line_segment([left, left + vec2(width, 0.0)], stroke);
    for x in [0.0, width] {
        p.line_segment([left + vec2(x, -4.0), left + vec2(x, 0.0)], stroke);
    }
    p.text(left + vec2(width + 6.0, 0.0), Align2::LEFT_CENTER, text::range(metres), font, MUTED);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_arcs_join_both_orbits_half_a_turn_apart() {
        for (r1, r2) in [(1.0e8, 4.0e8), (4.0e8, 1.0e8)] {
            let points: Vec<DVec3> = transfer_arc(r1, r2, 0.3).collect();
            let (first, last) = (points[0], points[ARC_POINTS]);
            assert!((first.length() / r1 - 1.0).abs() < 1e-9 && (last.length() / r2 - 1.0).abs() < 1e-9);
            assert!((longitude(first) - 0.3).abs() < 1e-9);
            assert!((longitude(last) - (0.3 + PI - TAU)).abs() < 1e-9);
            assert!(points.iter().all(|q| q.length() >= r1.min(r2) * 0.999 && q.length() <= r1.max(r2) * 1.001));
        }
    }

    #[test]
    fn scale_lengths_are_round() {
        for (metres, round) in [(730.0, 500.0), (2.4e8, 2.0e8), (1.1e3, 1.0e3)] {
            assert!((round_length(metres) / round - 1.0).abs() < 1e-9, "{metres}");
        }
    }

    #[test]
    fn extent_holds_the_orbits_of_the_children() {
        let system = space_nav::system().unwrap();
        let (star, home, moon) = (system.id("star").unwrap(), system.id("home").unwrap(), system.id("moon").unwrap());
        assert!(extent(system, star) > system.orbit(home).unwrap().a);
        assert!(extent(system, home) >= system.orbit(moon).unwrap().a);
        assert_eq!(extent(system, moon), system.radius(moon) * 6.0);
    }
}
