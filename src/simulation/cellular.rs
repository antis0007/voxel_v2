//! Synchronous conservative commits over asynchronously prepared source chunks.
use super::residency::Residency;
use crate::world::{
    BlockId as B, World, chunk_coord_from_world_block, mark_chunk_and_neighbors_dirty,
};
use glam::{IVec3, Vec3};
use std::collections::HashSet;
const HORIZONTAL: [IVec3; 4] = [IVec3::X, IVec3::Z, IVec3::NEG_X, IVec3::NEG_Z];
#[cfg(test)]
#[path = "water_tests.rs"]
mod water_tests;
pub(super) fn allowed(world: &World, p: IVec3) -> bool {
    let cfg = world.sample_config();
    p.y >= cfg.min_world_y()
        && p.y < cfg.max_world_y()
        && !world
            .factory
            .nodes
            .contains_key(&crate::world::factory::anchor(
                p,
                crate::world::factory::step(world),
            ))
}
pub(super) fn changed(p: IVec3, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) {
    blocks.insert(p);
    mark_chunk_and_neighbors_dirty(chunks, p);
}
pub(super) fn admission(world: &World, a: IVec3, b: IVec3) -> bool {
    let mut coords = [IVec3::ZERO; 22];
    let mut index = 0;
    for p in [a, b] {
        coords[index] = chunk_coord_from_world_block(p);
        index += 1;
        for d in crate::world::activity::WAKE_NEIGHBORS {
            coords[index] = chunk_coord_from_world_block(p + d);
            index += 1;
        }
    }
    allowed(world, a) && allowed(world, b) && world.activity.can_wake(&coords)
}
fn transfer(
    sources: &mut Residency,
    world: &mut World,
    from: IVec3,
    to: IVec3,
    max: u8,
    visited: &mut HashSet<IVec3>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> usize {
    if !admission(world, from, to) {
        world.activity.wake(from);
        return 0;
    }
    let Some((B::Water, source)) = sources.read(world, from) else {
        world.activity.wake(from);
        return 0;
    };
    let Some((target_block, target)) = sources.read(world, to) else {
        world.activity.wake(from);
        return 0;
    };
    if !matches!(target_block, B::Air | B::Water) {
        return 0;
    }
    let moved = source.min(8 - target).min(max);
    if moved == 0 {
        return 0;
    }
    if !sources.prepare(world, &[from, to]) {
        world.activity.wake(from);
        return 0;
    }
    // Bounds, assembly guards, residency and both amounts have been checked
    // before either setter. No asynchronous completion can interleave here.
    world.set_water_volume(from, source - moved);
    world.set_water_volume(to, target + moved);
    // A recipient can receive more mass but cannot transport it again this
    // tick. Otherwise queue order allows water to traverse an entire chunk.
    visited.insert(to);
    changed(from, blocks, chunks);
    changed(to, blocks, chunks);
    2
}

// A bounded hydraulic connection search lets a submerged outlet push water
// upward below the source column's free surface. No mass is created, no solid
// is crossed, and unavailable source data postpones the rule.
fn pressure_outlet(sources: &mut Residency, world: &mut World, p: IVec3) -> Option<IVec3> {
    let owner = chunk_coord_from_world_block(p);
    let mut head = p.y * 8;
    for rise in 1..=8 {
        let q = p + IVec3::Y * rise;
        if !sources.request_for(world, chunk_coord_from_world_block(q), owner) {
            world.activity.wake(p); return None;
        }
        match sources.read(world, q) {
            Some((B::Water, amount)) => {
                head = q.y * 8 + amount as i32;
                if amount < 8 { break; }
            }
            _ => break,
        }
    }
    if head <= (p.y + 1) * 8 { return None; }
    let mut queue = [IVec3::ZERO; 48];
    queue[0] = p;
    let (mut cursor, mut len) = (0, 1);
    while cursor < len {
        let from = queue[cursor]; cursor += 1;
        for d in [IVec3::NEG_Y, IVec3::X, IVec3::Z, IVec3::NEG_X, IVec3::NEG_Z, IVec3::Y] {
            let q = from + d;
            if q.y * 8 >= head || (q-p).abs().max_element() > 8
                || queue[..len].contains(&q) || !allowed(world,q) { continue; }
            if !sources.request_for(world,chunk_coord_from_world_block(q),owner) {
                world.activity.wake(p); return None;
            }
            let Some((id,amount)) = sources.read(world,q) else { return None; };
            if !matches!(id,B::Air | B::Water) { continue; }
            if amount < 8 && head - (q.y*8+amount as i32) >= 2 { return Some(q); }
            // Only filled water transmits pressure. Never jump an air gap.
            if id == B::Water && amount == 8 && len < queue.len() {
                queue[len] = q; len += 1;
            }
        }
    }
    None
}
pub(super) fn step(
    sources: &mut Residency,
    world: &mut World,
    p: IVec3,
    body: (Vec3, Vec3),
    visited: &mut HashSet<IVec3>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> usize {
    if !allowed(world, p) {
        return 0;
    }
    let Some((b, _)) = sources.read(world, p) else {
        world.activity.wake(p);
        return 0;
    };
    if b == B::RigidCore { super::rigid::capture(sources, world, p); return 0; }
    if super::thermal::active(b) || (world.sandbox.cell(p).temperature - 20.0).abs() >= 0.25 {
        let count = super::thermal::step(sources, world, p, visited, blocks, chunks);
        if super::thermal::active(b) || sources.read(world,p).is_none_or(|(id,_)|id != b) { return count; }
    }
    if !matches!(b, B::Water | B::Sand | B::Gravel | B::Ash) {
        return 0;
    }
    let down = p - IVec3::Y;
    let cfg = world.sample_config();
    if down.y < cfg.min_world_y() {
        return 0;
    }
    // A rule sees a complete neighbourhood. Missing sources suspend it; they
    // are never interpreted as air or procedural replacements for saved edits.
    let positions = [
        p,
        down,
        p + IVec3::X,
        p - IVec3::X,
        p + IVec3::Z,
        p - IVec3::Z,
    ];
    let mut ready = true;
    for q in positions {
        ready &= sources.request_for(
            world,
            chunk_coord_from_world_block(q),
            chunk_coord_from_world_block(p),
        );
    }
    if !ready {
        world.activity.wake(p);
        return 0;
    }
    let (below, _) = sources.read(world, down).unwrap();
    if matches!(b, B::Sand | B::Gravel | B::Ash) {
        let mut target = None;
        if matches!(below, B::Air | B::Water) {
            target = Some(down);
        } else {
            // All possible landing cells must be known before selecting one.
            // An unloaded neighbour is not a wall and must not bias the slope.
            for d in HORIZONTAL {
                ready &= sources.request_for(world, chunk_coord_from_world_block(down + d),
                    chunk_coord_from_world_block(p));
            }
            if !ready { world.activity.wake(p); return 0; }
            let phase = (world.activity.tick as usize)
                .wrapping_add(p.x as usize).wrapping_add(p.z as usize) % 4;
            for i in 0..4 {
                let d = HORIZONTAL[(i + phase) % 4];
                let side = p + d;
                let landing = down + d;
                let clear = |q: IVec3| sources.read(world, q)
                    .is_some_and(|(id, _)| matches!(id, B::Air | B::Water));
                let overlaps_player = |q: IVec3| q.as_vec3().cmplt(body.1).all()
                    && (q.as_vec3() + Vec3::ONE).cmpgt(body.0).all();
                if clear(side) && clear(landing) && allowed(world, side) {
                    if overlaps_player(side) || overlaps_player(landing) {
                        // Leaving the area is not a voxel edit, so retain the
                        // wakeup rather than sleeping behind a moving player.
                        world.activity.wake(p);
                        continue;
                    }
                    target = Some(landing);
                    break;
                }
            }
        }
        let Some(to) = target else { return 0; };
        if to.as_vec3().cmplt(body.1).all() && (to.as_vec3() + Vec3::ONE).cmpgt(body.0).all() {
            world.activity.wake(p);
            return 0;
        }
        let displaced = sources.read(world, to).unwrap().1;
        if !admission(world, p, to) || !sources.prepare(world, &[p, to]) {
            world.activity.wake(p);
            return 0;
        }
        world.set_many(&[(p, B::Air), (to, b)]);
        if displaced > 0 { world.set_water_volume(p, displaced); }
        visited.insert(to);
        changed(p, blocks, chunks);
        changed(to, blocks, chunks);
        return 2;
    }
    if b != B::Water {
        return 0;
    }
    let mut updates = transfer(sources, world, p, down, 8, visited, blocks, chunks);
    // Decorrelate neighbouring columns. A global rotating order made entire
    // sheets prefer the same axis on each tick.
    let phase = (p.x as u32).wrapping_mul(73856093)
        ^ (p.z as u32).wrapping_mul(19349663) ^ world.activity.tick as u32;
    for i in 0..4 {
        let to = p + HORIZONTAL[(i + phase as usize) % 4];
        let source = sources.read(world, p).unwrap().1;
        let target = sources.read(world, to).unwrap().1;
        let moved = source.saturating_sub(target) / 2;
        if moved > 0 {
            updates += transfer(sources, world, p, to, moved.min(2), visited, blocks, chunks);
        } else if source == 1 && target == 0
            && sources.read(world, to).is_some_and(|(id, _)| id == B::Air) {
            // The final eighth-cell used to stay forever at a ledge because
            // integer equalisation rounded 1/2 to zero. Allow it to drain only
            // toward a real lower outlet; flat puddles cannot wander endlessly.
            let outlet = to - IVec3::Y;
            if !sources.request_for(world, chunk_coord_from_world_block(outlet), chunk_coord_from_world_block(p)) {
                world.activity.wake(p);
            } else if sources.read(world, outlet).is_some_and(|(id, amount)|
                id == B::Air || (id == B::Water && amount < 8)) && allowed(world, to) {
                updates += transfer(sources, world, p, outlet, 1, visited, blocks, chunks);
            }
        }
    }
    if updates == 0 && sources.read(world,p).is_some_and(|(id,v)| id == B::Water && v > 1) {
        if let Some(outlet) = pressure_outlet(sources,world,p) {
            updates += transfer(sources,world,p,outlet,2,visited,blocks,chunks);
        }
    }
    updates
}
