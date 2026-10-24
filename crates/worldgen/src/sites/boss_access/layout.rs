//! Shrine layout boxes and per-family rules for generator 32 boss access.
use super::B;

/// Boss family, keyed by the reliquary's relic kind (see the app's `bosses::kind`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Family {
    /// Relic 3, the Unfinished Engine: feed ore into the furnace hopper.
    Foundry,
    /// Relic 10, the Last Tollkeeper: pull the lever pin from its bracket.
    Toll,
    /// Relic 5, the Glassweaver: light both braziers.
    Archive,
    /// Relic 8, the Prism Hart: climb the grown ledges and break the roof prisms.
    Hart,
}

/// `[u0, u1, y0, y1, w0, w1]`, half-open, shrine-local metres.
pub type Box3 = [i32; 6];

pub const DOOR: Box3 = [-1, 1, 1, 4, -4, -3];
/// Cracked fallback into the entry hall, from the courtyard's right alley.
pub const WEAK_WALL: Box3 = [3, 4, 1, 3, -1, 1];
pub(super) const MAIN: Box3 = [-4, 4, 0, 6, -4, 4];
pub(super) const MAIN_ROOM: Box3 = [-3, 3, 1, 5, -3, 3];
/// Arena floor kept free of pillars, rubble and dressing around the relic.
pub const CLEAR: Box3 = [-4, 4, 1, 5, 15, 23];
/// Reliquary spot (shrine-local metres), in the middle of the arena.
pub(super) const RELIC: [f32; 3] = [0.0, 1.0, 19.0];

/// How a treasure room opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Opens {
    /// Any seal cell no longer holds weathered bricks.
    Cracked,
    /// The log pin in this box is broken; the app clears the gate.
    Lever(Box3),
    /// The boss has fallen; the app clears the void-stone ward.
    Boss,
}

/// One treasure room. Boxes are shrine-local and already mirrored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Treasure {
    /// Stable per complex: 0 hidden, 1 lever, 2 boss vault.
    pub index: u8,
    /// Loot tier 1-3.
    pub tier: u8,
    pub seal: Box3,
    /// Void stone until the app grants the cache, then tombstone (once only).
    pub pedestal: Box3,
    /// Air until granted; the app fills it with `Family::cache_block`.
    pub cache: Box3,
    pub opens: Opens,
}

/// Canonical (unmirrored) treasure rooms: seal, pedestal, cache, room air, wing.
pub(super) const HIDDEN: [Box3; 5] =
    [[6, 8, 1, 3, 10, 11], [8, 9, 1, 2, 7, 8], [8, 9, 2, 3, 7, 8], [5, 9, 1, 4, 6, 10], [4, 10, 0, 5, 5, 11]];
pub(super) const LEVER_ROOM: [Box3; 5] = [
    [-13, -11, 1, 3, 18, 20],
    [-15, -14, 1, 2, 19, 20],
    [-15, -14, 2, 3, 19, 20],
    [-15, -13, 1, 4, 16, 22],
    [-16, -11, 0, 5, 15, 23],
];
pub(super) const VAULT_ROOM: [Box3; 5] = [
    [11, 13, 1, 3, 18, 20],
    [14, 15, 1, 2, 19, 20],
    [14, 15, 2, 3, 19, 20],
    [13, 15, 1, 4, 16, 22],
    [11, 16, 0, 5, 15, 23],
];

pub(super) const HOPPER: Box3 = [2, 5, 1, 2, -8, -5];
pub(super) const HOPPER_SOCKET: Box3 = [3, 4, 1, 2, -7, -6];
pub(super) const HOPPER_COAL: [Box3; 2] = [[2, 3, 1, 2, -8, -7], [4, 5, 1, 2, -6, -5]];
pub(super) const HOPPER_TORCH: Box3 = [4, 5, 2, 3, -8, -7];
pub(super) const LEVER_BRACKET: Box3 = [2, 3, 1, 3, -6, -5];
pub(super) const LEVER_PIN: Box3 = [2, 3, 2, 3, -7, -6];
pub(super) const LEVER_TORCH: Box3 = [2, 3, 3, 4, -6, -5];
pub(super) const BRAZIERS: [Box3; 2] = [[-3, -2, 1, 2, -7, -6], [2, 3, 1, 2, -7, -6]];
pub(super) const BRAZIER_SOCKETS: [Box3; 2] = [[-3, -2, 2, 3, -7, -6], [2, 3, 2, 3, -7, -6]];
pub(super) const WALL_CRYSTALS: [Box3; 2] = [[-3, -2, 3, 4, -4, -3], [2, 3, 3, 4, -4, -3]];
pub(super) const PRISMS: [Box3; 3] = [[-3, -2, 6, 7, -3, -2], [-3, -2, 6, 7, 2, 3], [2, 3, 6, 7, -3, -2]];

/// How a key box reads as solved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    /// Any cell holds one of the accepted blocks (torches match by `is_torch`).
    Fill(&'static [B]),
    /// Any cell no longer holds the generated block.
    Break(B),
}

pub(super) const ORES: &[B] = &[B::CoalOre, B::IronOre, B::CopperOre, B::TinOre];
pub(super) const LIGHTS: &[B] = &[B::Torch, B::Crystal, B::GlowCap, B::Fire];

impl Family {
    pub fn door_block(self) -> B {
        match self {
            Self::Foundry => B::BlastBrick,
            Self::Toll => B::Planks,
            Self::Archive => B::StarGlass,
            Self::Hart => B::Ice,
        }
    }
    pub(super) fn wall(self) -> B {
        match self {
            Self::Foundry => B::Basalt,
            Self::Toll | Self::Archive => B::StoneBricks,
            Self::Hart => B::Limestone,
        }
    }
    /// Reward vault contents; one metre of an existing loot cache.
    pub fn cache_block(self) -> B {
        match self {
            Self::Foundry => B::SalvageCrate,
            Self::Hart => B::CultivatorCache,
            Self::Toll | Self::Archive => B::SurveyCache,
        }
    }
    /// Log pin (canonical box) on this family's upper level that opens the lever room.
    pub(super) fn lever(self) -> Box3 {
        match self {
            Self::Foundry | Self::Toll => [-11, -10, 6, 7, 21, 22],
            Self::Archive => [-8, -7, 9, 10, 18, 19],
            Self::Hart => [10, 11, 7, 8, 25, 26],
        }
    }
    /// Every key box must be solved to open the door.
    pub fn keys(self) -> (&'static [Box3], Rule) {
        match self {
            Self::Foundry => (std::slice::from_ref(&HOPPER_SOCKET), Rule::Fill(ORES)),
            Self::Toll => (std::slice::from_ref(&LEVER_PIN), Rule::Break(B::Log)),
            Self::Archive => (&BRAZIER_SOCKETS, Rule::Fill(LIGHTS)),
            Self::Hart => (&PRISMS, Rule::Break(B::Crystal)),
        }
    }
    pub fn opened_message(self) -> &'static str {
        match self {
            Self::Foundry => "The hopper catches. The furnace door grinds open.",
            Self::Toll => "The pin drops. The toll gate swings aside.",
            Self::Archive => "Both braziers burn. The glass vault door clears.",
            Self::Hart => "The prisms shatter. The ice door falls to frost.",
        }
    }
}
