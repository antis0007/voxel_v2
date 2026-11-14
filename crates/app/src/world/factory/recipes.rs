use super::{GOLD_CONCENTRATE, Kind, Node, blocks::Material};

pub const GOLD_MODE: u32 = 2;
/// Electric arc furnace mode: raw ore and steel recipes 12..16.
pub const ARC_MODE: u32 = 3;
pub const ARC_RECIPES: std::ops::Range<usize> = 12..16;
pub const GOLD_RECIPE: usize = 22;
pub const QUARTZ_RECIPE: usize = 3;
pub const CONCENTRATE_PER_INGOT: u32 = 16 * 64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Process {
    pub inputs: [(usize, u32); 3],
    pub block_input: Option<Material>,
    pub output: (usize, u32),
    /// Second product of the same batch; a full byproduct buffer stalls the machine.
    pub byproduct: Option<(usize, u32)>,
    pub ticks: u32,
    pub power: u32,
}
impl Process {
    pub fn block_cost(self, cells_per_metre: i32) -> Option<(Material, u32)> {
        self.block_input.map(|material| (material, cells_per_metre.max(1).pow(3) as u32))
    }

    pub fn ready(self, n: &Node, size: i32) -> bool {
        self.inputs.iter().all(|&(i, count)| n.input[i] >= count)
            && self.block_cost(size).is_none_or(|(material, count)| n.blocks.input[material.index()] >= count)
    }
}

/// Kiln mode 0 uses the same validated batch as the physical smeltery. Only
/// metals with an existing liquid-to-resource mapping can become kiln inputs.
fn bronze_process(rows: &[engine_content::reactions::ReactionRow]) -> Option<Process> {
    use engine_content::{BlockId, reactions::Reactant};
    let row = rows.iter().find(|row| row.batch_counts.is_some() && row.a_into == Some(BlockId::MoltenBronze))?;
    let (Reactant::Block(a), Some(Reactant::Block(b)), Some(product), Some((na, nb))) =
        (row.a, row.b, row.a_into, row.batch_counts) else { return None; };
    let resource = |liquid| crate::world::smeltery::DEF.metals.iter()
        .find(|metal| metal.liquid == liquid).map(|metal| metal.resource);
    // Factory resource buffers store 64 subunits per ingot. Preserve the
    // authored volume ratio and the kiln's original work and power costs.
    Some(Process {
        inputs: [(resource(a)?, u32::from(na) * 64), (resource(b)?, u32::from(nb) * 64), (0, 0)],
        block_input: None,
        output: (resource(product)?, (u32::from(na) + u32::from(nb)) * 64),
        byproduct: None,
        ticks: 24,
        power: 10,
    })
}

fn bronze_label(process: Option<Process>) -> String {
    use crate::world::inventory::Item;
    let Some(process) = process else { return "Bronze: no compatible batch reaction".into(); };
    let stack = |(item, units): (usize, u32)| format!("{} × {}", units / 64, Item::Resource(item as u8).name());
    format!("Bronze: {} + {} → {}", stack(process.inputs[0]), stack(process.inputs[1]), stack(process.output))
}

static KILN_BRONZE: std::sync::LazyLock<Option<Process>> =
    std::sync::LazyLock::new(|| bronze_process(&engine_content::reactions::ROWS));
static KILN_MODES: std::sync::LazyLock<[&'static str; 2]> = std::sync::LazyLock::new(|| [
    Box::leak(bronze_label(*KILN_BRONZE).into_boxed_str()),
    "Steel: 2 iron + 1 coal → 2 steel",
]);

use engine_content::fabrication::Fabrication;

/// Factory-lab drafts read from disk in place of the compiled tables, so table
/// edits are tested without a rebuild. Set before the first process lookup.
pub static PROCESS_DRAFT: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
pub static FABRICATION_DRAFT: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

/// Checks and installs draft tables; every table problem is reported.
pub fn load_drafts(processes: Option<String>, fabrication: Option<String>) -> Result<(), String> {
    if let Some(text) = processes {
        super::process_table::build(&text).map_err(|errors| format!("processes: {}", errors.join("; ")))?;
        PROCESS_DRAFT.set(Box::leak(text.into_boxed_str())).map_err(|_| "process table already loaded")?;
    }
    if let Some(text) = fabrication {
        engine_content::fabrication::parse(&text).map_err(|errors| format!("fabrication: {}", errors.join("; ")))?;
        FABRICATION_DRAFT.set(Box::leak(text.into_boxed_str())).map_err(|_| "fabrication table already loaded")?;
    }
    Ok(())
}

