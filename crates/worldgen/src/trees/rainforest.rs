use super::{TreeForm, TreeProfile, WorldSampleConfig, landscape::Biome};
use crate::{BlockId, hash2, smoothstep, value_noise2};

pub(crate) fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= engine_voxel::RAINFOREST_GENERATOR
}

pub(super) fn tall_canopy(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= engine_voxel::TALL_RAINFOREST_GENERATOR
}

pub(super) fn amazonian(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= engine_voxel::AMAZONIAN_CANOPY_GENERATOR
}

pub(crate) fn forest_floor(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= engine_voxel::RAINFOREST_FLOOR_GENERATOR
}

/// Shallow ribbons carry damp soil across the floor between drier rises.
pub(crate) fn swale(drainage: f32) -> f32 {
    1.0 - smoothstep(0.035, 0.15, (drainage - 0.48).abs())
}

pub(crate) fn floor_relief(swale: f32, mound: f32, height: f32, slope: f32, footprint: f32) -> f32 {
    let gentle = 1.0 - smoothstep(0.24, 0.52, slope);
    let inland = smoothstep(2.2, 6.5, height - crate::WATER_LEVEL_METERS);
    let resolved = 1.0 - smoothstep(3.0, 9.0, footprint);
    (0.75 * smoothstep(0.44, 0.76, mound) * (1.0 - swale) - 1.2 * swale)
        * gentle * inland * resolved
}

pub(crate) fn floor_cover(swale: f32, mound: f32, slope: f32) -> Option<(BlockId, BlockId)> {
    if slope < 0.28 && swale > 0.79 {
        Some((BlockId::Mud, BlockId::Clay))
    } else if slope < 0.49 && swale > 0.34 && mound > 0.42 {
        Some((BlockId::GroveMoss, BlockId::Dirt))
    } else {
        None
    }
}

pub(crate) fn hills_floor(cfg: &WorldSampleConfig, x: f32, z: f32) -> (f32, f32) {
    let drainage = value_noise2(x / 26.0, z / 26.0, cfg.seed.wrapping_add(169_021));
    let mound = value_noise2(x / 8.0, z / 8.0, cfg.seed.wrapping_add(169_027));
    (swale(drainage), mound)
}

const CEIBA_HEIGHT: f32 = 52.0;
const ANGELIM_HEIGHT: f32 = 66.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Emergent {
    Ceiba,
    Angelim,
}

pub(super) fn emergent_form(cfg: &WorldSampleConfig, tree: &TreeProfile) -> Option<Emergent> {
    if !amazonian(cfg) || tree.form != TreeForm::Rainforest {
        return None;
    }
    let height = cfg.world_y(tree.trunk_h);
    if height >= ANGELIM_HEIGHT {
        Some(Emergent::Angelim)
    } else if height >= CEIBA_HEIGHT {
        Some(Emergent::Ceiba)
    } else {
        None
    }
}

pub(crate) fn opening(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    if cfg.generator_version < engine_voxel::RAINFOREST_UNDERSTORY_GENERATOR {
        return 0.0;
    }
    smoothstep(0.54, 0.76, value_noise2(x / 47.0, z / 47.0, cfg.seed.wrapping_add(166_031)))
}

