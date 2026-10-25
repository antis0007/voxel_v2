//! Inventory presentation reads gameplay values; it never owns balance numbers.
use super::{ArmorTier, EquipSlot, Item};
use engine_content::descriptions::{self, Description};

impl Item {
    pub fn description_entry(self) -> Option<&'static Description> {
        let key = match self {
            Self::PowerKit(_) => "power_kit",
            Self::Weapon(3) => "cinderbrand",
            Self::Weapon(4) => "winterglass",
            Self::Weapon(5) => "kinetic_maul",
            Self::Armor(EquipSlot::Head, ArmorTier::Lumen) => "lumen_helm",
            Self::Tool(1 | 2) => "bore",
            Self::Tool(_) => "legacy_focus",
            Self::Wand { .. } => "wand",
            // Standard blocks, food and components earn no prose by default.
            _ => return None,
        };
        descriptions::get(key)
    }

    pub fn description(self) -> &'static str {
        self.description_entry().map_or("", |entry| entry.text.as_str())
    }

    pub fn codex(self) -> &'static str {
        self.description_entry().map_or("", |entry| entry.codex.as_str())
    }

    pub fn stats_line(self) -> String {
        match self {
            Self::Food(food) => format!(
                "Hunger +{:.0} · Heals {:.0}", food.hunger_restore(), food.health_restore()
            ),
            Self::Armor(_, _) => format!("Defence +{:.0}%", self.armor_reduction() * 100.0),
            Self::Weapon(tier) => format!("Base damage {:.0}", super::weapon_damage(u32::from(tier))),
            Self::Tool(tier @ (1 | 2)) => format!("Radius {:.1} m",
                super::expedition::bore_radius(u32::from(tier))),
            Self::PowerKit(kit) => format!("Stored {} · Upgrade {}",
                super::factory::power::fmt_energy(kit.energy as f64), kit.upgrade),
            _ => String::new(),
        }
    }

    pub fn cost_line(self) -> String {
        match self {
            Self::Tool(1 | 2) => format!("{} {}",
                super::expedition::bore_fuel_cost(), super::expedition::NAMES[4]),
            _ => String::new(),
        }
    }
}
