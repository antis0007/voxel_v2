//! Melee factors for the held weapon, from an assembled tool or the legacy forge fields.
use super::{status::Element, tinkering};
use engine_tools::{ToolAssembly, TraitEffect, modifiers::stat};

/// Bit the habitat reward sets; it stays a player perk whatever is held.
const LEGACY_MOSS: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blade {
    /// Multiplies the whole hit.
    pub damage: f32,
    pub sweep: f32,
    /// Higher swings faster.
    pub speed: f32,
    pub edge: Option<Element>,
    pub color: Option<[u8; 3]>,
    pub hit_damage: f32,
    pub hit_mana: f32,
    pub hit_heal: f32,
    pub finisher_stagger: f32,
    /// An assembled tool in hand earns the armed reach bonus like a legacy blade.
    pub armed: bool,
}

impl Default for Blade {
    fn default() -> Self { Self::legacy(0, 0, 0, 0) }
}

impl Blade {
    /// Pre-merge forge numbers, kept exact so old saves fight the same.
    pub fn legacy(head: u8, bind: u8, grip: u8, mods: u8) -> Self {
        Self {
            damage: tinkering::head_multiplier(head) * if mods & 1 != 0 { 1.08 } else { 1.0 },
            sweep: tinkering::binding_multiplier(bind),
            speed: tinkering::grip_multiplier(grip),
            edge: tinkering::edge(head),
            color: (head != 0).then(|| tinkering::material_color(head)),
            hit_damage: if mods & 16 != 0 { 4.0 } else { 0.0 },
            hit_mana: if mods & 2 != 0 { 2.0 } else { 0.0 },
            hit_heal: if mods & LEGACY_MOSS != 0 { 2.0 } else { 0.0 },
            finisher_stagger: if mods & 4 != 0 { 0.25 } else { 0.0 },
            armed: false,
        }
    }

    /// Attack is already in the base hit; parts shape reach and tempo, traits give the edge.
    pub fn assembled(tool: &ToolAssembly, mods: u8) -> Self {
        let traits = tool.traits().unwrap_or_default();
        let has = |effect| traits.iter().any(|t| t.effect == effect);
        let edge = if has(TraitEffect::Conductive) { Some(Element::Shock) } else if has(TraitEffect::LavaProof) { Some(Element::Fire) } else { None };
        Self {
            damage: 1.0,
            sweep: sweep(tool),
            speed: speed(tool),
            edge,
            color: None,
            hit_damage: stat(tool, "hit_damage", 0.0) + jagged(tool, &traits),
            hit_mana: stat(tool, "hit_mana", 0.0),
            hit_heal: stat(tool, "hit_heal", 0.0) + if mods & LEGACY_MOSS != 0 { 2.0 } else { 0.0 },
            finisher_stagger: stat(tool, "finisher_stagger", 0.0),
            armed: true,
        }
    }
}

/// Jagged (crystalline) heads cut deeper as they chip: up to 2 extra damage
/// per stack when the tool is nearly broken, none when it is whole.
fn jagged(tool: &ToolAssembly, traits: &[engine_tools::AppliedTrait]) -> f32 {
    let stacks: f32 = traits.iter().filter(|t| t.effect == TraitEffect::Jagged).map(|t| f32::from(t.stacks)).sum();
    let full = tool.stats().map_or(0, |s| s.durability);
    if stacks == 0.0 || full == 0 {
        return 0.0;
    }
    let worn = 1.0 - tool.durability_left as f32 / full as f32;
    2.0 * stacks * worn.clamp(0.0, 1.0)
}

/// Bigger working parts cover a wider arc.
fn sweep(tool: &ToolAssembly) -> f32 {
    let cost = tool.resolved().ok().and_then(|parts| parts.first().map(|(kind, _)| f32::from(kind.material_cost))).unwrap_or(2.0);
    (0.85 + 0.075 * cost).clamp(0.85, 1.2)
}

/// Swing speed against the same tool made all of iron: lighter swings faster.
fn speed(tool: &ToolAssembly) -> f32 {
    let Ok(stats) = tool.stats() else { return 1.0 };
    let iron = vec![engine_tools::MaterialId("iron".into()); tool.parts.len()];
    let reference = ToolAssembly::new(&tool.schema, &iron).ok().and_then(|t| t.stats().ok()).map_or(stats.mass, |s| s.mass);
    if stats.mass <= 0.0 { return 1.0; }
    (reference / stats.mass).powf(0.25).clamp(0.8, 1.25)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sword(materials: [&str; 3]) -> ToolAssembly {
        let schema = engine_tools::PARTS.schemas.iter().find(|s| s.id == "sword").map(|s| s.parts.len()).unwrap_or(3);
        let ids: Vec<_> = (0..schema).map(|i| engine_tools::MaterialId(materials[i.min(2)].into())).collect();
        ToolAssembly::new("sword", &ids).unwrap()
    }

    #[test]
    fn legacy_default_is_neutral() {
        let b = Blade::default();
        assert_eq!((b.damage, b.sweep, b.speed, b.hit_damage, b.edge), (1.0, 1.0, 1.0, 0.0, None));
    }

    #[test]
    fn legacy_matches_old_forge_formulas() {
        let b = Blade::legacy(3, 2, 5, 1 | 16);
        assert!((b.damage - tinkering::head_multiplier(3) * 1.08).abs() < 1e-6);
        assert_eq!(b.sweep, tinkering::binding_multiplier(2));
        assert_eq!(b.speed, tinkering::grip_multiplier(5));
        assert_eq!(b.edge, Some(Element::Fire));
        assert_eq!(b.hit_damage, 4.0);
    }

    #[test]
    fn lighter_assembled_sword_swings_faster() {
        let heavy = Blade::assembled(&sword(["steel", "steel", "steel"]), 0);
        let light = Blade::assembled(&sword(["bone", "bone", "wood"]), 0);
        assert!(light.speed > heavy.speed, "{light:?} vs {heavy:?}");
    }

    #[test]
    fn assembled_tool_counts_as_armed() {
        assert!(Blade::assembled(&sword(["iron", "iron", "wood"]), 0).armed);
        assert!(!Blade::default().armed);
    }

    #[test]
    fn copper_conducts_a_shock_edge() {
        assert_eq!(Blade::assembled(&sword(["copper", "copper", "copper"]), 0).edge, Some(Element::Shock));
    }

    #[test]
    fn hit_modifiers_come_from_the_tool() {
        let mut tool = sword(["iron", "iron", "wood"]);
        engine_tools::modifiers::apply(&mut tool, "flux").unwrap();
        let b = Blade::assembled(&tool, 0);
        assert_eq!(b.hit_mana, 2.0);
        assert_eq!(Blade::assembled(&tool, LEGACY_MOSS).hit_heal, 2.0);
    }
}
