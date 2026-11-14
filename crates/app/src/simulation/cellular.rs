//! Synchronous conservative commits over asynchronously prepared source chunks.
use super::residency::Residency;
use crate::world::{
    BlockId as B, World, chunk_coord_from_world_block, mark_chunk_and_neighbors_dirty,
};
use glam::{IVec3, Vec2, Vec3};
use std::collections::HashSet;
const HORIZONTAL: [IVec3; 4] = [IVec3::X, IVec3::Z, IVec3::NEG_X, IVec3::NEG_Z];
mod drainage;
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
    // Deduplicate chunk coordinates before admission. Adjacent water cells
    // usually touch one or two chunks; handing `can_wake` all 22 raw probes
    // made it repeat work proportional to the probe list square on every
    // transfer.
    let mut coords = [IVec3::ZERO; 22];
    let mut index = 0;
    for p in [a, b] {
        let c = chunk_coord_from_world_block(p);
        if !coords[..index].contains(&c) {
            coords[index] = c;
            index += 1;
        }
        for d in crate::world::activity::WAKE_NEIGHBORS {
            let c = chunk_coord_from_world_block(p + d);
            if !coords[..index].contains(&c) {
                coords[index] = c;
                index += 1;
            }
        }
    }
    allowed(world, a) && allowed(world, b) && world.activity.can_wake(&coords[..index])
}
fn commit_transfer(
    sources: &mut Residency,
    world: &mut World,
    from: IVec3,
    to: IVec3,
    source: u8,
    target_block: B,
    target: u8,
    moved: u8,
    visited: &mut HashSet<IVec3>,
) -> usize {
    if moved == 0 || !matches!(target_block, B::Air | B::Water) {
        return 0;
    }
    let mut moved = moved;
    if crate::world::environment::State::enabled(world.generator_version) {
        while moved > 0 && !world.environment.transfer_fits(from, to, source, target, moved) {
            moved -= 1;
        }
        if moved == 0 { return 0; }
    }
    if !admission(world, from, to) {
        world.activity.wake(from);
        return 0;
    }
    if !sources.prepare(world, &[from, to]) {
        world.activity.wake(from);
        return 0;
    }
    // Bounds, assembly guards, residency and both amounts have been checked
    // before the paired commit. One transaction keeps the moved mass atomic,
    // and pays revision/snapshot/wake bookkeeping once per transfer.
    if !world.apply_water_pair(
        from,
        source.saturating_sub(moved),
        to,
        target.saturating_add(moved),
        moved,
    ) {
        world.activity.wake(from);
        return 0;
    }
    let horizontal = Vec2::new((to.x - from.x) as f32, (to.z - from.z) as f32);
    if horizontal.length_squared() > 0.25 {
        let direction = horizontal.normalize();
        let packet = moved as f32;
        // Momentum is intentionally weaker than the delivered packet. It
        // biases the next equalisation pass without turning a single transfer
        // into a runaway current.
        if source > moved {
            let source_motion = (world.water_velocity(from) - direction * (packet * 0.34)) * 0.94;
            world.set_water_velocity(from, source_motion);
        } else {
            world.remove_water_velocity(from);
        }
        let target_motion = (world.water_velocity(to) + direction * (packet * 0.56)) * 0.97;
        world.set_water_velocity(to, target_motion);
    }
    // A recipient can receive more mass but cannot transport it again this
    // tick. Otherwise queue order allows water to traverse an entire chunk.
    visited.insert(to);
    2
}

fn transfer(
    sources: &mut Residency,
    world: &mut World,
    from: IVec3,
    to: IVec3,
    max: u8,
    visited: &mut HashSet<IVec3>,
) -> usize {
    let Some((B::Water, source)) = sources.read(world, from) else {
        world.activity.wake(from);
        return 0;
    };
    let Some((target_block, target)) = sources.read(world, to) else {
        world.activity.wake(from);
        return 0;
    };
    let moved = source.min(8 - target).min(max);
    commit_transfer(
        sources,
        world,
        from,
        to,
        source,
        target_block,
        target,
        moved,
        visited,
    )
}

