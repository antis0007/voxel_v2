//! Noita-style level modifiers for boss sites and ruins (generator 32+).
//! A modifier is a pure function of the world seed and the site's region key
//! `(x, z)` (the same pair `sites::encounters` packs into `IVec3::new(x, z, 0)`),
//! so nothing is saved. `dressing` restyles blocks; the app module reads the
//! modifier for gameplay effects.
use super::super::{BlockId as B, WorldSampleConfig};
use engine_core::hash::hash_u32;
use glam::IVec3;

/// First generator version that rolls site modifiers.
pub const MODIFIER_GENERATOR_VERSION: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SiteModifier {
    None,
    Flooded,
    Overgrown,
    Smouldering,
    Frostbound,
    Gloom,
    Volatile,
    Echoing,
    Crystalline,
    Haunted,
    Unstable,
}

impl SiteModifier {
    /// Every rollable modifier (excludes `None`).
    pub const ALL: [SiteModifier; 10] = [
        Self::Flooded,
        Self::Overgrown,
        Self::Smouldering,
        Self::Frostbound,
        Self::Gloom,
        Self::Volatile,
        Self::Echoing,
        Self::Crystalline,
        Self::Haunted,
        Self::Unstable,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Flooded => "Flooded",
            Self::Overgrown => "Overgrown",
            Self::Smouldering => "Smouldering",
            Self::Frostbound => "Frostbound",
            Self::Gloom => "Gloom",
            Self::Volatile => "Volatile",
            Self::Echoing => "Echoing",
            Self::Crystalline => "Crystalline",
            Self::Haunted => "Haunted",
            Self::Unstable => "Unstable",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Flooded => "Water pools in the low channels; foes here are hardier.",
            Self::Overgrown => "Roots choke the halls; foes regenerate.",
            Self::Smouldering => "Embers glow in the cracks; foes burn what they touch.",
            Self::Frostbound => "Ice grips the floor and your limbs; you move slower.",
            Self::Gloom => "The dark presses in; your light reaches less far.",
            Self::Volatile => "Foes burst when they fall.",
            Self::Echoing => "Every sound carries; foes hear you from twice as far.",
            Self::Crystalline => "Spells cost less mana, but crystal hides foes from harm.",
            Self::Haunted => "Restless wisps drift in now and then.",
            Self::Unstable => "The air warps; your casts wobble.",
        }
    }
}

fn mix(cfg: &WorldSampleConfig, x: i32, z: i32) -> u32 {
    hash_u32(
        cfg.seed
            ^ hash_u32((x as u32).wrapping_mul(0x9e37_79b1) ^ 0x51_7e00)
            ^ hash_u32((z as u32).wrapping_mul(0x85eb_ca6b).wrapping_add(0xc2b2_ae35)),
    )
}

/// The modifier for a site region key. ~40% of sites stay unmodified.
pub fn site_modifier(cfg: &WorldSampleConfig, site_key: (i32, i32)) -> SiteModifier {
    if cfg.generator_version < MODIFIER_GENERATOR_VERSION {
        return SiteModifier::None;
    }
    let h = mix(cfg, site_key.0, site_key.1);
    if h % 100 < 40 {
        return SiteModifier::None;
    }
    let n = SiteModifier::ALL.len() as u32;
    SiteModifier::ALL[((h >> 8) % n) as usize]
}

fn cell_hash(p: IVec3, salt: u32) -> u32 {
    hash_u32(
        (p.x as u32).wrapping_mul(73_856_093)
            ^ (p.y as u32).wrapping_mul(19_349_663)
            ^ (p.z as u32).wrapping_mul(83_492_791)
            ^ salt,
    )
}

