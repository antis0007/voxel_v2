//! One searchable index of every way to make or use an item, in the spirit of
//! JEI: grid recipes, machine processes, multiblocks, fuels and the field
//! smelter. The interface reads only this; it never re-derives recipes.
//!
//! Amounts are whole items (`0.25` coal is a quarter lump). Energy is in
//! factory units (250 J), so `energy / ticks` is kilowatts.
use super::factory::{self, Kind, multiblock, power, recipes};
use super::inventory::{Item, all_recipes};
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
    /// The field ore processor (F at a smelter block).
    Smelter,
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
            Self::Smelter => "Field ore processor",
        }
    }
    /// The item that stands for the station in the interface.
    pub fn icon(self) -> Item {
        match self {
            Self::Hand => Item::Block(super::BlockId::Planks),
            Self::Bench => Item::Block(super::BlockId::Workbench),
            Self::Machine(k) | Self::Fuel(k) | Self::Multiblock(k) => Item::Block(k.block()),
            Self::Smelter => Item::Block(super::BlockId::Smelter),
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
        (Kind::Fabricator, 2) => "Condenses resonance into essence. Power-hungry.",
        (Kind::Fabricator, 3) => "Salvage circuits build the first fabricator. Manufactured crystal circuits sustain coils, cores and advanced machinery.",
        _ => "",
    }
}

fn build() -> Vec<Entry> {
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
    // Factorio-style: every grid recipe made only of resources can also run
    // in an assembling machine, unattended.
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
            note: "Automated: pipe the ingredients in.",
            grid: None,
        });
    }
    for kind in Kind::ALL {
        for id in 0..24 {
            let Some(p) = recipes::process(kind, id) else {
                continue;
            };
            let multi = matches!(kind, Kind::CokeOven | Kind::BlastFurnace);
            let mut outputs = vec![res(p.output.0, p.output.1)];
            if kind == Kind::CokeOven {
                outputs.push(res(factory::CREOSOTE, multiblock::CREOSOTE_PER_BATCH));
            }
            let label = match (kind, id) {
                (Kind::Furnace, 0..=2) => format!("Smelt {}", super::expedition::NAMES[id]),
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
    for ore in 0..3 {
        out.push(Entry {
            station: Station::Smelter,
            label: format!("Field-smelt {}", super::expedition::NAMES[ore]),
            inputs: vec![res(ore, 64), res(3, 16)],
            outputs: vec![res(ore + 6, 64)],
            seconds: 0.0,
            energy: 0.0,
            yields: 0.0,
            note: "Press F at a field ore processor. Manual, one batch at a time.",
            grid: None,
        });
    }
    out
}

pub fn entries() -> &'static [Entry] {
    static ALL: LazyLock<Vec<Entry>> = LazyLock::new(build);
    &ALL
}

pub fn producing(item: Item) -> Vec<&'static Entry> {
    entries().iter().filter(|e| e.makes(item) > 0.0).collect()
}

pub fn using(item: Item) -> Vec<&'static Entry> {
    entries()
        .iter()
        .filter(|e| e.takes(item) > 0.0 || e.station.icon() == item)
        .collect()
}

/// Every item that appears in any recipe, resources first, in a stable order.
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
        items
    });
    &ITEMS
}

/// Gathered rather than made, even when a recipe also produces them.
pub fn is_raw(item: Item) -> bool {
    match item {
        Item::Resource(i) => matches!(i, 0..=5 | 12 | 13 | 18),
        _ => producing(item).is_empty(),
    }
}

/// A node of a crafting plan: how many of `item` are needed, how many come
/// from what the player already carries, and the recipe chosen for the rest.
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

/// Default recipe: the one with the fewest non-raw inputs, so a plan prefers
/// a short chain; ties keep index order (grid recipes come first).
pub fn default_choice(item: Item) -> usize {
    producing(item)
        .iter()
        .enumerate()
        .min_by_key(|(_, e)| e.inputs.iter().filter(|(i, _)| !is_raw(*i)).count())
        .map(|(k, _)| k)
        .unwrap_or(0)
}

/// Expand a target into a tree, drawing on `stock` (whole items) as it goes,
/// like an autocrafting calculator. Stock is consumed depth first, so two
/// branches never count the same ingots twice.
pub fn plan(item: Item, need: f32, stock: &mut HashMap<Item, f32>, choices: &Choices) -> Plan {
    let mut path = Vec::new();
    plan_inner(item, need, stock, choices, &mut path)
}

fn plan_inner(
    item: Item,
    need: f32,
    stock: &mut HashMap<Item, f32>,
    choices: &Choices,
    path: &mut Vec<Item>,
) -> Plan {
    let have = stock.get(&item).copied().unwrap_or(0.0);
    let from_stock = have.min(need);
    if from_stock > 0.0 {
        stock.insert(item, have - from_stock);
    }
    let rest = need - from_stock;
    let options = producing(item);
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
            .push(plan_inner(input, amount, stock, choices, path));
    }
    path.pop();
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