// A bounded hydraulic connection search lets a submerged outlet push water
// upward below the source column's free surface. No mass is created, no solid
// is crossed, and unavailable source data postpones the rule.
fn pressure_outlet(
    sources: &mut Residency,
    world: &mut World,
    p: IVec3,
    resting_head: i64,
) -> Option<(IVec3, IVec3, u64)> {
    let owner = chunk_coord_from_world_block(p);
    let mut surface = p;
    let exact = crate::world::environment::State::enabled(world.generator_version);
    let volume = if exact { world.environment.quantum() as i64 * 8 } else { 8 };
    let read = |sources: &Residency, world: &World, q| {
        if exact {
            sources.water_cell(world, q).map(|(id, fill)| (id, fill as i64))
        } else {
            sources.read(world, q).map(|(id, fill)| (id, i64::from(fill)))
        }
    };
    let mut head = i64::from(p.y) * volume;
    for rise in 1..=9 {
        let q = p + IVec3::Y * rise;
        if !sources.request_for(world, chunk_coord_from_world_block(q), owner) {
            world.activity.wake(p);
            return None;
        }
        match read(sources, world, q)? {
            (B::Water, fill) => {
                if rise > 8 {
                    return None;
                }
                surface = q;
                head = i64::from(q.y) * volume + fill;
            }
            _ => break,
        }
    }
    if surface == p {
        return None;
    }
    let mut queue = [IVec3::ZERO; 48];
    queue[0] = p;
    let (mut cursor, mut len) = (0, 1);
    while cursor < len {
        let from = queue[cursor];
        cursor += 1;
        for d in [
            IVec3::NEG_Y,
            IVec3::X,
            IVec3::Z,
            IVec3::NEG_X,
            IVec3::NEG_Z,
            IVec3::Y,
        ] {
            let q = from + d;
            let base = i64::from(q.y) * volume;
            if base >= head
                || (q - p).abs().max_element() > 8
                || queue[..len].contains(&q)
                || !allowed(world, q)
            {
                continue;
            }
            if !sources.request_for(world, chunk_coord_from_world_block(q), owner) {
                world.activity.wake(p);
                return None;
            }
            let (id, fill) = read(sources, world, q)?;
            if !matches!(id, B::Air | B::Water) {
                continue;
            }
            let difference = head - (base + fill);
            if fill < volume && difference > resting_head {
                return Some((surface, q, difference as u64));
            }
            // Only filled water transmits pressure. Never jump an air gap.
            if id == B::Water && fill == volume && len < queue.len() {
                queue[len] = q;
                len += 1;
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
    let Some((b, source_volume)) = sources.read(world, p) else {
        world.activity.wake(p);
        return 0;
    };
    if b == B::Water && world.is_steady_surface_water(p) {
        return 0;
    }
    if b == B::SulfuricAcid {
        return super::thermal::step_acid(sources, world, p, visited, blocks, chunks);
    }
    if b == B::RigidCore {
        super::rigid::capture(sources, world, p);
        return 0;
    }
    if super::rigid::fragment_material(b) {
        super::rigid::capture(sources, world, p);
        // Construction still participates in heat and combustion below.
    }
    if super::thermal::active(b)
        || world
            .sandbox
            .cells
            .contains_key(&crate::simulation::sandbox::key(p))
    {
        let count = super::thermal::step(sources, world, p, visited, blocks, chunks);
        if super::thermal::active(b) || sources.read(world, p).is_none_or(|(id, _)| id != b) {
            return count;
        }
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
    let owner = chunk_coord_from_world_block(p);
    let mut requested = [owner; 6];
    let mut count = 0;
    let mut ready = true;
    for q in positions {
        let coord = chunk_coord_from_world_block(q);
        if !requested[..count].contains(&coord) {
            requested[count] = coord;
            count += 1;
            ready &= sources.request_for(world, coord, owner);
        }
    }
    if !ready {
        world.activity.wake(p);
        return 0;
    }
    if matches!(b, B::Sand | B::Gravel | B::Ash) {
        let (below, _) = sources.read(world, down).unwrap();
        let mut target = None;
        if matches!(below, B::Air | B::Water) {
            target = Some(down);
        } else {
            // All possible landing cells must be known before selecting one.
            // An unloaded neighbour is not a wall and must not bias the slope.
            for d in HORIZONTAL {
                ready &= sources.request_for(
                    world,
                    chunk_coord_from_world_block(down + d),
                    chunk_coord_from_world_block(p),
                );
            }
            if !ready {
                world.activity.wake(p);
                return 0;
            }
            let phase = (world.activity.tick as usize)
                .wrapping_add(p.x as usize)
                .wrapping_add(p.z as usize)
                % 4;
            for i in 0..4 {
                let d = HORIZONTAL[(i + phase) % 4];
                let side = p + d;
                let landing = down + d;
                let clear = |q: IVec3| {
                    sources
                        .read(world, q)
                        .is_some_and(|(id, _)| matches!(id, B::Air | B::Water))
                };
                let overlaps_player = |q: IVec3| {
                    q.as_vec3().cmplt(body.1).all() && (q.as_vec3() + Vec3::ONE).cmpgt(body.0).all()
                };
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
        let Some(to) = target else {
            return 0;
        };
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
        if world.get(to) != b {
            world.set(p,b);
            world.activity.wake(p);
            return 0;
        }
        if displaced > 0 && !crate::world::environment::State::enabled(world.generator_version) {
            world.set_water_volume(p, displaced);
        }
        visited.insert(to);
        return 2;
    }
    if b != B::Water {
        return 0;
    }
    if crate::world::environment::State::enabled(world.generator_version) {
        return drainage::step(sources, world, p, visited, blocks, chunks);
    }
    let mut updates = match sources.read(world, down) {
        Some((down_block, down_volume)) => commit_transfer(
            sources,
            world,
            p,
            down,
            source_volume,
            down_block,
            down_volume,
            source_volume.min(8 - down_volume).min(8),
            visited,
        ),
        None => {
            world.activity.wake(p);
            0
        }
    };
    // Current is presentation data. It cannot move water against its head.
    world.set_water_velocity(p, world.water_velocity(p) * 0.75);
    // Decorrelate neighbouring columns. A global rotating order made entire
    // sheets prefer the same axis on each tick.
    let phase = (p.x as u32).wrapping_mul(73856093)
        ^ (p.z as u32).wrapping_mul(19349663)
        ^ world.activity.tick as u32;
    for i in 0..4 {
        let to = p + HORIZONTAL[(i + phase as usize) % 4];
        let Some((source_block, source)) = sources.read(world, p) else {
            world.activity.wake(p);
            break;
        };
        let Some((target_block, target)) = sources.read(world, to) else {
            world.activity.wake(p);
            break;
        };
        if source_block != B::Water {
            break;
        }
        let moved = source.saturating_sub(target) / 2;
        if moved > 0 {
            updates += commit_transfer(
                sources,
                world,
                p,
                to,
                source,
                target_block,
                target,
                moved,
                visited,
            );
        } else if source == 1
            && target == 0
            && sources.read(world, to).is_some_and(|(id, _)| id == B::Air)
        {
            // The final eighth-cell used to stay forever at a ledge because
            // integer equalisation rounded 1/2 to zero. Allow it to drain only
            // toward a real lower outlet; flat puddles cannot wander endlessly.
            let outlet = to - IVec3::Y;
            if !sources.request_for(
                world,
                chunk_coord_from_world_block(outlet),
                chunk_coord_from_world_block(p),
            ) {
                world.activity.wake(p);
            } else if sources
                .read(world, outlet)
                .is_some_and(|(id, amount)| id == B::Air || (id == B::Water && amount < 8))
                && allowed(world, to)
            {
                updates += transfer(sources, world, p, outlet, 1, visited);
            }
        }
    }
    if updates == 0
        && (world.activity.tick == 0
            || (world.activity.tick as usize)
                .wrapping_add(p.x as usize)
                .wrapping_add(p.z as usize).is_multiple_of(8))
        && sources
            .read(world, p)
            .is_some_and(|(id, v)| id == B::Water && v == 8)
    {
        // A still pool can contain thousands of water cells. Searching for a
        // hydraulic outlet from every one of them on every tick was the most
        // expensive part of deep-water scenes. The lookup is staggered; each
        // cell is still revisited regularly, but not eight times per second.
        if let Some((surface, outlet, head)) = pressure_outlet(sources, world, p, 1) {
            // A hydraulic connection is expensive to find, so move a useful
            // packet once found. Two units left deep outlets visibly starved;
            // four still cannot exceed the recipient's remaining capacity.
            updates += transfer(sources, world, surface, outlet, (head / 2).min(4) as u8, visited);
        }
    }
    if updates == 0 {
        world.remove_water_velocity(p);
    }
    updates
}
