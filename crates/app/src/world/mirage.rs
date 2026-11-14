//! Dispel passable appearances through normal saved edits. Solid terrain is never changed.
use super::{BlockId, World, chunk_coord_from_world_block};
use glam::IVec3;
use std::collections::{HashSet, VecDeque};

pub(crate) fn reveal(world: &mut World, origin: IVec3) -> Vec<IVec3> {
    let appearance = world.get(origin);
    if !appearance.is_mirage() || appearance.is_solid() { return Vec::new(); }
    let radius = (1.5 / world.voxel_scale.meters()).ceil() as i32;
    let mut edits = Vec::new();
    let mut seen = HashSet::from([origin]);
    let mut queue = VecDeque::from([origin]);
    while let Some(cell) = queue.pop_front() {
        if !world.chunks.contains_key(&chunk_coord_from_world_block(cell)) || world.get(cell) != appearance {
            continue;
        }
        edits.push((cell, BlockId::Air));
        if edits.len() >= 96 { break; }
        for step in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
            let next = cell + step;
            if (next - origin).abs().max_element() <= radius && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    let mut changed = Vec::new();
    world.set_many_committed(&edits, |cell, _, _| changed.push(cell));
    changed
}
