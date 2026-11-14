//! Persistent cache contents. Opening a cache never destroys its blocks.
use super::{World, BlockId, inventory::{Item, ItemStack, SurvivalInventory}};
use glam::IVec3;
use std::collections::{HashSet, VecDeque};

pub const SLOTS: usize = 27;
pub const LIMIT: usize = 4096;
#[derive(Clone)]
pub struct Cache {
    pub cells: Vec<IVec3>,
    pub slots: [Option<ItemStack>; SLOTS],
}

pub fn is_cache(block: BlockId) -> bool {
    matches!(block, BlockId::SalvageCrate | BlockId::SurveyCache | BlockId::CultivatorCache)
}

pub fn flush(world: &mut World) {
    // Online edits arrive as ordered semantic commands. A player's displayed
    // chest copy must never overwrite a newer command from another player.
    #[cfg(feature = "multiplayer")]
    if world.network_host {
        refresh(world);
        return;
    }
    commit(world);
}

pub(crate) fn commit(world: &mut World) {
    if let Some((id, slots)) = world.inventory.container.as_ref()
        && let Some(cache) = world.caches.get_mut(*id) {
        cache.slots = *slots;
    }
}

pub(crate) fn refresh(world: &mut World) {
    if let Some((id, slots)) = world.inventory.container.as_mut() {
        if let Some(cache) = world.caches.get(*id) {
            *slots = cache.slots;
        } else {
            world.inventory.container = None;
        }
    }
}

/// Validate the host-held open chest against the player's current body.
pub(crate) fn reachable(world: &World, player: &crate::player::Player) -> bool {
    let Some((id, _)) = world.inventory.container.as_ref() else { return false; };
    let Some(cache) = world.caches.get(*id) else { return false; };
    let scale = world.voxel_scale.meters();
    let eye = player.eye(world.voxel_scale) * scale;
    eye.is_finite() && cache.cells.iter().any(|&cell| {
        let block = world.get(cell);
        (is_cache(block) || super::factory::Kind::from_block(block) == Some(super::factory::Kind::Storage))
            && eye.distance_squared((cell.as_vec3() + glam::Vec3::splat(0.5)) * scale) <= 36.0
            && {
                let delta = (cell.as_vec3() + glam::Vec3::splat(0.5)) * scale - eye;
                super::raycast_block(world, eye / scale, delta.normalize_or_zero(), (delta.length() + 0.05) / scale)
                    .is_some_and(|(hit, _)| hit == cell || (super::factory::Kind::from_block(block) == Some(super::factory::Kind::Storage)
                        && super::factory::anchor(hit, super::factory::step(world)) == cell))
            }
    })
}

