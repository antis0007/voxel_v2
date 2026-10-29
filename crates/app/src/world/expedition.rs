//! A small complete exploration -> salvage -> processing -> tool upgrade loop.
//! Quantities use 1/64 cubic metre units, so fine voxels do not multiply rewards.
use super::{BlockId as B, World, effects, mark_chunk_and_neighbors_dirty};
use glam::{IVec3, Vec3};
use std::collections::{HashSet, VecDeque};

pub const NAMES: [&str; super::factory::RESOURCE_COUNT] = [
    "Copper ore",
    "Tin ore",
    "Iron ore",
    "Coal",
    "Lumen fibre",
    "Resonant crystal",
    "Copper ingots",
    "Tin ingots",
    "Iron ingots",
    "Copper dust",
    "Tin dust",
    "Iron dust",
    "Ancient circuits",
    "Fungal biomass",
    "Bronze ingots",
    "Steel ingots",
    "Induction coils",
    "Arcane cores",
    "Wild essence",
    "Coal coke",
    "Creosote oil",
    "Bio-ethanol",
];
const UNIT: u32 = 64;
const BORE_FUEL_UNITS: u32 = UNIT / 8;

/// Physical radius shared by excavation and the inventory's tool comparison.
pub fn bore_radius(tier: u32) -> f32 {
    if tier == 1 { 1.6 } else { 2.8 }
}

/// Lumen fibre consumed by an excavation that finds at least one target.
pub fn bore_fuel_cost() -> f64 {
    BORE_FUEL_UNITS as f64 / UNIT as f64
}

/// Five container families. The block picks the family; the cell picks the roll,
/// so two coffers in one crypt are not the same payout.
fn container_loot(block: B, p: IVec3) -> (&'static str, &'static [(usize, u32)]) {
    let roll =
        (p.x.wrapping_mul(31) ^ p.y.wrapping_mul(13) ^ p.z.wrapping_mul(17)).unsigned_abs() % 3;
    let deep = p.y < 0;
    match block {
        B::SurveyCache => match roll {
            0 => (
                "Relic coffer: circuits, a coil and resonant crystal. Bring the coil to a smelter.",
                if deep {
                    &[(12, 3), (16, 2), (5, 2), (17, 1)]
                } else {
                    &[(12, 2), (16, 1), (5, 1), (8, 2)]
                },
            ),
            1 => (
                "Archive chest: steel, circuits and an arcane core.",
                &[(15, 2), (12, 2), (17, 1), (3, 4)],
            ),
            _ => (
                "Survey locker: refined metal and crystal for a blade head.",
                &[(8, 4), (6, 3), (5, 2), (15, 1)],
            ),
        },
        B::CultivatorCache => match roll {
            0 => (
                "Ration cask: biomass, fibre and a grip wrapping.",
                &[(13, 8), (4, 4), (12, 1)],
            ),
            1 => (
                "Spore barrel: caps' worth of fibre and a coil.",
                &[(4, 6), (13, 4), (16, 1), (5, 1)],
            ),
            _ => (
                "Cultivator crate: food stocks and a little crystal.",
                &[(13, 6), (4, 3), (5, 2), (3, 2)],
            ),
        },
        _ => match roll {
            0 => (
                "Supply crate: ore and coal for the forge.",
                &[(0, 6), (2, 6), (3, 8), (8, 1)],
            ),
            1 => (
                "Armory locker: ingots and a binding coil.",
                &[(6, 4), (8, 3), (16, 1), (3, 4)],
            ),
            _ => (
                "Workshop chest: mixed ore, fibre and a core if you are deep enough.",
                if deep {
                    &[(1, 4), (2, 4), (4, 3), (17, 1)]
                } else {
                    &[(0, 4), (1, 4), (3, 6), (4, 2)]
                },
            ),
        },
    }
}

