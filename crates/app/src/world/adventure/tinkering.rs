//! Tinkers' Construct-inspired modular melee fittings.
//!
//! The three saved part bytes already existed in the adventure save block
//! (head, binding, grip), so this module gives them meaning without a save
//! version bump. Every value maps onto a material palette that is reachable
//! from the existing resource economy; unknown bytes are rejected by the
//! decoder before they reach this code.

use super::status;

/// The three fitted slots on a melee tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Part {
    Head = 0,
    Binding = 1,
    Grip = 2,
}

impl Part {
    pub const ALL: [Self; 3] = [Self::Head, Self::Binding, Self::Grip];

    pub fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Head),
            1 => Some(Self::Binding),
            2 => Some(Self::Grip),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::Binding => "Binding",
            Self::Grip => "Grip",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Self::Head => "Sets raw damage and the elemental edge.",
            Self::Binding => "Widens the sweep and steadies follow-through.",
            Self::Grip => "Shortens the wind-up and speeds the combo.",
        }
    }

    pub fn stat_label(self) -> &'static str {
        match self {
            Self::Head => "Damage",
            Self::Binding => "Sweep",
            Self::Grip => "Swing",
        }
    }
}

/// Fitting materials. Index 0 is deliberately neutral so old saves and fresh
/// worlds keep their previous melee numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Material {
    Copper = 0,
    Bronze = 1,
    Steel = 2,
    Cinder = 3,
    Rime = 4,
    /// PlusTiC-style synthetic fitting: light, fast and cheap to replace.
    Polymer = 5,
}

impl Material {
    pub const ALL: [Self; 6] = [
        Self::Copper,
        Self::Bronze,
        Self::Steel,
        Self::Cinder,
        Self::Rime,
        Self::Polymer,
    ];

    pub fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Copper),
            1 => Some(Self::Bronze),
            2 => Some(Self::Steel),
            3 => Some(Self::Cinder),
            4 => Some(Self::Rime),
            5 => Some(Self::Polymer),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Copper => "Copper",
            Self::Bronze => "Bronze",
            Self::Steel => "Steel",
            Self::Cinder => "Cinder",
            Self::Rime => "Rime",
            Self::Polymer => "Polymer",
        }
    }

    pub fn adjective(self) -> &'static str {
        match self {
            Self::Copper => "Coppered",
            Self::Bronze => "Bronzen",
            Self::Steel => "Steeled",
            Self::Cinder => "Emberforged",
            Self::Rime => "Rimeforged",
            Self::Polymer => "Cast-polymer",
        }
    }

    pub fn color(self) -> [u8; 3] {
        match self {
            Self::Copper => [196, 116, 68],
            Self::Bronze => [211, 154, 84],
            Self::Steel => [166, 182, 192],
            Self::Cinder => [232, 92, 44],
            Self::Rime => [126, 202, 226],
            Self::Polymer => [222, 210, 184],
        }
    }

    /// Small multiplier layered on the part's fitted tier. Index 0 is neutral,
    /// so old saves keep their previous 1 + 0.15 × head progression.
    pub fn damage_factor(self) -> f32 {
        match self {
            Self::Copper => 1.0,
            Self::Bronze => 1.02,
            Self::Steel => 1.08,
            Self::Cinder => 1.05,
            Self::Rime => 1.03,
            Self::Polymer => 0.96,
        }
    }

    pub fn sweep_factor(self) -> f32 {
        match self {
            Self::Copper => 1.0,
            Self::Bronze => 1.06,
            Self::Steel => 1.02,
            Self::Cinder => 1.00,
            Self::Rime => 1.04,
            Self::Polymer => 1.08,
        }
    }

    /// Multiplier on swing speed layered over the fitted grip tier. Higher is faster.
    pub fn speed_factor(self) -> f32 {
        match self {
            Self::Copper => 1.0,
            Self::Bronze => 1.05,
            Self::Steel => 1.01,
            Self::Cinder => 1.03,
            Self::Rime => 1.04,
            Self::Polymer => 1.08,
        }
    }

    /// Elemental edge granted by a head of this material.
    pub fn edge(self) -> Option<status::Element> {
        match self {
            Self::Cinder => Some(status::Element::Fire),
            Self::Rime => Some(status::Element::Frost),
            Self::Copper | Self::Bronze | Self::Steel | Self::Polymer => None,
        }
    }

    pub fn trait_name(self) -> &'static str {
        match self {
            Self::Copper => "Plain",
            Self::Bronze => "Balanced",
            Self::Steel => "Grave weight",
            Self::Cinder => "Searing",
            Self::Rime => "Rimed",
            Self::Polymer => "Flexible",
        }
    }

    pub fn trait_detail(self) -> &'static str {
        match self {
            Self::Copper => "Neutral: no bonus, cheap to fit.",
            Self::Bronze => "Balanced guard: +6% sweep and +5% swing speed.",
            Self::Steel => "Heaviest head: +8% damage, but adds weight.",
            Self::Cinder => "Hits set the target burning; wet foes flash to steam.",
            Self::Rime => "Hits chill the target, setting up shatter and freezing shards.",
            Self::Polymer => "Light and springy: +8% sweep/speed, but 4% less damage.",
        }
    }
}