static FABRICATION: std::sync::LazyLock<(Vec<Fabrication>, Vec<&'static str>)> =
    std::sync::LazyLock::new(|| {
        let rows = engine_content::fabrication::parse(FABRICATION_DRAFT.get().copied().unwrap_or(include_str!("../../../../../assets/fabrication.ron")))
            .unwrap_or_else(|errors| panic!("assets/fabrication.ron: {}", errors.join("; ")));
        let labels = rows.iter().map(|row| &*Box::leak(row.name.clone().into_boxed_str())).collect();
        (rows, labels)
    });

pub(super) fn fabrication_output(mode: u32) -> &'static [usize] {
    FABRICATION.0.get(mode as usize).map_or(&[], |row| std::slice::from_ref(&row.output.0))
}

pub fn process(kind: Kind, recipe: usize) -> Option<Process> {
    if (kind, recipe) == (Kind::AlloyKiln, 0) {
        return *KILN_BRONZE;
    }
    if kind == Kind::Fabricator {
        let row = FABRICATION.0.get(recipe)?;
        let mut inputs = [(0, 0); 3];
        inputs[..row.inputs.len()].copy_from_slice(&row.inputs);
        return Some(Process { inputs, block_input: None, output: row.output, byproduct: None, ticks: row.ticks, power: row.power });
    }
    super::process_table::get(kind, recipe)
}
impl Kind {
    pub fn processor(self) -> bool {
        matches!(
            self,
            Self::Washer
                | Self::Crusher
                | Self::Furnace
                | Self::AlloyKiln
                | Self::Fabricator
                | Self::Cultivator
        )
    }
    pub fn modes(self) -> &'static [&'static str] {
        match self {
            Self::WetScrubber => &["Wet scrubber", "Water separator", "Steam boiler", "Fuel processor"],
            Self::GasGenerator => &["Fuel Combustion", "Steam Turbine"],
            Self::GasTank => &["All gases", "Fuel gas", "Oxygen", "Steam"],
            Self::AlloyKiln => &*KILN_MODES,
            Self::Assembler => &["Continuous", "Single Batch"],
            Self::Fabricator => &FABRICATION.1,
            Self::Pipe => &[
                "Transfer: routes machine output to accepting inputs",
                "Extract: pulls from a connected container or output",
                "Distribute: balances connected containers",
            ],
            Self::Storage => &[
                "Crate: logistics box on the item bus. A filter dedicates it to one resource",
                "Chest: 27 slots · Transfer In · Extract Out",
                "Barrel: bulk storage for one item. The first item locks it until the barrel is empty",
            ],
            Self::Generator => &[
                "Combustion: burns coke, coal, bio-ethanol or creosote for 16 kW",
                "Arcane pile: burns arcane cores for 64 kW. Needs coolant water or it melts down",
                "Biomass burner: burns fungal biomass, mulched logs and leaves at 12 kW",
                "Thermoelectric: opposite side faces. Lava + ice 4 kW or lava + water 2 kW per axis",
                "Waterwheel: water above a free drop beside the rotor. Up to 6 kW",
                "Fuel-fired boiler",
                "Hedron tap: stands within 8 m of a captured mana hedron. 6 kW",
            ],
            Self::InductionPort => &super::induction::PORT_MODES,
            Self::Cultivator => &[
                "Biomass: 1 reusable culture + 0.25 fibre → 2 biomass",
                "Lumen: 2 biomass → 1 fibre",
                "Fermenter: 2 biomass + 0.25 fibre → 1 bio-ethanol",
                "Grass Press",
                "Wet Fermentation",
            ],
            Self::Furnace => &[
                "Dust: 1 dust → 1 ingot",
                "Ore: 1 ore → 1 ingot, slower, no yield bonus",
                "Gold",
                "Arc: 1 ore → 1.5 ingots at 32 kW · 1 iron + 0.25 coke → 1 steel",
            ],
            Self::Washer => &["Ore", "Sluice"],
            Self::BlastFurnace => &[
                "Ironmaking: 2 iron ore + 0.5 coke → 3 iron ingots",
                "Steelmaking: 2 iron ingots + 0.5 coke → 2 steel ingots",
            ],
            Self::Battery => &super::power::BATTERY_MODES,
            Self::MiningDrill => &[
                "3 m Shaft",
                "9 m Quarry",
                "15 m Quarry",
            ],
            Self::Quarry => &["3 m", "9 m", "15 m"],
            _ => &[],
        }
    }
}

