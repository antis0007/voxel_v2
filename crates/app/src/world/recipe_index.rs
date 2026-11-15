//! One searchable index of every way to make or use an item, in the spirit of
//! JEI: grid recipes, machine processes, multiblocks, fuels and the field
//! smelter. The interface reads only this; it never re-derives recipes.
//!
//! Amounts are whole items (`0.25` coal is a quarter lump). Energy is in
//! factory units (250 J), so `energy / ticks` is kilowatts.
use super::factory::{self, Kind, power, recipes};
use super::inventory::{Item, all_recipes};
use super::{BlockId, expedition, mining};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Station {
    /// The 2 × 2 grid, anywhere.
    Hand,
    /// The 3 × 3 grid at a workbench.
    Bench,
    /// A machine running one of its recipes.
    Machine(Kind),
    /// A brick multiblock; the controller kind names it.
    Multiblock(Kind),
    /// Burned in a generator.
    Fuel(Kind),
    /// The hand-fed field smelter block.
    Smelter,
    /// A placed station with its own interaction panel.
    Utility(BlockId),
}

impl Station {
    pub fn name(self) -> &'static str {
        match self {
            Self::Hand => "Crafting (2×2)",
            Self::Bench => "Workbench (3×3)",
            Self::Machine(k) | Self::Fuel(k) => k.name(),
            Self::Multiblock(Kind::CokeOven) => "Coke oven",
            Self::Multiblock(Kind::BlastFurnace) => "Blast furnace",
            Self::Multiblock(k) => k.name(),
            Self::Smelter => "Field smelter",
            Self::Utility(block) => block.display_name(),
        }
    }
    /// The item that stands for the station in the interface.
    pub fn icon(self) -> Item {
        match self {
            Self::Hand => Item::Block(super::BlockId::Planks),
            Self::Bench => Item::Block(super::BlockId::Workbench),
            Self::Machine(k) | Self::Fuel(k) | Self::Multiblock(k) => Item::Block(k.block()),
            Self::Smelter => Item::Block(super::BlockId::Smelter),
            Self::Utility(block) => Item::Block(block),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub station: Station,
    pub label: String,
    pub inputs: Vec<(Item, f32)>,
    pub outputs: Vec<(Item, f32)>,
    /// Seconds per batch at the base drive.
    pub seconds: f32,
    /// Electrical energy one batch consumes (units).
    pub energy: f32,
    /// Electrical energy one batch releases (fuels only).
    pub yields: f32,
    pub note: &'static str,
    /// Index into `inventory::all_recipes` for grid recipes.
    pub grid: Option<usize>,
}

impl Entry {
    fn unpacks_stock(&self) -> bool {
        let ([(packed @ Item::Block(_), packed_count)], [(item @ Item::Resource(_), count)]) =
            (self.inputs.as_slice(), self.outputs.as_slice())
        else { return false; };
        *packed_count == 1.0 && *count > 1.0 && all_recipes().iter().any(|recipe| {
            recipe.output.item == *packed && recipe.output.count == 1
                && recipe.ingredients == &[(*item, *count as u32)]
        })
    }

    pub fn makes(&self, item: Item) -> f32 {
        self.outputs
            .iter()
            .filter(|(i, _)| *i == item)
            .map(|(_, n)| n)
            .sum()
    }
    pub fn takes(&self, item: Item) -> f32 {
        self.inputs
            .iter()
            .filter(|(i, _)| *i == item)
            .map(|(_, n)| n)
            .sum()
    }
    /// Average electrical draw while running, in units per tick (= kW).
    pub fn draw_kw(&self) -> f32 {
        if self.seconds > 0.0 {
            self.energy / (self.seconds / power::TICK_SECONDS as f32)
        } else {
            0.0
        }
    }
}

fn res(i: usize, sub: u32) -> (Item, f32) {
    (Item::Resource(i as u8), sub as f32 / 64.0)
}

fn machine_note(kind: Kind, id: usize) -> &'static str {
    match (kind, id) {
        (Kind::Washer, _) => "Also takes 250 L of piped water per batch.",
        (Kind::Cultivator, 0) => "The biomass is a living culture: it stays in the machine.",
        (Kind::Furnace, 0..=2) => "Ore mode. Crushing or washing first doubles or triples the yield.",
        (Kind::Fabricator, 2) => "Makes essence.",
        (Kind::Fabricator, 3) => "",
        _ => "",
    }
}

