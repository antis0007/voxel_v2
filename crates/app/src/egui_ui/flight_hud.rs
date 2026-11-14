//! Flight instruments: body markers, target bracket, velocity marker, orbit lines.
//!
//! Drawn over the world while the player is off the ground of a planet world,
//! and for a chosen target anywhere. Directions come from the same rails the
//! sky is drawn from, so a marker sits on its body.
use super::presentation::{AMBER, JADE, MUTED, PAPER};
use crate::space_nav::{self, Contact, Fix, text};
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, pos2, vec2};
use engine_space::{BodyId, BodyKind, G, System};
use glam::{DVec3, Vec3};
use std::sync::OnceLock;

#[path = "flight_hud_orbits.rs"]
mod orbits;

/// Speed below which the velocity marker hides, m/s.
const MARKER_SPEED_MPS: f64 = 20.0;
/// Bodies drawn wider than this need no ring to be found, points.
const RING_MAX: f32 = 40.0;

pub(super) fn body_color(kind: &BodyKind) -> Color32 {
    match kind {
        BodyKind::Star { .. } => Color32::from_rgb(255, 221, 150),
        BodyKind::Planet => Color32::from_rgb(120, 190, 220),
        BodyKind::Moon => Color32::from_rgb(190, 196, 200),
        BodyKind::GasGiant => Color32::from_rgb(222, 170, 120),
        BodyKind::Barycenter => MUTED,
    }
}

/// Camera axes and lens that place world directions on the screen.
pub(super) struct Lens {
    right: Vec3,
    up: Vec3,
    forward: Vec3,
    /// Points per unit of tangent, both axes.
    scale: f32,
    pub(super) screen: Rect,
}

impl Lens {
    pub(super) fn new(forward: Vec3, up: Vec3, fov_y_degrees: f32, screen: Rect) -> Self {
        let right = forward.cross(up).normalize_or(Vec3::X);
        let scale = screen.height() * 0.5 / (fov_y_degrees.to_radians() * 0.5).tan().max(1e-3);
        Self { right, up: right.cross(forward), forward, scale, screen }
    }

    /// Camera-space coordinates: x right, y up, z ahead.
    pub(super) fn camera(&self, v: DVec3) -> DVec3 {
        DVec3::new(v.dot(self.right.as_dvec3()), v.dot(self.up.as_dvec3()), v.dot(self.forward.as_dvec3()))
    }

    /// Screen point of a camera-space position ahead of the eye.
    pub(super) fn place(&self, c: DVec3) -> Pos2 {
        let centre = self.screen.center();
        pos2(centre.x + (c.x / c.z) as f32 * self.scale, centre.y - (c.y / c.z) as f32 * self.scale)
    }

    /// Screen point of a world direction; `None` behind the eye.
    fn project(&self, dir: Vec3) -> Option<Pos2> {
        let c = self.camera(dir.as_dvec3());
        (c.z > 1e-3).then(|| self.place(c))
    }

    fn points(&self, angle: f32) -> f32 {
        angle.tan() * self.scale
    }
}

/// Gravity at the fix toward its reference body, m/s².
fn gravity(system: &System, fix: &Fix) -> f64 {
    let body = fix.reference;
    let radius = space_nav::surface_radius(system, body);
    let distance = radius + fix.altitude_m(system);
    if system.name(body) == "home" {
        // Home's ground pull is the game's 9.81, not the rails' mass.
        return 9.81 * (radius / distance).powi(2);
    }
    G * system.mass(body) / (distance * distance)
}

fn in_flight(system: &System, fix: &Fix, player: &crate::player::Player) -> bool {
    player.anchor.is_some()
        || player.void.is_some()
        || system.name(fix.reference) != "home"
        || fix.altitude_m(system) > crate::altitude::TRANSITION_M
}

pub fn draw(ctx: &egui::Context, world: &crate::world::World, player: &crate::player::Player, fov_y_degrees: f32) {
    let (Some(system), Some(fix)) = (space_nav::system(), space_nav::fix(world, player)) else { return };
    let target = space_nav::target();
    let flying = in_flight(system, &fix, player);
    if !flying && target.is_none() {
        return;
    }
    let lens = Lens::new(player.forward_dir(), player.up_dir(), fov_y_degrees, ctx.content_rect());
    let painter = ctx.layer_painter(egui::LayerId::background());
    let speed_mps = f64::from(player.vel.length() * world.voxel_scale.meters());
    if flying {
        // Under the atmosphere the system's lines only cross the sky; they
        // read as paths once the player is flying among them.
        let in_space = player.orbit.is_some() || player.void.is_some() || player.anchor.is_some()
            || system.name(fix.reference) != "home";
        if space_nav::orbit_lines() && (in_space || target.is_some()) {
            orbits::draw(&painter, &lens, system, &fix, target, in_space);
        }
        if let Some(orbit) = &player.orbit {
            orbits::own_path(&painter, &lens, system, &fix, orbit);
        }
        for contact in fix.contacts(system).iter().filter(|c| Some(c.body) != target) {
            marker(&painter, &lens, system, contact);
        }
        if speed_mps > MARKER_SPEED_MPS {
            velocity_marker(&painter, &lens, player.vel.normalize_or_zero());
        }
    }
    let tracked = target.map(|body| {
        let contact = fix.contact(system, body);
        let closing = space_nav::closing_mps(body, fix.tick, contact.range_m);
        bracket(&painter, &lens, system, &contact);
        (contact, closing)
    });
    if flying {
        readout(ctx, system, &fix, speed_mps, tracked, player.orbit.as_ref());
    }
    if let Some(orbit) = &player.orbit {
        notice(ctx, orbit);
    }
}