#[cfg(test)]
#[path = "recipes_tests.rs"]
mod tests;
impl Node {
    pub fn accepts(&self, i: usize) -> bool {
        if self.kind == Kind::WetScrubber && self.mode == 3 {
            return i == super::ETHANOL;
        }
        if self.kind == Kind::Fabricator && let Some(recipe) = self.lab_recipe {
            return i < super::RESOURCE_COUNT && super::chips::production::OPERATIONS.get(recipe as usize).is_some_and(|op|
                op.raw.iter().any(|(item, _)| *item == crate::world::inventory::Item::Resource(i as u8)));
        }
        if self.kind == Kind::Storage {
            if self.mode == 2 {
                let locked = self
                    .filter
                    .or_else(|| self.input.iter().position(|&n| n > 0));
                if let Some(locked) = locked {
                    return i == locked;
                }
            }
            return self.filter.is_none_or(|filter| filter == i) && i < super::RESOURCE_COUNT;
        }
        if self.filter.is_some_and(|filter| filter != i) {
            return false;
        }
        if let Some(order) = super::stock_order::read(self.kind, self.mode, self.target) {
            return process(self.kind, super::stock_order::recipe(self, order))
                .is_some_and(|r| r.inputs.iter().any(|&(item, n)| item == i && n > 0));
        }
        if self.kind == Kind::Generator {
            return if self.mode == 1 {
                i == super::pile::FUEL
            } else if self.mode == 2 {
                i == super::power::BIOMASS_FUEL.0
            } else {
                (self.mode == 0 || super::industry::boiler::active(self))
                    && super::power::COMBUSTION_FUELS.iter().any(|f| f.0 == i)
            };
        }
        if self.kind == Kind::Furnace {
            return match self.mode {
                1 => i < 3,
                GOLD_MODE => i == GOLD_CONCENTRATE,
                ARC_MODE => i < 3 || i == 8 || i == super::COKE,
                _ => (9..12).contains(&i),
            };
        }
        if self.kind == Kind::Washer {
            return !super::placer::sluice(self) && self.kind.accepts(i);
        }
        if self.kind == Kind::CokeOven {
            return i == 3;
        }
        if self.kind == Kind::Assembler {
            return super::assembler::accepts(self, i);
        }
        if self.kind.modes().is_empty() {
            return self.kind.accepts(i);
        }
        process(self.kind, self.mode as usize)
            .is_some_and(|r| r.inputs.iter().any(|&(item, n)| n > 0 && item == i))
    }
    pub fn process(&self) -> Option<Process> {
        self.recipe.and_then(|r| process(self.kind, r))
    }
    /// The same candidates drive automatic processing and physical chest intake.
    pub fn recipe_candidates(&self) -> std::ops::Range<usize> {
        if let Some(order) = super::stock_order::read(self.kind, self.mode, self.target) {
            let recipe = super::stock_order::recipe(self, order);
            return recipe..recipe + 1;
        }
        let (start, end) = match self.kind {
            Kind::Furnace if self.mode == GOLD_MODE => (GOLD_RECIPE, GOLD_RECIPE + 1),
            Kind::Washer if super::placer::sluice(self) => return 0..0,
            Kind::Crusher | Kind::Washer => (0, QUARTZ_RECIPE + 1),
            Kind::Furnace if self.mode == 1 => (0, 3),
            Kind::Furnace if self.mode == ARC_MODE => (ARC_RECIPES.start, ARC_RECIPES.end),
            Kind::Furnace => (9, 12),
            _ => (self.mode as usize, self.mode as usize + 1),
        };
        start..end
    }
    pub fn choose_recipe(&self, size: i32) -> Option<usize> {
        self.recipe_candidates().find(|&i| {
            process(self.kind, i).is_some_and(|recipe| recipe.ready(self, size))
        })
    }
}
