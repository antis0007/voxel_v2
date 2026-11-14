//! Orbit lines drawn across the sky from the eye's place in the system.
use super::{Lens, body_color, orbit_paths};
use crate::space_nav::Fix;
use egui::Stroke;
use engine_space::{BodyId, System};
use glam::DVec3;

/// Nearest depth kept, as a fraction of a segment's farther end.
const NEAR: f64 = 0.02;

/// Part of a camera-space segment ahead of the near depth, scaled to unit size.
fn clip(a: DVec3, b: DVec3) -> Option<(DVec3, DVec3)> {
    let size = a.length().max(b.length());
    if size <= 0.0 {
        return None;
    }
    let (a, b) = (a / size, b / size);
    match (a.z >= NEAR, b.z >= NEAR) {
        (true, true) => Some((a, b)),
        (false, false) => None,
        (ahead, _) => {
            let cut = a + (b - a) * ((NEAR - a.z) / (b.z - a.z));
            Some(if ahead { (a, cut) } else { (cut, b) })
        }
    }
}

/// Segments of the player's own coasting path.
const OWN_SEGMENTS: usize = 128;

/// The coasting path around home: one thin line, cut where it meets the ground.
pub(super) fn own_path(painter: &egui::Painter, lens: &Lens, system: &System, fix: &Fix, orbit: &crate::planet::orbit::Orbit) {
    use crate::planet::orbit::{MU, Path};
    let Some(home) = system.id("home") else { return };
    let centre = fix.snapshot.position(home);
    let (r, v) = (orbit.pos, orbit.vel);
    let h = r.cross(v);
    let h2 = h.length_squared();
    if h2 < 1.0 {
        return;
    }
    let ecc = v.cross(h) / MU - r.normalize_or_zero();
    let e = ecc.length();
    // A circle has no periapsis direction; start the sweep at the player.
    let x = if e > 1e-6 { ecc / e } else { r.normalize_or_zero() };
    let y = h.normalize().cross(x);
    let p = h2 / MU;
    // An open path is drawn out to where it would leave the sky's scale.
    let reach = if e < 1.0 { std::f64::consts::PI } else { (-1.0 / e).acos() * 0.96 };
    let ground = engine_core::cube_sphere::atlas::RADIUS_M;
    let color = match orbit.elements().path() {
        Path::Orbit => super::JADE,
        _ => super::AMBER,
    };
    let stroke = Stroke::new(1.0, color.gamma_multiply(0.55));
    let bounds = lens.screen.expand(lens.screen.width());
    let point = |i: usize| {
        let angle = -reach + 2.0 * reach * i as f64 / OWN_SEGMENTS as f64;
        let radius = p / (1.0 + e * angle.cos()).max(1e-6);
        (radius >= ground).then(|| lens.camera(fix.view_of(centre + (x * angle.cos() + y * angle.sin()) * radius)))
    };
    let mut last = point(0);
    for i in 1..=OWN_SEGMENTS {
        let next = point(i);
        if let (Some(a), Some(b)) = (last, next)
            && let Some((a, b)) = clip(a, b)
        {
            let (a, b) = (lens.place(a), lens.place(b));
            if bounds.contains(a) || bounds.contains(b) {
                painter.line_segment([a, b], stroke);
            }
        }
        last = next;
    }
}

pub(super) fn draw(painter: &egui::Painter, lens: &Lens, system: &System, fix: &Fix, target: Option<BodyId>, all: bool) {
    let bounds = lens.screen.expand(lens.screen.width());
    for body in system.bodies() {
        // The line of the body underfoot runs through the eye.
        let Some(parent) = system.parent(body).filter(|_| body != fix.reference) else { continue };
        let centre = fix.snapshot.position(parent);
        let stroke = if Some(body) == target {
            Stroke::new(1.0, super::AMBER.gamma_multiply(0.6))
        } else if all {
            Stroke::new(1.0, body_color(system.kind(body)).gamma_multiply(0.16))
        } else {
            continue;
        };
        let mut points = orbit_paths(system)[body.0 as usize].iter().map(|p| lens.camera(fix.view_of(centre + *p)));
        let Some(mut last) = points.next() else { continue };
        for next in points {
            if let Some((a, b)) = clip(last, next) {
                let (a, b) = (lens.place(a), lens.place(b));
                if bounds.contains(a) || bounds.contains(b) {
                    painter.line_segment([a, b], stroke);
                }
            }
            last = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_keeps_the_part_ahead_of_the_eye() {
        let ahead = DVec3::new(0.0, 0.0, 10.0);
        let behind = DVec3::new(4.0, 0.0, -10.0);
        assert!(clip(behind, behind).is_none());
        let (a, b) = clip(ahead, behind).unwrap();
        assert_eq!(a, ahead / behind.length());
        assert!((b.z - NEAR).abs() < 1e-12 && b.x > 0.0);
        let (c, d) = clip(behind, ahead).unwrap();
        assert!((c.z - NEAR).abs() < 1e-12);
        assert_eq!(d, ahead / behind.length());
    }
}