fn build(size: i32) -> Vec<Entry> {
    let mut out = Vec::new();
    for (index, recipe) in all_recipes().iter().enumerate() {
        out.push(Entry {
            station: if recipe.needs_bench() {
                Station::Bench
            } else {
                Station::Hand
            },
            label: recipe.name.to_string(),
            inputs: recipe
                .ingredients
                .iter()
                .map(|&(item, n)| (item, n as f32))
                .collect(),
            outputs: vec![(recipe.output.item, recipe.output.count as f32)],
            seconds: 0.0,
            energy: 0.0,
            yields: 0.0,
            note: recipe.description,
            grid: Some(index),
        });
    }
    // Grid recipes supported by the assembling machine.
    for recipe in all_recipes().iter() {
        if !factory::assembler::automatable(recipe) {
            continue;
        }
        out.push(Entry {
            station: Station::Machine(Kind::Assembler),
            label: recipe.name.to_string(),
            inputs: recipe
                .ingredients
                .iter()
                .map(|&(item, n)| (item, n as f32))
                .collect(),
            outputs: vec![(recipe.output.item, recipe.output.count as f32)],
            seconds: factory::assembler::CRAFT_TICKS as f32 * power::TICK_SECONDS as f32,
            energy: (factory::assembler::CRAFT_TICKS * factory::assembler::POWER) as f32,
            yields: 0.0,
            note: "Resources by pipe. Whole blocks and finished items by adjacent chest.",
            grid: None,
        });
    }
    for kind in Kind::ALL {
        let count = if kind == Kind::Fabricator { kind.modes().len() } else { 24 };
        for id in 0..count {
            let Some(p) = recipes::process(kind, id) else {
                continue;
            };
            let multi = matches!(kind, Kind::CokeOven | Kind::BlastFurnace);
            let mut outputs = vec![res(p.output.0, p.output.1)];
            if let Some((side, amount)) = p.byproduct {
                outputs.push(res(side, amount));
            }
            let label = match (kind, id) {
                (Kind::Crusher, recipes::QUARTZ_RECIPE) => "Crush Gold Quartz".into(),
                (Kind::Washer, recipes::QUARTZ_RECIPE) => "Wash Gold Quartz".into(),
                (Kind::Furnace, 0..=2) => format!("Smelt {}", super::expedition::NAMES[id]),
                (Kind::Furnace, 12..=14) => format!("Arc Smelt {}", super::expedition::NAMES[id - 12]),
                (Kind::Furnace, 15) => "Arc Steel".into(),
                (Kind::Furnace, _) => format!("Smelt {}", super::expedition::NAMES[id]),
                (Kind::CokeOven, _) => "Coke coal".to_string(),
                _ => kind
                    .modes()
                    .get(id)
                    .map(|m| m.split(':').next().unwrap_or(m).to_string())
                    .unwrap_or_else(|| kind.name().to_string()),
            };
            out.push(Entry {
                station: if multi {
                    Station::Multiblock(kind)
                } else {
                    Station::Machine(kind)
                },
                label,
                inputs: p
                    .inputs
                    .iter()
                    .filter(|(_, n)| *n > 0)
                    .map(|&(i, n)| res(i, n))
                    .chain(p.block_cost(size).map(|(material, count)| (material.item(), count as f32)))
                    .collect(),
                outputs,
                seconds: p.ticks as f32 * power::TICK_SECONDS as f32,
                energy: (p.ticks * p.power) as f32,
                yields: 0.0,
                note: if multi {
                    if kind == Kind::CokeOven {
                        "Heat-driven. Works at about 1000 °C; no electricity."
                    } else {
                        "Burns extra coke to hold 1400–1500 °C; no electricity."
                    }
                } else {
                    machine_note(kind, id)
                },
                grid: None,
            });
        }
    }
    for (i, recipe) in factory::biomatter::RECIPES.iter().enumerate() {
        out.push(Entry {
            station: Station::Machine(Kind::Cultivator),
            label: Kind::Cultivator.modes()[factory::biomatter::FIRST_MODE as usize + i].to_string(),
            inputs: vec![(Item::Block(super::BlockId::Grass), recipe.grass as f32)],
            outputs: vec![res(factory::biomatter::RESOURCE, 64)],
            seconds: recipe.ticks as f32 * power::TICK_SECONDS as f32,
            energy: (recipe.ticks * recipe.power) as f32,
            yields: 0.0,
            note: if recipe.water > 0 { "Water · 250 L" } else { "" },
            grid: None,
        });
    }
    let concentrate = factory::GOLD_CONCENTRATE;
    let sediment_blocks = (factory::placer::VOLUME_PER_M3 / factory::placer::cell_volume(size)) as f32;
    out.push(Entry {
        station: Station::Machine(Kind::Washer),
        label: "Sluice · 1 m³".into(),
        inputs: vec![(Item::Block(super::BlockId::Paydirt), sediment_blocks)],
        outputs: vec![
            res(concentrate, factory::placer::Feed::from_grade(
                factory::placer::PAYDIRT_GRADE, factory::placer::VOLUME_PER_M3,
            ).sluice_recovery()),
            (Item::Block(super::BlockId::Sand), sediment_blocks),
        ],
        seconds: factory::placer::SLUICE_TICKS as f32 * power::TICK_SECONDS as f32,
        energy: (factory::placer::SLUICE_TICKS * factory::placer::SLUICE_POWER) as f32,
        yields: 0.0,
        note: "250 L Water",
        grid: None,
    });
    out.push(Entry {
        station: Station::Multiblock(Kind::SmelteryController),
        label: "Cast Gold Ingots".into(),
        inputs: vec![res(concentrate, recipes::CONCENTRATE_PER_INGOT * super::smeltery::INGOTS_PER_BLOCK)],
        outputs: vec![(Item::Resource(factory::GOLD_INGOT as u8), super::smeltery::INGOTS_PER_BLOCK as f32)],
        seconds: 0.0,
        energy: 0.0,
        yields: 0.0,
        note: "Coal · Casting Table",
        grid: None,
    });
    for &(fuel, portion, units) in &power::COMBUSTION_FUELS {
        out.push(Entry {
            station: Station::Fuel(Kind::Generator),
            label: format!("Burn {}", super::expedition::NAMES[fuel]),
            inputs: vec![res(fuel, portion)],
            outputs: vec![],
            seconds: units as f32 / 16.0 * power::TICK_SECONDS as f32,
            energy: 0.0,
            yields: units as f32,
            note: "Combustion mode, 16 kW.",
            grid: None,
        });
    }
    out.push(Entry {
        station: Station::Fuel(Kind::Generator),
        label: "Burn biomass".into(),
        inputs: vec![res(power::BIOMASS_FUEL.0, power::BIOMASS_FUEL.1)],
        outputs: vec![],
        seconds: power::BIOMASS_FUEL.2 as f32 / power::BIOMASS_RATE as f32 * power::TICK_SECONDS as f32,
        energy: 0.0,
        yields: power::BIOMASS_FUEL.2 as f32,
        note: "Biomass burner mode, 12 kW.",
        grid: None,
    });
    out.push(Entry {
        station: Station::Fuel(Kind::Bioreactor),
        label: "Digest biomass".into(),
        inputs: vec![res(13, 32)],
        outputs: vec![],
        seconds: 800.0 / 32.0 * power::TICK_SECONDS as f32,
        energy: 0.0,
        yields: 800.0,
        note: "32 kW.",
        grid: None,
    });
    out.push(Entry {
        station: Station::Fuel(Kind::Generator),
        label: "Arcane pile fission".into(),
        inputs: vec![res(factory::pile::FUEL, factory::pile::PORTION)],
        outputs: vec![],
        seconds: factory::pile::CAPACITY as f32 / factory::pile::RATE as f32
            * power::TICK_SECONDS as f32,
        energy: 0.0,
        yields: factory::pile::CAPACITY as f32,
        note: "Arcane pile mode, 64 kW. Needs coolant water or it melts down.",
        grid: None,
    });
    for (ore, amount, metal) in expedition::FIELD_SMELTING {
        out.push(Entry {
            station: Station::Smelter,
            label: expedition::NAMES[metal].into(),
            inputs: vec![res(ore, amount), res(3, expedition::SMELT_COAL)],
            outputs: vec![res(metal, 64)],
            seconds: 0.0,
            energy: 0.0,
            yields: 0.0,
            note: "Use the smelter with ore and coal in inventory.",
            grid: None,
        });
    }
    out.push(Entry {
        station: Station::Smelter,
        label: "Gold from quartz".into(),
        inputs: vec![
            (Item::Block(BlockId::GoldQuartz), expedition::field_quartz_cost(size) as f32),
            res(3, expedition::SMELT_COAL),
        ],
        outputs: vec![(Item::Resource(factory::GOLD_INGOT as u8), super::smeltery::INGOTS_PER_BLOCK as f32)],
        seconds: 0.0,
        energy: 0.0,
        yields: 0.0,
        note: "Use the smelter with quartz and coal in inventory.",
        grid: None,
    });
    use super::stations::table::{BREWING, STATIONS};
    for row in &BREWING.recipes {
        let output = Item::Food(row.output);
        out.push(Entry {
            station: Station::Utility(BlockId::BrewingStand),
            label: output.name().into(),
            inputs: vec![(row.base, 1.0), (row.modifier, 1.0)],
            outputs: vec![(output, row.count as f32)],
            seconds: BREWING.seconds,
            energy: 0.0,
            yields: 0.0,
            note: "Adjacent water required.",
            grid: None,
        });
    }
    for row in &STATIONS.stonecutter {
        out.push(Entry {
            station: Station::Utility(BlockId::Stonecutter),
            label: row.output.name().into(),
            inputs: vec![(row.input, 1.0)],
            outputs: vec![(row.output, row.count as f32)],
            seconds: 0.0,
            energy: 0.0,
            yields: 0.0,
            note: "",
            grid: None,
        });
    }
    out.extend(forge_entries(size));
    out
}

