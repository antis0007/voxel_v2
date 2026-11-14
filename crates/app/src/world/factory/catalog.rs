//! Stable machine identities and authored machine definitions. Simulation and
//! persistence consume this catalogue through the factory facade; machine data
//! remains in assets/machines.ron.
use super::{B, power};

// One declaration owns the enum, its iteration order and its current count.
// IDs are persisted in saves and used as kit-array indices. Append new kinds;
// never renumber an existing kind or reuse its ID.
macro_rules! machine_kinds {
    ($($kind:ident = $id:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[repr(u32)]
        pub enum Kind {
            $($kind = $id),+
        }
        impl Kind {
            pub const ALL: [Self; Self::COUNT] = [$(Self::$kind),+];
            pub const COUNT: usize = [$(Self::$kind),+].len();
        }
    };
}

machine_kinds! {
    Generator = 0,
    Crusher = 1,
    Furnace = 2,
    Feeder = 3,
    Storage = 4,
    Pipe = 5,
    Cable = 6,
    Bioreactor = 7,
    AlloyKiln = 8,
    Fabricator = 9,
    Cultivator = 10,
    MiningDrill = 11,
    Pump = 12,
    FluidPipe = 13,
    Tank = 14,
    Washer = 15,
    Battery = 16,
    CokeOven = 17,
    BlastFurnace = 18,
    CokeBrick = 19,
    BlastBrick = 20,
    InductionCasing = 21,
    InductionPort = 22,
    InductionCell = 23,
    InductionProvider = 24,
    Transformer = 25,
    Assembler = 26,
    SearedBrick = 27,
    SmelteryController = 28,
    SmelteryDrain = 29,
    SmelteryFaucet = 30,
    CastingTable = 31,
    CastingBasin = 32,
    ExhaustDuct = 33,
    Chimney = 34,
    DustFilter = 35,
    WetScrubber = 36,
    GasTank = 37,
    GasGenerator = 38,
    Quarry = 39,
}

// Binary decoding indexes ALL with a saved ID. Catch gaps and reordering at
// compile time instead of silently loading a different machine.
const _: () = {
    let mut id = 0;
    while id < Kind::COUNT {
        assert!(
            Kind::ALL[id] as usize == id,
            "machine IDs must match catalogue order"
        );
        id += 1;
    }
};

impl Kind {
    /// Kinds that existed before save version 20.
    pub const LEGACY_COUNT: usize = 16;
    fn def(self) -> &'static MachineDef {
        &MACHINES[self as usize]
    }
    pub fn block(self) -> B {
        self.def().block
    }
    pub fn from_block(b: B) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.block() == b)
    }
    pub fn name(self) -> &'static str {
        self.def().name
    }
    /// Workbench cost in resource units (index, count).
    pub fn recipe(self) -> &'static [(usize, u32)] {
        &self.def().recipe
    }
    pub fn yield_count(self) -> u32 {
        self.def().yield_count
    }
    pub fn accepts(self, i: usize) -> bool {
        self.def().accepts.contains(&i)
    }
    /// Multiblock casing. Not a port, not a machine: it only gives shape to a
    /// coke oven or blast furnace.
    pub fn structural(self) -> bool {
        self.def().structural
    }
    /// Holds grid energy in a pool: batteries, transformers, matrix ports.
    pub fn stores_power(self) -> bool {
        self.def().stores_power
    }
    pub fn conduit(self) -> bool {
        self.def().conduit
    }
    pub fn powered(self) -> bool {
        self.def().powered
    }
    pub fn excavates(self) -> bool {
        matches!(self, Self::MiningDrill | Self::Quarry)
    }
    /// Machines that take the three-step upgrade: drives for consumers,
    /// cell chemistry for batteries.
    pub fn upgradable(self) -> bool {
        self.def().upgradable
    }

    pub(super) fn max_upgrade(self) -> u32 {
        if self == Self::Battery {
            power::MITHRIL_TIER
        } else {
            2
        }
    }
    /// Cost of the first upgrade, then of every later one.
    pub fn upgrade_cost(self, level: u32) -> &'static [(usize, u32)] {
        &self.def().upgrade_cost[usize::from(level != 0)]
    }
    pub fn water_port(self) -> bool {
        self.def().water_port
    }
    pub(crate) fn generator(self) -> bool {
        self.def().generator
    }
}

/// One machine kind's data row, from `assets/machines.ron`; indexed by `Kind as usize`.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MachineDef {
    pub kind: Kind,
    pub block: B,
    #[serde(deserialize_with = "leak")]
    pub name: &'static str,
    pub recipe: Vec<(usize, u32)>,
    pub yield_count: u32,
    /// Resource indices the machine takes as input.
    pub accepts: Vec<usize>,
    pub structural: bool,
    pub stores_power: bool,
    pub conduit: bool,
    pub powered: bool,
    pub upgradable: bool,
    /// Cost of upgrade level 0, then of every later level.
    pub upgrade_cost: [Vec<(usize, u32)>; 2],
    pub water_port: bool,
    pub generator: bool,
}

const MACHINES_RON: &str = include_str!("../../../../../assets/machines.ron");
pub static MACHINES: std::sync::LazyLock<Vec<MachineDef>> = std::sync::LazyLock::new(|| {
    let rows: Vec<MachineDef> =
        ron::from_str(MACHINES_RON).unwrap_or_else(|error| panic!("assets/machines.ron: {error}"));
    assert_eq!(
        rows.len(),
        Kind::COUNT,
        "assets/machines.ron must list every Kind once, in enum order"
    );
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(
            row.kind as usize, i,
            "assets/machines.ron row {i} is {:?}, expected enum order",
            row.kind
        );
    }
    rows
});

fn leak<'de, D: serde::Deserializer<'de>>(d: D) -> Result<&'static str, D::Error> {
    <String as serde::Deserialize>::deserialize(d).map(|s| &*Box::leak(s.into_boxed_str()))
}

#[cfg(test)]
mod tests;
