//! What a complex was built to do. Every ruin had a job (`WORLD_LORE.md`):
//! the job picks its depth, its stone, the rooms it needs, the hazard that is
//! the job gone wrong, and the landmark in its tall hall.
use super::Kind;
use crate::BlockId as B;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Job {
    /// Stratum I. The Ballast buried their dead and kept count.
    Catacombs,
    /// Stratum II. A cistern and ballast tank: water was weight.
    Undercroft,
    /// Stratum II. Braid-part foundries; more flawed couplings than one.
    ForgeHalls,
    /// Stratum I. A fungus farm that took the farmers.
    Warren,
    /// Stratum II. A quarantine maze; doors sealed from outside.
    Quarantine,
    /// Stratum II–III. An orrery that tracked the habitats.
    Orrery,
    /// Stratum III. A surveyors' hall whose floor holds nothing up.
    HollowFloor,
    /// Any stratum. A pilgrims' landing; depth is age.
    Sanctum,
    /// Stratum II. An engine works: couplings machined under seal, the maze
    /// its test floor. Generator 72 and later.
    Workshop,
}

/// Wall, trim, floor and accent stone for one complex.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub wall: B,
    pub alt: B,
    pub trim: B,
    pub floor: B,
    pub accent: B,
}

/// Ceiling hazards and states a room can roll.
#[derive(Clone, Copy, Debug)]
pub struct Odds {
    pub collapsed: f32,
    pub flooded: f32,
    pub overgrown: f32,
    pub breach: f32,
}

pub const ALL: [Job; 8] = [
    Job::Catacombs,
    Job::Undercroft,
    Job::ForgeHalls,
    Job::Warren,
    Job::Quarantine,
    Job::Orrery,
    Job::HollowFloor,
    Job::Sanctum,
];
/// Jobs a complex can roll from `WORKSHOP_VERSION` on.
pub const ALL_V72: [Job; 9] = [
    Job::Catacombs,
    Job::Undercroft,
    Job::ForgeHalls,
    Job::Warren,
    Job::Quarantine,
    Job::Orrery,
    Job::HollowFloor,
    Job::Sanctum,
    Job::Workshop,
];
/// First generator that rolls the engine works.
pub const WORKSHOP_VERSION: u32 = crate::gates::CONNECTED_ROOFS_GENERATOR;

impl Job {
    pub fn roll(r: f32) -> Self {
        ALL[(r * ALL.len() as f32) as usize % ALL.len()]
    }

    /// Shipped generators keep their eight-job roll bit for bit; from
    /// `WORKSHOP_VERSION` a separate roll `w` turns one complex in six into
    /// the engine works and leaves every other job where it was.
    pub fn roll_versioned(r: f32, w: f32, version: u32) -> Self {
        if version >= WORKSHOP_VERSION && w < 1.0 / 6.0 {
            Self::Workshop
        } else {
            Self::roll(r)
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Catacombs => "Forgotten catacombs",
            Self::Undercroft => "Sunken undercroft",
            Self::ForgeHalls => "Cinder forge-halls",
            Self::Warren => "Mycelial warren",
            Self::Quarantine => "Labyrinth of Sealed Doors",
            Self::Orrery => "Hall of Wandering Stars",
            Self::HollowFloor => "Hall of the Hollow Floor",
            Self::Sanctum => "Sanctum landing",
            Self::Workshop => "Buried engine works",
        }
    }

    /// Depth of the top level below the ground, in metres: deeper is older.
    pub fn depth(self, r: f32) -> f32 {
        let (lo, hi) = match self {
            Self::Catacombs | Self::Warren => (16.0, 24.0),
            Self::Undercroft | Self::ForgeHalls | Self::Quarantine => (24.0, 34.0),
            Self::Orrery => (28.0, 42.0),
            Self::HollowFloor => (36.0, 48.0),
            Self::Sanctum => (16.0, 44.0),
            Self::Workshop => (26.0, 40.0),
        };
        lo + (hi - lo) * r
    }

