//! Globe orientation and the spawn marker drawn over the shaded sphere.
use egui::{Color32, Painter, Pos2, Stroke, vec2};
use glam::{Mat3, Quat, Vec3};

use super::globe_gpu::Pose;

/// Sun from the upper left, slightly in front: a lit crescent with a terminator.
const SUN: Vec3 = Vec3::new(-0.62, 0.40, 0.67);

/// Keep the same camera through the transition from history to the spawn view.
pub(super) fn follow(ctx: &egui::Context, world: u64, mut target: Pose, now: f64, still: bool) -> Pose {
    let id = egui::Id::new("world_loader_camera");
    let rotation = Quat::from_mat3(&target.rotation);
    let shown = ctx.data(|d| d.get_temp::<(u64, f64, Quat)>(id));
    let rotation = if still { rotation } else {
        shown.filter(|(key, _, _)| *key == world).map_or(rotation, |(_, at, previous)| {
            let dt = (now - at).clamp(0.0, 0.1) as f32;
            // A newly discovered spawn must not whip the globe around while
            // the historical surface is dissolving into the finished terrain.
            let angle = previous.angle_between(rotation).abs();
            let blend = (1.0 - (-1.4 * dt).exp()).min(0.18 * dt / angle.max(0.0001));
            previous.slerp(rotation, blend).normalize()
        })
    };
    ctx.data_mut(|d| d.insert_temp(id, (world, now, rotation)));
    target.rotation = Mat3::from_quat(rotation);
    target
}

/// Viewer's drag offset on top of the automatic pose.
#[derive(Clone, Copy, Default)]
struct Orbit {
    world: u64,
    /// Spin about the planet's axis and extra tilt toward the viewer, radians.
    yaw: f32,
    tilt: f32,
    /// Release velocity in radians per second, decaying after a fling.
    spin: [f32; 2],
    dragging: bool,
    /// Easing back to the automatic pose after a double click.
    returning: bool,
    at: f64,
}

/// Left-drag on the globe turns it; a fling coasts, a double click returns
/// to the automatic view. Returns the pose with the offset applied and
/// whether the globe is moving under the viewer's hand (repaint promptly).
pub(super) fn orbit(ctx: &egui::Context, key: &str, world: u64, centre: Pos2, radius: f32, now: f64, still: bool, mut pose: Pose) -> (Pose, bool) {
    let id = egui::Id::new(("globe_orbit", key));
    let mut o = ctx.data(|d| d.get_temp::<Orbit>(id)).filter(|o| o.world == world)
        .unwrap_or(Orbit { world, at: now, ..Default::default() });
    let dt = (now - o.at).clamp(0.0, 0.1) as f32;
    o.at = now;
    let (pos, pressed, down, delta, double) = ctx.input(|i| (
        i.pointer.interact_pos(),
        i.pointer.primary_pressed(),
        i.pointer.primary_down(),
        i.pointer.delta(),
        i.pointer.button_double_clicked(egui::PointerButton::Primary),
    ));
    let over = pos.is_some_and(|p| p.distance(centre) <= radius * 1.05);
    if pressed && over {
        o.dragging = true;
        o.returning = false;
        o.spin = [0.0; 2];
    }
    if !down {
        o.dragging = false;
    }
    if double && over {
        o.returning = true;
        o.spin = [0.0; 2];
    }
    if o.dragging {
        // Grab the surface: a point under the cursor at the disc centre
        // follows it, so one radius of travel turns the globe one radian.
        let step = [delta.x / radius.max(1.0), delta.y / radius.max(1.0)];
        o.yaw += step[0];
        o.tilt += step[1];
        if dt > 0.0 {
            let k = (dt * 25.0).min(1.0);
            o.spin = [0, 1].map(|a| o.spin[a] + (step[a] / dt - o.spin[a]) * k);
        }
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if o.returning {
        o.yaw = o.yaw.rem_euclid(std::f32::consts::TAU);
        if o.yaw > std::f32::consts::PI {
            o.yaw -= std::f32::consts::TAU;
        }
        let k = 1.0 - (-6.0 * dt).exp();
        o.yaw -= o.yaw * k;
        o.tilt -= o.tilt * k;
        if o.yaw.abs() < 1e-3 && o.tilt.abs() < 1e-3 {
            o = Orbit { world, at: now, ..Default::default() };
        }
    } else {
        if still {
            o.spin = [0.0; 2];
        }
        o.yaw += o.spin[0] * dt;
        o.tilt += o.spin[1] * dt;
        let decay = (-3.5 * dt).exp();
        o.spin = o.spin.map(|s| if s.abs() * decay < 0.01 { 0.0 } else { s * decay });
        if over {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
    // The automatic pose is Rx(tilt) * Ry(yaw), so the offset composes as
    // Rx(user tilt) on the left and Ry(user yaw) on the right; keep the sum
    // of tilts short of the poles so the axis never flips over.
    let axis = pose.rotation.col(1);
    let base = axis.z.atan2(axis.y);
    o.tilt = o.tilt.clamp(-1.45 - base, 1.45 - base);
    pose.rotation = Mat3::from_rotation_x(o.tilt) * pose.rotation * Mat3::from_rotation_y(o.yaw);
    let moving = o.dragging || o.returning || o.spin != [0.0; 2];
    ctx.data_mut(|d| d.insert_temp(id, o));
    (pose, moving)
}

/// Rotation that brings `aim` to the front of the globe, then turns it with time.
/// A whole planet spins; a single solved face rocks so it stays in view.
/// `fixed` replaces the motion with a capture pose `(yaw offset, tilt)`.
pub fn pose(aim: Option<Vec3>, whole: bool, seconds: f32, still: bool, clouds: f32, fixed: Option<[f32; 2]>) -> Pose {
    let aim = aim.unwrap_or(Vec3::X).normalize_or(Vec3::X);
    let base = (-aim.x).atan2(aim.z);
    let lat = aim.y.clamp(-1.0, 1.0).asin();
    let t = if still { 0.0 } else { seconds };
    let (yaw, tilt) = match fixed {
        Some([yaw, tilt]) => (base + yaw, tilt),
        None if whole => (base + t * 0.06, 0.32 + lat * 0.5),
        None => (base + 0.45 * (t * 0.07).sin(), 0.30 + lat * 0.6),
    };
    Pose {
        rotation: Mat3::from_rotation_x(tilt) * Mat3::from_rotation_y(yaw),
        sun: SUN.normalize(),
        clouds,
        cloud_angle: t * 0.012,
        detail: 1.0,
    }
}

/// Ring on the spawn point while it faces the viewer.
pub fn marker(p: &Painter, centre: Pos2, radius: f32, pose: &Pose, spawn: Vec3, colour: Color32) {
    let at = pose.rotation * spawn.normalize_or_zero();
    if at.z <= 0.08 {
        return;
    }
    // Fades in from the limb so it never pops at the edge.
    let fade = ((at.z - 0.08) / 0.25).clamp(0.0, 1.0);
    let pos = centre + vec2(at.x, -at.y) * radius;
    p.circle_stroke(pos, 5.0, Stroke::new(1.4, colour.gamma_multiply(fade)));
    p.circle_filled(pos, 1.8, colour.gamma_multiply(fade));
}
