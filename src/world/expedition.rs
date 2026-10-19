//! A small complete exploration -> salvage -> processing -> tool upgrade loop.
//! Quantities use 1/64 cubic metre units, so fine voxels do not multiply rewards.
use super::{BlockId as B, World, mark_chunk_and_neighbors_dirty};
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
];
const UNIT: u32 = 64;
#[derive(Clone, Default)]
pub struct Expedition {
    pub resources: [u32; super::factory::RESOURCE_COUNT],
    pub tool_tier: u32,
    pub workshop_found: bool,
    pub lamp: bool,
    pub message: String,
    pub message_seconds: f32,
}
impl Expedition {
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
            "Build, explore and experiment with spells. Guardian relics are optional discoveries."
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
    pub fn inventory(&self) -> String {
        NAMES
            .iter()
            .enumerate()
            .filter(|(i, _)| self.resources[*i] > 0)
            .map(|(i, name)| format!("{name}: {:.2}", self.resources[i] as f32 / UNIT as f32))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
fn resource(block: B) -> Option<usize> {
    Some(match block {
        B::CopperOre => 0,
        B::TinOre => 1,
        B::IronOre => 2,
        B::CoalOre => 3,
        B::GlowCap => 4,
        B::Crystal => 5,
        B::RedCap | B::MushroomStem => 13,
        _ => return None,
    })
}
pub fn harvest(
    world: &mut World,
    p: IVec3,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
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
        let loot: &[(usize, u32)] = match block {
            B::SurveyCache => &[(8, 4), (6, 3), (12, 2), (3, 4)],
            B::CultivatorCache => &[(4, 4), (5, 2), (13, 8), (12, 1)],
            _ => &[(0, 6), (1, 4), (2, 8), (3, 8)],
        };
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
                    world
                        .expedition
                        .say("Inventory full — make room in your pack (E) before opening this cache.");
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
                .say("Inventory full — press E to free a slot before mining more.");
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
                .say("Inventory full — press E to free a slot before mining more.");
            return false;
        }
    }

