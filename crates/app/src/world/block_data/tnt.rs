//! Lit TNT: fixed-step fuses and budgeted blast clearing.
use super::{State, key};
use crate::world::{BlockId, World, effects, inventory::{DroppedStack, ItemStack}};
use engine_core::Budget;
use glam::{IVec3, Vec3};
use std::collections::HashSet;

/// Four seconds at the fixed 120 Hz step.
pub const FUSE_STEPS: u16 = 480;
pub const MAX_FUSES: usize = 256;
/// Blast radius in metres; the voxel radius is capped so fine scales stay bounded.
pub const RADIUS_M: f32 = 2.5;
const MAX_RADIUS_VOXELS: f32 = 6.0;
const MAX_QUEUED: usize = 16_384;
/// One removed cell in this many drops its item.
const DROP_EVERY: u32 = 4;

fn cell_hash(p: IVec3, salt: u32) -> u32 {
    engine_core::hash::hash_u32(
        (p.x as u32).wrapping_mul(0x9e37_79b9)
            ^ (p.y as u32).wrapping_mul(0x85eb_ca6b)
            ^ (p.z as u32).wrapping_mul(0xc2b2_ae35)
            ^ salt,
    )
}

/// Lights a charge at `p` unless one already burns there.
pub fn arm(state: &mut State, p: IVec3, steps: u16) {
    if state.fuses.len() < MAX_FUSES {
        state.fuses.entry(key(p)).or_insert(steps.max(1));
    }
}

/// Lights the TNT block at `p`, as a torch or flint does.
pub fn ignite(world: &mut World, p: IVec3) -> bool {
    if world.get(p) != BlockId::Tnt {
        return false;
    }
    arm(&mut world.block_data, p, FUSE_STEPS);
    true
}

/// Cells a blast at `centre` clears, nearest first, in a fixed order.
pub fn blast_cells(centre: IVec3, metres: f32) -> Vec<IVec3> {
    let r = (RADIUS_M / metres.max(1e-3)).clamp(1.0, MAX_RADIUS_VOXELS);
    let n = r.ceil() as i32;
    let mut cells = Vec::new();
    for y in -n..=n {
        for z in -n..=n {
            for x in -n..=n {
                let d = IVec3::new(x, y, z);
                if d.as_vec3().length() <= r {
                    cells.push(centre + d);
                }
            }
        }
    }
    cells.sort_by_key(|d| ((*d - centre).length_squared(), d.y, d.z, d.x));
    cells
}

/// Advances fuses one fixed step, then clears queued blast cells while `budget` lasts.
pub fn tick(
    world: &mut World,
    eye_m: Vec3,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
    budget: Budget,
) {
    let mut fired = Vec::new();
    for (&k, steps) in world.block_data.fuses.iter_mut() {
        *steps = steps.saturating_sub(1);
        if *steps == 0 {
            fired.push(IVec3::new(k.0, k.1, k.2));
        }
    }
    let metres = world.voxel_scale.meters();
    for centre in fired {
        world.block_data.fuses.remove(&key(centre));
        let centre_m = (centre.as_vec3() + Vec3::splat(0.5)) * metres;
        world.adventure.blast_damage(centre_m, RADIUS_M, eye_m);
        let room = MAX_QUEUED.saturating_sub(world.block_data.blasts.len());
        let cells = blast_cells(centre, metres);
        world.block_data.blasts.extend(cells.into_iter().take(room).map(|c| (c, centre)));
    }
    let mut done = 0u32;
    while let Some(&(cell, centre)) = world.block_data.blasts.front() {
        if done.is_multiple_of(32) && done > 0 && !budget.has_time() {
            break;
        }
        world.block_data.blasts.pop_front();
        done += 1;
        clear(world, cell, centre, blocks, chunks);
    }
}

fn clear(world: &mut World, cell: IVec3, centre: IVec3, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) {
    let block = world.get(cell);
    if block == BlockId::Air || block.is_liquid() {
        return;
    }
    if block == BlockId::Tnt && cell != centre {
        // Chained charges burn a short, positional fuse.
        arm(&mut world.block_data, cell, 30 + (cell_hash(cell, 7) % 60) as u16);
        return;
    }
    let Ok(commit) = effects::replace(world, cell, block, BlockId::Air) else { return };
    commit.publish(world, blocks, chunks);
    if block != BlockId::Tnt
        && cell_hash(cell, 3).is_multiple_of(DROP_EVERY)
        && world.inventory.dropped.len() < 1024
        && let Some(item) = super::blast_drop(block)
    {
        let pos = (cell.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters();
        world.inventory.dropped.push(DroppedStack { pos, stack: ItemStack::new(item, 1) });
    }
}