/// Read-only workshop operations and final cooled-metal collection, not tool assemblies.
fn forge_entries(size: i32) -> Vec<Entry> {
    use super::{smeltery, tool_workshop};
    use engine_tools::{MATERIALS, PARTS};

    let mut out = Vec::new();
    let voxel_m = 1.0 / size as f32;
    for (kind, part) in PARTS.part_kinds.iter().enumerate() {
        let Ok(kind) = u8::try_from(kind) else { continue; };
        for material in 0..MATERIALS.len() {
            let Ok(material) = u8::try_from(material) else { continue; };
            let Some(supply) = tool_workshop::supply(material as usize) else { continue; };
            if supply.cast { continue; }
            let Some((input, count)) = tool_workshop::material_cost(
                material as usize, u32::from(part.material_cost) * 64, voxel_m,
            ) else { continue; };
            let output = Item::Part { kind, material };
            out.push(Entry {
                station: Station::Utility(BlockId::Workbench),
                label: output.name().into(),
                inputs: vec![(input, count as f32)],
                outputs: vec![(output, 1.0)],
                seconds: 0.0,
                energy: 0.0,
                yields: 0.0,
                note: "",
                grid: None,
            });
        }
        for metal in &smeltery::DEF.metals {
            let Some(collection) = smeltery::part_collection(kind, metal.solid, size) else { continue; };
            let mut outputs = vec![(collection.output.item, collection.output.count as f32)];
            if let Some(excess) = collection.excess {
                outputs.push((excess.item, excess.count as f32));
            }
            out.push(Entry {
                station: Station::Utility(BlockId::CastingTable),
                label: collection.output.item.name().into(),
                inputs: vec![
                    (collection.metal.item, collection.metal.count as f32),
                    (Item::Cast(kind), 1.0),
                ],
                outputs,
                seconds: 0.0,
                energy: 0.0,
                yields: 0.0,
                note: "Cooled Metal · Mold Consumed",
                grid: None,
            });
        }
    }
    // 255 is the reusable, optional ingot mold, not a part mold.
    for kind in (0..PARTS.part_kinds.len().min(255)).map(|kind| kind as u8).chain([255]) {
        let output = Item::Cast(kind);
        out.push(Entry {
            station: Station::Utility(BlockId::Workbench),
            label: output.name().into(),
            inputs: vec![(Item::Block(BlockId::Clay), tool_workshop::mold_cost(voxel_m) as f32)],
            outputs: vec![(output, 1.0)],
            seconds: 0.0,
            energy: 0.0,
            yields: 0.0,
            note: "",
            grid: None,
        });
    }
    out
}

