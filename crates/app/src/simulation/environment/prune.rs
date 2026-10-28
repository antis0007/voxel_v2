//! Bounded sweep that keeps the patch table below its soft limit. It drops
//! records that regenerate identically and, under pressure, mineral residue
//! left over dug-out cells far from the player.
use super::*;
use std::ops::Bound;

/// The sweep idles until the table reaches three quarters of its soft limit.
pub(super) const PRESSURE: usize = env::MAX_PATCHES / 4 * 3;
/// Patches examined per fixed step: a full table is visited in about 8 s.
const PER_STEP: usize = 64;
/// Residue within this many cells of the player may be built on again soon.
const RESIDUE_KEEP_CELLS: i32 = 64;

/// An empty record over a non-porous cell carries nothing a later placement
/// would not recreate.
pub(super) fn inert(patch: &env::Patch, porous_cell: bool) -> bool {
    !porous_cell && patch.extent == 1 && patch.soil == Default::default()
}

enum Verdict {
    Keep,
    /// Regenerates identically; the stored water returns to the implicit reserve.
    Redundant(u64),
    /// Mineral nutrient and solver carries only, over a non-porous cell.
    Residue,
}

/// Runs a fixed count per step, never gated on wall time: residue eviction is
/// lossy, so which patches survive must not depend on frame timing.
pub(super) fn sweep(world: &mut World, player: IVec3) {
    if world.environment.patches.len() < PRESSURE {
        return;
    }
    let lower = world.environment.prune_cursor.map_or(Bound::Unbounded, Bound::Excluded);
    let keys: Vec<env::Key> =
        world.environment.patches.range((lower, Bound::Unbounded)).take(PER_STEP).map(|(k, _)| *k).collect();
    world.environment.prune_cursor = keys.last().copied();
    for k in keys {
        match verdict(world, k, player) {
            Verdict::Keep => {}
            Verdict::Redundant(water) => {
                world.environment.patches.remove(&k);
                let ledger = &mut world.environment.ledger;
                ledger.initial_soil_water = ledger.initial_soil_water.saturating_sub(water);
            }
            Verdict::Residue => {
                world.environment.patches.remove(&k);
            }
        }
    }
}

fn verdict(world: &World, k: env::Key, player: IVec3) -> Verdict {
    let state = &world.environment;
    let Some(patch) = state.patches.get(&k) else {
        return Verdict::Keep;
    };
    let p = env::position(k);
    if patch.extent != 1
        || state.scheduled.contains_key(&k)
        || !world.chunks.contains_key(&chunk(p))
        || !world.chunks.contains_key(&chunk(p + IVec3::Y))
    {
        return Verdict::Keep;
    }
    let Some(definition) = engine_content::environment::definition() else {
        return Verdict::Keep;
    };
    let material = world.get(p);
    let s = patch.soil;
    if definition.soil(material).is_none() {
        if inert(patch, false) {
            return Verdict::Redundant(0);
        }
        let mineral_only = s.water == 0
            && s.biomass == 0
            && s.plant_nutrient == 0
            && s.litter == 0
            && s.litter_nutrient == 0;
        let far = (p - player).abs().max_element() > RESIDUE_KEEP_CELLS;
        return if mineral_only && far && state.patches.len() >= PRESSURE { Verdict::Residue } else { Verdict::Keep };
    }
    // Cheap filters before sampling climate: a fresh natural patch carries no
    // credits, carries, litter or erosion work.
    if material != patch.material
        || s.infiltration_credit != 0
        || s.flux_carry != 0
        || s.erosion_work != 0
        || s.litter != 0
        || s.litter_nutrient != 0
        || s.growth_credit != 0
    {
        return Verdict::Keep;
    }
    let exposed = world.get(p + IVec3::Y) == B::Air;
    match state.fresh_patch(world.sample_config(), p, material, exposed, true) {
        Some(fresh) if fresh.soil == s && fresh.species == patch.species => Verdict::Redundant(s.water),
        _ => Verdict::Keep,
    }
}