/// A single installed modifier bit. The bits match the existing `tool_mods`
/// save word, so older reinforced saves keep their exact effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Modifier {
    pub bit: u8,
    pub name: &'static str,
    pub detail: &'static str,
    pub cost: &'static [(usize, u32)],
}

pub const MODIFIERS: [Modifier; 5] = [
    Modifier {
        bit: 1,
        name: "Jagged",
        detail: "+8% damage on every strike.",
        cost: &[(15, 8), (3, 16)],
    },
    Modifier {
        bit: 2,
        name: "Flux",
        detail: "Every hit returns 2 mana.",
        cost: &[(5, 2), (12, 1)],
    },
    Modifier {
        bit: 4,
        name: "Core",
        detail: "Finishers stagger for an extra 0.25 s.",
        cost: &[(8, 8), (16, 1)],
    },
    Modifier {
        bit: 8,
        name: "Moss",
        detail: "Every hit restores 2 health.",
        cost: &[(13, 8), (4, 8)],
    },
    Modifier {
        bit: 16,
        name: "Runed",
        detail: "Every hit deals 4 additional damage.",
        cost: &[(17, 2), (12, 2)],
    },
];

pub fn modifier(bit: u8) -> Option<&'static Modifier> {
    MODIFIERS.iter().find(|m| m.bit == bit)
}

/// Resource cost to fit a material into a part. The table is intentionally
/// per-part so a cheap grip does not gate the whole blade behind a head price.
pub fn part_cost(part: Part, material: Material) -> &'static [(usize, u32)] {
    match (part, material) {
        (Part::Head, Material::Copper) => &[(6, 48)],
        (Part::Head, Material::Bronze) => &[(14, 64), (6, 32)],
        (Part::Head, Material::Steel) => &[(15, 96), (3, 32)],
        (Part::Head, Material::Cinder) => &[(15, 64), (17, 1), (3, 64)],
        (Part::Head, Material::Rime) => &[(15, 64), (5, 3), (18, 1)],
        (Part::Head, Material::Polymer) => &[(13, 64), (3, 64)],

        (Part::Binding, Material::Copper) => &[(6, 32)],
        (Part::Binding, Material::Bronze) => &[(14, 48)],
        (Part::Binding, Material::Steel) => &[(15, 48), (12, 1)],
        (Part::Binding, Material::Cinder) => &[(17, 1), (16, 1)],
        (Part::Binding, Material::Rime) => &[(5, 2), (18, 1)],
        (Part::Binding, Material::Polymer) => &[(13, 32), (16, 1)],

        (Part::Grip, Material::Copper) => &[(4, 32)],
        (Part::Grip, Material::Bronze) => &[(14, 32), (4, 16)],
        (Part::Grip, Material::Steel) => &[(15, 24), (16, 1)],
        (Part::Grip, Material::Cinder) => &[(13, 24), (3, 32)],
        (Part::Grip, Material::Rime) => &[(5, 2), (4, 32)],
        (Part::Grip, Material::Polymer) => &[(13, 48), (4, 16)],
    }
}