/// A fallback fitting action at a smelter with nothing to smelt. It upgrades the
/// first part that has an affordable, strictly better material, then tries an
/// uninstalled modifier. The workbench Forge exposes the same costs with an
/// explicit preview, so this is only a convenience path for early play.
fn reinforce_weapon(world: &mut World) -> bool {
    use super::adventure::tinkering::{self, Material, Part};
    if world.adventure.weapon == 0 {
        return false;
    }
    for part in Part::ALL {
        let current = match part {
            Part::Head => world.adventure.tool_head,
            Part::Binding => world.adventure.tool_bind,
            Part::Grip => world.adventure.tool_grip,
        };
        for material in Material::ALL {
            if material.as_u8() <= current {
                continue;
            }
            if world.expedition.pay(tinkering::part_cost(part, material)) {
                world.adventure.fit_part(part, material);
                world.expedition.say(format!(
                    "{} {} fitted at the empty forge. {}",
                    material.name(),
                    part.label().to_lowercase(),
                    material.trait_detail()
                ));
                return true;
            }
        }
    }
    for modifier in tinkering::MODIFIERS {
        if !world.adventure.has_modifier(modifier.bit) && world.expedition.pay(modifier.cost) {
            world.adventure.toggle_modifier(modifier.bit);
            world
                .expedition
                .say(format!("{} installed. {}", modifier.name, modifier.detail));
            return true;
        }
    }
    false
}
#[derive(Clone, Default)]
pub struct Expedition {
    pub resources: [u32; super::factory::RESOURCE_COUNT],
    pub tool_tier: u32,
    pub workshop_found: bool,
    pub lamp: bool,
    /// Separate ownership from the switch so inventory reconciliation cannot relight it.
    pub lamp_disabled: bool,
    pub message: String,
    pub message_seconds: f32,
}
impl Expedition {
    pub fn lamp_enabled(&self) -> bool {
        self.lamp && !self.lamp_disabled
    }

    pub fn toggle_lamp(&mut self) {
        if !self.lamp {
            self.say("Lumen Lantern: Missing");
            return;
        }
        self.lamp_disabled = !self.lamp_disabled;
        self.say(if self.lamp_enabled() {
            "Lumen Lantern: On · [L]"
        } else {
            "Lumen Lantern: Off · [L]"
        });
    }

    pub fn say(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.message_seconds = 6.0;
    }
    pub fn tick(&mut self, dt: f32) {
        self.message_seconds = (self.message_seconds - dt).max(0.0);
    }
    pub fn tool_name(&self) -> &'static str {
        match self.tool_tier {
            0 => "Hand tools",
            1 => "Bronze bore",
            _ => "Resonant bore",
        }
    }
    pub fn objective(&self) -> &'static str {
        if !self.workshop_found {
            "Find the ruined workshop north of the clearing"
        } else if !self.lamp {
            "Follow the mine stairs and harvest luminous mushrooms"
        } else if self.tool_tier == 0 {
            "Process salvage; assemble a bronze bore at the workbench"
        } else if self.tool_tier == 1 {
            "Collect crystals and refine iron for the resonant bore"
        } else {
            ""
        }
    }
    pub(super) fn add(&mut self, index: usize, amount: u32) {
        self.resources[index] = self.resources[index].saturating_add(amount).min(64_000_000);
    }
    pub(super) fn pay(&mut self, recipe: &[(usize, u32)]) -> bool {
        if !recipe.iter().all(|&(i, n)| self.resources[i] >= n) {
            return false;
        }
        for &(i, n) in recipe {
            self.resources[i] -= n;
        }
        true
    }
}
pub(crate) fn resource(block: B) -> Option<usize> {
    Some(match block {
        B::CopperOre => 0,
        B::TinOre => 1,
        B::IronOre => 2,
        B::CoalOre => 3,
        B::GlowCap => 4,
        B::Crystal => 5,
        B::RedCap | B::MushroomStem | B::AmberCap | B::UmberCap => 13,
        _ => return None,
    })
}
const TREE_DIRECTIONS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];
const MAX_TREE_CELLS: usize = 1024;

