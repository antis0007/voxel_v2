//! Block light for crops and other systems that read the chunk nibble.
//! A torch writes 14 on its own cell and one less on each step through air.
//! Opaque voxels stop the spread. Editing a torch or a wall clears the old
//! reach and fills it again from the lamps that remain.
use super::{BlockId, CHUNK_SIZE, ChunkData, World, chunk_coord_from_world_block};
use glam::IVec3;
use std::sync::Arc;

pub(super) const REACH: i32 = 14;

pub fn affects(old: BlockId, new: BlockId) -> bool {
    old.is_torch() != new.is_torch()
        || old.is_light_source() != new.is_light_source()
        || old.is_opaque() != new.is_opaque()
}

pub fn propagate_light(world: &mut World, start_pos: IVec3, light_level: u8) {
    let mut boundary = std::collections::VecDeque::from([(start_pos, light_level)]);
    let mut local = Vec::new();
    while let Some((pos, level)) = boundary.pop_front() {
        let coord = chunk_coord_from_world_block(pos);
        let Some(mut resident) = world.chunks.get_mut(&coord) else {
            continue;
        };
        let index = ChunkData::local_index(pos.x, pos.y, pos.z);
        if resident.blocks[index].is_opaque() || level <= (resident.light[index] & 0x0F) {
            continue;
        }
        let chunk = Arc::make_mut(&mut resident);
        chunk.light[index] = (chunk.light[index] & 0xF0) | level;
        local.clear();
        local.push((index, level));
        let mut head = 0;
        // Hold one resident mutation guard while the flood remains in this chunk.
        while head < local.len() {
            let (index, level) = local[head];
            head += 1;
            if level <= 1 {
                continue;
            }
            let size = CHUNK_SIZE as usize;
            let x = index % size;
            let z = (index / size) % size;
            let y = index / (size * size);
            let next = level - 1;
            for (inside, offset, direction) in [
                (x + 1 < size, 1isize, IVec3::X),
                (x > 0, -1, -IVec3::X),
                (y + 1 < size, (size * size) as isize, IVec3::Y),
                (y > 0, -((size * size) as isize), -IVec3::Y),
                (z + 1 < size, size as isize, IVec3::Z),
                (z > 0, -(size as isize), -IVec3::Z),
            ] {
                if !inside {
                    let p = coord * CHUNK_SIZE + IVec3::new(x as i32, y as i32, z as i32);
                    boundary.push_back((p + direction, next));
                    continue;
                }
                let neighbor = (index as isize + offset) as usize;
                if !chunk.blocks[neighbor].is_opaque() && next > (chunk.light[neighbor] & 0x0F) {
                    chunk.light[neighbor] = (chunk.light[neighbor] & 0xF0) | next;
                    local.push((neighbor, next));
                }
            }
        }
        drop(resident);
        world.regions.mark_dirty(coord);
    }
}

/// Clear the reach around `lo..=hi`, then seed every lamp that can still touch it.
pub fn relight_bounds(world: &mut World, lo: IVec3, hi: IVec3) {
    let reach = IVec3::splat(REACH);
    zero_block_light(world, lo - reach, hi + reach);
    let scan_lo = lo - reach * 2;
    let scan_hi = hi + reach * 2;
    let mut sources = Vec::new();
    let c0 = chunk_coord_from_world_block(scan_lo);
    let c1 = chunk_coord_from_world_block(scan_hi);
    for y in c0.y..=c1.y {
        for z in c0.z..=c1.z {
            for x in c0.x..=c1.x {
                let Some(chunk) = world.chunks.get(&IVec3::new(x, y, z)) else {
                    continue;
                };
                for &cell in &chunk.torches {
                    if cell.cmpge(scan_lo).all() && cell.cmple(scan_hi).all() {
                        sources.push(cell);
                    }
                }
            }
        }
    }
    for cell in sources {
        if world.get(cell).is_light_source() {
            propagate_light(world, cell, REACH as u8);
        }
    }
}

fn zero_block_light(world: &mut World, lo: IVec3, hi: IVec3) {
    let c0 = chunk_coord_from_world_block(lo);
    let c1 = chunk_coord_from_world_block(hi);
    for y in c0.y..=c1.y {
        for z in c0.z..=c1.z {
            for x in c0.x..=c1.x {
                let coord = IVec3::new(x, y, z);
                let cleared = {
                    let Some(mut chunk) = world.chunks.get_mut(&coord) else {
                        continue;
                    };
                    let chunk = Arc::make_mut(&mut chunk);
                    let base = coord * CHUNK_SIZE;
                    let x0 = lo.x.max(base.x);
                    let y0 = lo.y.max(base.y);
                    let z0 = lo.z.max(base.z);
                    let x1 = hi.x.min(base.x + CHUNK_SIZE - 1);
                    let y1 = hi.y.min(base.y + CHUNK_SIZE - 1);
                    let z1 = hi.z.min(base.z + CHUNK_SIZE - 1);
                    if x0 > x1 || y0 > y1 || z0 > z1 {
                        false
                    } else {
                        let mut cleared = false;
                        for cy in y0..=y1 {
                            for cz in z0..=z1 {
                                for cx in x0..=x1 {
                                    let idx = ChunkData::local_index(cx, cy, cz);
                                    if chunk.light[idx] & 0x0F != 0 {
                                        chunk.light[idx] &= 0xF0;
                                        cleared = true;
                                    }
                                }
                            }
                        }
                        cleared
                    }
                };
                if cleared {
                    world.regions.mark_dirty(coord);
                }
            }
        }
    }
}

pub fn level_at(world: &World, p: IVec3) -> u8 {
    world
        .chunks
        .get(&chunk_coord_from_world_block(p))
        .map_or(0, |c| c.light[ChunkData::local_index(p.x, p.y, p.z)] & 0x0F)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::WorldType;

    #[test]
    fn a_wall_stops_torch_light_and_removing_the_torch_clears_it() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 1);
        let y = world.terrain_height_at(0, 0) + 2;
        let torch = IVec3::new(0, y, 0);
        let near = IVec3::new(1, y, 0);
        let far = IVec3::new(3, y, 0);
        let wall = IVec3::new(2, y, 0);
        world.set(torch, BlockId::Torch);
        assert!(level_at(&world, torch) >= 14);
        assert!(level_at(&world, near) >= 12, "open air beside the torch");
        assert!(level_at(&world, far) >= 10, "open air two cells past the wall site");
        // One stone does not black out the far cell: the lamp steps around it.
        // A closed shell leaves the room dark once the last opening is filled.
        let mut shell = Vec::new();
        for dy in -1..=1 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let cell = far + IVec3::new(dx, dy, dz);
                    if cell != far && cell != wall {
                        shell.push((cell, BlockId::Stone));
                    }
                }
            }
        }
        world.set_many(&shell);
        assert!(level_at(&world, far) >= 8, "the opening still admits the lamp");
        world.set(wall, BlockId::Stone);
        assert!(level_at(&world, near) >= 12, "the near side stays lit");
        assert_eq!(level_at(&world, far), 0, "the sealed room is dark");
        world.set(wall, BlockId::Air);
        assert!(level_at(&world, far) >= 8, "opening the wall lights the room again");
        world.set(torch, BlockId::Air);
        assert_eq!(level_at(&world, near), 0, "removing the lamp clears the cell");
        assert_eq!(level_at(&world, far), 0, "removing the lamp clears the room");
    }
}

#[cfg(test)]
#[path = "block_light/chunk_tests.rs"]
mod chunk_tests;
