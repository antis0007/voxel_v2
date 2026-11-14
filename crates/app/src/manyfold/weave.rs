//! Edited voxels control bounded folds. Prediction and host admission use the
//! same inscriptions, transforms and gravity-relative capsule clearance.
use crate::{
    player::{radial, Player},
    world::{
        self,
        interaction::Prompt,
        portals::{gates, Frame, Portal},
        BlockId, World, WorldType,
    },
};
use engine_worldgen::manyfold::weave as layout;
use glam::{DQuat, DVec2, IVec3, Vec3};
use std::f64::consts::{PI, TAU};

mod presentation;
pub use presentation::draw;

pub fn enabled(world: &World) -> bool {
    world.world_type == WorldType::Manyfold
        && world.generator_version >= layout::VERSION
        && world.generator_version < engine_worldgen::manyfold::INFINITE_MAZE
}

pub(super) fn known(world: &World, cell: IVec3) -> Option<BlockId> {
    let chunk = world::chunk_coord_from_world_block(cell);
    // Never reinterpret an unloaded, edited region as pristine terrain.
    if !world.chunks.contains_key(&chunk) && world.regions.view.records.contains_key(&chunk) {
        return None;
    }
    Some(world.get(cell))
}

pub fn armed(world: &World, district: usize) -> Option<bool> {
    known(world, layout::anchor(&world.sample_config(), district)).map(|b| b == BlockId::Crystal)
}

pub fn linked(world: &World, district: usize) -> bool {
    let cfg = world.sample_config();
    let other = layout::partner(district);
    armed(world, district) == Some(false)
        && armed(world, other) == Some(false)
        && [district, other]
            .into_iter()
            .all(|d| known(world, layout::keystone(&cfg, d)) == Some(BlockId::CopperBlock))
}

fn frame(district: usize, z: f32, positive: bool) -> Frame {
    Frame {
        center: layout::point(district, 0.0, layout::FRAME_HEIGHT, z).as_dvec3(),
        rotation: DQuat::from_rotation_z(district as f64 * TAU / 6.0)
            * if positive {
                DQuat::IDENTITY
            } else {
                DQuat::from_rotation_y(PI)
            },
        half_size: DVec2::new(
            layout::APERTURE_WIDTH as f64,
            layout::APERTURE_HEIGHT as f64,
        ),
    }
}

fn portal(world: &World, district: usize, trap: bool) -> Option<Portal> {
    let (entrance, exit) = if trap {
        if armed(world, district) != Some(true) {
            return None;
        }
        let (destination, z, positive) = match district % 3 {
            0 => (district, -2.5, false),
            1 => ((district + 2) % 6, -2.5, false),
            _ => (district, -7.0, true),
        };
        (
            frame(district, layout::TRAP_Z, true),
            frame(destination, z, positive),
        )
    } else {
        if !linked(world, district) {
            return None;
        }
        (
            frame(district, layout::GATE_Z, false),
            frame(layout::partner(district), layout::GATE_Z, false),
        )
    };
    Some(Portal {
        id: 0x8100 + district as u64 * 2 + u64::from(trap),
        source: gates::realm(),
        destination: gates::realm(),
        entrance,
        exit,
    })
}

fn body(world: &World, p: &Player) -> (f32, f32) {
    let scale = world.voxel_scale.meters();
    (
        world::player_height_blocks(world.voxel_scale, p.scale) * scale
            - 0.3 * p.scale * p.crouch_t,
        world::player_width_blocks(world.voxel_scale, p.scale) * scale * 0.5,
    )
}

