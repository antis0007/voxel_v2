//! Pollutants cross water faces only after the water transfer commits.
use super::*;
use crate::world::{effects, environment, chunk_coord_from_world_block};
use std::{collections::HashSet, ops::Bound::{Excluded, Unbounded}};

pub(super) fn condense(
    world: &mut World, state: &mut State, budget: engine_core::Budget,
    blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>,
) {
    if !budget.has_time() || state.air.is_empty() {
        return;
    }
    let size = crate::world::factory::step(world);
    let cfg = world.sample_config();
    let exact = environment::State::enabled(world.generator_version);
    let minimum = if exact { 2_000 } else {
        engine_core::environment::cell_volume(world.voxel_scale) / 8
    };
    let lower = state.condensate_cursor.map_or(Unbounded, Excluded);
    let mut candidates = [None; 32];
    let mut count = 0;
    for (&key, mass) in state.air.range((lower, Unbounded)).take(128) {
        state.condensate_cursor = Some(key);
        if mass[STEAM] >= minimum {
            candidates[count] = Some((key, mass[STEAM]));
            count += 1;
            if count == candidates.len() {
                break;
            }
        }
    }
    if state.condensate_cursor == state.air.last_key_value().map(|(&k, _)| k)
        || state.air.range((lower, Unbounded)).next().is_none()
    {
        state.condensate_cursor = None;
    }
    let resident = |world: &World, p| world.chunks.contains_key(&chunk_coord_from_world_block(p));
    for (key, steam) in candidates.into_iter().flatten() {
        if !budget.has_time() {
            break;
        }
        let centre = pos(key) + IVec3::splat(key.3 / 2);
        // Short, resident roof and floor rays keep condensation inside covered
        // structures and avoid pulling terrain into memory for a gas plume.
        let walled = |direction| (1..=size * 6).map(|distance| centre + direction * distance)
            .take_while(|&p| resident(world, p)).any(|p| world.get(p).is_solid());
        if !walled(IVec3::Y) || [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
            .into_iter().filter(|&direction| walled(direction)).take(2).count() < 2
        {
            continue;
        }
        let Some(floor) = (1..=size * 6).map(|y| centre - IVec3::Y * y)
            .take_while(|&p| resident(world, p)).find(|&p| {
                let b = world.get(p);
                b.is_solid() || b == B::Water
            }) else { continue; };
        let material = world.get(floor);
        let supply = if exact { (steam / 8).clamp(minimum, 32_768) } else { minimum };
        let mut infiltrated = 0;
        if exact
            && let Some(law) = engine_content::environment::definition().and_then(|d| d.soil(material))
            && world.environment.can_wake(&[floor])
        {
            world.environment.create_patch(cfg, floor, material, true, true);
            let owner = world.environment.owner(floor);
            if let Some(patch) = world.environment.patches.get_mut(&owner) {
                let room = law.capacity(patch.scale(world.voxel_scale)).saturating_sub(patch.soil.water);
                infiltrated = room.min(supply / 2);
                patch.soil.water += infiltrated;
            }
            if infiltrated > 0 {
                world.environment.wake(floor);
            }
        }
        let landing = if material == B::Water { floor } else { floor + IVec3::Y };
        let mut runoff = (supply - infiltrated) as u32;
        let pooled = effects::exchange_water_ul(
            world, landing, &mut runoff, supply as u32, effects::WaterDirection::IntoWorld,
        ).map_or(0, |receipt| receipt.publish(world, blocks, chunks));
        let condensed = infiltrated + pooled;
        if condensed > 0 {
            state.air.get_mut(&key).expect("condensate source")[STEAM] -= condensed;
            state.transformed[STEAM] += condensed;
            world.environment.ledger.edited_water_added_ul += condensed;
        }
    }
    state.air.retain(|_, m| total(m) > 0);
}

pub struct Transfer {
    from: (i32, i32, i32),
    to: (i32, i32, i32),
    mass: Mass,
}
fn key(p: IVec3, size: i32) -> (i32, i32, i32) {
    let p = p.div_euclid(IVec3::splat(size)) * size;
    (p.x, p.y, p.z)
}
pub fn prepare(world: &World, from: IVec3, to: IVec3, moved_ul: u64) -> Option<Transfer> {
    let size = super::super::factory::step(world);
    let a = key(from, size);
    let b = key(to, size);
    let mut transfer = Transfer {
        from: a,
        to: b,
        mass: [0; SPECIES],
    };
    if a == b {
        return Some(transfer);
    }
    let Some(stock) = world.industry.deposits.get(&a) else {
        return Some(transfer);
    };
    if !world.industry.deposits.contains_key(&b) && world.industry.deposits.len() >= MAX_DEPOSITS {
        return None;
    }
    let origin = IVec3::new(a.0, a.1, a.2);
    let mut water = 0u64;
    for y in 0..size {
        for z in 0..size {
            for x in 0..size {
                water = water.saturating_add(world.water_microlitres(origin + IVec3::new(x, y, z)));
            }
        }
    }
    for i in 0..SPECIES {
        transfer.mass[i] = ((u128::from(stock[i]) * u128::from(moved_ul))
            / u128::from(water.max(1)))
        .min(u128::from(stock[i])) as u64;
    }
    Some(transfer)
}
impl State {
    pub fn transfer_water(&mut self, t: Transfer) {
        if t.from == t.to || total(&t.mass) == 0 {
            return;
        }
        let Some(source) = self.deposits.get_mut(&t.from) else {
            return;
        };
        for i in 0..SPECIES {
            source[i] -= t.mass[i];
        }
        let dest = self.deposits.entry(t.to).or_default();
        for i in 0..SPECIES {
            dest[i] += t.mass[i];
        }
        self.deposits.retain(|_, m| total(m) > 0);
    }
}
