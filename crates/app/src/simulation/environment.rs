//! Environmental event service. Dependency loads use the existing bounded
//! residency queue; an unavailable recipient leaves its source untouched.
use super::{cellular::changed, residency::Residency};
use crate::world::{
    BlockId as B, World, chunk_coord_from_world_block as chunk,
    environment::{self as env, State},
};
use glam::IVec3;
use std::collections::HashSet;
#[cfg(test)]
mod activation_tests;
mod aggregation;
#[cfg(test)]
mod cost_tests;
#[cfg(test)]
mod interaction_tests;
mod light;
mod roots;
mod sediment;
mod soil;
#[cfg(test)]
mod tests;
const EVENTS_PER_SERVICE: usize = 512;

pub(super) fn update(
    world: &mut World,
    sources: &mut Residency,
    player: IVec3,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
    budget: super::Budget,
) {
    if !State::enabled(world.generator_version) || world.ca_view.is_some() {
        return;
    }
    world.environment.fixed_steps += 1;
    world.environment.target_tick = world.environment.fixed_steps / 120;
    // A bounded contact probe, independent of view direction or render LOD.
    // Ordinary edits and fluid transfers supply the rest of the frontier.
    if world.environment.fixed_steps.is_multiple_of(120) {
        world.environment.wake(player - IVec3::Y);
    }
    for _ in 0..EVENTS_PER_SERVICE {
        if !budget.has_time() {
            break;
        }
        let next = world.environment.tick + 1;
        if next > world.environment.target_tick {
            break;
        }
        let Some((due, p)) = world.environment.due.first().copied().filter(|(due, _)| *due <= next) else {
            world.environment.tick = world
                .environment
                .target_tick
                .min(world.environment.due.first().map_or(world.environment.target_tick, |(due, _)| due - 1));
            continue;
        };
        let p = env::position(p);
        let cfg = world.sample_config();
        if p.y < cfg.min_world_y() || p.y >= cfg.max_world_y() {
            world.environment.complete_event(due, env::key(p));
            continue;
        }
        let owner = world.environment.owner(p);
        let extent = world.environment.patches.get(&owner).map_or(1, |p| p.extent);
        let positions: Vec<_> = std::iter::once(p)
            .chain(env::faces(env::position(owner), extent).into_iter().map(|(q, _)| q))
            .chain((1..world.environment.cells_per_metre).map(|depth| p - IVec3::Y * depth))
            .filter(|q| q.y >= cfg.min_world_y() && q.y < cfg.max_world_y())
            .collect();
        let mut ready = true;
        for &q in &positions {
            ready &= sources.request_for(world, chunk(q), chunk(p));
        }
        if !ready || !sources.prepare(world, &positions) {
            world.environment.blocked += 1;
            break; // Keep exact order and the entire pending event.
        }
        if !light::prepare(world, sources, p) {
            world.environment.blocked += 1;
            break;
        }
        world.environment.event_tick = Some(next);
        let soil_repeat = soil::process(world, p, next, blocks, chunks);
        let sediment_repeat = sediment::process(world, p, blocks, chunks);
        world.environment.event_tick = None;
        world.environment.complete_event(due, env::key(p));
        if soil_repeat || sediment_repeat {
            let owner = world.environment.owner(p);
            let delay = if sediment_repeat { 1 } else { soil::interval(world, p) };
            world.environment.schedule(owner, next + delay);
        }
    }
}

/// Every stored patch belongs to one actual cell. Purely solid neighbors are
/// not recursively allocated: allocation follows a nonzero physical exchange.
fn ensure_patch(world: &mut World, p: IVec3) -> bool {
    if world.environment.patches.contains_key(&world.environment.owner(p)) {
        return true;
    }
    if aggregation::buried(world, p) {
        return true;
    }
    let cfg = world.sample_config();
    let material = world.get(p);
    let exposed = world.get(p + IVec3::Y) == B::Air;
    world.environment.create_patch(cfg, p, material, exposed, true)
}