/// Effective head multiplier for a saved part byte. The tier term preserves the
/// pre-forge `1 + 0.15 × head` progression; the material adds a small identity
/// bonus on top instead of replacing it.
pub fn head_multiplier(raw: u8) -> f32 {
    let Some(material) = Material::from_u8(raw) else {
        return 1.0;
    };
    (1.0 + raw as f32 * 0.15) * material.damage_factor()
}

/// Effective binding sweep multiplier for a saved part byte.
pub fn binding_multiplier(raw: u8) -> f32 {
    let Some(material) = Material::from_u8(raw) else {
        return 1.0;
    };
    (1.0 + raw as f32 * 0.12) * material.sweep_factor()
}

/// Effective grip speed multiplier for a saved part byte.
pub fn grip_multiplier(raw: u8) -> f32 {
    let Some(material) = Material::from_u8(raw) else {
        return 1.0;
    };
    (1.0 / (1.0 - raw as f32 * 0.06).max(0.5)) * material.speed_factor()
}

/// Elemental edge carried by the head fitting.
pub fn edge(raw: u8) -> Option<status::Element> {
    Material::from_u8(raw).and_then(Material::edge)
}

pub fn material_color(raw: u8) -> [u8; 3] {
    Material::from_u8(raw).map_or([196, 116, 68], Material::color)
}

/// A readable name for the fitted tool. Default copper fittings keep the base
/// weapon name so existing inventory tooltips do not change.
pub fn display_name(base: &str, head: u8, bind: u8, grip: u8) -> String {
    if head == 0 && bind == 0 && grip == 0 {
        return base.to_owned();
    }
    let material = Material::from_u8(head).unwrap_or(Material::Copper);
    format!("{} {}", material.adjective(), base)
}

/// Short stat line used by tooltips and the forge preview.
pub fn stat_summary(head: u8, bind: u8, grip: u8) -> String {
    format!(
        "×{:.2} damage · ×{:.2} sweep · ×{:.2} swing",
        head_multiplier(head),
        binding_multiplier(bind),
        grip_multiplier(grip)
    )
}

/// Part + material traits in slot order. Fixed-size so the Forge can render
/// every frame without a heap allocation.
pub fn traits(head: u8, bind: u8, grip: u8) -> [(&'static str, &'static str); 3] {
    let mut out = [("", ""); 3];
    for (slot, (part, raw)) in [
        (Part::Head, head),
        (Part::Binding, bind),
        (Part::Grip, grip),
    ]
    .into_iter()
    .enumerate()
    {
        let material = Material::from_u8(raw).unwrap_or(Material::Copper);
        out[slot] = (material.trait_name(), trait_line(part, material));
    }
    out
}