pub fn entries() -> &'static [Entry] {
    entries_at(1)
}

fn entries_at(size: i32) -> &'static [Entry] {
    static METRE: LazyLock<Vec<Entry>> = LazyLock::new(|| build(1));
    static HALF: LazyLock<Vec<Entry>> = LazyLock::new(|| build(2));
    static QUARTER: LazyLock<Vec<Entry>> = LazyLock::new(|| build(4));
    match size {
        4 => &QUARTER,
        2 => &HALF,
        _ => &METRE,
    }
}

pub fn producing(item: Item) -> Vec<&'static Entry> {
    producing_at(item, 1)
}

pub fn producing_at(item: Item, size: i32) -> Vec<&'static Entry> {
    let mut entries: Vec<_> = entries_at(size).iter().filter(|e| e.makes(item) > 0.0).collect();
    // Field smelting can supply the ingots needed to build the first electric furnace.
    entries.sort_by_key(|entry| (entry.unpacks_stock(), entry.station != Station::Smelter));
    entries
}

pub fn using(item: Item) -> Vec<&'static Entry> {
    using_at(item, 1)
}

pub fn using_at(item: Item, size: i32) -> Vec<&'static Entry> {
    entries_at(size)
        .iter()
        .filter(|e| e.takes(item) > 0.0 || (e.station != Station::Hand && e.station.icon() == item))
        .collect()
}

