//! XMB focus metrics for the wand deck. Menu motes stay out of the world pool.
use crate::world::adventure::growth::ParticleRecipe;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XmbCategory {
    Bolts,
    Modifiers,
    Effects,
    Summons,
    Gear,
}

/// Fixed slot geometry. Focus distance changes contrast only, never size:
/// variable-width slots made the whole deck reflow when a drag changed the
/// selected slot, which moved the drop target out from under the pointer.
pub fn slot_metrics(distance: i32) -> (f32, f32) {
    match distance.abs() {
        0 => (46.0, 1.0),
        1 => (46.0, 0.86),
        _ => (46.0, 0.62),
    }
}

pub fn category(spell: u32) -> XmbCategory {
    if matches!(spell, 36) {
        XmbCategory::Summons
    } else if crate::world::adventure::wands::is_effect(spell)
        || matches!(spell, 1 | 7 | 24 | 25 | 34)
    {
        XmbCategory::Effects
    } else if crate::world::adventure::wands::is_modifier(spell) {
        XmbCategory::Modifiers
    } else {
        XmbCategory::Bolts
    }
}

#[derive(Clone, Copy)]
pub struct UiMote {
    pub color: [f32; 3],
    pub life: f32,
}

pub fn push_focus_mote(pool: &mut Vec<UiMote>, recipe: &ParticleRecipe) {
    if pool.len() >= 64 {
        return;
    }
    pool.push(UiMote {
        color: recipe.color,
        life: recipe.life,
    });
}

pub const SLIDE_MS: f32 = 180.0;
pub const WORLD_DIM: f32 = 0.55;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::adventure::growth::recipe;

    #[test]
    fn focus_metrics_keep_a_fixed_slot_size() {
        assert_eq!(slot_metrics(0), (46.0, 1.0));
        assert_eq!(slot_metrics(1), (46.0, 0.86));
        assert_eq!(slot_metrics(2), (46.0, 0.62));
        assert!(slot_metrics(2).1 < slot_metrics(0).1);
    }

    #[test]
    fn ui_motes_do_not_require_a_world_particle() {
        let mut pool = Vec::new();
        push_focus_mote(&mut pool, &recipe(35));
        assert_eq!(pool.len(), 1);
        for _ in 0..80 {
            push_focus_mote(&mut pool, &recipe(35));
        }
        assert_eq!(pool.len(), 64);
    }
}
