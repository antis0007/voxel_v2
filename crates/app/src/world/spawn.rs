//! Spawn records use metres. Placement checks use voxel coordinates.
use super::{BlockId, World, chunk_coord_from_world_block, collides_world, player_aabb};
use crate::player::Player;
use glam::{IVec3, Vec3};

#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Binding {
    pub bed: Option<IVec3>,
    pub arrival: Option<Vec3>,
}

#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct RealmSpawn {
    pub point: Option<Vec3>,
    pub binding: Binding,
}

impl RealmSpawn {
    pub fn on_arrival(&mut self, world: &World, arrival_m: Vec3, replace_point: bool) {
        if bed_binds_spawn(world) {
            self.binding.arrival = Some(arrival_m);
            if self.binding.bed.is_some() { return; }
        }
        if replace_point || self.point.is_none() {
            self.point = Some(arrival_m);
        }
    }
}

pub fn bed_binds_spawn(world: &World) -> bool {
    world.sample_config().is_hell()
}

pub fn bind_bed(world: &mut World, player: &Player, bed: IVec3) -> bool {
    let Some(feet) = bed_feet(world, player, bed) else { return false; };
    let mut spawn = world.adventure.realm_spawn();
    spawn.binding.bed = Some(bed);
    // Retain the position for existing UI, save readers and coordinate tools.
    spawn.point = Some(feet * world.voxel_scale.meters());
    world.adventure.set_realm_spawn(spawn);
    true
}

pub fn record_arrival(world: &mut World, arrival_m: Vec3, replace_point: bool) {
    let mut spawn = world.adventure.realm_spawn();
    spawn.on_arrival(world, arrival_m, replace_point);
    world.adventure.set_realm_spawn(spawn);
}

pub(super) struct Resolved {
    pub feet: Vec3,
    pub bed_unavailable: bool,
}

pub(super) fn resolve(world: &mut World, player: &Player) -> Option<Resolved> {
    let scale = world.voxel_scale.meters();
    let spawn = world.adventure.realm_spawn();
    if !bed_binds_spawn(world) {
        let mut feet = spawn.point.map_or_else(|| world.suggested_spawn(), |point| point / scale);
        for _ in 0..4 {
            let (lo, hi) = player_aabb(feet, world.voxel_scale, player.scale, 0.0);
            if !collides_world(world, lo, hi) { break; }
            feet.y = feet.y.floor() + 1.0;
        }
        return Some(Resolved { feet, bed_unavailable: false });
    }
    if let Some(feet) = spawn.binding.bed.and_then(|bed| bed_feet(world, player, bed)) {
        return Some(Resolved { feet, bed_unavailable: false });
    }
    let bed_unavailable = spawn.binding.bed.is_some();
    let preferred = if bed_unavailable { spawn.binding.arrival } else { spawn.point.or(spawn.binding.arrival) };
    let feet = preferred.filter(|point| point.is_finite())
        .and_then(|point| respawn_near(world, player, point / scale))
        .or_else(|| {
            let center = world.suggested_spawn();
            respawn_near(world, player, center)
        })?;
    Some(Resolved { feet, bed_unavailable })
}

fn bed_feet(world: &mut World, player: &Player, bed: IVec3) -> Option<Vec3> {
    let feet = bed.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
    standing_bounds(world, player, feet, 0.0, bed, Some(BlockId::Bed), &[])?;
    Some(feet)
}

type Bounds = (Vec3, Vec3);

fn standing_bounds(
    world: &mut World, player: &Player, feet: Vec3, crouch: f32,
    support: IVec3, required_support: Option<BlockId>, occupied: &[Bounds],
) -> Option<Bounds> {
    if !feet.is_finite() { return None; }
    let bounds @ (lo, hi) = player_aabb(feet, world.voxel_scale, player.scale, crouch);
    if occupied.iter().any(|&(a, b)| lo.cmplt(b).all() && hi.cmpgt(a).all()) { return None; }
    let min = lo.floor().as_ivec3();
    let max = (hi - Vec3::splat(0.001)).floor().as_ivec3();
    if (max - min).max_element() > 64 { return None; }
    let first = chunk_coord_from_world_block(min.min(support));
    let last = chunk_coord_from_world_block(max.max(support));
    for z in first.z..=last.z {
        for y in first.y..=last.y {
            for x in first.x..=last.x {
                if !world.admit_edit_chunk(IVec3::new(x, y, z)) { return None; }
            }
        }
    }
    let floor = world.get(support);
    if required_support.map_or_else(|| !floor.is_solid(), |block| floor != block)
        || collides_world(world, lo, hi) { return None; }
    for z in min.z..=max.z {
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                if world.get(IVec3::new(x, y, z)).is_liquid() { return None; }
            }
        }
    }
    Some(bounds)
}

fn respawn_near(world: &mut World, player: &Player, center: Vec3) -> Option<Vec3> {
    let scale = world.voxel_scale.meters();
    for ring in [0.0, 1.0, 2.0, 4.0] {
        for step in 0..if ring == 0.0 { 1 } else { 8 } {
            let (sin, cos) = (step as f32 * std::f32::consts::FRAC_PI_4).sin_cos();
            for lift in [0.0, 1.0, -1.0, 2.0] {
                let feet = center + Vec3::new(cos * ring / scale, lift, sin * ring / scale);
                let floor = (feet - Vec3::Y * 0.05).floor().as_ivec3();
                if standing_bounds(world, player, feet, 0.0, floor, None, &[]).is_some() {
                    return Some(feet);
                }
            }
        }
    }
    None
}

#[cfg(feature = "multiplayer")]
pub(crate) fn portal_landing(world: &mut World, center: Vec3, player: &Player, occupied: &[Bounds]) -> Option<(Vec3, Bounds)> {
    let scale = world.voxel_scale.meters();
    for ring in [0.0, 1.0, 1.8, 2.6, 3.4, 4.2] {
        for step in 0..if ring == 0.0 { 1 } else { 16 } {
            let (sin, cos) = (step as f32 * std::f32::consts::TAU / 16.0).sin_cos();
            for lift in [0.0, 1.0, -1.0] {
                let feet = center + Vec3::new(cos * ring / scale, lift, sin * ring / scale);
                let floor = (feet - Vec3::Y * (0.1 / scale)).floor().as_ivec3();
                if let Some(bounds) = standing_bounds(world, player, feet, player.crouch_t, floor, None, occupied) {
                    return Some((feet, bounds));
                }
            }
        }
    }
    None
}
