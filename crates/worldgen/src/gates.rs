//! Generator gates that more than one module reads.
//!
//! A gate number has one literal definition. When several features shipped
//! under the same number, the number lives here and each module keeps its own
//! name as an alias, so a renumbering is one edit and two modules can never
//! disagree. Gates used by a single module stay in that module; the registry
//! is `docs/worldgen/GENERATOR_GATES.md`, audited by
//! `node tools/axiom-dev/gates-cli.mjs check`.

/// Dungeon v2 and Undercrown v2 layouts, boss access and site modifiers.
pub const DUNGEON_V2_GENERATOR: u32 = 32;
/// Fine architecture detail, above ground and in the Undercrown.
pub const FINE_ARCHITECTURE_GENERATOR: u32 = 51;
/// Giant trees, dungeon ruins and Undercrown settling.
pub const GIANTS_AND_RUINS_GENERATOR: u32 = 52;
/// Lakes, regional climate scale, smaller seas and river fit.
pub const LAKES_AND_CLIMATE_GENERATOR: u32 = 53;
/// Connected roofs and stairs, dungeon workshops and tech mazes.
pub const CONNECTED_ROOFS_GENERATOR: u32 = 72;
/// Shoulder outcrops and the Manyfold weave.
pub const OUTCROPS_AND_WEAVE_GENERATOR: u32 = 81;
/// Surveyor ziggurats and planet site profiles.
pub const SURVEYOR_SITES_GENERATOR: u32 = 128;
