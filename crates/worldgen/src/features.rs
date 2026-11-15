//! Which world kinds run each surface feature.
//!
//! Flat worlds (Hills, Ridged) and Planet worlds share one feature catalogue.
//! A module asks [`enabled`] whether its world kind carries the feature, and
//! keeps its own generator-version threshold beside it, so historical gates
//! stay literal in the module that owns them.
//!
//! Each [`Feature`] has two rules:
//! - `legacy`: the exact world-kind test the module used before this registry.
//!   It must never change; old saves regenerate from it.
//! - the shared surface line: from [`SHARED_SURFACE_GENERATOR`] on, every
//!   feature runs on every [`Surface`]. A new feature normally needs no legacy
//!   arm at all: add a variant, return `false` from `legacy`, and gate the
//!   module on `enabled` plus its own version constant.
use engine_voxel::{WorldSampleConfig, WorldType};

pub use engine_voxel::SHARED_SURFACE_GENERATOR;

/// An opt-in from the historical version line. Shared releases enable these
/// features individually.
pub fn legacy_preview(version: u32, first: u32) -> bool {
    (first..SHARED_SURFACE_GENERATOR).contains(&version)
}

/// The two playable surface families. Dimensions, realms and Superflat have
/// their own generators and never share surface features.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Surface {
    Flat,
    Planet,
}

pub fn surface(cfg: &WorldSampleConfig) -> Option<Surface> {
    match cfg.world_type {
        WorldType::Hills | WorldType::Ridged => Some(Surface::Flat),
        WorldType::Planet => Some(Surface::Planet),
        _ => None,
    }
}

/// True when the world is on the shared surface line.
pub fn shared_line(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= SHARED_SURFACE_GENERATOR && surface(cfg).is_some()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Feature {
    /// Perched karst gardens, sinkholes and their skylights (`karst`).
    Karst,
    /// Arches, stacks and other landmark vistas (`vistas`).
    Vistas,
    /// The Fernwake district; households and farm freight hang off it.
    Fernwake,
    /// Generated sanctuary hell gates (`hell_gate`).
    HellGate,
    /// Dungeon sprawl records for the HUD and encounters.
    DungeonSprawl,
    /// Riverside sugar cane (`plants`).
    Cane,
    /// Distant settlement silhouettes on the horizon (`sites::visit_skylines`).
    SettlementSkylines,
    /// The starter workshop home and its quest line.
    Workshop,
    /// The Fernwake nursery, its watershed and farm freight (app side).
    Nursery,
    /// The flooded supply gallery off the workshop decline.
    WorkshopSupplies,
    /// Landmark giant trees (`trees::giant`).
    GiantTrees,
    /// Plants inside dry cave pockets (`flora::cave`).
    CaveFlora,
    /// Surveyor ziggurats (`surveyor`).
    Surveyor,
    /// Generated settlements and their roads (`civilization`).
    Civilization,
    /// The freight market (`planet_freight`).
    Freight,
}

impl Feature {
    /// The world-kind rule each module used before the registry. Frozen.
    fn legacy(self, cfg: &WorldSampleConfig) -> bool {
        use WorldType::*;
        let t = cfg.world_type;
        match self {
            Self::Karst | Self::Vistas | Self::Fernwake | Self::SettlementSkylines | Self::Workshop => {
                matches!(t, Hills | Ridged)
            }
            Self::HellGate => matches!(t, Hills | Nether),
            Self::DungeonSprawl | Self::Cane | Self::Nursery | Self::WorkshopSupplies => t == Hills,
            Self::GiantTrees | Self::CaveFlora => t != Planet,
            Self::Surveyor | Self::Freight => t == Planet,
            Self::Civilization => crate::planetary::active(cfg),
        }
    }
}

/// Whether this world kind carries `feature`. Version thresholds stay with the
/// caller: `cfg.generator_version >= module::VERSION && enabled(cfg, ..)`.
pub fn enabled(cfg: &WorldSampleConfig, feature: Feature) -> bool {
    feature.legacy(cfg) || shared_line(cfg)
}