/// Seconds the orbital-flight line stays after the atmosphere is left.
const NOTICE_SECONDS: f32 = 7.0;

/// One fading line when the flight model changes at the top of the atmosphere.
fn notice(ctx: &egui::Context, orbit: &crate::planet::orbit::Orbit) {
    let fade = ((NOTICE_SECONDS - orbit.age) / 1.5).clamp(0.0, 1.0) * (orbit.age / 0.3).clamp(0.0, 1.0);
    if fade <= 0.0 {
        return;
    }
    let screen = ctx.content_rect();
    let at = pos2(screen.center().x, screen.bottom() - 96.0);
    let line = format!("Orbital Flight · [{}] Brake", crate::keybinds::hint(crate::keybinds::Action::Dash));
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.text(at + vec2(1.0, 1.0), Align2::CENTER_BOTTOM, &line, FontId::proportional(14.0), Color32::from_black_alpha((160.0 * fade) as u8));
    painter.text(at, Align2::CENTER_BOTTOM, line, FontId::proportional(14.0), AMBER.gamma_multiply(fade));
    ctx.request_repaint();
}

fn label_font() -> FontId {
    FontId::proportional(12.0)
}

/// Ring and name for a body that is not the target.
fn marker(painter: &egui::Painter, lens: &Lens, system: &System, contact: &Contact) {
    let Some(at) = lens.project(contact.dir).filter(|p| lens.screen.contains(*p)) else { return };
    let color = body_color(system.kind(contact.body));
    let disc = lens.points(contact.angular_radius);
    let ring = disc.max(5.0) + 3.0;
    if disc < RING_MAX {
        painter.circle_stroke(at, ring, Stroke::new(1.0, color.gamma_multiply(0.7)));
    }
    let label = format!("{} · {}", text::name(system, contact.body), text::range(contact.range_m));
    painter.text(at + vec2(ring.min(RING_MAX) + 5.0, 0.0), Align2::LEFT_CENTER, label, label_font(), color.gamma_multiply(0.85));
}

/// Corner bracket on the target, or an edge arrow toward it when it is out of view.
fn bracket(painter: &egui::Painter, lens: &Lens, system: &System, contact: &Contact) {
    let label = format!("{} · {}", text::name(system, contact.body), text::range(contact.range_m));
    let stroke = Stroke::new(1.5, AMBER);
    let inner = lens.screen.shrink(36.0);
    if let Some(at) = lens.project(contact.dir).filter(|p| inner.contains(*p)) {
        let half = (lens.points(contact.angular_radius) + 6.0).clamp(12.0, lens.screen.height() * 0.45);
        let arm = (half * 0.45).min(14.0);
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let corner = at + vec2(sx * half, sy * half);
            painter.line_segment([corner, corner - vec2(sx * arm, 0.0)], stroke);
            painter.line_segment([corner, corner - vec2(0.0, sy * arm)], stroke);
        }
        painter.text(at + vec2(0.0, half + 4.0), Align2::CENTER_TOP, label, label_font(), AMBER);
        return;
    }
    // Behind or beside the view: the camera-plane direction still points the short way round.
    let c = lens.camera(contact.dir.as_dvec3());
    let way = vec2(c.x as f32, -(c.y as f32));
    let way = if way.length() > 1e-4 { way.normalized() } else { vec2(0.0, 1.0) };
    let reach = inner.size() * 0.5;
    let scale = (reach.x / way.x.abs().max(1e-4)).min(reach.y / way.y.abs().max(1e-4));
    let tip = inner.center() + way * scale;
    let side = vec2(-way.y, way.x);
    let back = tip - way * 14.0;
    painter.add(egui::Shape::convex_polygon(vec![tip, back + side * 7.0, back - side * 7.0], AMBER, Stroke::NONE));
    let anchor = match (way.x.abs() > way.y.abs(), way.x > 0.0, way.y > 0.0) {
        (true, true, _) => Align2::RIGHT_CENTER,
        (true, false, _) => Align2::LEFT_CENTER,
        (false, _, true) => Align2::CENTER_BOTTOM,
        (false, _, false) => Align2::CENTER_TOP,
    };
    painter.text(back - way * 6.0, anchor, label, label_font(), AMBER);
}

