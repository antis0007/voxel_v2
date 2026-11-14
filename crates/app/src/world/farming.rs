//! Planted crops on tilled beds: interval growth, catch-up after unload, harvest.
//! Plots and the farming clock persist in `sidecar::FILE`, not the save body.
use super::{BlockId as B, World, chunk_coord_from_world_block as chunk_of, effects, inventory::{Item, ItemStack}};
use engine_core::Budget;
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet, HashSet};
pub(crate) mod sidecar;
pub(crate) mod watershed;
pub(crate) mod freight;
pub(crate) mod planet_freight;
pub(crate) mod jobs;
pub(crate) mod base;
pub(crate) mod pantry;
pub mod flowers;
pub mod forage;
pub(crate) mod harvester;
pub(crate) mod review;
pub mod sapling;
mod stage;
pub mod table;
#[cfg(test)]
mod tests;

pub use stage::{Conditions, Plot};
pub const MAX_PLOTS: usize = 4096;
/// Wild cells remembered as paid; past this no wild cell pays a seed.
pub const MAX_WILD_PAID: usize = 65_536;
const PLOTS_PER_STEP: usize = 32;
const ROLLS_PER_SERVICE: u32 = 512;
const WATER_RADIUS: i32 = 4;
const SKY_METRES: f32 = 8.0;
const REACH_METRES: f32 = 6.0;
const DIRTY_LIMIT: usize = 2048;
/// Loose stacks farming may add; matches the chest spill limit.
const MAX_WILD_DROPS: usize = 1024;
const TILLABLE: [B; 3] = [B::Grass, B::Dirt, B::SnowGrass];
const BED: B = B::FungalSoil;
type Key = (i32, i32, i32);

#[derive(Clone, Debug, Default)]
pub struct State {
    /// Fixed steps counted by `tick`; plot ages and herd timers read it.
    pub clock: u64,
    pub(crate) nursery_stock: u32,
    pub(crate) freight: freight::State,
    planet_freight: Option<planet_freight::State>,
    jobs: jobs::State,
    pub(crate) communities: jobs::communities::State,
    pub(crate) activities: jobs::activity::Runtime,
    pub(crate) base: base::State,
    pub(crate) pantry: Option<pantry::State>,
    pub(crate) plots: BTreeMap<Key, Plot>,
    /// Generated crop cells that already paid a wild seed; a block put back there pays nothing.
    pub(crate) wild_paid: BTreeSet<Key>,
    cursor: Option<Key>,
    /// Mature mist flowers (cell, radius in cells), rebuilt each flower pulse; never saved.
    pub(crate) mist: Vec<(IVec3, i32)>,
}

fn key(p: IVec3) -> Key {
    (p.x, p.y, p.z)
}

/// Advances the clock one fixed step and services a bounded slice of plots.
pub fn tick(world: &mut World, budget: Budget) {
    world.farming.clock += 1;
    if world.ca_view.is_some() {
        return;
    }
    freight::service(world);
    planet_freight::service(world);
    pantry::service(world);
    if world.farming.plots.is_empty() {
        return;
    }
    let nursery = watershed::service(world);
    let after = world.farming.cursor;
    use std::ops::Bound::{Excluded, Unbounded};
    let plots = &world.farming.plots;
    let mut keys: Vec<Key> = after.map_or_else(Vec::new, |a| {
        plots.range((Excluded(a), Unbounded)).map(|(&k, _)| k).take(PLOTS_PER_STEP).collect()
    });
    let wrap = PLOTS_PER_STEP - keys.len();
    keys.extend(plots.keys().copied().take_while(|k| after.is_none_or(|a| *k <= a)).take(wrap));
    let now = world.farming.clock;
    let mut changed = Vec::new();
    for k in keys {
        if !budget.has_time() {
            break;
        }
        if !nursery.contains(&Some(k)) {
            service(world, k, now, &mut changed);
        }
        world.farming.cursor = Some(k);
    }
    world.adventure.dirty_voxels.extend(changed);
}

fn resident(world: &World, p: IVec3) -> bool {
    world.chunks.contains_key(&chunk_of(p))
}

/// A block whose value is exact: resident, or generated and never edited.
fn known(world: &World, p: IVec3) -> Option<B> {
    let c = chunk_of(p);
    (world.chunks.contains_key(&c) || !world.regions.view.records.contains_key(&c)).then(|| world.get(p))
}

