//! `assets/biomes.ron`: one prototype per placed biome family, in the field space the worldgen
//! weights read. Ocean, coast and marsh are rule-placed and have no row.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const SOURCE: &str = include_str!("../../../assets/biomes.ron");

/// The side of an axis centre that carries no penalty, for limits like a treeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Open {
    #[default]
    Neither,
    /// Values above the centre keep full weight.
    Above,
    /// Values below the centre keep full weight.
    Below,
}

/// A tolerance curve on one field: full weight at `centre`, falling off over `width`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Axis {
    pub centre: f32,
    pub width: f32,
    #[serde(default)]
    pub open: Open,
}

/// Rock a family needs under it before it can win a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Substrate {
    Volcanic,
    Carbonate,
    Anomaly,
}

/// One family prototype. An absent axis does not constrain the family.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FamilyDef {
    /// `Biome` variant name, e.g. `VolcanicField`.
    pub name: String,
    /// Stable snake_case key for data and the naming generator; display text stays on `Biome::label`.
    pub id: String,
    pub temperature: Axis,
    pub moisture: Axis,
    /// Metres.
    #[serde(default)]
    pub elevation: Option<Axis>,
    #[serde(default)]
    pub slope: Option<Axis>,
    /// 0..1, drainage and lake edge.
    #[serde(default)]
    pub wetness: Option<Axis>,
    #[serde(default)]
    pub requires: Option<Substrate>,
    /// Added to the family's log-weight.
    #[serde(default)]
    pub bias: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BiomeTable {
    pub families: Vec<FamilyDef>,
}

/// The parsed `assets/biomes.ron`, loaded once.
pub fn biome_table() -> &'static BiomeTable {
    static TABLE: OnceLock<BiomeTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let table: BiomeTable = ron::from_str(SOURCE).unwrap_or_else(|e| panic!("assets/biomes.ron: {e}"));
        assert!(!table.families.is_empty(), "assets/biomes.ron: no families");
        let mut ids = std::collections::HashSet::new();
        for family in &table.families {
            assert!(ids.insert(&family.id), "assets/biomes.ron: duplicate family {}", family.id);
            assert!(family.bias.is_finite(), "assets/biomes.ron: invalid bias for {}", family.id);
            for axis in [Some(&family.temperature), Some(&family.moisture), family.elevation.as_ref(), family.slope.as_ref(), family.wetness.as_ref()].into_iter().flatten() {
                assert!(axis.centre.is_finite() && axis.width.is_finite() && axis.width > 0.0,
                    "assets/biomes.ron: invalid axis for {}", family.id);
            }
        }
        table
    })
}

