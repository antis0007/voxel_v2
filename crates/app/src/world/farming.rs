//! Planted crops on tilled beds: interval growth, catch-up after unload, harvest.
//! Plots and the farming clock persist in `sidecar::FILE`, not the save body.
use super::{BlockId as B, World, chunk_coord_from_world_block as chunk_of, effects, inventory::{Item, ItemStack}};
use engine_core::Budget;
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet, HashSet};
pub(crate) mod sidecar;
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
const TILLABLE: [B; 3] = [B::Grass, B::Dirt, B::SnowGrass];
const BED: B = B::FungalSoil;
type Key = (i32, i32, i32);

#[derive(Clone, Debug, Default)]
pub struct State {
    /// Fixed steps counted by `tick`; plot ages and herd timers read it.
    pub clock: u64,
    pub(crate) plots: BTreeMap<Key, Plot>,
    /// Generated crop cells that already paid a wild seed; a block put back there pays nothing.
    pub(crate) wild_paid: BTreeSet<Key>,
    cursor: Option<Key>,
}

fn key(p: IVec3) -> Key {
    (p.x, p.y, p.z)
}

/// Advances the clock one fixed step and services a bounded slice of plots.
pub fn tick(world: &mut World, budget: Budget) {
    world.farming.clock += 1;
    if world.farming.plots.is_empty() || world.ca_view.is_some() {
        return;
    }
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
        service(world, k, now, &mut changed);
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
    let soil = crop.soil.contains(&known(world, p - IVec3::Y)?);
    let torch = world.chunks.get(&chunk_of(p))
        .map_or(0, |c| c.light[super::ChunkData::local_index(p.x, p.y, p.z)] & 0x0F);
    let height = world.sample_config().blocks_for_height(SKY_METRES).max(1);
    let mut sky = true;
    for y in 1..=height {
        if !known(world, p + IVec3::Y * y)?.is_empty() {
            sky = false;
            break;
        }
    }
    let mut unknown = false;
    let mut wet = false;
    'scan: for dy in -1..=0 {
        for dz in -WATER_RADIUS..=WATER_RADIUS {
            for dx in -WATER_RADIUS..=WATER_RADIUS {
                match known(world, p + IVec3::new(dx, dy, dz)) {
                    Some(B::Water) => {
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
    Some(Conditions { light: if sky { 15 } else { torch }, wet, soil })
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
    if plot.stage < crop.mature() && now >= plot.next_roll(crop) {
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
    // The visible swap may wait for dirty-list room; the stage never does.
    if plot.stage >= crop.mature() && block == crop.sprout
        && world.adventure.dirty_voxels.len() + changed.len() < DIRTY_LIMIT
        && let Ok(commit) = effects::replace(world, p, crop.sprout, crop.crop)
    {
        commit.publish_voxel(world, changed);
    }
}

/// Use action with the held item: feed a grazer, plant a seed, or till ground.
pub fn interact(
    world: &mut World,
    hit: Option<(IVec3, IVec3)>,
    eye_m: Vec3,
    forward: Vec3,
    slot: usize,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    if super::adventure::wildlife::feed(world, eye_m, forward, slot) {
        return true;
    }
    let Some(item) = world.inventory.hotbar.get(slot).copied().flatten().map(|s| s.item) else {
        return false;
    };
    let Some((cell, place)) = hit else { return false };
    let scale = world.voxel_scale.meters();
    if ((cell.as_vec3() + Vec3::splat(0.5)) * scale).distance(eye_m) > REACH_METRES {
        return false;
    }
    if item == sapling::BIOMASS && world.farming.plots.contains_key(&key(cell)) {
        return sapling::fertilize(world, cell, slot);
    }
    if place != cell + IVec3::Y {
        return false;
    }
    if let Item::Block(seed) = item
        && let Some((index, crop)) = table::by_seed(seed)
        && crop.soil.contains(&world.get(cell))
        && world.get(place) == B::Air
    {
        return plant(world, place, index, crop, slot, blocks, chunks);
    }
    let digging = super::mining::Tool::of(item).is_some_and(|t| t.class == Some(super::mining::ToolClass::Dig));
    if digging && TILLABLE.contains(&world.get(cell)) && world.get(place).is_empty()
        && let Ok(commit) = effects::replace(world, cell, world.get(cell), BED)
    {
        commit.publish(world, blocks, chunks);
        return true;
    }
    false
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
    let Ok(commit) = effects::replace(world, p, B::Air, crop.sprout) else { return true };
    commit.publish(world, blocks, chunks);
    world.inventory.consume_from_hotbar(slot, Item::Block(crop.crop));
    world.sync_from_inventory();
    let now = world.farming.clock;
    world.farming.plots.insert(key(p), Plot::new(index, now));
    true
}

/// Breaks a planted cell. `None` when `p` holds no plot; otherwise whether it broke.
pub fn harvest(world: &mut World, p: IVec3, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> Option<bool> {
    let plot = world.farming.plots.get(&key(p)).copied()?;
    let block = world.get(p);
    let Some(crop) = table::get(plot.crop).filter(|c| block == c.sprout || block == c.crop) else {
        world.farming.plots.remove(&key(p));
        return None;
    };
    let item = Item::Block(crop.crop);
    let count = harvest_count(world.seed, p, &plot, crop);
    if !world.inventory.can_insert(ItemStack::new(item, count)) {
        world.inventory.notify_overflow(item);
        world.expedition.say("Inventory Full");
        return Some(false);
    }
    let Ok(commit) = effects::replace(world, p, block, B::Air) else { return Some(false) };
    commit.publish(world, blocks, chunks);
    world.farming.plots.remove(&key(p));
    let (inserted, _) = world.inventory.try_insert(ItemStack::new(item, count));
    if inserted > 0 {
        world.inventory.notify_pickup(item, inserted);
    }
    world.sync_from_inventory();
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
    let state = conditions(world, p, crop).map_or_else(String::new, |c| format!(
        " · Light: {} · Water: {}", c.light, if c.wet { "Near" } else { "None" }));
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
