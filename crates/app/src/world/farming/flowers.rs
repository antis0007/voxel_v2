//! Working flowers: mature plots that feed wand mana or gather loose stacks into a chest.
use super::table::{self, Aura};
use super::{World, conditions, resident};
use crate::world::inventory::ItemStack;
use glam::{IVec3, Vec3};

/// Flowers act once a second; the effect is scaled to the whole second.
const PERIOD: u64 = 120;
/// Flowers checked per period, so a large garden costs a bounded slice.
const MAX_FLOWERS: usize = 64;
pub(super) const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::Y, IVec3::NEG_Y];

/// Runs every mature working flower; `feet` is the player's position in metres.
pub fn service(world: &mut World, feet: Vec3) {
    if !world.farming.mist_ready {
        // A fresh load has no mist list yet; plots must not roll dry before the first pulse.
        rebuild_mist(world);
    }
    if !world.farming.clock.is_multiple_of(PERIOD) || world.farming.plots.is_empty() {
        return;
    }
    let mana_pulse = world.farming.clock.is_multiple_of(2 * PERIOD);
    pulse(world, feet, mana_pulse);
}

/// What one flower pass did; the dev panel reports it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pulse {
    /// Loose items mana flowers ate.
    pub eaten: u32,
    /// Mana sent to habitat receivers.
    pub stored: u32,
    /// Mana restored to the player.
    pub to_player: f32,
    /// Plots tending flowers hurried.
    pub tended: u32,
    /// Flowers that planted a copy beside themselves.
    pub spread: u32,
}

/// Brings every growing plot within `radius` (metres) `ticks` closer to its next roll.
/// One beat costs `cost` mana: first from a sunlit mana flower whose reach covers this
/// one (its unspent beat in `garden`), else from the nearest habitat receiver.
fn tend(world: &mut World, p: IVec3, centre: Vec3, radius: f32, ticks: u32, cost: u32,
        garden: &mut [(Vec3, f32, f32)]) -> u32 {
    let r = (radius / world.voxel_scale.meters()).floor() as i32;
    let candidates: Vec<super::Key> = world.farming.plots
        .range((p.x - r, i32::MIN, i32::MIN)..=(p.x + r, i32::MAX, i32::MAX))
        .filter(|&(&(x, y, z), plot)| {
            let q = IVec3::new(x, y, z);
            q != p && (q - p).length_squared() <= r * r
                && table::get(plot.crop).is_some_and(|c| c.aura.is_none() && plot.stage < c.mature())
        })
        .map(|(&k, _)| k)
        .collect();
    if candidates.is_empty() {
        return 0;
    }
    let need = cost as f32;
    let paid = cost == 0
        || garden.iter_mut()
            .find(|(at, reach, left)| *left >= need && at.distance_squared(centre) <= reach * reach)
            .map(|g| g.2 -= need)
            .is_some()
        || world.habitats.draw_flower_mana(centre, radius, cost);
    if !paid {
        return 0;
    }
    for k in &candidates {
        if let Some(plot) = world.farming.plots.get_mut(k) {
            plot.planted = plot.planted.saturating_sub(u64::from(ticks));
        }
    }
    candidates.len() as u32
}

/// Same-crop plots allowed within two cells before a flower stops spreading.
const SPREAD_CROWD: usize = 2;

/// Plants one copy of the flower at `p` on free soil beside it, if its seeded roll hits.
/// Spreading stops once two of its kind stand nearby, so a bed never carpets the field.
fn spread(world: &mut World, p: IVec3, index: u8, crop: &table::Crop, changed: &mut Vec<IVec3>) -> bool {
    let hash = super::stage::roll(world.seed, p, world.farming.clock ^ 0x5157_4ead);
    if crop.spread_per_mille == 0 || hash % 1000 >= crop.spread_per_mille
        || world.farming.plots.len() >= super::MAX_PLOTS || !crop.planting.is_soil()
    {
        return false;
    }
    let crowd = world.farming.plots.range((p.x - 2, i32::MIN, i32::MIN)..=(p.x + 2, i32::MAX, i32::MAX))
        .filter(|&(&(x, y, z), plot)| plot.crop == index && (IVec3::new(x, y, z) - p).abs().max_element() <= 2)
        .count();
    if crowd > SPREAD_CROWD {
        return false;
    }
    let start = (hash / 1000) as usize;
    for i in 0..4 {
        let q = p + SIDES[(start + i) % 4];
        if !resident(world, q) || !resident(world, q - IVec3::Y) || world.get(q) != crate::world::BlockId::Air {
            continue;
        }
        let ground = world.get(q - IVec3::Y);
        if !(crop.soil.contains(&ground) || ground == super::BED) {
            continue;
        }
        if let Ok(commit) = super::effects::replace(world, q, crate::world::BlockId::Air, crop.sprout) {
            commit.publish_voxel(world, changed);
            world.farming.plots.insert(super::key(q), super::Plot::new(index, world.farming.clock));
            return true;
        }
    }
    false
}