/// Blocks that provide a resource directly, as opposed to metal cladding
/// and other crafted blocks that just return some of their inputs.
pub fn harvest_sources(item: Item) -> &'static [BlockId] {
    use BlockId as B;
    static RESOURCES: LazyLock<[Vec<BlockId>; factory::RESOURCE_COUNT]> = LazyLock::new(|| {
        let mut sources = std::array::from_fn(|_| Vec::new());
        for &block in engine_content::ALL_BLOCK_IDS {
            if let Some(i) = expedition::ore_resource(block).or_else(|| match block {
                B::GlowCap | B::RedCap | B::MushroomStem | B::AmberCap | B::UmberCap => expedition::resource(block),
                _ => None,
            }) {
                sources[i].push(block);
            }
        }
        sources
    });
    match item {
        Item::Resource(i) => RESOURCES.get(i as usize).map_or(&[], Vec::as_slice),
        Item::Block(B::Stone) => &[B::Stone],
        Item::Block(B::Clay) => &[B::Clay],
        Item::Block(B::Sand) => &[B::Sand],
        Item::Block(B::Gravel) => &[B::Gravel],
        Item::Block(B::Log) => &[B::Log],
        Item::Block(B::GoldQuartz) => &[B::GoldQuartz],
        Item::Block(B::Paydirt) => &[B::Paydirt],
        _ => &[],
    }
}

/// Number of cells needed for one whole item, using the harvest transaction's yield.
pub fn cells_per_harvest_item(item: Item, size: i32) -> u32 {
    if matches!(item, Item::Resource(_)) {
        64 / expedition::harvest_units(1.0 / size.max(1) as f32)
    } else {
        1
    }
}

/// An accessible grid-crafted tool that meets the same rule as the break action.
pub fn harvest_tool(block: BlockId) -> Option<Item> {
    let (class, tier) = mining::requirement(block);
    if tier == 0 {
        return None;
    }
    mining::table().tools.iter()
        .filter(|row| row.class == class && row.tier >= tier)
        .filter(|row| all_recipes().iter().any(|recipe| recipe.output.item == row.item))
        .min_by_key(|row| row.tier)
        .map(|row| row.item)
}

/// Catalogue tab of an item; declaration order is the list order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    Materials,
    Building,
    Utility,
    Stations,
    Machines,
    Tools,
    Traversal,
    Combat,
    Food,
    Arcane,
}