/// Turn the remaining trunk of a chopped tree into one falling assembly.
///
/// The broken log has already been removed and collected by the caller. This
/// function finds the connected trunk/canopy, removes every leaf immediately,
/// and converts the remaining logs into a runtime rigid body with an initial
/// sideways impulse. `rigid::tick` drops the logs as items once it settles.
/// Returns the cells whose meshes must be refreshed immediately.
pub fn fell_tree(world: &mut World, removed: IVec3) -> Option<Vec<IVec3>> {
    let mut queue = VecDeque::new();
    let mut seen = HashSet::new();
    for d in TREE_DIRECTIONS {
        let q = removed + d;
        if matches!(world.get(q), B::Log | B::Leaves) {
            queue.push_back(q);
        }
    }
    let mut logs = Vec::new();
    let mut leaves = Vec::new();
    while let Some(cell) = queue.pop_front() {
        if seen.len() >= MAX_TREE_CELLS || !seen.insert(cell) {
            continue;
        }
        match world.get(cell) {
            B::Log => logs.push(cell),
            B::Leaves => leaves.push(cell),
            _ => continue,
        }
        for d in TREE_DIRECTIONS {
            let q = cell + d;
            if !seen.contains(&q) && matches!(world.get(q), B::Log | B::Leaves) {
                queue.push_back(q);
            }
        }
    }
    // A lone log or a pile without a canopy is not a tree.
    if logs.len() < 2 || leaves.len() < 4 {
        return None;
    }

    let leaf_edits: Vec<(IVec3, B)> = leaves.iter().map(|p| (*p, B::Air)).collect();
    let mut removed_leaves = Vec::new();
    world.set_many_committed(&leaf_edits, |p, _, _| removed_leaves.push(p));
    // Admission can skip individual cells. Only removed leaves produce loot.
    let loot = canopy_loot(&removed_leaves);
    for (p, item, count) in loot {
        give_or_drop(world, p, item, count);
    }
    let metres = world.voxel_scale.meters();
    for &p in &removed_leaves {
        if let Some(item) = super::farming::sapling::leaf_drop(world.seed, metres, p, B::Leaves) {
            give_or_drop(world, p, item, 1);
        }
    }

    let mut touched = removed_leaves;
    let body_possible = logs.len() <= crate::simulation::sandbox::MAX_BODY_VOXELS
        && world.sandbox.bodies.len() < crate::simulation::sandbox::MAX_BODIES
        && !world.sandbox.bodies.iter().any(|body| {
            body.occupied
                .iter()
                .any(|(p, _)| logs.iter().any(|q| p == q))
        })
        && logs.iter().all(|p| world.get(*p) == B::Log);
    if body_possible {
        let origin = *logs.iter().min_by_key(|p| (p.y, p.x, p.z)).unwrap();
        let occupied: Vec<_> = logs.iter().map(|p| (*p, B::Log)).collect();
        let fall = Vec3::new(
            (removed.x as f32 * 0.73).sin(),
            0.0,
            (removed.z as f32 * 0.91).cos(),
        )
        .normalize_or_zero();
        let fall = if fall.length_squared() < 1e-4 {
            Vec3::X
        } else {
            fall
        };
        world
            .sandbox
            .bodies
            .push(crate::simulation::sandbox::RigidBody {
                origin,
                position: Vec3::ZERO,
                velocity: fall * 1.1,
                rotation: glam::Quat::IDENTITY,
                angular: Vec3::new(fall.z, 0.0, -fall.x) * 2.2,
                debt: 0.0,
                settle_time: 0.0,
                was_submerged: false,
                voxels: logs.iter().map(|p| (*p - origin, B::Log)).collect(),
                occupied,
            });
        world.activity.wake(origin);
    } else {
        drop_log_cells(world, &logs, &mut HashSet::new(), &mut HashSet::new());
        touched.extend(logs);
    }
    Some(touched)
}

/// Leaf litter and fruit shaken from a canopy. Litter is one item per eight
/// leaf cells; fruit is a deterministic ~1.5% of cells, so the same tree always
/// bears the same fruit. Cells are sorted first so the result ignores HashSet
/// order.
fn canopy_loot(leaves: &[IVec3]) -> Vec<(IVec3, super::inventory::Item, u32)> {
    use super::inventory::{FoodKind, Item};
    let mut sorted = leaves.to_vec();
    sorted.sort_unstable_by_key(|p| (p.y, p.z, p.x));
    let mut loot = Vec::new();
    for (i, &p) in sorted.iter().enumerate() {
        let h = (p.x as u32).wrapping_mul(0x9E37_79B1)
            ^ (p.y as u32).wrapping_mul(0x85EB_CA77)
            ^ (p.z as u32).wrapping_mul(0xC2B2_AE3D);
        let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
        if (h >> 8) % 1000 < 15 {
            loot.push((p, Item::Food(FoodKind::Trailcake), 1));
        }
        if i % 8 == 0 {
            loot.push((p, Item::Block(B::Leaves), 1));
        }
    }
    loot
}