/// Restyle one block of a modified site. `local_pos` is relative to the site
/// floor (`y == 0` is the walkable floor cell, negative is below it); `block`
/// is what the layout would place. `None` keeps the original block.
pub fn dressing(modifier: SiteModifier, local_pos: IVec3, block: B) -> Option<B> {
    let h = cell_hash(local_pos, (modifier as u32).wrapping_mul(0x2545_f491));
    let floor = local_pos.y <= 0;
    let solid = !matches!(block, B::Air | B::Water);
    match modifier {
        SiteModifier::None => None,
        SiteModifier::Flooded => (block == B::Air && local_pos.y <= -1).then_some(B::Water),
        SiteModifier::Overgrown => {
            if block == B::Air && local_pos.y == 1 && h.is_multiple_of(7) {
                Some(B::Bush)
            } else if solid && floor && h.is_multiple_of(3) {
                Some(B::FungalSoil)
            } else if solid && !floor && h.is_multiple_of(11) {
                Some(B::Leaves)
            } else {
                None
            }
        }
        SiteModifier::Smouldering => {
            if solid && floor && h.is_multiple_of(9) {
                Some(B::Basalt)
            } else if solid && floor && h.is_multiple_of(41) {
                Some(B::Lava)
            } else if block == B::Air && local_pos.y == 1 && h.is_multiple_of(53) {
                Some(B::Ash)
            } else {
                None
            }
        }
        SiteModifier::Frostbound => {
            if solid && floor && h.is_multiple_of(2) {
                Some(B::Ice)
            } else if solid && !floor && h.is_multiple_of(6) {
                Some(B::SnowGrass)
            } else {
                None
            }
        }
        SiteModifier::Crystalline => {
            let on_surface = block == B::Air && local_pos.y == 1 && h.is_multiple_of(29);
            ((solid && h.is_multiple_of(17)) || on_surface).then_some(B::Crystal)
        }
        SiteModifier::Gloom => (solid && h.is_multiple_of(5)).then_some(B::Basalt),
        SiteModifier::Haunted => (solid && floor && h.is_multiple_of(13)).then_some(B::TombStone),
        SiteModifier::Unstable => (solid && h.is_multiple_of(10)).then_some(B::Gravel),
        SiteModifier::Volatile | SiteModifier::Echoing => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(v: u32) -> WorldSampleConfig {
        WorldSampleConfig {
            generator_version: v,
            seed: 1234,
            world_type: crate::WorldType::Hills,
            voxel_scale: engine_core::VoxelScalePreset::QuarterMeter,
        }
    }

    #[test]
    fn modifier_is_deterministic() {
        let c = cfg(32);
        for x in -20..20 {
            for z in -20..20 {
                assert_eq!(site_modifier(&c, (x, z)), site_modifier(&c, (x, z)));
            }
        }
    }

    #[test]
    fn modifier_distribution_covers_all() {
        let c = cfg(32);
        let mut none = 0;
        let mut seen = std::collections::HashSet::new();
        for x in -50..50 {
            for z in -50..50 {
                let m = site_modifier(&c, (x, z));
                if m == SiteModifier::None {
                    none += 1;
                }
                seen.insert(m);
            }
        }
        for m in SiteModifier::ALL {
            assert!(seen.contains(&m), "{m:?} never rolled");
            assert!(!m.name().is_empty() && !m.description().is_empty());
        }
        assert!((3500..4500).contains(&none), "none share {none}/10000");
    }

    #[test]
    fn modifier_v31_is_none() {
        let c = cfg(31);
        for x in -10..10 {
            assert_eq!(site_modifier(&c, (x, 3)), SiteModifier::None);
        }
    }

    #[test]
    fn modifier_dressing_restyles() {
        let p = IVec3::new(0, -1, 0);
        assert_eq!(dressing(SiteModifier::Flooded, p, B::Air), Some(B::Water));
        assert_eq!(dressing(SiteModifier::None, p, B::Stone), None);
        let iced = (0..64)
            .filter(|&i| dressing(SiteModifier::Frostbound, IVec3::new(i, 0, 0), B::Stone) == Some(B::Ice))
            .count();
        assert!(iced > 10);
    }
}
