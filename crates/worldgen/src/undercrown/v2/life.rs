//! Generator 302: the inhabited Undercrown, first slice (the Drowned Borough).
//!
//! Loot and spell finds on top of the authored rooms: banded strongboxes and
//! reading stands, each a one-shot claim keyed by its `ObjectId` (prop index
//! 2 and up, so the rooms' original props keep index 1). What each one gives
//! is a row in `assets/undercrown_rewards.ron`; the app owns the grant.
//! Placement follows `docs/superpowers/specs/2026-11-13-undercrown-inhabited-design.md`:
//! a reward beat every two or three rooms, the biggest behind the sluice.
//!
//! Positions are metres from the anchor; a model stands on the floor 1.8 m
//! under its prop point (`props::floor_point`).
use super::layout::Builder;
use crate::undercrown::plan::{District as D, PropKind, Style as S};

/// First generator with Undercrown loot and spell finds. Dormant until bumped.
pub const VERSION: u32 = 302;

/// Prop point over a floor at `floor`: the model's foot sits on it.
fn on(floor: f32) -> f32 {
    floor + 1.8
}

pub(super) fn author(b: &mut Builder, bo: f32) {
    let mut put = |room: u16, kind: PropKind, label: &'static str, x: f32, floor: f32, z: f32| {
        if let Some(r) = b.room_of(D::DrownedBorough, room) {
            b.prop_n(r, 2, kind, label, x, on(floor), z, S::Canal);
        }
    };
    // Street: a stand on the far bank, past the houses, in the shallows.
    put(2, PropKind::Lectern, "Canal lectern", 50.0, bo, -62.0);
    // Courthouse: the evidence chest lies under the flood until the sluice
    // drains it, the district's main reward.
    put(3, PropKind::Cache, "Evidence chest", 93.0, bo - 2.0, -77.0);
    // Reservoir control: a lockbox by the bypass sluice.
    put(4, PropKind::Cache, "Reservoir lockbox", 66.5, bo, -98.0);
    // Ferry landing, the dry refuge: the ferryman's strongbox.
    put(5, PropKind::Cache, "Ferry strongbox", 43.5, bo + 1.0, -97.5);
    // The duct over the water: a stand someone carried up out of the flood.
    put(6, PropKind::Lectern, "Duct lectern", 88.0, bo + 4.0, -60.5);
    // Bathhouse: a stand half drowned in the reverse flow.
    put(7, PropKind::Lectern, "Drowned lectern", 120.0, bo + 1.0, -68.0);
    // Aqueduct house: a cache on the dry side of the old channel.
    put(8, PropKind::Cache, "Aqueduct cache", 35.0, bo + 2.0, -67.5);
}
