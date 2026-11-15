//! A small complete exploration -> salvage -> processing -> tool upgrade loop.
//! Quantities use 1/64 cubic metre units, so fine voxels do not multiply rewards.
use super::{BlockId as B, World, effects, mark_chunk_and_neighbors_dirty};
use glam::{IVec3, Vec3};
use std::collections::{HashSet, VecDeque};

pub use engine_content::factory_resources::NAMES;

const UNIT: u32 = engine_content::fabrication::UNITS_PER_ITEM;
pub(super) const SMELT_COAL: u32 = UNIT / 4;
const FIELD_BATCH_LIMIT: u32 = 4;
/// Resource input, quantity, and ingot output, in inventory subunits.
pub(super) const FIELD_SMELTING: [(usize, u32, usize); 4] = [
    (0, UNIT, 6),
    (1, UNIT, 7),
    (2, UNIT, 8),
    (super::factory::GOLD_CONCENTRATE, super::factory::recipes::CONCENTRATE_PER_INGOT, super::factory::GOLD_INGOT),
];
const BORE_FUEL_UNITS: u32 = UNIT / 8;

pub(super) fn harvest_units(voxel_meters: f32) -> u32 {
    (voxel_meters.powi(3) * UNIT as f32).round().max(1.0) as u32
}

pub(super) fn field_quartz_cost(cells_per_metre: i32) -> u32 {
    8 * cells_per_metre.pow(3) as u32
}

/// Physical radius shared by excavation and the inventory's tool comparison.
pub fn bore_radius(tier: u32) -> f32 {
    if tier == 1 { 1.6 } else { 2.8 }
}

/// Lumen fibre consumed by an excavation that finds at least one target.
pub fn bore_fuel_cost() -> f64 {
    BORE_FUEL_UNITS as f64 / UNIT as f64
}

