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
            Self::Armor(EquipSlot::Chest, ArmorTier::Kinetic) => "kinetic_vest",
            Self::Armor(EquipSlot::Legs, ArmorTier::Kinetic) => "kinetic_greaves",
            Self::Armor(EquipSlot::Feet, ArmorTier::Kinetic) => "kinetic_boots",
            Self::Armor(_, ArmorTier::Ballast) => "octo_arms",
            Self::Armor(_, ArmorTier::Burner) => "burner_jetpack",
            Self::Armor(_, ArmorTier::Induction) => "induction_jetpack",
            Self::Armor(_, ArmorTier::Slime) => "slime_boots",
            Self::Armor(_, ArmorTier::Glider) => "hang_glider",
            Self::Armor(EquipSlot::Head, ArmorTier::Survey) => "survey_goggles",
            Self::Armor(EquipSlot::Head, ArmorTier::Runesight) => "rune_goggles",
            Self::Block(engine_content::BlockId::ClaimStake) => "claim_stake",
            Self::Block(engine_content::BlockId::LightlessShaft) => "lightless_shaft",
            Self::Block(engine_content::BlockId::GildedGate) => "gilded_gate",
            Self::Tool(19) => "slime_sling",
            Self::HOE => return None,
            Self::Tool(1 | 2) => "bore",
            Self::Tool(9 | 10 | 40 | 41) => return None,
            Self::Tool(6..=8) => return None,
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
            Self::Block(block) if block.chest_tier().is_some() => format!("Capacity: {} slots", block.chest_slots()),
            Self::Tool(40) => String::new(),
            Self::Tool(41) => "[RMB] Map".into(),
            Self::HOE => "[RMB] Till soil".into(),
            Self::Bucket(amount) => format!("{} · [RMB] Scoop · [Shift+RMB] Pour", super::super::water_tools::bucket_contents(amount)),
            Self::Food(food) => format!(
                "Hunger +{:.0} · Heals {:.0}", food.hunger_restore(), food.health_restore()
            ),
            Self::Armor(_, ArmorTier::Ballast) => format!("Reach: {:.0} m · [Alt+{}] Grip", crate::player::traversal::ARM_REACH, crate::keybinds::hint(crate::keybinds::Action::Jump)),
            Self::Armor(_, ArmorTier::Burner) => format!("Fuel: {:.0} s / Bio-ethanol", crate::player::traversal::BURN_SECONDS),
            Self::Armor(_, ArmorTier::Induction) => format!("Capacity: {:.0} FE · {:.0} FE/s", crate::player::traversal::CHARGE_CAPACITY, crate::player::traversal::POWER_PER_SECOND),
            Self::Armor(_, ArmorTier::Slime) => format!("Bounce: {:.0} m/s max · Fall damage: 0", crate::player::traversal::BOUNCE_CAP),
            Self::Armor(_, ArmorTier::Glider) => format!("Cruise: {:.0} m/s · [{}] Deploy", crate::player::glide::TRIM_SPEED, crate::keybinds::hint(crate::keybinds::Action::Jump)),
            Self::Tool(19) => format!("Charge: {:.1} s · Launch: {:.0} m/s", crate::player::traversal::SLING_SECONDS, crate::player::traversal::SLING_SPEED),
            Self::Armor(_, _) => format!("Defence +{:.0}%", self.armor_reduction() * 100.0),
            Self::Weapon(tier) => format!("Base damage {:.0}", super::weapon_damage(u32::from(tier))),
            Self::Tool(6..=8) => super::super::mining::Tool::of(self).map_or(String::new(), |t| format!("Tier {} · Speed {}×", t.tier, t.speed)),
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
