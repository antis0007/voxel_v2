//! Part kinds, tool schemas and the progression curve from `assets/tools/parts.ron`.

use serde::{Deserialize, Serialize};

/// Part kind id in `assets/tools/parts.ron`, such as `"head"`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PartKind(pub String);

impl PartKind {
    pub fn new(id: &str) -> Self {
        Self(id.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a part does in the assembly; the code branches on this, not on the kind id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartRole {
    /// Working part: sets mining level, repairs the tool.
    Head,
    Binding,
    /// Its material's handle modifier multiplies durability.
    Handle,
    Reinforcement,
    String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartKindDef {
    pub id: PartKind,
    pub display_name: String,
    pub role: PartRole,
    /// Material units to cast one part; also the part's share of tool mass.
    pub material_cost: u8,
}

/// How much of a part material's properties reach the tool stats.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatWeights {
    pub durability: f32,
    pub mining_speed: f32,
    pub attack: f32,
    /// The part's material mining level counts toward the tool's.
    pub mining_level: bool,
    /// The part's material handle modifier multiplies tool durability.
    pub handle: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SchemaPart {
    pub kind: PartKind,
    pub weights: StatWeights,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolSchema {
    pub id: String,
    pub display_name: String,
    /// Required parts in assembly order; a kind may repeat.
    pub parts: Vec<SchemaPart>,
    pub modifier_slots: u8,
}

/// XP curve and repair rate shared by every schema.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Progression {
    /// XP from level 0 to level 1.
    pub base_xp: u32,
    /// Extra XP each further level costs over the previous one.
    pub step_xp: u32,
    pub max_level: u8,
    /// Fraction of the head material's durability one material unit restores.
    pub repair_per_unit: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartsFile {
    pub part_kinds: Vec<PartKindDef>,
    pub schemas: Vec<ToolSchema>,
    pub progression: Progression,
}

impl PartsFile {
    pub fn part_kind(&self, id: &str) -> Option<&PartKindDef> {
        self.part_kinds.iter().find(|k| k.id.0 == id)
    }

    pub fn schema(&self, id: &str) -> Option<&ToolSchema> {
        self.schemas.iter().find(|s| s.id == id)
    }
}

/// Table errors: duplicate ids, unknown kinds, schemas without a head, bad weights.
pub fn validate(file: &PartsFile) -> Vec<String> {
    let mut errors = Vec::new();
    for (i, kind) in file.part_kinds.iter().enumerate() {
        if file.part_kinds[..i].iter().any(|k| k.id == kind.id) {
            errors.push(format!("part kind {:?} appears twice", kind.id.0));
        }
        if kind.material_cost == 0 {
            errors.push(format!("part kind {:?} costs no material", kind.id.0));
        }
    }
    for (i, schema) in file.schemas.iter().enumerate() {
        if file.schemas[..i].iter().any(|s| s.id == schema.id) {
            errors.push(format!("schema {:?} appears twice", schema.id));
        }
        if schema.parts.is_empty() {
            errors.push(format!("schema {:?} has no parts", schema.id));
        }
        let mut heads = 0;
        for part in &schema.parts {
            match file.part_kind(&part.kind.0) {
                None => errors.push(format!("schema {:?} uses unknown part kind {:?}", schema.id, part.kind.0)),
                Some(def) if def.role == PartRole::Head => heads += 1,
                Some(_) => {}
            }
            let w = part.weights;
            if ![w.durability, w.mining_speed, w.attack].iter().all(|v| v.is_finite() && *v >= 0.0) {
                errors.push(format!("schema {:?} part {:?} has a negative or non-finite weight", schema.id, part.kind.0));
            }
        }
        if heads == 0 {
            errors.push(format!("schema {:?} has no head part", schema.id));
        }
    }
    let p = file.progression;
    if p.base_xp == 0 || p.max_level == 0 || !(p.repair_per_unit.is_finite() && p.repair_per_unit > 0.0) {
        errors.push("progression needs base_xp > 0, max_level > 0 and a positive repair_per_unit".to_owned());
    }
    errors
}