impl Group {
    pub const ALL: [Group; 10] = [
        Self::Materials,
        Self::Building,
        Self::Utility,
        Self::Stations,
        Self::Machines,
        Self::Tools,
        Self::Traversal,
        Self::Combat,
        Self::Food,
        Self::Arcane,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Materials => "Materials",
            Self::Building => "Building",
            Self::Utility => "Utility",
            Self::Stations => "Stations",
            Self::Machines => "Machines",
            Self::Tools => "Tools",
            Self::Traversal => "Traversal",
            Self::Combat => "Combat",
            Self::Food => "Food",
            Self::Arcane => "Arcane",
        }
    }

    /// The item drawn on the group's tab.
    pub fn icon(self) -> Item {
        use super::BlockId as B;
        match self {
            Self::Materials => Item::Resource(8),
            Self::Building => Item::Block(B::StoneBricks),
            Self::Utility => Item::Block(B::Chest),
            Self::Stations => Item::Block(B::Workbench),
            Self::Machines => Item::Block(Kind::Generator.block()),
            Self::Tools => Item::Tool(8),
            Self::Traversal => Item::Tool(19),
            Self::Combat => Item::Weapon(2),
            Self::Food => Item::Food(super::inventory::FoodKind::Trailcake),
            Self::Arcane => Item::Resource(5),
        }
    }

    pub fn of(item: Item) -> Self {
        use super::BlockId as B;
        use super::inventory::{ArmorTier, EquipSlot};
        match item {
            Item::Tool(19)
            | Item::Armor(EquipSlot::Feet, ArmorTier::Slime)
            | Item::Armor(EquipSlot::Legs, ArmorTier::Kinetic)
            | Item::Armor(EquipSlot::Chest, ArmorTier::Ballast | ArmorTier::Burner | ArmorTier::Induction | ArmorTier::Glider) => Self::Traversal,
            Item::Resource(_) | Item::Bucket(_) | Item::Cast(_) => Self::Materials,
            Item::Part { .. } | Item::Assembled { .. } | Item::Tool(1 | 2 | 6..=20) => Self::Tools,
            Item::Tool(_) | Item::Wand { .. } => Self::Arcane,
            Item::Weapon(_) | Item::Armor(..) => Self::Combat,
            Item::Food(_) => Self::Food,
            Item::PowerKit(_) => Self::Machines,
            Item::Block(b) if Kind::from_block(b).is_some() => Self::Machines,
            Item::Block(
                B::Workbench
                | B::Smelter
                | B::Anvil
                | B::BrewingStand
                | B::Stonecutter
                | B::Grindstone
                | B::SmelteryController
                | B::SmelteryDrain
                | B::SmelteryFaucet
                | B::CastingTable
                | B::CastingBasin,
            ) => Self::Stations,
            Item::Block(b) if b.is_torch() => Self::Utility,
            Item::Block(
                B::Chest
                | B::StorageCrate
                | B::Bed
                | B::Ladder
                | B::Lever
                | B::MechanicalButton
                | B::PressurePlate
                | B::OakDoor
                | B::OakTrapdoor
                | B::SignPost
                | B::Lectern
                | B::Tnt
                | B::Bookcase,
            ) => Self::Utility,
            Item::Block(B::Crystal | B::RigidCore | B::StarGlass | B::MirageBrick) => Self::Arcane,
            Item::Block(_) => Self::Building,
        }
    }
}

/// Every item that appears in any recipe, sorted by group, then name.
pub fn known_items() -> &'static [Item] {
    static ITEMS: LazyLock<Vec<Item>> = LazyLock::new(|| {
        let mut seen = HashSet::new();
        let mut items: Vec<Item> = (0..factory::RESOURCE_COUNT as u8)
            .map(Item::Resource)
            .collect();
        seen.extend(items.iter().copied());
        for e in entries() {
            for &(item, _) in e.outputs.iter().chain(&e.inputs) {
                if seen.insert(item) {
                    items.push(item);
                }
            }
        }
        // Resources keep their processing order (ore, ingot, dust); the rest sort by name.
        items.sort_by_key(|&item| {
            let index = match item {
                Item::Resource(i) => i,
                _ => u8::MAX,
            };
            (Group::of(item), index, item.name())
        });
        items
    });
    &ITEMS
}

