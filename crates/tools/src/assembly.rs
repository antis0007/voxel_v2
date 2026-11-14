//! A tool as parts with materials, and the stats and traits computed from them.

use crate::materials::{MaterialId, ToolMaterial};
use crate::parts::{PartKind, PartKindDef, PartRole, ToolSchema};
use crate::traits::AppliedTrait;
use crate::{MATERIALS, PARTS, TRAITS};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolStats {
    pub durability: u32,
    pub mining_level: u8,
    pub mining_speed: f32,
    pub attack: f32,
    /// Sum of part material cost times density, in material units times g/cm3.
    pub mass: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolError {
    UnknownSchema(String),
    UnknownPartKind(String),
    UnknownMaterial(String),
    PartCount { expected: usize, found: usize },
    PartMismatch { index: usize, expected: String, found: String },
    ModifierSlotsFull { slots: u8 },
    NotHeadMaterial { expected: String, found: String },
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSchema(id) => write!(f, "unknown tool schema {id:?}"),
            Self::UnknownPartKind(id) => write!(f, "unknown part kind {id:?}"),
            Self::UnknownMaterial(id) => write!(f, "unknown tool material {id:?}"),
            Self::PartCount { expected, found } => write!(f, "schema needs {expected} parts, {found} given"),
            Self::PartMismatch { index, expected, found } => {
                write!(f, "part {index} must be {expected:?}, found {found:?}")
            }
            Self::ModifierSlotsFull { slots } => write!(f, "all {slots} modifier slots used"),
            Self::NotHeadMaterial { expected, found } => {
                write!(f, "repair needs {expected:?}, found {found:?}")
            }
        }
    }
}

impl std::error::Error for ToolError {}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolAssembly {
    pub schema: String,
    pub parts: Vec<(PartKind, MaterialId)>,
    pub modifiers: Vec<String>,
    pub xp: u32,
    pub level: u8,
    pub durability_left: u32,
}

/// A part with its kind row and material row looked up.
pub type ResolvedPart = (&'static PartKindDef, &'static ToolMaterial);

impl ToolAssembly {
    /// Assembles `schema` from one material per schema part, in schema order.
    pub fn new(schema: &str, materials: &[MaterialId]) -> Result<Self, ToolError> {
        let def = PARTS.schema(schema).ok_or_else(|| ToolError::UnknownSchema(schema.to_owned()))?;
        if def.parts.len() != materials.len() {
            return Err(ToolError::PartCount { expected: def.parts.len(), found: materials.len() });
        }
        let parts: Vec<(PartKind, MaterialId)> =
            def.parts.iter().zip(materials).map(|(p, m)| (p.kind.clone(), m.clone())).collect();
        let mut tool = Self { schema: schema.to_owned(), parts, modifiers: Vec::new(), xp: 0, level: 0, durability_left: 0 };
        tool.durability_left = tool.stats()?.durability;
        Ok(tool)
    }

