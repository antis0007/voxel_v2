//! Storage scanner: nearby chests and storage machines read as one indexed inventory.
//!
//! Sources are chests with stored records, the chest the scan starts from, and
//! storage machines whose slots exist. Loot caches and machine buffers are never
//! sources. Every move happens inside one `&mut World` call, so a container
//! broken earlier simply fails validation and is skipped.
use super::inventory::{sorting, stacks, Item, ItemStack};
use super::{block_data, containers, factory, BlockId, World};
use glam::{IVec3, Vec3};

pub mod ops;
pub mod policy;
pub mod view;
pub mod commands;
pub mod crafting;
pub mod review;
#[cfg(test)]
mod tests;

pub use ops::Reach;

pub const RADIUS_M: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Chest(IVec3),
    /// Stable world position of a storage cache, independent of vector order.
    Cache(IVec3),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Container {
    pub source: Source,
    pub pos: IVec3,
    pub distance_m: f32,
}

/// One item's total across the scanned containers.
#[derive(Clone, Debug, PartialEq)]
pub struct Stock {
    pub item: Item,
    pub total: u64,
    /// Quantity remaining after each source's reserve is protected.
    pub available: u64,
    /// Container index and units held there, nearest first.
    pub sources: Vec<(usize, u32)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Index {
    pub containers: Vec<Container>,
    pub stock: Vec<Stock>,
    pub free_slots: usize,
}

impl Index {
    /// Stock whose name or category contains `query`, ignoring case.
    pub fn search<'a>(&'a self, query: &str) -> impl Iterator<Item = &'a Stock> + 'a {
        let query = query.trim().to_lowercase();
        self.stock.iter().filter(move |s| {
            let text = format!("{} {}", s.item.name(), s.item.category()).to_lowercase();
            query.split_whitespace().all(|term| text.contains(term))
        })
    }
}

fn storage_cache(world: &World, cache: &containers::Cache) -> Option<IVec3> {
    let [p] = cache.cells.as_slice() else { return None };
    (factory::Kind::from_block(world.get(*p)) == Some(factory::Kind::Storage)).then_some(*p)
}

fn valid(world: &World, source: Source) -> bool {
    let p = source.pos();
    if !world.chunks.contains_key(&super::chunk_coord_from_world_block(p))
        || world.ca_reservations.contains(&p) || !policy::get(world, p).enabled { return false; }
    match source {
        Source::Chest(p) => world.get(p).chest_tier().is_some() && world.block_data.can_store_chest(p),
        Source::Cache(p) => cache_id(world, p).is_some(),
    }
}

impl Source {
    pub fn pos(self) -> IVec3 { match self { Self::Chest(p) | Self::Cache(p) => p } }
}

fn cache_id(world: &World, pos: IVec3) -> Option<usize> {
    world.caches.iter().position(|c| storage_cache(world, c) == Some(pos))
}

/// Containers within `radius_m` of `centre_m`, nearest first; `anchor` is always included.
pub fn discover(world: &World, centre_m: Vec3, radius_m: f32, anchor: Option<IVec3>) -> Vec<Container> {
    let m = world.voxel_scale.meters();
    let distance = |p: IVec3| ((p.as_vec3() + Vec3::splat(0.5)) * m).distance(centre_m);
    let mut found: Vec<Container> = Vec::new();
    let mut add = |source, pos: IVec3, force: bool| {
        let d = distance(pos);
        if (force || d <= radius_m) && valid(world, source) && !found.iter().any(|c| c.source == source) {
            found.push(Container { source, pos, distance_m: d });
        }
    };
    if let Some(p) = anchor.filter(|&p| world.get(p).chest_tier().is_some()) {
        add(Source::Chest(p), p, true);
    }
    // Records sort by x first, so one range covers the slab that can be in reach.
    let (cx, r) = (centre_m.x / m, radius_m / m);
    let low = ((cx - r).floor() as i32, i32::MIN, i32::MIN);
    let high = ((cx + r).ceil() as i32, i32::MAX, i32::MAX);
    for (&(x, y, z), entry) in world.block_data.entries.range(low..=high) {
        let p = IVec3::new(x, y, z);
        if matches!(entry, block_data::Entry::Chest(_)) && world.get(p).chest_tier().is_some() {
            add(Source::Chest(p), p, false);
        }
    }
    for cache in &world.caches {
        if let Some(p) = storage_cache(world, cache) {
            add(Source::Cache(p), p, false);
        }
    }
    found.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m).then(a.pos.to_array().cmp(&b.pos.to_array())));
    found
}