/// Gathered rather than made, even when a recipe also produces them.
pub fn is_raw(item: Item) -> bool {
    match item {
        Item::Resource(i) => matches!(i, 0..=5 | 13),
        _ => producing(item).is_empty(),
    }
}

/// A node of a crafting plan: how many of `item` are needed, how many come
/// from available stock or earlier batches, and the recipe for the rest.
#[derive(Clone, Debug)]
pub struct Plan {
    pub item: Item,
    pub need: f32,
    pub from_stock: f32,
    pub recipe: Option<&'static Entry>,
    pub batches: u32,
    /// Alternatives available for this item (for the "cycle" control).
    pub options: usize,
    pub children: Vec<Plan>,
    /// The planner stopped here because the chain loops back on itself.
    pub cycle: bool,
}

/// Choice per item: index into `producing(item)`. Missing means "default".
pub type Choices = HashMap<Item, usize>;

/// Prefer new production over unpacking stock, then the shortest input chain.
pub fn default_choice(item: Item) -> usize {
    producing(item)
        .iter()
        .enumerate()
        .min_by_key(|(_, e)| (e.unpacks_stock(), e.inputs.iter().filter(|(i, _)| !is_raw(*i)).count()))
        .map(|(k, _)| k)
        .unwrap_or(0)
}

/// Expand a target into a tree, drawing on `stock` (whole items) as it goes.
/// Later branches can consume leftovers from earlier batches.
pub fn plan(item: Item, need: f32, stock: &mut HashMap<Item, f32>, choices: &Choices) -> Plan {
    let mut path = Vec::new();
    plan_inner(item, need, stock, choices, &mut path, &mut None, 1)
}

/// A complete production dependency graph: construct reusable stations once,
/// then resolve their ingredients and the requested batch against the same stock.
/// Placed facilities can be supplied as one stock item by the caller. Power,
/// utility connections and multiblock casings remain separate runtime conditions.
pub fn plan_with_facilities(item: Item, need: f32, stock: &mut HashMap<Item, f32>, choices: &Choices, size: i32) -> Plan {
    plan_inner(item, need, stock, choices, &mut Vec::new(), &mut Some(HashSet::new()), size)
}

fn plan_inner(
    item: Item,
    need: f32,
    stock: &mut HashMap<Item, f32>,
    choices: &Choices,
    path: &mut Vec<Item>,
    facilities: &mut Option<HashSet<Item>>,
    size: i32,
) -> Plan {
    let have = stock.get(&item).copied().unwrap_or(0.0);
    let from_stock = have.min(need);
    if from_stock > 0.0 {
        stock.insert(item, have - from_stock);
    }
    let rest = need - from_stock;
    let options = producing_at(item, size);
    let mut node = Plan {
        item,
        need,
        from_stock,
        recipe: None,
        batches: 0,
        options: options.len(),
        children: Vec::new(),
        cycle: false,
    };
    if rest <= 1e-4 || is_raw(item) || options.is_empty() {
        return node;
    }
    if path.contains(&item) || path.len() >= 10 {
        node.cycle = true;
        return node;
    }
    let pick = choices
        .get(&item)
        .copied()
        .unwrap_or_else(|| default_choice(item))
        .min(options.len() - 1);
    let recipe = options[pick];
    let per = recipe.makes(item);
    let batches = (rest / per - 1e-4).ceil().max(1.0) as u32;
    node.recipe = Some(recipe);
    node.batches = batches;
    path.push(item);
    if recipe.station != Station::Hand {
        let station = recipe.station.icon();
        if facilities.as_mut().is_some_and(|seen| seen.insert(station)) {
            node.children.push(plan_inner(station, 1.0, stock, choices, path, facilities, size));
        }
    }
    for &(input, n) in &recipe.inputs {
        // A catalyst such as the cultivator's culture is needed once.
        let amount = if recipe.station == Station::Machine(Kind::Cultivator)
            && recipe.makes(input) > 0.0
        {
            n
        } else {
            n * batches as f32
        };
        node.children
            .push(plan_inner(input, amount, stock, choices, path, facilities, size));
    }
    path.pop();
    // The requested quantity is already allocated to this node. Only credit
    // surplus and by-products after its ingredients have been resolved.
    let surplus = per * batches as f32 - rest;
    if surplus > 1e-4 {
        *stock.entry(item).or_default() += surplus;
    }
    for &(output, count) in &recipe.outputs {
        if output != item {
            *stock.entry(output).or_default() += count * batches as f32;
        }
    }
    node
}