/// Lists mature resident mist flowers without running any other aura.
fn rebuild_mist(world: &mut World) {
    let scale = world.voxel_scale.meters();
    let mist: Vec<(IVec3, i32)> = world.farming.plots.iter()
        .filter_map(|(&(x, y, z), plot)| {
            let crop = table::get(plot.crop)?;
            let Some(Aura::Mist { radius_metres }) = crop.aura else { return None };
            let p = IVec3::new(x, y, z);
            (plot.stage >= crop.mature() && resident(world, p))
                .then_some((p, (radius_metres / scale).floor() as i32))
        })
        .collect();
    world.farming.mist = mist;
    world.farming.mist_ready = true;
}

/// One flower pass; `mana_pulse` marks the two-second mana beat.
pub fn pulse(world: &mut World, feet: Vec3, mana_pulse: bool) -> Pulse {
    let mut report = Pulse::default();
    let scale = world.voxel_scale.meters();
    let flowers: Vec<(IVec3, Aura)> = world.farming.plots.iter()
        .filter_map(|(&(x, y, z), plot)| {
            let crop = table::get(plot.crop)?;
            let aura = crop.aura?;
            let p = IVec3::new(x, y, z);
            (plot.stage >= crop.mature() && resident(world, p)
                && (mana_pulse || !matches!(aura, Aura::Mana { .. } | Aura::Tend { .. })))
                .then_some((p, aura))
        })
        .take(MAX_FLOWERS)
        .collect();
    world.farming.mist.clear();
    world.farming.mist_ready = true;
    // Tending runs first so a garden's own mana flowers pay it before the player or receivers.
    let mut flowers = flowers;
    flowers.sort_by_key(|(_, a)| !matches!(a, Aura::Tend { .. }));
    // Each sunlit mana flower's beat: (centre, reach, mana left this beat).
    let mut garden: Vec<(Vec3, f32, f32)> = flowers.iter()
        .filter_map(|&(p, aura)| {
            let Aura::Mana { per_second, radius_metres, .. } = aura else { return None };
            let crop = table::get(world.farming.plots[&super::key(p)].crop)?;
            conditions(world, p, crop).is_some_and(|c| c.light >= 15)
                .then(|| ((p.as_vec3() + Vec3::splat(0.5)) * scale, radius_metres, 2.0 * per_second))
        })
        .collect();
    if mana_pulse && world.adventure.dirty_voxels.len() < super::DIRTY_LIMIT {
        let mut changed = Vec::new();
        for &(p, _) in &flowers {
            let index = world.farming.plots[&super::key(p)].crop;
            if let Some(crop) = table::get(index) && spread(world, p, index, crop, &mut changed) {
                report.spread += 1;
            }
        }
        world.adventure.dirty_voxels.extend(changed);
    }
    let mut mana = 0.0;
    for (p, aura) in flowers {
        let centre = (p.as_vec3() + Vec3::splat(0.5)) * scale;
        match aura {
            Aura::Mana { radius_metres, feed, .. } => {
                let beat = garden.iter().find(|g| g.0 == centre).map(|g| g.2);
                let player_in = centre.distance_squared(feet) <= radius_metres * radius_metres;
                if let Some(pulse) = beat.filter(|&m| m > 0.0) {
                    // Two seconds make one whole mana at the standard flower rate, less what tending spent.
                    let stored = world.habitats.store_flower_mana(centre, radius_metres, pulse.floor() as u32);
                    report.stored += stored;
                    if player_in {
                        mana += pulse - stored as f32;
                    }
                }
                // Eating works under cover too: the item, not the sky, pays.
                if let Some(feed) = feed {
                    let Some(at) = world.inventory.dropped.iter()
                        .position(|d| d.stack.item == feed.item && d.stack.count > 0
                            && d.pos.distance_squared(centre) <= feed.reach_metres * feed.reach_metres)
                    else { continue };
                    let stored = world.habitats.store_flower_mana(centre, radius_metres, feed.mana_per_item);
                    let player_wants = player_in && world.adventure.mana + mana < world.adventure.max_mana();
                    if stored == 0 && !player_wants {
                        continue;
                    }
                    report.stored += stored;
                    report.eaten += 1;
                    if player_wants {
                        mana += (feed.mana_per_item - stored) as f32;
                    }
                    let drop = &mut world.inventory.dropped[at];
                    drop.stack.count -= 1;
                    if drop.stack.count == 0 {
                        world.inventory.dropped.remove(at);
                    }
                }
            }
            Aura::Collect { radius_metres } => collect(world, p, centre, radius_metres),
            Aura::Harvest { radius_metres } => super::harvester::service(world, p, radius_metres),
            Aura::Mist { radius_metres } => {
                world.farming.mist.push((p, (radius_metres / scale).floor() as i32));
            }
            Aura::Tend { radius_metres, ticks, mana: cost } => {
                report.tended += tend(world, p, centre, radius_metres, ticks, cost, &mut garden);
            }
        }
    }
    if mana > 0.0 {
        let cap = world.adventure.max_mana();
        let before = world.adventure.mana;
        world.adventure.mana = (before + mana).min(cap);
        report.to_player = world.adventure.mana - before;
    }
    report
}