    pub fn palette(self) -> Palette {
        let p = |wall, alt, trim, floor, accent| Palette { wall, alt, trim, floor, accent };
        match self {
            Self::Catacombs => p(B::StoneBricks, B::WeatheredBricks, B::Cobblestone, B::Cobblestone, B::SilverBlock),
            Self::Undercroft => p(B::Limestone, B::StoneBricks, B::StoneBricks, B::Limestone, B::CopperBlock),
            Self::ForgeHalls => p(B::Basalt, B::BlastBrick, B::CokeOvenBrick, B::Basalt, B::CopperBlock),
            Self::Warren => p(B::Limestone, B::Cobblestone, B::MushroomStem, B::FungalSoil, B::GlowCap),
            Self::Quarantine => p(B::WeatheredBricks, B::Cobblestone, B::StoneBricks, B::Cobblestone, B::Planks),
            Self::Orrery => p(B::VoidStone, B::StoneBricks, B::SilverBlock, B::VoidStone, B::StarGlass),
            Self::HollowFloor => p(B::Basalt, B::Limestone, B::Limestone, B::Basalt, B::Crystal),
            Self::Sanctum => p(B::StoneBricks, B::Limestone, B::Limestone, B::StoneBricks, B::GoldBlock),
            Self::Workshop => p(B::DressedStone, B::SearedBrick, B::IronBlock, B::Basalt, B::CopperBlock),
        }
    }

    /// Room kinds the job needs, repeated to weight them.
    pub fn kinds(self) -> &'static [Kind] {
        use Kind as K;
        match self {
            Self::Catacombs => &[K::Ossuary, K::Ossuary, K::Crypt, K::Crypt, K::Shrine, K::Barracks, K::Chasm],
            Self::Undercroft => &[K::Cistern, K::Cistern, K::Sluice, K::Store, K::Shrine, K::Chasm],
            Self::ForgeHalls => &[K::Forge, K::Forge, K::Foundry, K::Store, K::Barracks],
            Self::Warren => &[K::Garden, K::Garden, K::Garden, K::Crypt, K::Store, K::Chasm],
            Self::Quarantine => &[K::Cells, K::Cells, K::Barracks, K::Store, K::Shrine],
            Self::Orrery => &[K::Orrery, K::Library, K::Library, K::Shrine, K::Store],
            Self::HollowFloor => &[K::Pitfall, K::Store, K::Library, K::Shrine, K::Chasm, K::Chasm],
            Self::Sanctum => &[K::Shrine, K::Shrine, K::Crypt, K::Library, K::Store],
            Self::Workshop => &[K::Forge, K::Foundry, K::Store, K::Store, K::Cells, K::Cistern],
        }
    }

    /// Puzzle that guards the sealed core.
    pub fn guard_puzzle(self, r: f32) -> Kind {
        match self {
            Self::Quarantine | Self::Workshop => Kind::Labyrinth,
            Self::HollowFloor => Kind::Pitfall,
            Self::Orrery => Kind::Orrery,
            _ if r < 0.5 => Kind::Labyrinth,
            _ => Kind::Pitfall,
        }
    }

    pub fn odds(self) -> Odds {
        let o = |collapsed, flooded, overgrown, breach| Odds { collapsed, flooded, overgrown, breach };
        match self {
            Self::Catacombs => o(0.35, 0.05, 0.10, 0.10),
            Self::Undercroft => o(0.15, 0.45, 0.10, 0.05),
            Self::ForgeHalls => o(0.30, 0.0, 0.0, 0.15),
            Self::Warren => o(0.10, 0.10, 0.70, 0.15),
            Self::Quarantine => o(0.20, 0.05, 0.15, 0.05),
            Self::Orrery => o(0.15, 0.0, 0.05, 0.10),
            Self::HollowFloor => o(0.25, 0.0, 0.0, 0.25),
            Self::Sanctum => o(0.15, 0.05, 0.10, 0.05),
            Self::Workshop => o(0.20, 0.05, 0.15, 0.20),
        }
    }

    /// Bay width: piers, ribs and sconces repeat at this spacing.
    pub fn bay(self) -> f32 {
        match self {
            Self::ForgeHalls | Self::HollowFloor | Self::Workshop => 6.0,
            Self::Catacombs | Self::Quarantine => 4.0,
            _ => 5.0,
        }
    }
}