impl Plan {
    /// Raw materials still to gather, summed over the whole tree.
    pub fn missing(&self) -> Vec<(Item, f32)> {
        let mut out: Vec<(Item, f32)> = Vec::new();
        self.collect_missing(&mut out);
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }
    fn collect_missing(&self, out: &mut Vec<(Item, f32)>) {
        let rest = self.need - self.from_stock;
        if self.recipe.is_none() && rest > 1e-4 {
            match out.iter_mut().find(|(i, _)| *i == self.item) {
                Some(row) => row.1 += rest,
                None => out.push((self.item, rest)),
            }
        }
        for c in &self.children {
            c.collect_missing(out);
        }
    }
    /// Machine and station time, and electrical energy, for the whole tree.
    pub fn totals(&self) -> (f32, f32) {
        let (mut seconds, mut energy) = self
            .recipe
            .map(|r| (r.seconds * self.batches as f32, r.energy * self.batches as f32))
            .unwrap_or((0.0, 0.0));
        for c in &self.children {
            let (s, e) = c.totals();
            seconds += s;
            energy += e;
        }
        (seconds, energy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_machine_process_is_indexed() {
        assert!(entries().iter().any(|e| e.station == Station::Multiblock(Kind::CokeOven)));
        assert!(entries().iter().any(|e| e.station == Station::Multiblock(Kind::BlastFurnace)));
        assert!(!producing(Item::Resource(factory::COKE as u8)).is_empty());
        assert!(!using(Item::Resource(3)).is_empty());
    }

    /// The grid takes the first matching row, so a repeated shape hides the later recipe.
    #[test]
    fn shaped_recipes_are_unique() {
        let shaped: Vec<_> = all_recipes().iter().filter_map(|r| Some((r.name, r.shape()?))).collect();
        for (i, (a, sa)) in shaped.iter().enumerate() {
            for (b, sb) in &shaped[i + 1..] {
                let (w, h) = (sa.width, sa.height);
                let fits = |mirror: bool| {
                    (0..h).all(|r| (0..w).all(|c| sa.cells[r * 3 + c] == sb.cells[r * 3 + if mirror { w - 1 - c } else { c }]))
                };
                let same = w == sb.width && h == sb.height && (fits(false) || fits(true));
                assert!(!same, "{a} and {b} share a pattern");
            }
        }
    }

    #[test]
    fn chest_is_craftable() {
        assert!(!producing(Item::Block(super::super::BlockId::Chest)).is_empty());
    }

    #[test]
    fn machine_kits_are_bench_recipes() {
        let kit = Item::Block(Kind::Battery.block());
        let made = producing(kit);
        assert!(made.iter().any(|e| e.station == Station::Bench));
    }

    #[test]
    fn a_plan_uses_stock_before_recipes() {
        let target = Item::Block(Kind::Generator.block());
        let mut stock = HashMap::from([(Item::Resource(6), 2.0), (Item::Resource(8), 2.0)]);
        let plan = plan(target, 1.0, &mut stock, &Choices::new());
        assert!(plan.recipe.is_some());
        assert!(plan.missing().is_empty(), "{:?}", plan.missing());
    }

    #[test]
    fn a_plan_bottoms_out_in_raw_materials() {
        let target = Item::Block(Kind::Battery.block());
        let plan = plan(target, 1.0, &mut HashMap::new(), &Choices::new());
        let missing = plan.missing();
        assert!(!missing.is_empty());
        assert!(missing.iter().all(|(i, _)| is_raw(*i)), "{missing:?}");
    }
}

#[cfg(test)]
#[path = "recipe_index/forge_tests.rs"]
mod forge_tests;