pub(super) fn profile(cfg: &WorldSampleConfig, tree: &mut TreeProfile, site: &super::landscape::Landscape, age: f32, individual: f32) {
    let x = cfg.world_x(tree.anchor_x);
    let z = cfg.world_z(tree.anchor_z);
    let old_growth = value_noise2(x / 72.0, z / 72.0, cfg.seed.wrapping_add(162_011));
    let opening = opening(cfg, x, z);
    let emergent = individual > 0.93 - 0.14 * old_growth + 0.14 * opening;
    let recruit = individual < 0.27 + 0.42 * opening;
    // Separate the species lottery from stand age. Nearby sites share a bias
    // towards palms, while figs make occasional low canopies within the stand.
    let form = if forest_floor(cfg) && !emergent {
        let species = hash2(tree.cell_x, tree.cell_z, cfg.seed.wrapping_add(169_037));
        let palms = value_noise2(x / 48.0, z / 48.0, cfg.seed.wrapping_add(169_041));
        let mut palm_share = 0.11 + 0.25 * palms + 0.09 * opening;
        let mut fig_threshold = 0.82;
        if super::habitats::active(cfg) {
            let hollow = smoothstep(0.61, 0.79, site.moisture)
                * (1.0 - smoothstep(0.13, 0.34, site.slope));
            palm_share += 0.19 * hollow.max(site.riparian * 0.7) + 0.04 * opening;
            fig_threshold -= 0.14 * opening + 0.06 * (1.0 - hollow);
        }
        if species < palm_share {
            TreeForm::RainforestPalm
        } else if species > fig_threshold {
            TreeForm::RainforestFig
        } else {
            TreeForm::Rainforest
        }
    } else {
        TreeForm::Rainforest
    };
    let (height, radius, crown, fork) = if form == TreeForm::RainforestPalm {
        (8.5 + age * 10.0, 3.5 + age * 2.7, 2.0 + age * 1.5, 1.0)
    } else if form == TreeForm::RainforestFig {
        let spread = if super::habitats::active(cfg) { 1.8 + 1.6 * opening } else { 0.0 };
        (9.0 + age * 9.0, 5.0 + age * 3.0 + spread, 3.5 + age * 2.0, 0.44)
    } else if amazonian(cfg) {
        // The rarest emergents cluster in intact old growth. The wider ceiba
        // stays below the angelim and spreads a flat roof above the canopy.
        if emergent && individual > 0.995 - 0.035 * old_growth + 0.06 * opening {
            (ANGELIM_HEIGHT + age * 10.0, 13.0 + age * 3.5, 9.0 + age * 4.0, 0.79)
        } else if emergent {
            (CEIBA_HEIGHT + age * 13.0, 14.0 + age * 3.0, 5.0 + age * 3.0, 0.82)
        } else if recruit {
            (5.0 + age * 6.0, 3.2 + age * 2.0, 3.0 + age * 2.0, 0.43)
        } else {
            (24.0 + age * 13.0, 8.5 + age * 3.3, 5.5 + age * 3.0, 0.69)
        }
    } else {
        match (emergent, recruit, tall_canopy(cfg)) {
            (true, _, true) => (38.0 + age * 16.0, 11.5 + age * 4.5, 7.0 + age * 4.0, 0.73),
            (true, _, false) => (27.0 + age * 13.0, 8.3 + age * 3.0, 6.0 + age * 3.0, 0.69),
            (_, true, _) => (5.0 + age * 6.0, 3.2 + age * 2.0, 3.0 + age * 2.0, 0.43),
            (_, _, true) => (20.0 + age * 11.0, 7.6 + age * 3.4, 5.0 + age * 2.5, 0.66),
            (_, _, false) => (13.0 + age * 10.0, 5.8 + age * 2.7, 4.0 + age * 2.5, 0.60),
        }
    };
    tree.form = form;
    tree.conifer = false;
    tree.trunk_h = cfg.blocks_for_span(height);
    tree.canopy_radius = cfg.blocks_for_span(radius);
    tree.canopy_height = cfg.blocks_for_span(crown);
    tree.branch_start_y = tree.ground_y + cfg.blocks_for_span(height * fork);
}

/// Shrub colonies leave room between thickets for walking under the canopy.
pub(crate) fn thicket(cfg: &WorldSampleConfig, x: i32, z: i32) -> f32 {
    if !active(cfg) {
        return 0.0;
    }
    let (mx, mz) = (cfg.world_x(x), cfg.world_z(z));
    let patch = value_noise2(mx / 19.0, mz / 19.0, cfg.seed.wrapping_add(162_029))
        + 0.18 * opening(cfg, mx, mz);
    if patch <= 0.38 || super::landscape::sample(cfg, x, z).biome != Biome::Rainforest {
        return 0.0;
    }
    smoothstep(0.38, 0.67, patch)
}