/// Moves loose stacks near `centre` into the first touching chest with room.
fn collect(world: &mut World, p: IVec3, centre: Vec3, radius: f32) {
    let Some(chest) = SIDES.iter().map(|&d| p + d).find(|&q| world.get(q).chest_tier().is_some()) else { return };
    let slots_len = world.get(chest).chest_slots();
    let mut slots = world.block_data.chest(chest);
    slots.resize(slots_len, None);
    let mut moved = false;
    let mut kept = Vec::with_capacity(world.inventory.dropped.len());
    for mut drop in std::mem::take(&mut world.inventory.dropped) {
        if drop.pos.distance(centre) <= radius {
            let limit = world.inventory.stack_limit(drop.stack.item);
            drop.stack.count = store(&mut slots, drop.stack, limit);
            moved = true;
        }
        if drop.stack.count > 0 {
            kept.push(drop);
        }
    }
    world.inventory.dropped = kept;
    if moved && world.block_data.ensure_chest(chest) {
        world.block_data.set_chest(chest, slots);
    }
}

/// Adds `stack` to matching then empty slots; returns the count left over.
pub(super) fn store(slots: &mut [Option<ItemStack>], stack: ItemStack, limit: u32) -> u32 {
    let mut left = stack.count;
    for slot in slots.iter_mut().flatten().filter(|s| s.item == stack.item) {
        let add = left.min(limit.saturating_sub(slot.count));
        slot.count += add;
        left -= add;
    }
    for slot in slots.iter_mut().filter(|s| s.is_none()) {
        if left == 0 {
            break;
        }
        let add = left.min(limit.max(1));
        *slot = Some(ItemStack::new(stack.item, add));
        left -= add;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::inventory::Item;

    #[test]
    fn store_fills_matching_stacks_before_empty_slots() {
        let apple = Item::Block(crate::world::BlockId::Apple);
        let mut slots = vec![Some(ItemStack::new(apple, 60)), None, None];
        assert_eq!(store(&mut slots, ItemStack::new(apple, 10), 64), 0);
        assert_eq!(slots[0].map(|s| s.count), Some(64));
        assert_eq!(slots[1].map(|s| s.count), Some(6));
        let mut full = vec![Some(ItemStack::new(apple, 64))];
        assert_eq!(store(&mut full, ItemStack::new(apple, 3), 64), 3);
    }

    #[test]
    fn working_flowers_have_auras() {
        for b in [crate::world::BlockId::ManaFlower, crate::world::BlockId::CollectorFlower] {
            assert!(table::by_seed(b).is_some_and(|(_, c)| c.aura.is_some()), "{b:?}");
        }
    }
}