/// Growth inputs at `p`, or `None` while a cell that could change them is paged out.
pub fn conditions(world: &World, p: IVec3, crop: &table::Crop) -> Option<Conditions> {
    conditions_with_water(world, p, crop, watershed::irrigation(world, p, crop))
}

fn conditions_with_water(world: &World, p: IVec3, crop: &table::Crop, intake: Option<bool>) -> Option<Conditions> {
    let mut soil = false;
    for &d in crop.planting.supports() {
        let block = known(world, p + d)?;
        soil |= crop.soil.contains(&block) || (block == BED && crop.planting == table::Planting::Soil);
    }
    let torch = world.chunks.get(&chunk_of(p))
        .map_or(0, |c| c.light[super::ChunkData::local_index(p.x, p.y, p.z)] & 0x0F);
    // Fernwake's high orchard vault still shades the managed Glowcap bed.
    let sky_metres = if intake.is_some() { watershed::SKY_METRES } else { SKY_METRES };
    let height = world.sample_config().blocks_for_height(sky_metres).max(1);
    let mut sky = true;
    for y in 1..=height {
        let b = known(world, p + IVec3::Y * y)?;
        // A stalk's own cells must not shade its base.
        if !b.is_empty() && !(crop.stalk.is_some() && b == crop.crop) {
            sky = false;
            break;
        }
    }
    let soil = soil && !super::industry::dirty_soil(world,p-IVec3::Y);
    let light = if sky { 15 } else { torch };
    if let Some(wet) = intake {
        return Some(Conditions { light, wet, soil });
    }
    if world.farming.mist.iter().any(|&(m, r)| (m - p).length_squared() <= r * r) {
        return Some(Conditions { light, wet: true, soil });
    }
    let mut unknown = false;
    let mut wet = false;
    let scale = world.voxel_scale.meters();
    let radius = crop.irrigation_metres.map_or(WATER_RADIUS, |m| (m / scale).floor() as i32);
    let depth = crop.irrigation_metres.map_or(1, |_| (1.0 / scale).round() as i32);
    'scan: for dy in -depth..=0 {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if crop.irrigation_metres.is_some() && dx * dx + dz * dz > radius * radius {
                    continue;
                }
                let at = p + IVec3::new(dx, dy, dz);
                match known(world, at) {
                    Some(B::Water) if crop.irrigation_metres.is_none() || world.water_volume(at) > 0 => {
                        wet = true;
                        break 'scan;
                    }
                    None => unknown = true,
                    _ => {}
                }
            }
        }
    }
    if unknown && !wet {
        return None;
    }
    Some(Conditions { light, wet, soil })
}

/// True when every support cell is known and none can hold the plant: its soil,
/// log or ceiling was dug out. A support swapped for another solid block only stalls growth.
fn unsupported(world: &World, p: IVec3, crop: &table::Crop) -> bool {
    crop.planting.supports().iter().all(|&d| {
        known(world, p + d).is_some_and(|b| !crop.soil.contains(&b) && !b.is_solid() && b != crop.crop)
    })
}