/// Give one item to the pack, or drop it at `p` when the pack is full.
fn give_or_drop(world: &mut World, p: IVec3, item: super::inventory::Item, count: u32) {
    let scale = world.voxel_scale.meters();
    let (inserted, remainder) = world
        .inventory
        .try_insert(super::inventory::ItemStack::new(item, count));
    if inserted > 0 {
        world.inventory.notify_pickup(item, inserted);
    }
    if remainder > 0 && world.inventory.dropped.len() < 1024 {
        world
            .inventory
            .dropped
            .push(super::inventory::DroppedStack {
                pos: (p.as_vec3() + Vec3::splat(0.5)) * scale,
                stack: super::inventory::ItemStack::new(item, remainder),
            });
    }
}

/// Remove log cells and give every log back as an item (or a nearby drop when
/// the pack is full). Used when a felled assembly settles.
pub(crate) fn drop_log_cells(
    world: &mut World,
    cells: &[IVec3],
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) {
    let item = super::inventory::Item::Block(B::Log);
    for &p in cells {
        let Ok(commit) = effects::replace(world, p, B::Log, B::Air) else {
            continue;
        };
        commit.publish(world, blocks, chunks);
        give_or_drop(world, p, item, 1);
    }
    world.sync_from_inventory();
}

pub fn harvest(
    world: &mut World,
    p: IVec3,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    if super::fittings::harvest(world, p, blocks, chunks) { return true; }
    if let Some(done) = super::farming::harvest(world, p, blocks, chunks) { return done; }
    if super::factory::dismantle(world, p, blocks, chunks) {
        world.reconcile_legacy_to_inventory();
        return true;
    }
    let block = world.get(p);
    if block == B::Air {
        return false;
    }

    let units = (world.voxel_scale.meters().powi(3) * UNIT as f32)
        .round()
        .max(1.0) as u32;

    // Pre-check inventory capacity before destroying the block so a full pack never loses items.
    if matches!(block, B::SalvageCrate | B::SurveyCache | B::CultivatorCache) {
        let (_, loot) = container_loot(block, p);
        let mut probe = world.inventory.clone();
        for &(i, amount) in loot {
            let add_whole = (probe.resource_remainders[i] + amount * units) / UNIT;
            probe.resource_remainders[i] = (probe.resource_remainders[i] + amount * units) % UNIT;
            if add_whole > 0 {
                let (_, rem) = probe.try_insert(super::inventory::ItemStack {
                    item: super::inventory::Item::Resource(i as u8),
                    count: add_whole,
                });
                if rem > 0 {
                    world
                        .inventory
                        .notify_overflow(super::inventory::Item::Resource(i as u8));
                    world.expedition.say(
                        "Inventory Full",
                    );
                    return false;
                }
            }
        }
    } else if let Some(i) = resource(block) {
        let add_whole = (world.inventory.resource_remainders[i] + units) / UNIT;
        if add_whole > 0
            && !world.inventory.can_insert(super::inventory::ItemStack {
                item: super::inventory::Item::Resource(i as u8),
                count: add_whole,
            })
        {
            world
                .inventory
                .notify_overflow(super::inventory::Item::Resource(i as u8));
            world
                .expedition
                .say("Inventory Full");
            return false;
        }
    } else if let Some(drop_block) = super::inventory::Item::canonical_block(block) {
        let item = super::inventory::Item::Block(drop_block);
        if !world
            .inventory
            .can_insert(super::inventory::ItemStack::new(item, 1))
        {
            world.inventory.notify_overflow(item);
            world
                .expedition
                .say("Inventory Full");
            return false;
        }
    }

    let Ok(commit) = effects::replace(world, p, block, B::Air) else {
        return false;
    };
    commit.publish(world, blocks, chunks);
    if let Some(seed) = super::farming::wild_seed(world, p, block) {
        give_or_drop(world, p, seed, 1);
    }
    if let Some(item) = super::farming::sapling::leaf_drop(world.seed, world.voxel_scale.meters(), p, block) {
        give_or_drop(world, p, item, 1);
    }
    if let Some(i) = resource(block) {
        if world.adventure.sounds.len() < 64 {
            world.adventure.sounds.push(crate::audio::Event {
                cue: crate::audio::Cue::Mine,
                pos: Some((p.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters()),
                gain: 0.4, pitch: if i == 5 { 1.3 } else { 0.9 },
            });
        }
        world.expedition.add(i, units);
        if i == 4 && !world.expedition.lamp {
            world.expedition.lamp = true;
            world
                .expedition
                .say("Lumen Lantern: On · [L]");
        }
        world.reconcile_legacy_to_inventory();
    } else if matches!(block, B::SalvageCrate | B::SurveyCache | B::CultivatorCache) {
        let (label, loot) = container_loot(block, p);
        for &(i, amount) in loot {
            world.expedition.add(i, amount * units);
        }
        world.reconcile_legacy_to_inventory();
        let m = p.as_vec3() * world.voxel_scale.meters();
        let place = super::sites::location(&world.sample_config(), m);
        match super::lore::record(world.seed, p, place, m.y) {
            Some(record) => world.expedition.say(format!("{label} {record}")),
            None => world.expedition.say(label),
        }
    } else if let Some(drop_block) = super::inventory::Item::canonical_block(block) {
        let item = super::inventory::Item::Block(drop_block);
        let (ins, _) = world
            .inventory
            .try_insert(super::inventory::ItemStack::new(item, 1));
        if ins > 0 {
            world.inventory.notify_pickup(item, ins);
        }
        world.sync_from_inventory();
    }
    if block == B::Log
        && let Some(cells) = fell_tree(world, p) {
            for cell in cells {
                blocks.insert(cell);
                mark_chunk_and_neighbors_dirty(chunks, cell);
            }
        }
    true
}

/// Fast path for spell-generated craters.
///
/// A spell blast is a world-shaping event, not a mining action: running the
/// full harvest/inventory pipeline for every stone voxel made large craters
/// crawl and silently filled the pack with rubble. Magic removes the terrain
/// directly. Resource-bearing cells keep the same volume-weighted yields as
/// mining; ordinary rubble stays out of the pack.
pub fn spell_blast(
    world: &mut World,
    p: IVec3,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    if super::fittings::harvest(world, p, blocks, chunks) { return true; }
    let b = world.get(p);
    if b == B::Air || b.is_liquid() {
        return false;
    }
    if world
        .factory
        .nodes
        .contains_key(&super::factory::anchor(p, super::factory::step(world)))
    {
        return false;
    }
    if resource(b).is_some() || matches!(b, B::SalvageCrate | B::SurveyCache | B::CultivatorCache) {
        let changed = blast_harvest(world, p, blocks, chunks);
        world.sync_from_inventory();
        return changed;
    }
    let Ok(commit) = effects::replace(world, p, b, B::Air) else {
        return false;
    };
    commit.publish(world, blocks, chunks);
    if b == B::Log {
        give_or_drop(world, p, super::inventory::Item::Block(B::Log), 1);
        world.sync_from_inventory();
    }
    if let Some(cells) = fell_tree(world, p) {
        for cell in cells {
            blocks.insert(cell);
            mark_chunk_and_neighbors_dirty(chunks, cell);
        }
    }
    true
}

/// A detonation still breaks ordinary terrain with a full pack. Preserve the
/// harvest as nearby recoverable stacks instead of leaving unexplained pillars.
pub fn blast_harvest(
    world: &mut World,
    p: IVec3,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    if super::fittings::harvest(world, p, blocks, chunks) { return true; }
    let b = world.get(p);
    // Ordinary terrain changes inventory directly. Rebuilding all legacy
    // resource counters for every tiny voxel makes large blasts crawl; the
    // caller synchronizes once after this frame's batch instead.
    let ordinary = resource(b).is_none()
        && super::inventory::Item::canonical_block(b).is_some()
        && !matches!(b, B::SalvageCrate | B::SurveyCache | B::CultivatorCache)
        && !world
            .factory
            .nodes
            .contains_key(&super::factory::anchor(p, super::factory::step(world)));
    if !ordinary && harvest(world, p, blocks, chunks) {
        return true;
    }
    if world.get(p) == B::Air
        || matches!(b, B::SalvageCrate | B::SurveyCache | B::CultivatorCache)
        || super::factory::Kind::from_block(b).is_some()
    {
        return false;
    }
    use super::inventory::{DroppedStack, Item, ItemStack};
    let pos = (p.as_vec3() + glam::Vec3::splat(0.5)) * world.voxel_scale.meters();
    let (item, count, remainder) = if let Some(i) = resource(b) {
        let units = (world.voxel_scale.meters().powi(3) * UNIT as f32)
            .round()
            .max(1.0) as u32;
        let amount = world.inventory.resource_remainders[i] + units;
        (
            Item::Resource(i as u8),
            amount / UNIT,
            Some((i, amount % UNIT)),
        )
    } else if let Some(block) = Item::canonical_block(b) {
        (Item::Block(block), 1, None)
    } else {
        return false;
    };
    let limit = world.inventory.stack_limit(item);
    let fits = count == 0 || world.inventory.can_insert(ItemStack::new(item, count));
    let existing = world.inventory.dropped.iter().position(|d| {
        d.stack.item == item
            && d.pos.distance_squared(pos) < 16.0
            && d.stack.count <= limit.saturating_sub(count)
    });
    // The save format bounds persistent drops. Do not destroy recoverable loot
    // if a pathological world already exhausts that bound.
    if !fits && existing.is_none() && world.inventory.dropped.len() >= 1024 {
        return false;
    }
    let Ok(commit) = effects::replace(world, p, b, B::Air) else {
        return false;
    };
    commit.publish(world, blocks, chunks);
    if let Some((i, left)) = remainder {
        world.inventory.resource_remainders[i] = left;
    }
    if count > 0 {
        if fits {
            let (inserted, _) = world.inventory.try_insert(ItemStack::new(item, count));
            if inserted > 0 {
                world.inventory.notify_pickup(item, inserted);
            }
        } else if let Some(i) = existing {
            world.inventory.dropped[i].stack.count += count;
        } else {
            world.inventory.dropped.push(DroppedStack {
                pos,
                stack: ItemStack::new(item, count),
            });
        }
    }
    if b == B::Log
        && let Some(cells) = fell_tree(world, p) {
            for cell in cells {
                blocks.insert(cell);
                mark_chunk_and_neighbors_dirty(chunks, cell);
            }
        }
    true
}

pub fn hint(block: B, tier: u32) -> &'static str {
    if let Some(s)=engine_content::fittings::state(block) {
        return if s.lever { "F / Right mouse  Switch" } else if s.open { "F / Right mouse  Close" } else { "F / Right mouse  Open" };
    }
    if super::factory::Kind::from_block(block).is_some() {
        return "F  Inspect   ·   E  Inventory";
    }
    match block {
        B::SalvageCrate | B::SurveyCache | B::CultivatorCache => "F  Open salvage cache",
        B::Workbench => "F  Use workbench   ·   E  Craft",
        B::Bed => "F  Sleep until dawn",
        B::Smelter => "F  Smelt ore   ·   Requires coal",
        B::RedCap | B::MushroomStem | B::AmberCap | B::UmberCap => "F  Harvest biomass",
        B::GlowCap => "F  Harvest lumen fibre",
        B::CopperOre
        | B::TinOre
        | B::IronOre
        | B::CoalOre
        | B::Crystal
        | B::Stone
        | B::Limestone
        | B::Basalt
            if tier > 0 =>
        {
            "Alt + left mouse  Bore seam · consumes lumen fibre"
        }
        B::CopperOre | B::TinOre | B::IronOre | B::CoalOre | B::Crystal => {
            "Left mouse  Mine material"
        }
        _ => "",
    }
}

