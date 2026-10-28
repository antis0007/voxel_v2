//! Tool XP, levels and repair. Each level adds one modifier slot.

use crate::assembly::{ToolAssembly, ToolError};
use crate::materials::MaterialId;
use crate::parts::Progression;

/// Total XP needed to hold `level`, counted from level 0.
pub fn xp_to_reach(curve: &Progression, level: u8) -> u32 {
    let n = u64::from(level);
    let total = n * u64::from(curve.base_xp) + u64::from(curve.step_xp) * n * (n.saturating_sub(1)) / 2;
    u32::try_from(total).unwrap_or(u32::MAX)
}

/// Highest level `xp` reaches, capped at the curve's maximum.
pub fn level_for_xp(curve: &Progression, xp: u32) -> u8 {
    let mut level = 0;
    while level < curve.max_level && xp >= xp_to_reach(curve, level + 1) {
        level += 1;
    }
    level
}

impl ToolAssembly {
    /// Adds XP; returns the number of levels gained.
    pub fn gain_xp(&mut self, amount: u32) -> u8 {
        let curve = &crate::PARTS.progression;
        self.xp = self.xp.saturating_add(amount);
        let level = level_for_xp(curve, self.xp).max(self.level);
        let gained = level - self.level;
        self.level = level;
        gained
    }

    /// XP still missing for the next level; 0 at the maximum level.
    pub fn xp_to_next_level(&self) -> u32 {
        let curve = &crate::PARTS.progression;
        if self.level >= curve.max_level {
            return 0;
        }
        xp_to_reach(curve, self.level + 1).saturating_sub(self.xp)
    }

    /// Restores durability from `units` of the head material; returns the amount restored.
    pub fn repair(&mut self, material: &MaterialId, units: u32) -> Result<u32, ToolError> {
        let head = self.head_material()?;
        if head.id != *material {
            return Err(ToolError::NotHeadMaterial { expected: head.id.0.clone(), found: material.0.clone() });
        }
        let per_unit = (head.props.durability * crate::PARTS.progression.repair_per_unit).round().max(1.0) as u32;
        let full = self.stats()?.durability;
        let missing = full.saturating_sub(self.durability_left);
        let restored = missing.min(per_unit.saturating_mul(units));
        self.durability_left += restored;
        Ok(restored)
    }
}