    pub fn schema_def(&self) -> Result<&'static ToolSchema, ToolError> {
        PARTS.schema(&self.schema).ok_or_else(|| ToolError::UnknownSchema(self.schema.clone()))
    }

    /// Shared authored behavior for every consumer. The schema ID is identity only.
    pub fn capabilities(&self) -> Result<crate::capabilities::Capabilities, ToolError> {
        Ok(self.schema_def()?.capabilities)
    }

    /// Preserve absolute remaining uses after authored stat rebalancing. A lower
    /// maximum clamps an older save; a higher maximum never repairs the tool.
    pub fn reconcile_durability(&mut self) -> Result<(), ToolError> {
        self.durability_left = self.durability_left.min(self.stats()?.durability);
        Ok(())
    }

    /// Parts with their rows; errors when a part disagrees with the schema.
    pub fn resolved(&self) -> Result<Vec<ResolvedPart>, ToolError> {
        let schema = self.schema_def()?;
        if schema.parts.len() != self.parts.len() {
            return Err(ToolError::PartCount { expected: schema.parts.len(), found: self.parts.len() });
        }
        let mut out = Vec::with_capacity(self.parts.len());
        for (index, (expected, (kind, material))) in schema.parts.iter().zip(&self.parts).enumerate() {
            if expected.kind != *kind {
                return Err(ToolError::PartMismatch {
                    index,
                    expected: expected.kind.0.clone(),
                    found: kind.0.clone(),
                });
            }
            let def = PARTS.part_kind(&kind.0).ok_or_else(|| ToolError::UnknownPartKind(kind.0.clone()))?;
            let row = crate::material(&material.0).ok_or_else(|| ToolError::UnknownMaterial(material.0.clone()))?;
            out.push((def, row));
        }
        Ok(out)
    }

    /// Weighted sums over parts; handle modifiers multiply durability.
    pub fn stats(&self) -> Result<ToolStats, ToolError> {
        let schema = self.schema_def()?;
        let parts = self.resolved()?;
        let (mut durability, mut speed, mut attack, mut mass, mut handle) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 1.0f32);
        let mut mining_level = 0u8;
        for (schema_part, (kind, material)) in schema.parts.iter().zip(&parts) {
            let w = schema_part.weights;
            let p = material.props;
            durability += w.durability * p.durability;
            speed += w.mining_speed * p.mining_speed;
            attack += w.attack * p.attack;
            mass += f32::from(kind.material_cost) * p.density;
            if w.handle {
                handle *= p.handle_modifier;
            }
            if w.mining_level {
                mining_level = mining_level.max(p.mining_level);
            }
        }
        Ok(ToolStats {
            durability: crate::modifiers::stat(self, "durability", durability * handle).round().max(1.0) as u32,
            mining_level: crate::modifiers::stat(self, "mining_level", mining_level as f32).min(255.0) as u8,
            mining_speed: crate::modifiers::stat(self, "mining_speed", speed),
            attack: crate::modifiers::stat(self, "attack", attack),
            mass,
        })
    }

    pub fn traits(&self) -> Result<Vec<AppliedTrait>, ToolError> {
        let parts = self.resolved()?;
        let roles: Vec<(PartRole, &crate::materials::MaterialProps)> =
            parts.iter().map(|(kind, material)| (kind.role, &material.props)).collect();
        Ok(crate::traits::assemble_traits(&TRAITS, &roles))
    }

    /// Material of the first head-role part, else of the first part that sets mining level.
    /// Validation guarantees one of the two exists; a shield's plate is its working part.
    pub fn head_material(&self) -> Result<&'static ToolMaterial, ToolError> {
        let schema = self.schema_def()?;
        let parts = self.resolved()?;
        parts.iter()
            .find(|(kind, _)| kind.role == PartRole::Head)
            .or_else(|| schema.parts.iter().zip(&parts).find(|(p, _)| p.weights.mining_level).map(|(_, part)| part))
            .map(|(_, material)| *material)
            .ok_or_else(|| ToolError::UnknownSchema(self.schema.clone()))
    }

    /// Schema slots plus one per level.
    pub fn modifier_slots(&self) -> Result<u8, ToolError> {
        Ok(self.schema_def()?.modifier_slots.saturating_add(self.level))
    }

    pub fn add_modifier(&mut self, id: &str) -> Result<(), ToolError> {
        let slots = self.modifier_slots()?;
        if self.modifiers.len() >= usize::from(slots) {
            return Err(ToolError::ModifierSlotsFull { slots });
        }
        self.modifiers.push(id.to_owned());
        Ok(())
    }

    /// Spends durability; returns true once the tool is broken.
    pub fn wear(&mut self, amount: u32) -> bool {
        self.durability_left = self.durability_left.saturating_sub(amount);
        self.is_broken()
    }

    pub fn is_broken(&self) -> bool {
        self.durability_left == 0
    }

    /// A replacement preserves earned progress and absolute wear, never repairs for free.
    pub fn replace_part(&mut self, index: usize, material: &MaterialId) -> Result<(), ToolError> {
        self.resolved()?;
        if index >= self.parts.len() {
            return Err(ToolError::PartCount { expected: self.parts.len(), found: index + 1 });
        }
        let missing = self.stats()?.durability.saturating_sub(self.durability_left);
        let broken = self.is_broken();
        let mut next = self.clone();
        next.parts[index].1 = material.clone();
        let full = next.stats()?.durability;
        next.traits()?;
        next.durability_left = if broken { 0 } else { full.saturating_sub(missing) };
        *self = next;
        Ok(())
    }
}

/// Every material row, for callers that list choices.
pub fn materials() -> &'static [ToolMaterial] {
    &MATERIALS
}
