//! Player crossings. The eye triggers the crossing so the camera never renders
//! from behind an entrance plane; the whole body then moves with it.
use super::{Transit, gates};
use crate::player::Player;
use crate::world::{World, collides_world, player_aabb};
use glam::{DQuat, DVec2, DVec3, Quat, Vec3};

/// Pose before a movement step, captured by the fixed-step driver.
#[derive(Clone, Copy)]
pub struct Before {
    pos: Vec3,
    precise: DVec3,
    eye_m: DVec3,
}

pub fn before(world: &World, p: &Player) -> Before {
    Before {
        pos: p.pos,
        precise: p.precise_pos,
        eye_m: p.precise_eye(world.voxel_scale) * f64::from(world.voxel_scale.meters()),
    }
}

/// Returns true when the player crossed into another place this step.
pub fn step(world: &mut World, p: &mut Player, before: Before) -> bool {
    // Manyfold's radial controller handles its authored openings separately.
    let network = if world.world_type == crate::world::WorldType::Manyfold {
        std::borrow::Cow::Borrowed(world.portals.network())
    } else {
        super::authored::combined(world.world_type, world.portals.network())
    };
    // Radial gravity frames are not portal-transformable yet; upright only.
    if network.portals.is_empty() || p.gravity_frame != Quat::IDENTITY {
        return false;
    }
    let scale = world.voxel_scale.meters();
    let m = f64::from(scale);
    let eye = p.precise_eye(world.voxel_scale) * m;
    let feet_from_eye = p.precise_pos * m - eye;
    let half = DVec2::new(f64::from(crate::world::player_width_blocks(world.voxel_scale, p.scale) * scale) * 0.5, 0.1);
    let voxel_scale = world.voxel_scale;
    let (body, crouch) = (p.scale, p.crouch_t);
    let transit = network.transit(
        &gates::realm(),
        before.eye_m,
        eye,
        (p.vel * scale).as_dvec3(),
        DQuat::IDENTITY,
        half,
        |_, start, end, _| {
            // Check short swept bounds so the residual movement can't skip
            // a thin obstruction just beyond the exit.
            let steps = ((end - start).abs().max_element() / m * 4.0).ceil();
            if steps > 256.0 {
                return false;
            }
            let steps = (steps as u32).max(1);
            let bounds = |eye: DVec3| {
                let feet = (eye + feet_from_eye) / m;
                player_aabb(feet.as_vec3(), voxel_scale, body, crouch)
            };
            let (mut lo, mut hi) = bounds(start);
            for i in 1..=steps {
                let (next_lo, next_hi) = bounds(start.lerp(end, f64::from(i) / f64::from(steps)));
                if collides_world(world, lo.min(next_lo), hi.max(next_hi)) {
                    return false;
                }
                (lo, hi) = (next_lo, next_hi);
            }
            true
        },
    );
    match transit {
        Transit::Unchanged => false,
        Transit::Blocked => {
            // A body that does not fit, or an obstructed exit, stops at the plane.
            p.pos = before.pos;
            p.precise_pos = before.precise;
            p.vel.x = 0.0;
            p.vel.z = 0.0;
            false
        }
        Transit::Crossed(c) => {
            p.precise_pos = (c.position + feet_from_eye) / m;
            p.pos = p.precise_pos.as_vec3();
            p.vel = (c.velocity / m).as_vec3();
            let (sy, cy) = f64::from(p.yaw).sin_cos();
            let f = c.orientation * DVec3::new(sy, 0.0, -cy);
            p.yaw = f.x.atan2(-f.z) as f32;
            p.step_camera_offset = 0.0;
            p.map_crossings.record(world.regions.view.root.clone(), c.portal_id, world.revision(),
                before.pos * scale, p.pos * scale, Vec3::Y, p.up_dir(), p.forward_dir());
            if world.adventure.sounds.len() < 64 {
                world.adventure.sounds.push(crate::audio::Event {
                    cue: crate::audio::Cue::Rift,
                    pos: None,
                    gain: 0.3,
                    pitch: 1.25,
                });
            }
            true
        }
    }
}