fn locate(world: &mut World, pos: IVec3) -> Option<usize> {
    if let Some(id) = world.caches.iter().position(|c| c.cells.contains(&pos)) { return Some(id); }
    if world.caches.len() >= LIMIT { return None; }
    let block = world.get(pos);
    if super::factory::Kind::from_block(block) == Some(super::factory::Kind::Storage) {
        let p = super::factory::anchor(pos, super::factory::step(world));
        if let Some(id) = world.caches.iter().position(|c| c.cells == [p]) { return Some(id); }
        let id = world.caches.len();
        world.caches.push(Cache { cells: vec![p], slots: [None; SLOTS] });
        return Some(id);
    }
    if !is_cache(block) { return None; }
    let cfg = world.sample_config();
    if block == BlockId::SurveyCache && engine_worldgen::sites::workshop_supply_cache(&cfg, pos) {
        let mut slots = [None; SLOTS];
        slots[0] = Some(ItemStack::new(Item::Block(BlockId::Smelter), 1));
        slots[1] = Some(ItemStack::new(Item::Resource(0), 2)); // Copper ore
        slots[2] = Some(ItemStack::new(Item::Resource(3), 1)); // Coal
        slots[3] = Some(ItemStack::new(Item::Bucket(0), 1));
        slots[4] = Some(ItemStack::new(Item::Resource(4), 1)); // Lumen fibre for the copper sword
        let id = world.caches.len();
        world.caches.push(Cache { cells: vec![pos], slots });
        return Some(id);
    }
    if block == BlockId::SurveyCache && cfg.world_type.realm() == Some(engine_voxel::RealmKind::Lightless) {
        // A lost miners' camp: what they carried down, rolled from the cell.
        use engine_core::hash::hash_u32;
        // One chest per connected cache, at every voxel scale.
        let mut cells = Vec::new();
        let mut queue = VecDeque::from([pos]);
        let mut seen = HashSet::new();
        while let Some(p) = queue.pop_front() {
            if cells.len() >= 512 || !seen.insert(p) || world.get(p) != block
                || (p - pos).as_vec3().length() * world.voxel_scale.meters() > 1.8 { continue; }
            cells.push(p);
            for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] { queue.push_back(p + d); }
        }
        let key = cells.iter().copied().min_by_key(|p| p.to_array()).unwrap_or(pos);
        let h = hash_u32(world.seed ^ hash_u32(key.x as u32) ^ hash_u32(key.y as u32).rotate_left(11) ^ (key.z as u32).rotate_left(22));
        let roll = |salt: u32, lo: u32, hi: u32| lo + hash_u32(h ^ salt) % (hi - lo + 1);
        let mut slots = [None; SLOTS];
        let mut put = |i: usize, item: Item, n: u32| if n > 0 && i < SLOTS { slots[i] = Some(ItemStack::new(item, n)); };
        put(0, Item::Block(BlockId::Torch), roll(1, 6, 14));
        put(1, Item::Resource(8), roll(2, 2, 6)); // Iron ingots
        put(2, Item::Resource(6), roll(3, 2, 6)); // Copper ingots
        put(3, Item::Resource(3), roll(4, 4, 10)); // Coal
        put(4, Item::Resource(5), roll(5, 0, 3)); // Resonant crystal
        put(5, Item::Resource(23), u32::from(hash_u32(h ^ 6) % 4 == 0) * roll(7, 1, 3)); // Gold ingot
        put(6, Item::Resource(15), u32::from(hash_u32(h ^ 8) % 5 == 0) * roll(9, 1, 2)); // Steel ingots
        put(7, Item::Block(BlockId::LightlessShaft), u32::from(hash_u32(h ^ 10) % 7 == 0));
        let id = world.caches.len();
        world.caches.push(Cache { cells, slots });
        return Some(id);
    }
    if let Some(loot) = engine_worldgen::ruins::cache_loot(&cfg, pos, block) {
        if let Some(text) = engine_worldgen::civilization::archive_text(&cfg, pos, block) {
            world.expedition.say(text);
        }
        let id = world.caches.len();
        world.caches.push(Cache { cells: vec![pos], slots: city_salvage(&cfg, pos, loot) });
        return Some(id);
    }
    let mut cells = Vec::new();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([pos]);
    let mut units = [0u32; super::factory::RESOURCE_COUNT];
    let scale_units = (world.voxel_scale.meters().powi(3) * 64.0).round().max(1.0) as u32;
    while let Some(p) = queue.pop_front() {
        if cells.len() >= 512 || !seen.insert(p) || world.get(p) != block
            || (p-pos).as_vec3().length() * world.voxel_scale.meters() > 1.8
            || world.caches.iter().any(|c| c.cells.contains(&p))
            || engine_worldgen::ruins::cache_loot(&cfg, p, block).is_some()
            || (block == BlockId::SurveyCache && engine_worldgen::sites::workshop_supply_cache(&cfg, p)) { continue; }
        cells.push(p);
        for &(item, count) in super::expedition::container_loot(block, p).1 {
            units[item] += count * scale_units;
        }
        for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] { queue.push_back(p+d); }
    }
    let mut slots = [None; SLOTS];
    // Each connected cache is one chest; fractional voxel rewards round once.
    let mut index = 0;
    for (id, units) in units.into_iter().enumerate() {
        let mut count = units.div_ceil(64);
        while count > 0 {
            if index >= SLOTS { return None; }
            let n = count.min(64);
            slots[index] = Some(ItemStack::new(Item::Resource(id as u8), n));
            index += 1; count -= n;
        }
    }
    if let Some(&anchor) = cells.iter().min_by_key(|p| (p.x, p.y, p.z)) {
        let center = (anchor.as_vec3() + glam::Vec3::splat(0.5)) * cfg.voxel_meters();
        if matches!(block, BlockId::SalvageCrate | BlockId::SurveyCache)
            && (engine_worldgen::dungeon::name_at(&cfg, center).is_some()
                // Hell camp crates are guarded; they pay like a dungeon chest.
                || (cfg.is_hell() && block == BlockId::SalvageCrate)
                // Every cache in the Gilded Cave is treasure.
                || cfg.world_type.realm() == Some(engine_voxel::RealmKind::Gilded))
            && engine_worldgen::sample_block(&cfg, anchor) == block
        {
            dungeon_treasure(world, center.floor().as_ivec3(), block == BlockId::SurveyCache, &mut slots[index..]);
        } else if index < SLOTS {
            slots[index] = found_potion(world.seed, anchor);
        }
    }
    let id = world.caches.len();
    world.caches.push(Cache { cells, slots });
    Some(id)
}