fn service(world: &mut World, k: Key, now: u64, changed: &mut Vec<IVec3>) {
    let p = IVec3::new(k.0, k.1, k.2);
    // An unloaded plot keeps its last evaluated interval and catches up on return.
    if !resident(world, p) {
        return;
    }
    let Some(mut plot) = world.farming.plots.get(&k).copied() else { return };
    let Some(crop) = table::get(plot.crop) else {
        world.farming.plots.remove(&k);
        return;
    };
    let block = world.get(p);
    if block != crop.sprout && block != crop.crop {
        world.farming.plots.remove(&k);
        return;
    }
    if unsupported(world, p, crop) {
        let room = world.adventure.dirty_voxels.len() + changed.len() < DIRTY_LIMIT;
        if room && let Ok(commit) = effects::replace(world, p, block, forage::vacated(block)) {
            commit.publish_voxel(world, changed);
            world.farming.plots.remove(&k);
            if world.inventory.dropped.len() < MAX_WILD_DROPS {
                let pos = (p.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters();
                world.inventory.dropped.push(super::inventory::DroppedStack { pos, stack: ItemStack::new(Item::Block(crop.crop), 1) });
            }
        }
        return;
    }
    if plot.stage < crop.mature() && now >= plot.next_roll(crop) {
        if crop.irrigation_metres.is_some() {
            // Sample current irrigation at most once; never replay old rolls
            // using the water that happens to be present when a plot reloads.
            let due = now.saturating_sub(plot.planted) / u64::from(crop.roll_ticks.max(1));
            plot.rolled = plot.rolled.max(u32::try_from(due.saturating_sub(1)).unwrap_or(u32::MAX));
        }
        let Some(c) = conditions(world, p, crop) else { return };
        stage::advance(&mut plot, crop, world.seed, p, now, c, ROLLS_PER_SERVICE);
        world.farming.plots.insert(k, plot);
    }
    if crop.tree.is_some() {
        let room = DIRTY_LIMIT.saturating_sub(world.adventure.dirty_voxels.len() + changed.len());
        if plot.stage >= crop.mature() && block == crop.sprout && sapling::grow(world, p, crop, room, changed) {
            world.farming.plots.remove(&k);
        }
        return;
    }
    if let Some(stalk) = crop.stalk {
        let room = DIRTY_LIMIT.saturating_sub(world.adventure.dirty_voxels.len() + changed.len());
        if plot.stage >= crop.mature() && room > 0 && grow_stalk(world, p, crop, stalk, changed) {
            // Fewer rolls per cell at finer scales keeps growth in metres per day.
            let per_cell = (f32::from(crop.mature()) * world.voxel_scale.meters()).round().max(1.0) as u8;
            plot.stage = plot.stage.saturating_sub(per_cell);
            world.farming.plots.insert(k, plot);
        }
        return;
    }
    if crop.gourd {
        let room = DIRTY_LIMIT.saturating_sub(world.adventure.dirty_voxels.len() + changed.len());
        if plot.stage >= crop.mature() && room > 0 && forage::set_gourd(world, p, crop, changed) {
            world.farming.plots.insert(k, Plot::new(plot.crop, now));
        }
        return;
    }
    // The visible swap may wait for dirty-list room; the stage never does.
    if plot.stage >= crop.mature() && block == crop.sprout
        && world.adventure.dirty_voxels.len() + changed.len() < DIRTY_LIMIT
        && let Ok(commit) = effects::replace(world, p, crop.sprout, crop.crop)
    {
        commit.publish_voxel(world, changed);
    }
}

/// Turns the shoot into stalk, or adds one stalk cell on top. False when full or blocked.
fn grow_stalk(world: &mut World, p: IVec3, crop: &table::Crop, stalk: table::Stalk, changed: &mut Vec<IVec3>) -> bool {
    let (from, at) = if world.get(p) == crop.sprout {
        (crop.sprout, p)
    } else {
        let cells = world.sample_config().blocks_for_height(stalk.max_metres).max(1);
        let mut top = p;
        loop {
            if top.y - p.y + 1 >= cells {
                return false;
            }
            let above = top + IVec3::Y;
            if !resident(world, above) { return false; }
            if world.get(above) != crop.crop { break; }
            top = above;
        }
        let at = top + IVec3::Y;
        let medium = crop.planting.medium();
        if !resident(world, at) || world.get(at) != medium {
            return false;
        }
        // Kelp stops one cell under the surface so its top stays wet.
        if medium == B::Water && (!resident(world, at + IVec3::Y) || world.get(at + IVec3::Y) != B::Water) {
            return false;
        }
        (medium, at)
    };
    if crop.irrigation_metres.is_some()
        && !conditions(world, p, crop).is_some_and(|c| stage::chance(crop, c) > 0)
    {
        return false;
    }
    let Ok(commit) = effects::replace(world, at, from, crop.crop) else { return false };
    commit.publish_voxel(world, changed);
    true
}

/// Use action with the held item: shear or feed a grazer, plant a seed, or till ground.
pub fn interact(
    world: &mut World,
    hit: Option<(IVec3, IVec3)>,
    eye_m: Vec3,
    forward: Vec3,
    slot: usize,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    if super::adventure::wildlife::shear(world, eye_m, forward, slot)
        || super::adventure::wildlife::feed(world, eye_m, forward, slot) {
        return true;
    }
    let scale = world.voxel_scale.meters();
    let in_reach = |cell: IVec3| ((cell.as_vec3() + Vec3::splat(0.5)) * scale).distance(eye_m) <= REACH_METRES;
    if let Some((cell, _)) = hit
        && in_reach(cell)
        && forage::pick(world, cell, blocks, chunks)
    {
        return true;
    }
    let Some(item) = world.inventory.hotbar.get(slot).copied().flatten().map(|s| s.item) else {
        return false;
    };
    let eat = |world: &mut World| matches!(item, Item::Block(b) if forage::eat(world, slot, b));
    let Some((cell, place)) = hit.filter(|&(cell, _)| in_reach(cell)) else { return eat(world) };
    if item == sapling::BIOMASS && world.farming.plots.contains_key(&key(cell)) {
        return sapling::fertilize(world, cell, slot);
    }
    if let Item::Block(seed) = item
        && let Some((index, crop)) = table::by_seed(seed)
        && let Some(at) = forage::target(world, crop, cell, place)
    {
        return plant(world, at, index, crop, slot, blocks, chunks);
    }
    if place != cell + IVec3::Y {
        return eat(world);
    }
    let can_till = item == Item::HOE
        || super::mining::Tool::of(item).is_some_and(|t| t.class == Some(super::mining::ToolClass::Dig));
    if can_till && TILLABLE.contains(&world.get(cell)) && world.get(place).is_empty()
        && let Ok(commit) = effects::replace(world, cell, world.get(cell), BED)
    {
        commit.publish(world, blocks, chunks);
        return true;
    }
    eat(world)
}

fn plant(
    world: &mut World,
    p: IVec3,
    index: u8,
    crop: &table::Crop,
    slot: usize,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    if world.farming.plots.len() >= MAX_PLOTS {
        world.expedition.say(format!("Plots Full: {MAX_PLOTS}/{MAX_PLOTS}"));
        return true;
    }
    let Ok(commit) = effects::replace(world, p, crop.planting.medium(), crop.sprout) else { return true };
    commit.publish(world, blocks, chunks);
    world.inventory.consume_from_hotbar(slot, Item::Block(crop.crop));
    world.sync_from_inventory();
    let now = world.farming.clock;
    world.farming.plots.insert(key(p), Plot::new(index, now));
    true
}

/// Cuts a column from the top down so a deferred edit cannot leave floating stalks.
/// A cut above the base leaves its plot in place for regrowth.
pub(crate) fn cut_stalk(
    world: &mut World,
    p: IVec3,
    collect: bool,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
    mut history: Option<&mut super::edit_history::EditHistory>,
) -> Option<bool> {
    let first = world.get(p);
    let stalk = match first {
        B::Bamboo | B::BambooShoot => B::Bamboo,
        B::Sugarcane | B::SugarcaneShoot => B::Sugarcane,
        B::Kelp => B::Kelp,
        B::Cactus => B::Cactus,
        _ => return None,
    };
    let empty = forage::vacated(stalk);
    let mut top = p;
    let ceiling = world.sample_config().max_world_y();
    while top.y + 1 < ceiling && world.get(top + IVec3::Y) == stalk {
        top += IVec3::Y;
    }
    let item = Item::Block(stalk);
    let mut complete = false;
    for y in (p.y..=top.y).rev() {
        let cell = IVec3::new(p.x, y, p.z);
        if collect && world.inventory.dropped.len() >= 1024
            && !world.inventory.can_insert(ItemStack::new(item, 1))
        {
            world.inventory.notify_overflow(item);
            break;
        }
        if !world.admit_edit_chunk(chunk_of(cell)) {
            break;
        }
        let block = if cell == p { first } else { stalk };
        let Ok(commit) = effects::replace(world, cell, block, empty) else { break };
        commit.publish(world, blocks, chunks);
        world.farming.plots.remove(&key(cell));
        if collect {
            super::expedition::give_or_drop(world, cell, item, 1);
            world.stats.add(super::stats::Stat::Mined, 1);
            super::progression::on_harvest(world, block);
        }
        if let Some(history) = history.as_deref_mut() {
            history.record(world, cell, block, empty);
        }
        complete = cell == p;
    }
    if collect {
        world.sync_from_inventory();
    }
    Some(complete)
}

/// Breaks a planted cell. `None` when `p` holds no plot; otherwise whether it broke.
pub fn harvest(world: &mut World, p: IVec3, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> Option<bool> {
    let plot = world.farming.plots.get(&key(p)).copied()?;
    let block = world.get(p);
    let Some(crop) = table::get(plot.crop).filter(|c| block == c.sprout || block == c.crop) else {
        world.farming.plots.remove(&key(p));
        return None;
    };
    let mature = plot.stage >= crop.mature() && block == crop.crop;
    // An unripe tree bud is foliage: it breaks without paying a fruit.
    if crop.planting == table::Planting::Grafted && !mature {
        let Ok(commit) = effects::replace(world, p, block, B::Air) else { return Some(false) };
        commit.publish(world, blocks, chunks);
        world.farming.plots.remove(&key(p));
        return Some(true);
    }
    let produce = crop.produce.filter(|_| mature).map(Item::Resource);
    let item = produce.unwrap_or(Item::Block(crop.crop));
    let studied_harvest = block == crop.crop
        && plot.stage >= crop.mature()
        && super::progression::study::food_crop(block)
        && super::progression::study::available(world);
    let count = if crop.gourd { 1 } else { harvest_count(world.seed, p, &plot, crop) }
        + u32::from(studied_harvest && world.progression.horticulture);
    let seed = ItemStack::new(Item::Block(crop.crop), u32::from(produce.is_some()));
    if !world.inventory.can_insert(ItemStack::new(item, count)) {
        world.inventory.notify_overflow(item);
        world.expedition.say("Inventory Full");
        return Some(false);
    }
    let Ok(commit) = effects::replace(world, p, block, forage::vacated(block)) else { return Some(false) };
    commit.publish(world, blocks, chunks);
    world.farming.plots.remove(&key(p));
    for stack in [ItemStack::new(item, count), seed] {
        if stack.count == 0 {
            continue;
        }
        let (inserted, _) = world.inventory.try_insert(stack);
        if inserted > 0 {
            world.inventory.notify_pickup(stack.item, inserted);
        }
    }
    world.sync_from_inventory();
    if studied_harvest {
        super::progression::grant_xp(world, 3);
    }
    Some(true)
}

/// Produce plus seed for a mature plot; an unripe one returns its seed.
pub fn harvest_count(seed: u32, p: IVec3, plot: &Plot, crop: &table::Crop) -> u32 {
    if plot.stage < crop.mature() {
        return 1;
    }
    crop.yield_min + stage::roll(seed, p, plot.planted ^ 0x7969_656c) % (crop.yield_max - crop.yield_min + 1)
}

/// Shows an unripe plot's state instead of uprooting it. True when handled.
pub fn inspect(world: &mut World, p: IVec3) -> bool {
    let Some(plot) = world.farming.plots.get(&key(p)).copied() else { return false };
    let Some(crop) = table::get(plot.crop) else { return false };
    if plot.stage >= crop.mature() || world.get(p) != crop.sprout {
        return false;
    }
    let misted = world.farming.mist.iter().any(|&(m, r)| (m - p).length_squared() <= r * r);
    let state = conditions(world, p, crop).map_or_else(String::new, |c| format!(
        " · Light: {} · Water: {}", c.light,
        if misted { "Mist" } else if c.wet { "Near" } else { "None" }));
    world.expedition.say(format!("{} · Stage {}/{}{state}", crop.crop.display_name(), plot.stage + 1, crop.stages));
    true
}

/// A seed from a broken wild crop block, rolled per cell and scaled by cell volume.
/// Only the generator's own block pays, once per cell, so placed blocks never do.
pub fn wild_seed(world: &mut World, p: IVec3, block: B) -> Option<Item> {
    let cfg = world.sample_config();
    let metres = world.voxel_scale.meters();
    wild_pay(&mut world.farming, world.seed, metres, p, block, || super::sample_block(&cfg, p))
}

/// `generated` is the block worldgen put at `p`; it is sampled only for a winning roll.
fn wild_pay(state: &mut State, seed: u32, metres: f32, p: IVec3, block: B, generated: impl FnOnce() -> B) -> Option<Item> {
    let (_, crop) = table::by_seed(block)?;
    let pays = wild_roll(seed, metres, p, crop)
        && state.wild_paid.len() < MAX_WILD_PAID
        && !state.wild_paid.contains(&key(p))
        && generated() == block;
    pays.then(|| {
        state.wild_paid.insert(key(p));
        Item::Block(block)
    })
}

fn wild_roll(seed: u32, metres: f32, p: IVec3, crop: &table::Crop) -> bool {
    let threshold = (crop.wild_seed_per_mille as f32 * metres.powi(3)).round().max(1.0) as u32;
    stage::roll(seed, p, 0x5eed) % 1000 < threshold
}