/// Where the velocity points: a ticked circle ahead, a crossed one behind.
fn velocity_marker(painter: &egui::Painter, lens: &Lens, heading: Vec3) {
    let stroke = Stroke::new(1.5, JADE);
    let r = 7.0;
    if let Some(at) = lens.project(heading).filter(|p| lens.screen.contains(*p)) {
        painter.circle_stroke(at, r, stroke);
        for tick in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0)] {
            painter.line_segment([at + tick * r, at + tick * (r + 6.0)], stroke);
        }
    } else if let Some(at) = lens.project(-heading).filter(|p| lens.screen.contains(*p)) {
        painter.circle_stroke(at, r, stroke);
        let d = r * std::f32::consts::FRAC_1_SQRT_2;
        painter.line_segment([at + vec2(-d, -d), at + vec2(d, d)], stroke);
        painter.line_segment([at + vec2(-d, d), at + vec2(d, -d)], stroke);
    }
}

/// One or two quiet lines in the lower left corner: where, how fast, and the path.
fn readout(
    ctx: &egui::Context,
    system: &System,
    fix: &Fix,
    speed_mps: f64,
    tracked: Option<(Contact, f64)>,
    orbit: Option<&crate::planet::orbit::Orbit>,
) {
    use crate::planet::orbit::Path;
    let mut lines: Vec<(String, Color32)> = Vec::new();
    lines.push((
        format!("{} · {} · {} · {:.2} m/s²", text::name(system, fix.reference), text::range(fix.altitude_m(system)),
            text::speed(speed_mps), gravity(system, fix)),
        PAPER.gamma_multiply(0.8),
    ));
    if let Some(orbit) = orbit {
        let e = orbit.elements();
        let (state, color) = match e.path() {
            Path::Orbit => ("Orbit", JADE),
            Path::Suborbital => ("Suborbital", AMBER),
            Path::Escape => ("Escape", AMBER),
        };
        let low = if e.periapsis_m > 0.0 { text::range(e.periapsis_m) } else { "Ground".to_owned() };
        let high = e.apoapsis_m.map_or_else(|| "Open".to_owned(), text::range);
        lines.push((
            format!("{state} · Low {low} · High {high} · Circular {}", text::speed(e.circular_mps)),
            color.gamma_multiply(0.8),
        ));
    }
    if let Some((contact, closing)) = tracked {
        let mut line = format!("{} · {} · {}", text::name(system, contact.body), text::range(contact.range_m), text::speed(closing));
        if closing > 1.0 {
            line.push_str(&format!(" · {}", text::duration(contact.range_m / closing)));
        }
        lines.push((line, AMBER.gamma_multiply(0.85)));
    }
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let font = FontId::monospace(12.0);
    let mut at = pos2(screen.left() + 14.0, screen.bottom() - 14.0);
    for (line, color) in lines.into_iter().rev() {
        painter.text(at + vec2(1.0, 1.0), Align2::LEFT_BOTTOM, &line, font.clone(), Color32::from_black_alpha(140));
        painter.text(at, Align2::LEFT_BOTTOM, line, font.clone(), color);
        at.y -= 16.0;
    }
}

/// Cached parent-relative orbit polylines, indexed by body.
pub(super) fn orbit_paths(system: &System) -> &'static [Vec<DVec3>] {
    static PATHS: OnceLock<Vec<Vec<DVec3>>> = OnceLock::new();
    PATHS.get_or_init(|| {
        system
            .bodies()
            .map(|b: BodyId| {
                let mut path = Vec::new();
                let tolerance = system.orbit(b).map_or(1.0, |o| o.a * 2.0e-4);
                engine_space::orbit_path(system, b, tolerance, &mut path);
                path
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lens_centres_the_view_axis_and_keeps_up_up() {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0));
        let lens = Lens::new(Vec3::NEG_Z, Vec3::Y, 90.0, screen);
        assert_eq!(lens.project(Vec3::NEG_Z), Some(screen.center()));
        assert!(lens.project(Vec3::Z).is_none());
        // 45 degrees up at a 90 degree lens is the top edge.
        let top = lens.project(Vec3::new(0.0, 1.0, -1.0).normalize()).unwrap();
        assert!((top.y - 0.0).abs() < 0.01 && (top.x - 800.0).abs() < 0.01);
        let right = lens.project(Vec3::new(0.5, 0.0, -1.0).normalize()).unwrap();
        assert!(right.x > 800.0 && (right.y - 450.0).abs() < 0.01);
    }

    #[test]
    fn orbit_paths_close_and_stay_small() {
        let system = space_nav::system().unwrap();
        let paths = orbit_paths(system);
        assert_eq!(paths.len(), system.len());
        assert!(paths[0].is_empty());
        for path in &paths[1..] {
            assert!(path.len() > 16 && path.len() < 400, "{} points", path.len());
            assert_eq!(path.first(), path.last());
        }
    }
}
