//! Tool material rows: the numeric properties trait rules and stat aggregation read.

use serde::{Deserialize, Serialize};

/// Row id in `assets/tools/materials.ron`, such as `"iron"`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MaterialId(pub String);

impl MaterialId {
    pub fn new(id: &str) -> Self {
        Self(id.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Numeric material property a predicate can compare.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Prop {
    Durability,
    MiningLevel,
    MiningSpeed,
    Attack,
    HandleModifier,
    Conductivity,
    Flammability,
    Density,
    Hardness,
}

/// Boolean material property a predicate can test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Flag {
    Magnetic,
    Living,
    Crystalline,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaterialProps {
    /// Uses before a head of this material breaks.
    pub durability: f32,
    pub mining_level: u8,
    pub mining_speed: f32,
    pub attack: f32,
    /// Multiplies tool durability when the material is a handle.
    pub handle_modifier: f32,
    /// 0 insulator to 1 pure metal.
    pub conductivity: f32,
    /// 0 inert to 1 tinder.
    pub flammability: f32,
    /// g/cm3.
    pub density: f32,
    /// Mohs scale.
    pub hardness: f32,
    pub magnetic: bool,
    pub living: bool,
    pub crystalline: bool,
}

impl MaterialProps {
    pub fn get(&self, prop: Prop) -> f32 {
        match prop {
            Prop::Durability => self.durability,
            Prop::MiningLevel => f32::from(self.mining_level),
            Prop::MiningSpeed => self.mining_speed,
            Prop::Attack => self.attack,
            Prop::HandleModifier => self.handle_modifier,
            Prop::Conductivity => self.conductivity,
            Prop::Flammability => self.flammability,
            Prop::Density => self.density,
            Prop::Hardness => self.hardness,
        }
    }

    pub fn flag(&self, flag: Flag) -> bool {
        match flag {
            Flag::Magnetic => self.magnetic,
            Flag::Living => self.living,
            Flag::Crystalline => self.crystalline,
        }
    }

    fn finite(&self) -> bool {
        [
            self.durability,
            self.mining_speed,
            self.attack,
            self.handle_modifier,
            self.conductivity,
            self.flammability,
            self.density,
            self.hardness,
        ]
        .iter()
        .all(|v| v.is_finite())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolMaterial {
    pub id: MaterialId,
    pub display_name: String,
    /// `engine_content::BlockId` variant name when the material exists as a block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    pub props: MaterialProps,
}

/// True when `name` is a `BlockId` variant.
pub fn block_exists(name: &str) -> bool {
    engine_content::ALL_BLOCK_IDS.iter().any(|id| format!("{id:?}") == name)
}

/// Table errors: duplicate ids, unknown block names, non-finite or non-positive values.
pub fn validate(rows: &[ToolMaterial]) -> Vec<String> {
    let mut errors = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if rows[..i].iter().any(|r| r.id == row.id) {
            errors.push(format!("material {:?} appears twice", row.id.0));
        }
        if row.id.0.is_empty() || row.id.0.contains(char::is_whitespace) {
            errors.push(format!("material id {:?} must be a single token", row.id.0));
        }
        if let Some(block) = &row.block
            && !block_exists(block)
        {
            errors.push(format!("material {:?} names unknown block {block:?}", row.id.0));
        }
        if !row.props.finite() {
            errors.push(format!("material {:?} has a non-finite property", row.id.0));
        }
        if row.props.durability < 1.0 {
            errors.push(format!("material {:?} durability below 1", row.id.0));
        }
        if row.props.handle_modifier <= 0.0 {
            errors.push(format!("material {:?} handle_modifier must be positive", row.id.0));
        }
        if row.props.density <= 0.0 {
            errors.push(format!("material {:?} density must be positive", row.id.0));
        }
    }
    errors
}