    world.set(p, B::Air);
    if world.get(p) != B::Air {
        return false;
    }
    if let Some(i) = resource(block) {
        world.expedition.add(i, units);
        if i == 4 && !world.expedition.lamp {
            world.expedition.lamp = true;
            world
                .expedition
                .say("Lumen lantern discovered — it lights your way underground");
        }
        world.reconcile_legacy_to_inventory();
    } else if matches!(block, B::SalvageCrate | B::SurveyCache | B::CultivatorCache) {
        let loot: &[(usize, u32)] = match block {
            B::SurveyCache => &[(8, 4), (6, 3), (12, 2), (3, 4)],
            B::CultivatorCache => &[(4, 4), (5, 2), (13, 8), (12, 1)],
            _ => &[(0, 6), (1, 4), (2, 8), (3, 8)],
        };
        for &(i, amount) in loot {
            world.expedition.add(i, amount * units);
        }
        world.reconcile_legacy_to_inventory();
        world.expedition.say(match block {
            B::SurveyCache => {
                "Engineer cache: refined metals and ancient circuits for advanced machinery."
            }
            B::CultivatorCache => {
                "Mycologist cache: crystals, fibre, biomass and an ancient circuit."
            }
            _ => "Workshop supplies recovered: ore and coal to build your first processing line.",
        });
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
    blocks.insert(p);
    mark_chunk_and_neighbors_dirty(chunks, p);
    true
}

/// A detonation still breaks ordinary terrain with a full pack. Preserve the
/// harvest as nearby recoverable stacks instead of leaving unexplained pillars.
pub fn blast_harvest(world:&mut World,p:IVec3,blocks:&mut HashSet<IVec3>,chunks:&mut HashSet<IVec3>) -> bool {
    let b=world.get(p);
    // Ordinary terrain changes inventory directly. Rebuilding all legacy
    // resource counters for every tiny voxel makes large blasts crawl; the
    // caller synchronizes once after this frame's batch instead.
    let ordinary=resource(b).is_none() && super::inventory::Item::canonical_block(b).is_some()
        && !matches!(b,B::SalvageCrate|B::SurveyCache|B::CultivatorCache)
        && !world.factory.nodes.contains_key(&super::factory::anchor(p,super::factory::step(world)));
    if !ordinary && harvest(world,p,blocks,chunks) {return true;}
    if world.get(p)==B::Air || matches!(b,B::SalvageCrate|B::SurveyCache|B::CultivatorCache)
        || super::factory::Kind::from_block(b).is_some() {return false;}
    use super::inventory::{Item,ItemStack,DroppedStack};
    let pos=(p.as_vec3()+glam::Vec3::splat(0.5))*world.voxel_scale.meters();
    let (item,count,remainder)=if let Some(i)=resource(b) {
        let units=(world.voxel_scale.meters().powi(3)*UNIT as f32).round().max(1.0) as u32;
        let amount=world.inventory.resource_remainders[i]+units;
        (Item::Resource(i as u8),amount/UNIT,Some((i,amount%UNIT)))
    } else if let Some(block)=Item::canonical_block(b) {(Item::Block(block),1,None)} else {return false;};
    let limit=world.inventory.stack_limit(item);
    let fits=count==0 || world.inventory.can_insert(ItemStack::new(item,count));
    let existing=world.inventory.dropped.iter().position(|d|d.stack.item==item && d.pos.distance_squared(pos)<16.0 && d.stack.count<=limit.saturating_sub(count));
    // The save format bounds persistent drops. Do not destroy recoverable loot
    // if a pathological world already exhausts that bound.
    if !fits && existing.is_none() && world.inventory.dropped.len()>=1024 {return false;}
    world.set(p,B::Air);
    if world.get(p)!=B::Air {return false;}
    if let Some((i,left))=remainder {world.inventory.resource_remainders[i]=left;}
    if count>0 {
        if fits {
            let (inserted,_)=world.inventory.try_insert(ItemStack::new(item,count));
            if inserted>0 {world.inventory.notify_pickup(item,inserted);}
        } else if let Some(i)=existing {world.inventory.dropped[i].stack.count+=count;}
        else {world.inventory.dropped.push(DroppedStack{pos,stack:ItemStack::new(item,count)});}
    }
    blocks.insert(p);mark_chunk_and_neighbors_dirty(chunks,p);
    true
}

pub fn hint(block: B, tier: u32) -> &'static str {
    if super::factory::Kind::from_block(block).is_some() {
        return "F  Inspect   ·   E  Inventory";
    }
    match block {
        B::SalvageCrate | B::SurveyCache | B::CultivatorCache => "F  Open salvage cache",
        B::Workbench => "F  Use workbench   ·   E  Craft",
        B::Smelter => "F  Smelt ore   ·   Requires coal",
        B::RedCap | B::MushroomStem => "F  Harvest biomass",
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
    let b = world.get(p);
    if super::factory::Kind::from_block(b).is_some() {
        let anchor = super::factory::anchor(p, super::factory::step(world));
        world.factory.focus = Some(anchor);
        if let Some(node) = world.factory.nodes.get(&anchor) {
            world.expedition.say(format!("{} · {} · E to load, collect or pause. Connect item pipes and power cables to any face.",node.kind.name(),node.status));
        }
        return;
    }
    match b {
        B::Workbench => {
            world.expedition.workshop_found = true;
            world.factory.bench = Some(p);
            world.expedition.say("Workbench ready. E opens crafting: assemble a bore or start a generator → crusher → electric furnace line.");
        }
        B::Smelter => {
            world.expedition.workshop_found = true;
            let mut processed = 0;
            for (ore, metal) in [(0, 6), (1, 7), (2, 8)] {
                if world.expedition.pay(&[(ore, UNIT), (3, UNIT / 4)]) {
                    world.expedition.add(metal, UNIT);
                    processed += 1;
                }
            }
            world.reconcile_legacy_to_inventory();
            world.expedition.say(if processed>0 {"Smelted batch: 1 ingot per ore using 0.25 coal. A powered crusher doubles your ore yield."} else {"Processor needs at least 1 copper/tin/iron ore and 0.25 coal. Salvage nearby caches or mine a seam."});
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
        B::GlowCap | B::RedCap | B::MushroomStem => {
            harvest(world, p, blocks, chunks);
        }
        _ => {}
    }
}

/// Explicit mining action; ordinary interaction never excavates terrain.
pub fn bore_targets(world: &mut World, p: IVec3, player: Vec3) -> Vec<IVec3> {
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
    if world.expedition.resources[4] < UNIT / 8 {
        world
            .expedition
            .say("Bore needs 0.125 lumen fibre. Harvest luminous mushrooms.");
        return Vec::new();
    }
    let radius = if tier == 1 { 1.6 } else { 2.8 };
    let voxel = world.voxel_scale.meters();
    let budget = ((if tier == 1 { 128.0 } else { 384.0 }) / voxel.powi(3)) as usize;
    let mut queue = VecDeque::from([p]);
    let mut visited = HashSet::new();
    let mut targets = Vec::new();
    let started = std::time::Instant::now();
    while let Some(cell) = queue.pop_front() {
        if !targets.is_empty() && started.elapsed() > std::time::Duration::from_millis(2) {
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
        world.expedition.resources[4] -= UNIT / 8;
        world.reconcile_legacy_to_inventory();
        world.expedition.say(format!(
            "Bore queued {:.2} m³",
            targets.len() as f32 * voxel.powi(3)
        ));
    }
    targets
}
