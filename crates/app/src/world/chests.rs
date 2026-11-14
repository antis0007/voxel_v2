//! Presentation state only. Canonical contents remain in block_data and the scanner.
use super::{World, block_data};
use glam::{IVec3, Vec3};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Chest {
    pub tier: u8,
    pub facing: u8,
    pub lid: f32,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub cells: BTreeMap<block_data::Key, Chest>,
}

pub fn lid_curve(t: f32) -> f32 { 1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3) }

pub fn step(world: &mut World, open: Option<IVec3>, viewer: Vec3, dt: f32) {
    let scale = world.voxel_scale.meters();
    let range = 65.0 / scale;
    let low = ((viewer.x - range).floor() as i32, i32::MIN, i32::MIN);
    let high = ((viewer.x + range).ceil() as i32, i32::MAX, i32::MAX);
    let mut previous = std::mem::take(&mut world.chests.cells);
    for (&key, entry) in world.block_data.entries.range(low..=high) {
        if !matches!(entry, block_data::Entry::Chest(_)) { continue; }
        let p = IVec3::new(key.0, key.1, key.2);
        if p.as_vec3().distance_squared(viewer) > range * range
            || !world.chunks.contains_key(&super::chunk_coord_from_world_block(p)) { continue; }
        let Some(tier) = world.get(p).chest_tier() else { continue; };
        let mut chest = previous.remove(&key).unwrap_or(Chest { tier, facing: 0, lid: 0.0 });
        chest.tier = tier;
        let target = if open == Some(p) { 1.0 } else { 0.0 };
        let amount = dt.clamp(0.0, 0.1) / 0.25;
        chest.lid += (target - chest.lid).clamp(-amount, amount);
        world.chests.cells.insert(key, chest);
    }
}