/// Common caches contain field supplies. Finished industrial parts and arcane
/// cores come from manufacturing and guarded rewards, regardless of altitude.
/// The block picks the family; the cell picks the roll,
/// so two coffers in one crypt are not the same payout.
pub(super) fn container_loot(block: B, p: IVec3) -> (&'static str, &'static [(usize, u32)]) {
    let roll =
        (p.x.wrapping_mul(31) ^ p.y.wrapping_mul(13) ^ p.z.wrapping_mul(17)).unsigned_abs() % 3;
    match block {
        B::SurveyCache => match roll {
            0 => (
                "Survey Samples · Crystal · Copper Ore · Coal",
                &[(5, 1), (0, 3), (3, 2)],
            ),
            1 => (
                "Survey Supplies · Iron Ore · Tin Ore · Coal",
                &[(2, 3), (1, 2), (3, 4)],
            ),
            _ => (
                "Survey Samples · Copper · Iron · Crystal",
                &[(8, 1), (6, 2), (5, 1)],
            ),
        },
        B::CultivatorCache => match roll {
            0 => (
                "Garden Supplies · Biomass · Lumen Fibre",
                &[(13, 4), (4, 3)],
            ),
            1 => (
                "Garden Supplies · Lumen Fibre · Biomass",
                &[(4, 4), (13, 3)],
            ),
            _ => (
                "Garden Supplies · Biomass · Fibre · Coal",
                &[(13, 4), (4, 3), (3, 2)],
            ),
        },
        _ => match roll {
            0 => (
                "Supply crate: ore and coal for the forge.",
                &[(0, 6), (2, 6), (3, 8), (8, 1)],
            ),
            1 => (
                "Workshop Supplies · Copper · Iron · Coal",
                &[(6, 2), (8, 1), (3, 4)],
            ),
            _ => (
                "Workshop Supplies · Mixed Ore · Coal · Fibre",
                &[(0, 4), (1, 4), (3, 6), (4, 2)],
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
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Expedition {
    #[cfg_attr(feature = "multiplayer", serde(deserialize_with = "super::factory::read_resources"))]
    pub resources: [u32; super::factory::RESOURCE_COUNT],
    pub tool_tier: u32,
    pub workshop_found: bool,
    pub lamp: bool,
    /// Separate ownership from the switch so inventory reconciliation cannot relight it.
    pub lamp_disabled: bool,
    pub message: String,
    pub message_seconds: f32,
    /// `Label: value` lines for the message's log tooltip, newline separated.
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub detail: String,
    /// Where the message happened, in metres, for distance and bearing.
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub at: Option<[f32; 3]>,
    /// Counts messages so the log sees a repeat of the same text.
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub serial: u32,
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
        self.say_for(message, 6.0);
    }
    pub fn say_for(&mut self, message: impl Into<String>, seconds: f32) {
        self.message = message.into();
        self.message_seconds = seconds;
        self.detail.clear();
        self.at = None;
        self.serial = self.serial.wrapping_add(1);
    }
    /// A message with log detail lines and a position in metres.
    pub fn say_at(&mut self, message: impl Into<String>, detail: impl Into<String>, at: glam::Vec3) {
        self.say(message);
        self.detail = detail.into();
        self.at = Some(at.to_array());
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
pub(crate) fn ore_resource(block: B) -> Option<usize> {
    use engine_content::ore::Family;
    Some(match engine_content::ore::family(block)? {
        Family::Copper => 0,
        Family::Tin => 1,
        Family::Iron => 2,
        Family::Coal => 3,
        Family::Crystal => 5,
        Family::GoldQuartz => return None,
    })
}

pub(crate) fn resource(block: B) -> Option<usize> {
    Some(match block {
        B::CopperBlock => 6,
        B::TinBlock => 7,
        B::IronBlock => 8,
        B::BronzeBlock => 14,
        B::GlowCap => 4,
        B::RedCap | B::MushroomStem | B::AmberCap | B::UmberCap => 13,
        _ => return ore_resource(block),
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
mod woodcutting;
pub(crate) use woodcutting::{finish_tree, harvest_log};
pub use woodcutting::fell_tree;

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
pub(super) fn give_or_drop(world: &mut World, p: IVec3, item: super::inventory::Item, count: u32) {
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
    if super::containers::is_cache(block) { return super::containers::harvest(world, p, blocks, chunks); }
    if block == B::Air {
        return false;
    }
    // Only a registered assembly can return a kit. Loose casing can remain
    // after a rejected placement or be present in an older save.
    if super::factory::Kind::from_block(block).is_some() {
        let Ok(commit) = effects::replace(world, p, block, B::Air) else { return false };
        commit.publish(world, blocks, chunks);
        return true;
    }

    let units = harvest_units(world.voxel_scale.meters());

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

    // A sub-metre plant stamp comes away whole and drops once, for the aimed cell.
    let stamp = engine_content::stamp::cells(block, p, world.voxel_scale.meters(), |q| world.get(q));
    let Ok(commit) = effects::replace(world, p, block, super::farming::forage::vacated(block)) else {
        return false;
    };
    commit.publish(world, blocks, chunks);
    for q in stamp.into_iter().skip(1) {
        if let Ok(commit) = effects::replace(world, q, block, super::farming::forage::vacated(block)) {
            commit.publish(world, blocks, chunks);
        }
    }
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
    true
}

fn ejectable_rock(material: B) -> bool {
    matches!(material, B::Stone | B::Sandstone | B::Limestone | B::Basalt | B::Cobblestone | B::StoneBricks)
}

/// Transfer an exposed rock cell to a body without editing or awarding it.
pub(crate) fn eject_terrain(world: &mut World, p: IVec3, push: Vec3, speed: f32) -> bool {
    use crate::simulation::sandbox::MAX_BODIES;
    if !push.is_finite() || !speed.is_finite() || speed <= 0.0 { return false; }
    let material = world.get(p);
    if !ejectable_rock(material)
        || world.sandbox.bodies.len() >= MAX_BODIES - 16
        || effects::admit(world, p).is_err()
        || !super::orbital::habitat::ordinary_spell_allowed(world, p) {
        return false;
    }
    let exit = TREE_DIRECTIONS.into_iter()
        .filter(|&d| effects::admit(world, p + d).is_ok()
            && super::orbital::habitat::ordinary_spell_allowed(world, p + d)
            && world.get(p + d) == B::Air)
        .max_by(|a, b| a.as_vec3().dot(push).total_cmp(&b.as_vec3().dot(push)));
    let Some(exit) = exit else { return false };
    let scale = world.voxel_scale.meters();
    let direction = (exit.as_vec3() + Vec3::Y * 0.18).normalize();
    let density = (material.material().physics.density / 1000.0).max(0.5);
    if !world.sandbox.add_debris(p, &[(p, material)], direction * (speed / density.sqrt() / scale)) {
        return false;
    }
    if let Some(body) = world.sandbox.bodies.last_mut() {
        body.angular = direction.cross(push).clamp_length_max(1.0) * 3.0;
    }
    world.adventure.spawn_voxel_burst(
        (p.as_vec3() + Vec3::splat(0.5) + exit.as_vec3() * 0.5) * scale,
        material.color(p), 4, 1.8, (scale * 0.12).clamp(0.015, 0.06),
    );
    world.activity.wake(p);
    true
}

/// Carve a spell crater without filling the pack with ordinary rubble.
/// Resource cells keep their volume-weighted yields.
pub fn spell_blast(
    world: &mut World,
    p: IVec3,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    if !super::orbital::habitat::ordinary_spell_allowed(world, p) { return false; }
    if super::fittings::harvest(world, p, blocks, chunks) { return true; }
    let b = world.get(p);
    if b == B::Air || b.is_liquid() {
        return false;
    }
    // A budgeted crater can span several frames. Airborne chips may cross
    // later cells in the same stroke; the blast impulse already moved them.
    if ejectable_rock(b) && world.sandbox.bodies.iter().any(|body| body.occupied.as_slice() == [(p, b)]) {
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
    if b == B::Log {
        fell_tree(world, p, Vec3::ZERO);
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
    if !super::orbital::habitat::ordinary_spell_allowed(world, p) { return false; }
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
        let units = harvest_units(world.voxel_scale.meters());
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
    if b == B::Log {
        fell_tree(world, p, Vec3::ZERO);
    }
    true
}

pub fn hint(block: B, tier: u32) -> &'static str {
    if super::dungeon_quest::is_door(block) {
        return "RMB  Unlock";
    }
    if let Some(s)=engine_content::fittings::state(block) {
        return if s.switch.is_some() { "RMB  Switch" } else if s.open { "RMB  Close" } else { "RMB  Open" };
    }
    if super::factory::Kind::from_block(block).is_some() {
        return "RMB  Open";
    }
    if engine_content::ore::family(block).is_some() {
        return if tier > 0 { "Alt + left mouse  Bore seam · consumes lumen fibre" } else { "Left mouse  Mine material" };
    }
    match block {
        B::SalvageCrate | B::SurveyCache | B::CultivatorCache => "RMB  Open",
        B::Workbench => "RMB  Craft",
        B::Bed => "RMB  Sleep",
        B::Smelter => "Use  ·  Smelt ore and coal from pack",
        B::RedCap | B::MushroomStem | B::AmberCap | B::UmberCap => "F  Harvest biomass",
        B::SweetBerries | B::Grapes | B::Tomato | B::GlowBerries | B::Apple | B::Cherry | B::Fig => "RMB  Pick",
        B::CropHarvester => "RMB on soil  Set · Range 4 m",
        B::GlowCap => "F  Harvest lumen fibre",
        B::Stone
        | B::Limestone
        | B::Basalt
            if tier > 0 =>
        {
            "Alt + left mouse  Bore seam · consumes lumen fibre"
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
    if super::smeltery::interact(world, p, blocks, chunks) { return; }
    if super::farming::inspect(world, p) {
        return;
    }
    let b = world.get(p);
    if super::factory::Kind::from_block(b).is_some() {
        let anchor = super::factory::anchor(p, super::factory::step(world));
        world.factory.focus = Some(anchor);
        if super::factory::Kind::from_block(b) == Some(super::factory::Kind::Storage) {
            super::containers::open(world, anchor);
        } else { world.inventory.container = None; }
        if let Some(node) = world.factory.nodes.get(&anchor) {
            world.expedition.say(format!("{} | {} | E  Open",node.kind.name(),node.status));
        }
        return;
    }
    match b {
        B::Workbench => {
            world.inventory.container = None;
            world.factory.focus = None;
            world.expedition.workshop_found = true;
            world.factory.bench = Some(p);
            world.expedition.say("E  Craft");
            super::workshop::use_bench(world, _player);
        }
        B::Smelter => {
            use super::factory::GOLD_INGOT;
            world.expedition.workshop_found = true;
            let mut processed = 0;
            for (ore, amount, metal) in FIELD_SMELTING {
                for _ in 0..FIELD_BATCH_LIMIT {
                    if !super::inventory::resources::exchange(&mut world.inventory,
                        &[(ore, amount), (3, SMELT_COAL)], &[(metal, UNIT)])
                    {
                        break;
                    }
                    processed += 1;
                }
            }
            let quartz = super::inventory::Item::Block(B::GoldQuartz);
            let quartz_batch = field_quartz_cost(super::factory::step(world));
            if world.inventory.count_item(quartz) >= quartz_batch {
                let mut trial = world.inventory.clone();
                if trial.remove_item(quartz, quartz_batch)
                    && super::inventory::resources::exchange(&mut trial, &[(3, SMELT_COAL)],
                        &[(GOLD_INGOT, super::smeltery::INGOTS_PER_BLOCK * UNIT)])
                {
                    world.inventory = trial;
                    processed += 1;
                }
            }
            world.sync_from_inventory();
            if processed > 0 {
                world.goals.note_production(super::goals::Event::FieldSmelted);
                world.adventure.sounds.push(crate::audio::Event {
                    cue: crate::audio::Cue::Smelt, pos: Some((p.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters()),
                    gain: 0.55, pitch: 1.0,
                });
                world
                    .expedition
                    .say(format!("Smelted {processed} batches"));
            } else if reinforce_weapon(world) {
                world.reconcile_legacy_to_inventory();
                world.sync_from_inventory();
            } else {
                let stock = &world.expedition.resources;
                let has_batch = world.inventory.count_item(quartz) >= quartz_batch
                    || FIELD_SMELTING.iter().any(|&(ore, amount, _)| stock[ore] >= amount);
                let message = if has_batch && stock[3] >= SMELT_COAL {
                    "Pack full. Free space for ingots and use the smelter again."
                } else if stock[3] < SMELT_COAL {
                    "Smelting needs ¼ coal per batch. Mine coal ore and keep it in your pack."
                } else {
                    "Carry a full piece of copper, tin or iron ore to smelt an ingot."
                };
                world.expedition.say(message);
            }
        }
        B::SalvageCrate | B::SurveyCache | B::CultivatorCache => {
            if super::containers::open(world, p) {
                super::dungeon_quest::on_cache_open(world, p);
            }
        }
        B::LockedDoor | B::BossDoor => {
            super::dungeon_quest::try_open(world, p, blocks, chunks);
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
    if engine_content::ore::family(b).is_none() && !matches!(b, B::Stone | B::Limestone | B::Basalt) {
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
    fn chopped_tree_carries_its_canopy_and_drops_its_wood() {
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
        assert!(fell_tree(&mut world, base, Vec3::X));
        assert_eq!(world.get(IVec3::new(1, 103, 0)), B::Leaves);
        assert_eq!(world.sandbox.bodies.len(), 1);

        let mut body = world.sandbox.bodies.remove(0);
        let logs = body.occupied.iter().filter(|(_, b)| *b == B::Log).count();
        assert_eq!(logs, 3);
        assert!(body.occupied.iter().any(|(_, b)| *b == B::Leaves));
        assert!(finish_tree(&mut world, &mut body, &mut HashSet::new(), &mut HashSet::new()));
        assert_eq!(world.get(IVec3::new(1, 103, 0)), B::Air);
        let dropped: u32 = world.inventory.dropped.iter()
            .filter(|d| d.stack.item == super::super::inventory::Item::Block(B::Log))
            .map(|d| d.stack.count)
            .sum();
        assert_eq!(dropped, logs as u32);
    }
}
