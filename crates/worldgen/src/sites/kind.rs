//! Surface ruin kinds and the biome-weighted lottery that picks one per region.
use super::super::{WorldSampleConfig, hash2};
use crate::landscape::Biome;

/// Surface sites read the region's biome before rolling a kind.
pub const BIOME_SITES_GENERATOR: u32 = engine_voxel::RECOVERED_SYSTEMS_GENERATOR;

/// The six surface ruins; the index is the save-stable `Site::kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SiteKind {
    Workshop,
    /// A toll crossing from generator 26; a watchtower before it.
    Watchtower,
    Waterworks,
    Foundry,
    Conservatory,
    Archive,
}

impl SiteKind {
    pub const ALL: [SiteKind; 6] = [
        Self::Workshop,
        Self::Watchtower,
        Self::Waterworks,
        Self::Foundry,
        Self::Conservatory,
        Self::Archive,
    ];

    pub fn from_index(index: u32) -> Option<Self> {
        Self::ALL.get(index as usize).copied()
    }

    pub fn index(self) -> u32 {
        self as u32
    }

    /// Kinds that attach a boss court behind the ruin.
    pub fn has_boss_court(self) -> bool {
        matches!(self, Self::Watchtower | Self::Foundry | Self::Archive)
    }

    pub fn label(self, version: u32) -> &'static str {
        match self {
            Self::Workshop => "Ruined artificer workshop",
            Self::Watchtower if version >= 26 => "The ruined toll crossing",
            Self::Watchtower => "Overgrown watchtower",
            Self::Waterworks => "Sunken waterworks",
            Self::Foundry => "Abandoned foundry",
            Self::Conservatory => "Mycologist conservatory",
            Self::Archive => "Resonant archive",
        }
    }

    /// Lottery weight of this kind in a biome: 3 where the ruin's purpose fits
    /// the ground, 1 elsewhere, so every kind still appears in every world.
    pub fn biome_weight(self, biome: Biome) -> u32 {
        use Biome as Bm;
        let fits = match self {
            Self::Workshop => matches!(biome, Bm::Meadow | Bm::Forest | Bm::Savanna | Bm::OvergrownGardens | Bm::RedwoodGrove),
            Self::Watchtower => matches!(biome, Bm::Heathland | Bm::Alpine | Bm::Badlands | Bm::Coast),
            Self::Waterworks => matches!(biome, Bm::Fen | Bm::Marsh | Bm::Rainforest | Bm::Coast),
            Self::Foundry => matches!(biome, Bm::Badlands | Bm::Desert | Bm::Taiga | Bm::VolcanicField),
            Self::Conservatory => matches!(biome, Bm::Rainforest | Bm::Forest | Bm::Fen | Bm::CloudForest | Bm::FungalGrove | Bm::CycadMarsh | Bm::LostPlateau),
            Self::Archive => matches!(biome, Bm::Tundra | Bm::Taiga | Bm::Desert),
        };
        if fits { 3 } else { 1 }
    }
}

/// Kind index for the site in region `(rx, rz)`; `biome` is read lazily so
/// older generators never sample the landscape here.
pub fn pick(cfg: &WorldSampleConfig, rx: i32, rz: i32, seed: u32, biome: impl FnOnce() -> Biome) -> u32 {
    let roll = hash2(rx, rz, seed.wrapping_add(5));
    if cfg.generator_version >= BIOME_SITES_GENERATOR {
        let biome = biome();
        let total: u32 = SiteKind::ALL.iter().map(|k| k.biome_weight(biome)).sum();
        let mut ticket = (roll * total as f32) as u32 % total;
        for kind in SiteKind::ALL {
            let w = kind.biome_weight(biome);
            if ticket < w {
                return kind.index();
            }
            ticket -= w;
        }
        return SiteKind::Archive.index();
    }
    let kinds = if cfg.generator_version >= 4 { 6 } else { 3 };
    (roll * kinds as f32) as u32 % kinds
}

pub(super) fn pick_curated(biome: Biome, roll: f32) -> Option<u32> {
    let fits = |kind: &SiteKind| kind.biome_weight(biome) > 1;
    let count = SiteKind::ALL.into_iter().filter(fits).count();
    let index = ((roll * count as f32) as usize).min(count.saturating_sub(1));
    SiteKind::ALL.into_iter().filter(fits).nth(index)
        .map(SiteKind::index)
}

