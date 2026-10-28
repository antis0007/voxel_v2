use super::{COKE, ETHANOL, Kind, Node};

#[derive(Clone, Copy)]
pub struct Process {
    pub inputs: [(usize, u32); 3],
    pub output: (usize, u32),
    pub ticks: u32,
    pub power: u32,
}
pub fn process(kind: Kind, recipe: usize) -> Option<Process> {
    let (inputs, output, ticks, power) = match (kind, recipe) {
        (Kind::Washer, i @ 0..=2) => ([(i, 64), (0, 0), (0, 0)], (i + 9, 192), 24, 12),
        (Kind::Crusher, i @ 0..=2) => ([(i, 64), (0, 0), (0, 0)], (i + 9, 128), 16, 8),
        (Kind::Furnace, i @ 9..=11) => ([(i, 64), (0, 0), (0, 0)], (i - 3, 64), 12, 6),
        // Raw ore straight into the furnace: no yield bonus, and slower.
        (Kind::Furnace, i @ 0..=2) => ([(i, 64), (0, 0), (0, 0)], (i + 6, 64), 20, 6),
        (Kind::AlloyKiln, 0) => ([(6, 192), (7, 64), (0, 0)], (14, 256), 24, 10),
        (Kind::AlloyKiln, 1) => ([(8, 128), (3, 64), (0, 0)], (15, 128), 32, 12),
        (Kind::Fabricator, 0) => ([(6, 128), (4, 64), (0, 0)], (16, 128), 20, 10),
        (Kind::Fabricator, 1) => ([(5, 64), (12, 64), (18, 64)], (17, 64), 28, 14),
        (Kind::Cultivator, 0) => ([(13, 64), (4, 16), (0, 0)], (13, 128), 32, 8),
        (Kind::Cultivator, 1) => ([(13, 128), (0, 0), (0, 0)], (4, 64), 32, 8),
        (Kind::Cultivator, 2) => ([(13, 128), (4, 16), (0, 0)], (ETHANOL, 64), 40, 6),
        (Kind::Fabricator, 2) => ([(5, 64), (13, 128), (0, 0)], (18, 64), 48, 24),
        // Salvage bootstraps the first fabricator; crystal electronics then
        // support a sustained factory without repeatedly raiding containers.
        (Kind::Fabricator, 3) => ([(6, 128), (7, 64), (5, 64)], (12, 128), 48, 16),
        // Heat-driven multiblocks: no electricity (power 0). The coke oven's
        // creosote by-product is added in `multiblock`.
        (Kind::CokeOven, 0) => ([(3, 64), (0, 0), (0, 0)], (COKE, 64), 40, 0),
        (Kind::BlastFurnace, 0) => ([(2, 128), (COKE, 32), (0, 0)], (8, 192), 64, 0),
        (Kind::BlastFurnace, 1) => ([(8, 128), (COKE, 32), (0, 0)], (15, 128), 48, 0),
        _ => return None,
    };
    Some(Process {
        inputs,
        output,
        ticks,
        power,
    })
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
            Self::AlloyKiln => &[
                "Bronze: 3 copper + 1 tin → 4 bronze",
                "Steel: 2 iron + 1 coal → 2 steel",
            ],
            Self::Fabricator => &[
                "Coils: 2 copper + 1 fibre → 2 coils",
                "Core: 1 crystal + 1 circuit + 1 wild essence → 1 arcane core",
                "Wild Essence: 1 crystal + 2 biomass → 1 wild essence (heavy power draw)",
                "Circuits: 2 copper + 1 tin + 1 crystal → 2 circuits (16 kW, 12 s)",
            ],
            Self::Pipe => &[
                "Pipe in: pushes machine outputs into the next machine or container that accepts them",
                "Pipe out: pulls from the adjacent chest, crate, barrel or machine output",
                "Distributor: evens one stack at a time across every container on this line",
            ],
            Self::Storage => &[
                "Crate: logistics box on the item bus. A filter dedicates it to one resource",
                "Chest: 27-slot grid. Pipe in fills it; pipe out or a distributor empties it",
                "Barrel: bulk storage for one item. The first item locks it until the barrel is empty",
            ],
            Self::Generator => &[
                "Combustion: burns coke, coal, bio-ethanol or creosote for 16 kW",
                "Arcane pile: burns arcane cores for 64 kW. Needs coolant water or it melts down",
                "Biomass burner: burns fungal biomass, mulched logs and leaves at 12 kW",
            ],
            Self::InductionPort => &super::induction::PORT_MODES,
            Self::Cultivator => &[
                "Biomass: 1 reusable culture + 0.25 fibre → 2 biomass",
                "Lumen: 2 biomass → 1 fibre",
                "Fermenter: 2 biomass + 0.25 fibre → 1 bio-ethanol",
            ],
            Self::Furnace => &[
                "Dust: 1 dust → 1 ingot",
                "Ore: 1 ore → 1 ingot, slower, no yield bonus",
            ],
            Self::BlastFurnace => &[
                "Iron: 2 iron ore + 0.5 coke → 3 iron ingots at 1500 °C",
                "Steel: 2 iron ingots + 0.5 coke → 2 steel at 1400 °C",
            ],
            Self::Battery => &super::power::BATTERY_MODES,
            Self::MiningDrill => &[
                "Shaft: 3 m wide, 24 m deep",
                "Quarry: 9 m wide, 24 m deep",
                "Quarry: 15 m wide, 24 m deep",
            ],
            _ => &[],
        }
    }
}
impl Node {
    pub fn accepts(&self, i: usize) -> bool {
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
        if self.kind == Kind::Generator {
            return if self.mode == 1 {
                i == super::pile::FUEL
            } else if self.mode == 2 {
                i == super::power::BIOMASS_FUEL.0
            } else {
                super::power::COMBUSTION_FUELS.iter().any(|f| f.0 == i)
            };
        }
        if self.kind == Kind::Furnace {
            return if self.mode == 1 {
                i < 3
            } else {
                (9..12).contains(&i)
            };
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
    pub fn choose_recipe(&self) -> Option<usize> {
        let (start, end) = match self.kind {
            Kind::Crusher | Kind::Washer => (0, 3),
            Kind::Furnace if self.mode == 1 => (0, 3),
            Kind::Furnace => (9, 12),
            _ => (self.mode as usize, self.mode as usize + 1),
        };
        (start..end).find(|&i| {
            process(self.kind, i).is_some_and(|r| r.inputs.iter().all(|&(j, n)| self.input[j] >= n))
        })
    }
}