fn dungeon_treasure(world: &mut World, pos: IVec3, survey: bool, slots: &mut [Option<ItemStack>]) {
    use super::adventure::wand_items;
    use super::inventory::FoodKind as Food;
    use engine_core::hash::hash_u32;
    let key = hash_u32(world.seed
        ^ (pos.x as u32).wrapping_mul(0x8da6_b343)
        ^ (pos.y as u32).wrapping_mul(0xd816_3841)
        ^ (pos.z as u32).wrapping_mul(0xcb1a_b31f)
        ^ 0xce77_a629);
    let brews = [Food::HealingPotion, Food::HastePotion, Food::ResistancePotion, Food::StrengthPotion];
    let materials = [(5, 4), (12, 3), (16, 2), (17, 1)];
    if let Some(slot) = slots.first_mut() {
        *slot = Some(ItemStack::new(Item::Food(brews[(key & 3) as usize]), if survey { 2 } else { 1 }));
    }
    if let Some(slot) = slots.get_mut(1) {
        let (id, count) = materials[((key >> 2) & 3) as usize];
        *slot = Some(ItemStack::new(Item::Resource(id), count));
    }
    if let Some(slot) = slots.get_mut(2)
        && (survey || (key >> 4) & 7 == 0)
    {
        let item = world.adventure.mint_wand(wand_items::generate(key.max(1), if survey { 2 } else { 1 }))
            .unwrap_or(Item::Resource(17));
        *slot = Some(ItemStack::new(item, 1));
    }
}

fn city_salvage(
    cfg: &engine_worldgen::WorldSampleConfig, pos: IVec3, loot: engine_worldgen::ruins::CacheLoot,
) -> [Option<ItemStack>; SLOTS] {
    use engine_worldgen::ruins::CacheLoot::*;
    use super::inventory::FoodKind as Food;
    let [x, y, z] = ((pos.as_vec3() + glam::Vec3::splat(0.5)) * cfg.voxel_meters())
        .floor().as_ivec3().to_array();
    let roll = engine_worldgen::hash3(x, y, z, cfg.seed ^ 0x96a1_2003);
    let extra = u32::from(roll < 0.35);
    let res = |kind, count| Some(ItemStack::new(Item::Resource(kind), count));
    let food = |kind, count| Some(ItemStack::new(Item::Food(kind), count));
    // A planned cache is a single crate at any voxel scale.
    let contents = match loot {
        Household => [food(Food::FungalRation, 1 + extra), res(4, 1), res(13, 2)],
        Clinic => [food(Food::HealingPotion, 1), res(4, 2), (extra > 0).then(|| ItemStack::new(Item::Food(Food::RegenerationPotion), 1))],
        Dispatch => [res(6, 1), res(5, 1 + extra), food(Food::Trailcake, 1)],
        Freight => [res(2, 3 + extra), res(1, 2), res(3, 4)],
        Workshop => [res(8, 1 + extra), res(6, 2), res(3, 3)],
        Relay => [res(5, 3), res(16, 2), if extra > 0 { res(17, 1) } else { food(Food::HastePotion, 1) }],
    };
    let mut slots = [None; SLOTS];
    slots[..contents.len()].copy_from_slice(&contents);
    slots
}