fn crossing(world: &World, p: &Player, from: Vec3, to: Vec3) -> Option<(Portal, f64)> {
    if !enabled(world) || !world.rules.mode.interacts() {
        return None;
    }
    let scale = world.voxel_scale.meters();
    let from_m = from * scale;
    let (district, arc, _, _) = layout::coordinates(from_m);
    if arc.abs() > 5.0 {
        return None;
    }
    let (height, radius) = body(world, p);
    let up = p.up_dir().as_dvec3();
    let offset = up * f64::from(height * 0.5);
    let start = from_m.as_dvec3() + offset;
    let end = (to * scale).as_dvec3() + offset;
    [true, false]
        .into_iter()
        .filter_map(|trap| {
            let plane = if trap { layout::TRAP_Z } else { layout::GATE_Z };
            let crossed = if trap {
                from_m.z > plane && to.z * scale <= plane
            } else {
                from_m.z < plane && to.z * scale >= plane
            };
            if !crossed {
                return None;
            }
            let portal = portal(world, district, trap)?;
            let local_up = portal.entrance.rotation.conjugate() * up;
            let stem = f64::from((height * 0.5 - radius).max(0.0));
            let margin = DVec2::new(local_up.x.abs(), local_up.y.abs()) * stem
                + DVec2::splat(f64::from(radius));
            portal.intersect(start, end, margin).map(|t| (portal, t))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

fn landing(world: &World, p: &Player, portal: &Portal, entry: Vec3, end: Vec3) -> Option<Vec3> {
    let scale = world.voxel_scale.meters();
    let start = portal.point((entry * scale).as_dvec3()).as_vec3() / scale;
    let exit = portal.point((end * scale).as_dvec3()).as_vec3() / scale;
    let up = portal.vector(p.up_dir().as_dvec3()).as_vec3();
    // Rotating a voxel staircase can leave the lower cap touching the new
    // floor. Admit a bounded local step, never a search through a blocking wall.
    let rise = (0.75 * scale + 0.15) / scale;
    for step in 0..=8 {
        let lift = up * (rise * step as f32 / 8.0);
        if radial::clear_transit(world, p, start + lift, exit + lift, up)
            && (p.flying
                || world::raycast_block(world, exit + lift + up * (0.1 / scale), -up, 5.0 / scale)
                    .is_some())
        {
            return Some(exit + lift);
        }
    }
    None
}

/// Body, aim frame and momentum cross together. Only the residual motion is
/// swept at the exit; the intervening space is not a physical movement segment.
pub fn step(world: &World, p: &mut Player, before: Vec3) {
    let Some((portal, t)) = crossing(world, p, before, p.pos) else {
        return;
    };
    let entry = before.lerp(p.pos, t as f32);
    let Some(exit) = landing(world, p, &portal, entry, p.pos) else {
        p.pos = before;
        p.precise_pos = before.as_dvec3();
        p.vel = Vec3::ZERO;
        if p.manyfold_feedback_cooldown <= 0.0 {
            p.manyfold_crossing = 3;
            p.manyfold_feedback_cooldown = 0.8;
        }
        return;
    };
    let rotation = portal.rotation().as_quat();
    let source_up = p.up_dir();
    p.pos = exit;
    p.precise_pos = p.pos.as_dvec3();
    p.vel = rotation * p.vel;
    p.gravity_frame = (rotation * p.gravity_frame).normalize();
    p.step_camera_offset = 0.0;
    p.manyfold_crossing = if portal.id & 1 == 1 { 1 } else { 2 };
    let scale = world.voxel_scale.meters();
    p.map_crossings.record(
        world.regions.view.root.clone(),
        portal.id,
        world.revision(),
        entry * scale,
        exit * scale,
        source_up,
        p.up_dir(),
        p.forward_dir(),
    );
}

pub fn crosses(world: &World, p: &Player, to: Vec3) -> bool {
    crossing(world, p, p.pos, to).is_some()
}

pub enum WeightTransit {
    None,
    Blocked,
    Crossed(Vec3, Vec3),
}

/// Same authored folds, but a spherical body and one physical owner. Called
/// only by the host's bounded weight integration, never by render prediction.
pub fn weight_transit(world: &World, from: Vec3, to: Vec3, velocity: Vec3) -> WeightTransit {
    if !enabled(world) {
        return WeightTransit::None;
    }
    let (district, arc, _, _) = layout::coordinates(from);
    if arc.abs() > 5.0 {
        return WeightTransit::None;
    }
    let found = [true, false]
        .into_iter()
        .filter_map(|trap| {
            let portal = portal(world, district, trap)?;
            let t = portal.intersect(from.as_dvec3(), to.as_dvec3(), DVec2::splat(0.2))?;
            Some((portal, t))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((portal, t)) = found else {
        return WeightTransit::None;
    };
    let scale = world.voxel_scale.meters();
    let entry = from.lerp(to, t as f32);
    let start = portal.point(entry.as_dvec3()).as_vec3();
    let exit = portal.point(to.as_dvec3()).as_vec3();
    for point in [start, exit] {
        let lo = ((point - Vec3::splat(0.2)) / scale).floor().as_ivec3();
        let hi = ((point + Vec3::splat(0.2)) / scale).floor().as_ivec3();
        for z in lo.z..=hi.z {
            for y in lo.y..=hi.y {
                for x in lo.x..=hi.x {
                    if known(world, IVec3::new(x, y, z)).is_none_or(|b| b.is_solid()) {
                        return WeightTransit::Blocked;
                    }
                }
            }
        }
    }
    WeightTransit::Crossed(exit, portal.vector(velocity.as_dvec3()).as_vec3())
}

/// Unfold a reported jump into its short approach. An arbitrary large position
/// change cannot use this allowance without crossing the currently active line.
pub fn admit_owner(world: &World, p: &Player, to: Vec3, reach_m: f32) -> bool {
    if !enabled(world) {
        return false;
    }
    let scale = world.voxel_scale.meters();
    let (district, _, _, _) = layout::coordinates(p.pos * scale);
    for trap in [true, false] {
        let Some(portal) = portal(world, district, trap) else {
            continue;
        };
        let inverse = portal.rotation().conjugate();
        let unfolded = (portal.entrance.center
            + inverse * ((to * scale).as_dvec3() - portal.exit.center))
            .as_vec3()
            / scale;
        if (unfolded - p.pos).length() * scale > reach_m {
            continue;
        }
        let Some((first, t)) = crossing(world, p, p.pos, unfolded) else {
            continue;
        };
        if first.id != portal.id {
            continue;
        }
        let entry = p.pos.lerp(unfolded, t as f32);
        if radial::admit_move(world, p, entry)
            && landing(world, p, &portal, entry, unfolded)
                .is_some_and(|exit| (exit - to).length() * scale < 0.025)
        {
            return true;
        }
    }
    false
}

fn aimed(world: &World, eye_m: Vec3, forward: Vec3) -> Option<(usize, IVec3)> {
    if !enabled(world) {
        return None;
    }
    let scale = world.voxel_scale.meters();
    let (hit, _) = world::raycast_block(world, eye_m / scale, forward, 4.0 / scale)?;
    let cfg = world.sample_config();
    (0..6)
        .find(|&d| layout::anchor(&cfg, d) == hit)
        .map(|d| (d, hit))
}

pub fn focus(world: &World, eye_m: Vec3, forward: Vec3) -> Option<Prompt> {
    let (district, _) = aimed(world, eye_m, forward)?;
    Some(Prompt {
        title: layout::seal_name(district),
        action: if armed(world, district) == Some(true) {
            "[F] Erase seal"
        } else if linked(world, district) {
            "Paired arch open"
        } else {
            "Erase the opposite seal"
        },
    })
}

pub fn interact(world: &mut World, p: &Player) -> bool {
    let Some((district, cell)) = aimed(
        world,
        p.eye(world.voxel_scale) * world.voxel_scale.meters(),
        p.forward_dir(),
    ) else {
        return false;
    };
    if armed(world, district) != Some(true) {
        return true;
    }
    if let Ok(commit) = world::effects::replace(world, cell, BlockId::Crystal, BlockId::CarvedStone)
    {
        let mut changed = Vec::with_capacity(1);
        commit.publish_voxel(world, &mut changed);
        world.adventure.dirty_voxels.extend(changed);
        if world.adventure.sounds.len() < 64 {
            world.adventure.sounds.push(crate::audio::Event {
                cue: crate::audio::Cue::Ward,
                pos: Some((cell.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters()),
                gain: 0.6,
                pitch: 0.8,
            });
        }
        world.expedition.say(if linked(world, district) {
            "Paired arch opened."
        } else {
            "Seal erased. Its pair is across the hollow."
        });
    }
    true
}

pub fn feedback(world: &mut World, p: &mut Player) {
    let event = std::mem::take(&mut p.manyfold_crossing);
    if event == 0 {
        return;
    }
    if world.adventure.sounds.len() < 64 {
        world.adventure.sounds.push(crate::audio::Event {
            cue: if event == 2 {
                crate::audio::Cue::Rift
            } else {
                crate::audio::Cue::Warning
            },
            pos: None,
            gain: 0.4,
            pitch: if event == 2 { 0.7 } else { 1.2 },
        });
    }
    if event == 3 {
        world.expedition.say("The far landing is blocked.");
    }
}