fn slots_of(world: &World, source: Source) -> ops::Inventory {
    match source {
        Source::Chest(p) => world.block_data.chest(p),
        Source::Cache(p) => {
            let Some(i) = cache_id(world, p) else { return vec![None; ops::SLOTS] };
            match world.inventory.container {
            Some((open, view)) if open == i => view.to_vec(),
            _ => world.caches.get(i).map_or_else(|| vec![None; ops::SLOTS], |c| c.slots.to_vec()),
            }
        },
    }
}

/// Builds the aggregated view; read-only.
pub fn index(world: &World, containers: &[Container]) -> Index {
    let mut stock: Vec<Stock> = Vec::new();
    let mut lookup = std::collections::HashMap::new();
    let mut free_slots = 0;
    for (ci, c) in containers.iter().enumerate() {
        if !valid(world, c.source) { continue; }
        for slot in slots_of(world, c.source) {
            let Some(s) = slot else {
                free_slots += 1;
                continue;
            };
            let i = *lookup.entry(s.item.encode_u32()).or_insert_with(|| {
                stock.push(Stock { item: s.item, total: 0, available: 0, sources: Vec::new() });
                stock.len() - 1
            });
            let entry = &mut stock[i];
            entry.total += u64::from(s.count);
            match entry.sources.last_mut() {
                Some((last, n)) if *last == ci => *n += s.count,
                _ => entry.sources.push((ci, s.count)),
            }
        }
    }
    for s in &mut stock {
        s.available = s.sources.iter().map(|&(i, n)| u64::from(n.saturating_sub(policy::get(world, containers[i].pos).reserve))).sum();
    }
    stock.sort_by(|a, b| sorting::sort_key(a.item).cmp(&sorting::sort_key(b.item)));
    Index { containers: containers.to_vec(), stock, free_slots }
}

/// Still-valid containers and their slots; commits the open cache view first.
fn load(world: &mut World, containers: &[Container]) -> (Vec<Source>, Vec<ops::Inventory>) {
    containers::flush(world);
    let mut seen = std::collections::HashSet::new();
    let sources: Vec<Source> = containers.iter().map(|c| c.source).filter(|&s| {
        if !valid(world, s) || !seen.insert(s) { return false; }
        // Reserve each missing record now, so later sources see the remaining
        // capacity before any player or container inventory is debited.
        match s {
            Source::Chest(p) => world.block_data.ensure_chest(p),
            Source::Cache(_) => true,
        }
    }).collect();
    let slots = sources.iter().map(|&s| slots_of(world, s)).collect();
    (sources, slots)
}

fn store(world: &mut World, sources: &[Source], slots: &[ops::Inventory]) {
    for (&source, s) in sources.iter().zip(slots) {
        if slots_of(world, source) == *s { continue; }
        match source {
            Source::Chest(p) => world.block_data.set_chest(p, s.clone()),
            Source::Cache(p) => {
                let Some(i) = cache_id(world, p) else { continue };
                world.caches[i].slots = s.as_slice().try_into().expect("storage cache capacity");
                if let Some((open, view)) = world.inventory.container.as_mut()
                    && *open == i {
                    *view = s.as_slice().try_into().expect("storage cache capacity");
                }
            }
        }
    }
}

/// Moves up to `amount` of `item` into the hotbar and pack; returns units moved.
pub fn withdraw(world: &mut World, containers: &[Container], item: Item, amount: u32) -> u32 {
    let (sources, mut slots) = load(world, containers);
    let fit = world.inventory.remaining_capacity_for(item).min(amount);
    let policies: Vec<_> = sources.iter().map(|s| policy::get(world, s.pos())).collect();
    let taken = policy::take(&mut slots, &policies, item, fit);
    let (_, left) = world.inventory.try_insert(ItemStack { item, count: taken });
    debug_assert_eq!(left, 0, "capacity was checked");
    if left > 0 {
        let mut back = ItemStack { item, count: left };
        ops::put(&mut slots, &mut back, u32::MAX, Reach::Anywhere);
    }
    store(world, &sources, &slots);
    taken - left
}