pub fn interact(
    world: &mut World,
    p: IVec3,
    _player: Vec3,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) {
    if super::farming::inspect(world, p) {
        return;
    }
    let b = world.get(p);
    if super::factory::Kind::from_block(b).is_some() {
        let anchor = super::factory::anchor(p, super::factory::step(world));
        world.factory.focus = Some(anchor);
        if let Some(node) = world.factory.nodes.get(&anchor) {
            world.expedition.say(format!("{} | {} | E  Open",node.kind.name(),node.status));
        }
        return;
    }
    match b {
        B::Workbench => {
            world.expedition.workshop_found = true;
            world.factory.bench = Some(p);
            world.expedition.say("E  Craft");
            super::workshop::use_bench(world, _player);
        }
        B::Smelter => {
            world.expedition.workshop_found = true;
            let mut processed = 0;
            for (ore, metal) in [(0, 6), (1, 7), (2, 8)] {
                if super::inventory::resources::exchange(&mut world.inventory,
                    &[(ore, UNIT), (3, UNIT / 4)], &[(metal, UNIT)]) {
                    processed += 1;
                }
            }
            world.sync_from_inventory();
            if processed > 0 {
                world.adventure.sounds.push(crate::audio::Event {
                    cue: crate::audio::Cue::Smelt, pos: Some((p.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters()),
                    gain: 0.55, pitch: 1.0,
                });
                world
                    .expedition
                    .say("Smelted batch: 1 ingot per ore using 0.25 coal.");
            } else if reinforce_weapon(world) {
                world.reconcile_legacy_to_inventory();
                world.sync_from_inventory();
            } else {
                world.expedition.say("Need ore, 0.25 coal and room for the ingot. F  Fit a blade part or modifier");
            }
        }
        B::SalvageCrate | B::SurveyCache | B::CultivatorCache => {
            let mut queue = VecDeque::from([p]);
            let mut visited = HashSet::new();
            while let Some(cell) = queue.pop_front() {
                if visited.len() >= 512 || !visited.insert(cell) {
                    continue;
                }
                if (cell - p).as_vec3().length() * world.voxel_scale.meters() > 1.8
                    || world.get(cell) != b
                {
                    continue;
                }
                if harvest(world, cell, blocks, chunks) {
                    for d in [
                        IVec3::X,
                        IVec3::NEG_X,
                        IVec3::Y,
                        IVec3::NEG_Y,
                        IVec3::Z,
                        IVec3::NEG_Z,
                    ] {
                        queue.push_back(cell + d);
                    }
                }
            }
        }
        B::GlowCap | B::RedCap | B::MushroomStem | B::AmberCap | B::UmberCap => {
            harvest(world, p, blocks, chunks);
        }
        B::Crystal => {
            let scale = world.voxel_scale.meters();
            let metres = (p.as_vec3() + Vec3::splat(0.5)) * scale;
            let cfg = world.sample_config();
            let ground = super::sites::holy_ground(&cfg);
            if let Some(tier) = super::dungeon::wand_altar(&cfg, metres, ground) {
                let names = ["", "Lumen Twig", "Quarry Conduit", "Sanctum Sovereign"];
                // Gate on the best wand owned, so swapping to a weaker wand
                // cannot re-claim an altar.
                let owned = world.adventure.best_wand_tier();
                if owned >= tier {
                    world.expedition.say(format!(
                        "{} already attuned. The next wand is one landing deeper.",
                        names[tier as usize]
                    ));
                } else if owned + 1 < tier {
                    world
                        .expedition
                        .say("This altar is sealed. Claim the wand on the landing above first.");
                } else {
                    world.adventure.wand = tier;
                    let learned = world.adventure.grant_upgrade_spells(tier);
                    let log = world.adventure.spell_log;
                    super::adventure::wands::seed_unlocked_slots_known(
                        &mut world.adventure.loadout,
                        tier,
                        &|id| log.grants(id),
                    );
                    if !learned.is_empty() {
                        let names: Vec<_> = learned
                            .iter()
                            .map(|&id| super::adventure::spell_name(id))
                            .collect();
                        world
                            .expedition
                            .say(format!("Spells discovered: {}.", names.join(", ")));
                    }
                    world.expedition.say(format!(
                        "{} acquired. R  Cast | E  Edit spells",
                        names[tier as usize]
                    ));
                    // A second, found wand rests on the altar: its own chassis and deck.
                    let seed = engine_core::hash::hash_u32((p.x as u32).wrapping_mul(0x9E37_79B9) ^ (p.y as u32) ^ (p.z as u32).wrapping_mul(0x85EB_CA6B));
                    if let Some(item) = world.adventure.mint_wand(super::adventure::wand_items::generate(seed, tier)) {
                        world.inventory.dropped.push(super::inventory::DroppedStack {
                            pos: metres + Vec3::Y * 1.2,
                            stack: super::inventory::ItemStack::new(item, 1),
                        });
                        world.expedition.say("Another wand rests on the altar. Take it, or leave it for later.");
                    }
                }
            }
        }
        _ => {}
    }
}

/// Explicit mining action; ordinary interaction never excavates terrain.
/// `budget` bounds the flood fill; the windowed game passes 2 ms, headless
/// runs pass `Budget::UNBOUNDED` so the result depends only on the world.
pub fn bore_targets(world: &mut World, p: IVec3, player: Vec3, time_budget: crate::simulation::Budget) -> Vec<IVec3> {
    let b = world.get(p);
    if !matches!(
        b,
        B::CopperOre
            | B::TinOre
            | B::IronOre
            | B::CoalOre
            | B::Crystal
            | B::Stone
            | B::Limestone
            | B::Basalt
    ) {
        return Vec::new();
    }
    let tier = world.expedition.tool_tier;
    if tier == 0 {
        world
            .expedition
            .say("Craft a bore at a workbench first. Left mouse uses your selected mining size.");
        return Vec::new();
    }
    if world.expedition.resources[4] < BORE_FUEL_UNITS {
        world
            .expedition
            .say(format!("Bore needs {} lumen fibre. Harvest luminous mushrooms.", bore_fuel_cost()));
        return Vec::new();
    }
    let radius = bore_radius(tier);
    let voxel = world.voxel_scale.meters();
    let budget = ((if tier == 1 { 128.0 } else { 384.0 }) / voxel.powi(3)) as usize;
    let mut queue = VecDeque::from([p]);
    let mut visited = HashSet::new();
    let mut targets = Vec::new();
    while let Some(cell) = queue.pop_front() {
        if !targets.is_empty() && !time_budget.has_time() {
            break;
        }
        if visited.len() >= budget || !visited.insert(cell) {
            continue;
        }
        if (cell - p).as_vec3().length() * voxel > radius || world.get(cell) != b {
            continue;
        }
        // Do not excavate the support directly under the player's feet.
        let d = (cell.as_vec3() + Vec3::splat(0.5) - player) * voxel;
        if d.x.abs() < 0.65 && d.z.abs() < 0.65 && d.y < 0.0 && d.y > -1.2 {
            continue;
        }
        {
            targets.push(cell);
            for d in [
                IVec3::X,
                IVec3::NEG_X,
                IVec3::Y,
                IVec3::NEG_Y,
                IVec3::Z,
                IVec3::NEG_Z,
            ] {
                queue.push_back(cell + d);
            }
        }
    }
    if !targets.is_empty() {
        world.expedition.resources[4] -= BORE_FUEL_UNITS;
        world.reconcile_legacy_to_inventory();
        world.expedition.say(format!(
            "Bore queued {:.2} m³",
            targets.len() as f32 * voxel.powi(3)
        ));
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::WorldType;

    #[test]
    fn canopy_loot_is_deterministic_and_order_independent() {
        let cells: Vec<IVec3> = (0..40)
            .flat_map(|x| (0..6).map(move |y| IVec3::new(x, 100 + y, x % 7)))
            .collect();
        let mut reversed = cells.clone();
        reversed.reverse();
        let a = canopy_loot(&cells);
        let b = canopy_loot(&reversed);
        assert_eq!(a.len(), b.len());
        let litter = a
            .iter()
            .filter(|(_, item, _)| matches!(item, super::super::inventory::Item::Block(B::Leaves)))
            .count();
        assert_eq!(litter, cells.len().div_ceil(8));
        let fruit = a.len() - litter;
        assert!(fruit <= cells.len() / 20, "fruit should stay rare, got {fruit}");
        assert_eq!(
            a.iter().map(|(p, _, _)| *p).collect::<Vec<_>>(),
            b.iter().map(|(p, _, _)| *p).collect::<Vec<_>>()
        );
    }

    #[test]
    fn chopped_tree_fells_as_an_assembly_and_clears_the_canopy() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 1234);
        let base = IVec3::new(0, 100, 0);
        for y in 100..=103 {
            world.set(IVec3::new(0, y, 0), B::Log);
        }
        for y in 102..=104 {
            for x in -1..=1 {
                for z in -1..=1 {
                    if x != 0 || z != 0 {
                        world.set(IVec3::new(x, y, z), B::Leaves);
                    }
                }
            }
        }
        world.set(base, B::Air);
        let touched = fell_tree(&mut world, base).expect("tree should be recognized");
        assert!(!touched.is_empty());
        assert_eq!(world.get(IVec3::new(1, 103, 0)), B::Air);
        assert_eq!(world.sandbox.bodies.len(), 1);
        assert!(
            world.sandbox.bodies[0]
                .occupied
                .iter()
                .all(|(_, b)| *b == B::Log)
        );
        assert!(world.sandbox.bodies[0].occupied.len() >= 3);

        let body = world.sandbox.bodies.remove(0);
        let cells: Vec<_> = body.occupied.iter().map(|(p, _)| *p).collect();
        let before = world
            .inventory
            .count_item(crate::world::inventory::Item::Block(B::Log));
        drop_log_cells(&mut world, &cells, &mut HashSet::new(), &mut HashSet::new());
        assert!(cells.iter().all(|p| world.get(*p) == B::Air));
        assert!(
            world
                .inventory
                .count_item(crate::world::inventory::Item::Block(B::Log))
                >= before + cells.len() as u32
        );
    }
}