fn trait_line(part: Part, material: Material) -> &'static str {
    match (part, material) {
        (Part::Head, _) => material.trait_detail(),
        (Part::Binding, Material::Copper) => "Plain wrap: no sweep bonus.",
        (Part::Binding, Material::Bronze) => "Broad bronze guard: +6% sweep.",
        (Part::Binding, Material::Steel) => "Riveted band: +2% sweep, steady recovery.",
        (Part::Binding, Material::Cinder) => "Heat-scarred lace: no sweep bonus, burning edge.",
        (Part::Binding, Material::Rime) => "Frost-wound cord: +4% sweep, chilling edge.",
        (Part::Binding, Material::Polymer) => "Cast polymer guard: +8% sweep, very light.",
        (Part::Grip, Material::Copper) => "Plain grip: no speed bonus.",
        (Part::Grip, Material::Bronze) => "Balanced leather: +5% swing speed.",
        (Part::Grip, Material::Steel) => "Cold steel ferrule: +1% swing speed.",
        (Part::Grip, Material::Cinder) => "Warm wrap: +3% swing speed, keeps the edge hot.",
        (Part::Grip, Material::Rime) => "Rime-slick wrap: +4% swing speed.",
        (Part::Grip, Material::Polymer) => "Moulded grip: +8% swing speed, no upkeep.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_bytes_round_trip_and_reject_out_of_range() {
        for material in Material::ALL {
            assert_eq!(Material::from_u8(material.as_u8()), Some(material));
        }
        assert_eq!(Material::from_u8(6), None);
        assert_eq!(Material::from_u8(255), None);
    }

    #[test]
    fn steel_head_is_the_damage_choice() {
        assert!(Material::Steel.damage_factor() > Material::Bronze.damage_factor());
        assert!(Material::Bronze.damage_factor() > Material::Copper.damage_factor());
        assert_eq!(head_multiplier(0), 1.0);
        assert_eq!(head_multiplier(99), 1.0);
        // Tier progression of the pre-forge system is preserved.
        assert!(head_multiplier(4) >= 1.6);
    }

    #[test]
    fn bronze_grip_is_faster_than_steel() {
        assert!(Material::Bronze.speed_factor() > Material::Steel.speed_factor());
        assert_eq!(grip_multiplier(0), 1.0);
        assert!(grip_multiplier(4) >= 1.30);
    }

    #[test]
    fn elemental_edges_are_reserved_for_cinder_and_rime() {
        assert!(edge(Material::Cinder.as_u8()).is_some());
        assert!(edge(Material::Rime.as_u8()).is_some());
        assert!(edge(Material::Steel.as_u8()).is_none());
    }

    #[test]
    fn modifiers_use_unique_single_bits_with_valid_costs() {
        let mut seen = 0u8;
        for modifier in MODIFIERS {
            assert_eq!(modifier.bit.count_ones(), 1);
            assert_eq!(seen & modifier.bit, 0);
            seen |= modifier.bit;
            assert!(seen <= 31);
            assert!(!modifier.cost.is_empty());
            assert!(modifier.cost.iter().all(|(id, n)| *id < 19 && *n > 0));
        }
        assert_eq!(modifier(8).map(|m| m.name), Some("Moss"));
        assert!(modifier(32).is_none());
    }

    #[test]
    fn every_part_and_material_has_a_paid_cost() {
        for part in Part::ALL {
            for material in Material::ALL {
                let cost = part_cost(part, material);
                assert!(!cost.is_empty());
                assert!(cost.iter().all(|(id, n)| *id < 19 && *n > 0));
            }
        }
    }

    #[test]
    fn default_fittings_keep_the_base_name() {
        assert_eq!(display_name("Steel sword", 0, 0, 0), "Steel sword");
        assert_eq!(
            display_name("Steel sword", 3, 0, 0),
            "Emberforged Steel sword"
        );
        assert!(stat_summary(2, 2, 1).contains("damage"));
        assert_eq!(traits(0, 0, 0).len(), 3);
    }

    #[test]
    fn polymer_fittings_round_trip_through_the_adventure_save_block() {
        use super::super::Adventure;
        let mut adventure = Adventure::default();
        adventure.wand = 1;
        adventure.spell = 0;
        adventure.loadout.slots[0] = 0;
        adventure.tool_head = Material::Polymer.as_u8();
        adventure.tool_bind = Material::Bronze.as_u8();
        adventure.tool_grip = Material::Rime.as_u8();
        adventure.tool_mods = 1 | 8;
        let mut bytes = Vec::new();
        adventure.encode(&mut bytes);
        let mut words = bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect::<Vec<_>>()
            .into_iter();
        let mut read = || words.next().ok_or_else(|| anyhow::anyhow!("short"));
        let back = Adventure::decode(&mut read, true, true).expect("fittings must decode");
        assert_eq!(back.tool_head, 5);
        assert_eq!(back.tool_bind, 1);
        assert_eq!(back.tool_grip, 4);
        assert_eq!(back.tool_mods, 9);
        assert!(
            display_name(
                super::super::sword_name(back.weapon),
                back.tool_head,
                back.tool_bind,
                back.tool_grip
            )
            .starts_with("Cast-polymer")
        );
        // A forged byte outside the table is a corrupt save, not a silent fallback.
        let last = bytes.len() - 4;
        let mode_word = u32::from_le_bytes(bytes[last..].try_into().unwrap());
        bytes[last..].copy_from_slice(&(mode_word | (6 << 8)).to_le_bytes());
        let mut words = bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
        let mut read = || words.next().ok_or_else(|| anyhow::anyhow!("short"));
        assert!(Adventure::decode(&mut read, true, true).is_err());
    }
}