/// Moves pack stacks into storage; the hotbar, equipment and stateful items stay.
pub fn deposit(world: &mut World, containers: &[Container], reach: Reach) -> u32 {
    let (sources, mut slots) = load(world, containers);
    let mut moved = 0;
    let policies: Vec<_> = sources.iter().map(|s| policy::get(world, s.pos())).collect();
    for i in 0..world.inventory.pack.len() {
        if world.inventory.is_protected(super::inventory::SlotAddr::Pack(i)) { continue; }
        let Some(mut stack) = world.inventory.pack[i] else { continue };
        if stacks::class(stack.item) == stacks::StackClass::Unique {
            continue;
        }
        let (before, max) = (stack.count, world.inventory.stack_limit(stack.item));
        let allowed = before.min(world.inventory.count_item(stack.item).saturating_sub(world.inventory.minimum(stack.item)));
        let mut transfer = ItemStack { item: stack.item, count: allowed };
        policy::deposit(&mut slots, &policies, &mut transfer, max, reach);
        stack.count -= allowed - transfer.count;
        moved += before - stack.count;
        world.inventory.pack[i] = (stack.count > 0).then_some(stack);
    }
    store(world, &sources, &slots);
    moved
}

/// Tops hotbar stacks up to their limit from storage; returns units moved.
pub fn restock(world: &mut World, containers: &[Container]) -> u32 {
    let (sources, mut slots) = load(world, containers);
    let policies: Vec<_> = sources.iter().map(|s| policy::get(world, s.pos())).collect();
    let mut moved = 0;
    for i in 0..world.inventory.hotbar.len() {
        if world.inventory.is_protected(super::inventory::SlotAddr::Hotbar(i)) { continue; }
        let Some(mut stack) = world.inventory.hotbar[i] else { continue };
        let limit = world.inventory.stack_limit(stack.item);
        if limit <= 1 || stack.count >= limit {
            continue;
        }
        let n = policy::take(&mut slots, &policies, stack.item, limit - stack.count);
        stack.count += n;
        moved += n;
        world.inventory.hotbar[i] = Some(stack);
    }
    store(world, &sources, &slots);
    moved
}

/// Moves everything that fits into the hotbar and pack; returns units moved.
pub fn take_all(world: &mut World, containers: &[Container]) -> u32 {
    let items: Vec<Item> = index(world, containers).stock.iter().map(|s| s.item).collect();
    items.into_iter().map(|item| withdraw(world, containers, item, u32::MAX)).sum()
}

/// Merges and orders each container's own slots; nothing crosses containers.
pub fn sort(world: &mut World, containers: &[Container]) {
    let (sources, mut slots) = load(world, containers);
    let inventory = &world.inventory;
    for s in &mut slots {
        sorting::sort_slots(s, |item| inventory.stack_limit(item));
    }
    store(world, &sources, &slots);
}

/// The single chest at `p` as a scan list, for per-chest commands.
pub fn chest(p: IVec3) -> [Container; 1] {
    [Container { source: Source::Chest(p), pos: p, distance_m: 0.0 }]
}

/// Slots that `consolidate` would free, without moving anything.
pub fn consolidate_preview(world: &World, containers: &[Container]) -> usize {
    let sources: Vec<_> = containers.iter().filter(|c| valid(world, c.source)).map(|c| c.source).collect();
    let mut slots: Vec<_> = sources.iter().map(|&s| slots_of(world, s)).collect();
    organize(world, &sources, &mut slots)
}

/// Merges duplicate partial stacks across containers; returns slots freed.
pub fn consolidate(world: &mut World, containers: &[Container]) -> usize {
    let (sources, mut slots) = load(world, containers);
    let freed = organize(world, &sources, &mut slots);
    store(world, &sources, &slots);
    freed
}

fn organize(world: &World, sources: &[Source], slots: &mut [ops::Inventory]) -> usize {
    let policies: Vec<_> = sources.iter().map(|s| policy::get(world, s.pos())).collect();
    // Default storage keeps its established minimal-movement algorithm.
    if policies.iter().all(|p| *p == policy::Policy::default()) {
        return ops::consolidate(slots, |i| world.inventory.stack_limit(i));
    }
    let before = slots.iter().flatten().flatten().count();
    let mut items = std::collections::BTreeMap::new();
    for s in slots.iter().flatten().flatten() { items.insert(s.item.encode_u32(), s.item); }
    for item in items.into_values() {
        let snapshot = slots.to_vec();
        let n = policy::take(slots, &policies, item, u32::MAX);
        let mut stack = ItemStack { item, count: n };
        policy::deposit(slots, &policies, &mut stack, world.inventory.stack_limit(item), Reach::Smart);
        if stack.count > 0 { slots.clone_from_slice(&snapshot); }
    }
    before.saturating_sub(slots.iter().flatten().flatten().count())
}
