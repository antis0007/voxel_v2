//! One portal pair from the court throat into the block maze.
//! Radial steps cross it; the upright portal step does not, because that step
//! refuses a non-identity gravity frame.
use crate::{
    player::{radial, Player},
    world::{
        portals::{gates, Frame, Network, Portal},
        World, WorldType,
    },
};
use glam::{DMat3, DQuat, DVec2, Vec3};

const PAIR: u64 = 0x8200;

pub fn shown(world: &World) -> Network {
    let mut network = world.portals.network().clone();
    network.portals.extend(pair(world));
    if world.world_type != WorldType::Manyfold {
        crate::world::portals::authored::append(world.world_type, &mut network);
    }
    network
}

pub fn step(world: &World, p: &mut Player, before: Vec3) {
    if p.manyfold_crossing != 0 {
        return;
    }
    let Some((portal, t)) = crossing(world, p, before, p.pos) else {
        return;
    };
    let entry = before.lerp(p.pos, t as f32);
    let Some(exit) = landing(world, p, &portal, entry, p.pos) else {
        p.pos = before;
        p.precise_pos = before.as_dvec3();
        p.vel = Vec3::ZERO;
        return;
    };
    let rotation = portal.rotation().as_quat();
    let source_up = p.up_dir();
    p.pos = exit;
    p.precise_pos = p.pos.as_dvec3();
    p.vel = rotation * p.vel;
    p.gravity_frame = (rotation * p.gravity_frame).normalize();
    p.step_camera_offset = 0.0;
    p.manyfold_crossing = 2;
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

pub fn admit(world: &World, p: &Player, to: Vec3, reach_m: f32) -> bool {
    let scale = world.voxel_scale.meters();
    let Some(portals) = active(world) else {
        return false;
    };
    for portal in portals {
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

fn pair(world: &World) -> Vec<Portal> {
    active(world).unwrap_or_default()
}

fn active(world: &World) -> Option<Vec<Portal>> {
    if world.world_type != WorldType::Manyfold {
        return None;
    }
    let mut portals = Vec::new();
    if let Some((gallery, inside)) = engine_worldgen::manyfold::maze::mouths(world.generator_version) {
        let realm = gates::realm();
        let forward = Portal {
            id: PAIR + 1,
            source: realm.clone(),
            destination: realm.clone(),
            entrance: frame(&gallery, false),
            exit: frame(&inside, true),
        };
        portals.extend([forward.clone(), forward.reverse(PAIR + 2)]);
    }
    let authored = crate::world::portals::authored::portals(world.world_type);
    if world.portals.network().portals.len() + portals.len() + authored.len() <= crate::world::portals::MAX_PORTALS {
        portals.extend_from_slice(authored);
    }
    Some(portals)
}

fn frame(mouth: &engine_worldgen::manyfold::maze::Mouth, face_into_maze: bool) -> Frame {
    let z = if face_into_maze {
        mouth.into_maze
    } else {
        -mouth.into_maze
    };
    let y = mouth.up.normalize_or_zero();
    let z = z.normalize_or_zero();
    let x = y.cross(z).normalize_or_zero();
    Frame {
        center: mouth.center.as_dvec3(),
        rotation: DQuat::from_mat3(&DMat3::from_cols(x.as_dvec3(), y.as_dvec3(), z.as_dvec3())),
        half_size: DVec2::new(1.6, 1.35),
    }
}

fn crossing(world: &World, p: &Player, from: Vec3, to: Vec3) -> Option<(Portal, f64)> {
    let portals = active(world)?;
    let scale = world.voxel_scale.meters();
    let (height, radius) = body(world, p);
    let up = p.up_dir();
    let offset = up * (height * 0.5 / scale);
    let start = ((from + offset) * scale).as_dvec3();
    let end = ((to + offset) * scale).as_dvec3();
    let margin = DVec2::new(f64::from(radius), 0.15);
    portals
        .into_iter()
        .filter_map(|portal| portal.intersect(start, end, margin).map(|t| (portal, t)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

fn landing(world: &World, p: &Player, portal: &Portal, entry: Vec3, end: Vec3) -> Option<Vec3> {
    let scale = world.voxel_scale.meters();
    let start = portal.point((entry * scale).as_dvec3()).as_vec3() / scale;
    let exit = portal.point((end * scale).as_dvec3()).as_vec3() / scale;
    let up = portal.vector(p.up_dir().as_dvec3()).as_vec3();
    for step in 0..=8 {
        let lift = up * (0.4 * step as f32 / (8.0 * scale));
        if radial::clear_transit(world, p, start + lift, exit + lift, up)
            && (p.flying
                || crate::world::raycast_block(
                    world,
                    exit + lift + up * (0.1 / scale),
                    -up,
                    5.0 / scale,
                )
                .is_some())
        {
            return Some(exit + lift);
        }
    }
    None
}

fn body(world: &World, p: &Player) -> (f32, f32) {
    let scale = world.voxel_scale.meters();
    (
        crate::world::player_height_blocks(world.voxel_scale, p.scale) * scale
            - 0.3 * p.scale * p.crouch_t,
        crate::world::player_width_blocks(world.voxel_scale, p.scale) * scale * 0.5,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::VoxelScalePreset;
    use engine_voxel::{WorldSampleConfig, WorldType};

    #[test]
    fn walking_through_the_gallery_mouth_lands_in_the_maze() {
        let cfg = WorldSampleConfig {
            planet: None,
            generator_version: engine_worldgen::manyfold::maze::VERSION,
            seed: 7,
            world_type: WorldType::Manyfold,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let (gallery, inside) =
            engine_worldgen::manyfold::maze::mouths(cfg.generator_version).unwrap();
        let entrance = frame(&gallery, false);
        let exit = frame(&inside, true);
        let portal = Portal {
            id: 1,
            source: gates::realm(),
            destination: gates::realm(),
            entrance,
            exit,
        };
        let through = gallery.center + gallery.into_maze * 0.5;
        let landed = portal.point(through.as_dvec3()).as_vec3();
        let expected = inside.center + inside.into_maze * 0.5;
        assert!(
            (landed - expected).length() < 0.05,
            "{landed} vs {expected}"
        );
        assert!(portal.validate().is_ok());
    }
}