/// The potion a cache first opened at `pos` holds, if any; a pure function of seed and cell.
fn found_potion(seed: u32, pos: IVec3) -> Option<ItemStack> {
    use super::stations::table::BREWING;
    use engine_core::hash::hash01;
    let key = seed
        ^ (pos.x as u32).wrapping_mul(0x8da6_b343)
        ^ (pos.y as u32).wrapping_mul(0xd816_3841)
        ^ (pos.z as u32).wrapping_mul(0xcb1a_b31f)
        ^ 0x9071_0001;
    if hash01(key) >= BREWING.found_chance {
        return None;
    }
    let total: u32 = BREWING.found.iter().map(|f| f.weight).sum();
    let mut roll = (hash01(key ^ 0x5bd1_e995) * total as f32) as u32;
    for row in &BREWING.found {
        if roll < row.weight {
            return Some(ItemStack::new(Item::Food(row.output), row.count));
        }
        roll -= row.weight;
    }
    None
}

#[cfg(test)]
mod potion_tests {
    use super::*;

    #[test]
    fn some_caches_hold_potions_deterministically() {
        let cells: Vec<IVec3> = (0..400).map(|i| IVec3::new(i * 7, -(i % 30), i * 3)).collect();
        let found = cells.iter().filter(|p| found_potion(11, **p).is_some()).count();
        assert!((100..220).contains(&found), "{found}/400");
        assert_eq!(found_potion(11, cells[5]), found_potion(11, cells[5]));
    }
}

pub fn open(world: &mut World, pos: IVec3) -> bool {
    flush(world);
    let Some(id) = locate(world, pos) else { return false };
    world.inventory.container = Some((id, world.caches[id].slots));
    true
}

/// Breaking one member drops the remaining contents once. The empty record is
/// retained while other members exist, preventing fresh loot on reopening.
pub fn harvest(world: &mut World, pos: IVec3,
    blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> bool {
    flush(world);
    let Some(id) = locate(world, pos) else { return false };
    super::dungeon_quest::on_cache_open(world, pos);
    let contents = world.caches[id].slots;
    if world.inventory.dropped.len() + contents.iter().flatten().count() > 1024 { return false; }
    let Ok(commit) = super::effects::replace(world, pos, world.get(pos), BlockId::Air) else { return false };
    commit.publish(world, blocks, chunks);
    let centre = (pos.as_vec3() + glam::Vec3::splat(0.5)) * world.voxel_scale.meters();
    for stack in contents.into_iter().flatten() {
        world.inventory.dropped.push(super::inventory::DroppedStack { pos: centre, stack });
    }
    world.caches[id].slots = [None; SLOTS];
    if world.inventory.container.as_ref().is_some_and(|(active, _)| *active == id) {
        world.inventory.container = None;
    }
    true
}

pub fn encode(caches: &[Cache], bytes: &mut Vec<u8>) {
    let mut put = |n: u32| bytes.extend_from_slice(&n.to_le_bytes());
    put(caches.len() as u32);
    for cache in caches {
        put(cache.cells.len() as u32);
        for p in &cache.cells { for n in p.to_array() { put(n as u32); } }
        for slot in cache.slots {
            let (kind, payload, count) = slot.map_or((0, 0, 0), |s| { let (k,p) = s.item.encode_u32(); (k,p,s.count) });
            put(kind); put(payload); put(count);
        }
    }
}

pub fn decode(read: &mut impl FnMut() -> anyhow::Result<u32>, _inventory: &SurvivalInventory) -> anyhow::Result<Vec<Cache>> {
    let count = read()? as usize;
    anyhow::ensure!(count <= LIMIT, "Too many caches");
    let mut caches = Vec::with_capacity(count);
    let mut occupied = HashSet::new();
    for _ in 0..count {
        let n = read()? as usize;
        anyhow::ensure!((1..=512).contains(&n), "Invalid cache bounds");
        let mut cells = Vec::with_capacity(n);
        for _ in 0..n {
            let p = IVec3::new(read()? as i32, read()? as i32, read()? as i32);
            anyhow::ensure!(occupied.insert(p), "Overlapping caches");
            cells.push(p);
        }
        let mut slots = [None; SLOTS];
        for slot in &mut slots {
            let (kind, payload, count) = (read()?, read()?, read()?);
            if kind == 0 { anyhow::ensure!(payload == 0 && count == 0, "Invalid empty cache slot"); continue; }
            let item = Item::decode_u32(kind, payload)?;
            anyhow::ensure!((1..=1_048_576).contains(&count), "Invalid cache stack");
            *slot = Some(ItemStack { item, count });
        }
        caches.push(Cache { cells, slots });
    }
    Ok(caches)
}
