//! Actual water contact feeds the existing elemental encounter rules.
use super::{Enemy, World};
use crate::world::chunk_coord_from_world_block;
use glam::Vec3;

pub(super) fn update(enemy: &mut Enemy, world: &World) {
    if !enemy.pos.is_finite() {
        return;
    }
    let scale = world.voxel_scale.meters();
    // Small central foot contacts fit every existing creature body. Consult
    // the partial water height, not merely the cell's Water material tag.
    let contacts = [
        Vec3::ZERO,
        Vec3::X * 0.2,
        Vec3::NEG_X * 0.2,
        Vec3::Z * 0.2,
        Vec3::NEG_Z * 0.2,
    ];
    let wet = contacts.into_iter().any(|offset| {
        let sample = (enemy.pos + offset + Vec3::Y * 0.05) / scale;
        let cell = sample.floor().as_ivec3();
        // Reading contact must not generate or load distant chunks.
        world
            .chunks
            .contains_key(&chunk_coord_from_world_block(cell))
            && world.water_fill(cell) > sample.y - cell.y as f32
    });
    if wet {
        enemy.status.water_contact(enemy.affixes);
    }
}
