use super::{TreeForm, TreeProfile, WorldSampleConfig, landscape::{Biome, Landscape}};
use crate::value_noise2;

pub(super) fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= engine_voxel::TALL_WOODLAND_GENERATOR
}

pub(super) fn profile(cfg: &WorldSampleConfig, t: &mut TreeProfile, climate: &Landscape, age: f32, individual: f32) -> bool {
    if !active(cfg) {
        return false;
    }
    let varied = cfg.generator_version >= engine_voxel::WOODLAND_VARIETY_GENERATOR;
    let (form, conifer, height, radius, crown, fork) = match climate.biome {
        Biome::Forest | Biome::Taiga if varied && (0.22..0.47).contains(&climate.temperature)
            && individual < 0.16 + 0.32 * value_noise2(cfg.world_x(t.anchor_x) / 58.0, cfg.world_z(t.anchor_z) / 58.0, cfg.seed.wrapping_add(185_019)) =>
        {
            (TreeForm::Aspen, false, 13.0 + age * 13.0, 3.0 + age * 1.7, 4.2 + age * 2.0, 0.63)
        }
        Biome::Fen | Biome::Marsh if varied && individual > 0.39 && climate.temperature > 0.35 => {
            (TreeForm::Cypress, true, 13.0 + age * 12.0, 4.8 + age * 2.2, 4.5 + age * 2.0, 0.66)
        }
        Biome::Savanna if varied && age > 0.27 && individual > 0.31 => {
            (TreeForm::Acacia, false, 7.5 + age * 7.5, 4.9 + age * 2.4, 2.4 + age * 1.4, 0.74)
        }
        Biome::Taiga if age > 0.33 && individual > 0.20 => {
            (TreeForm::Classic, true, 22.0 + 19.0 * age, 4.8 + 2.2 * age, 1.0, 0.24)
        }
        Biome::Forest if age > 0.47 => {
            let grove = value_noise2(cfg.world_x(t.anchor_x) / 88.0, cfg.world_z(t.anchor_z) / 88.0, cfg.seed.wrapping_add(180_031));
            if individual < 0.92 - 0.24 * grove {
                return false;
            }
            (TreeForm::OldOak, false, 20.0 + 15.0 * age, 8.0 + 3.5 * age, 7.0 + 3.0 * age, 0.54)
        }
        _ => return false,
    };
    t.form = form;
    t.conifer = conifer;
    t.trunk_h = cfg.blocks_for_span(height);
    t.canopy_radius = cfg.blocks_for_span(radius);
    t.canopy_height = cfg.blocks_for_span(crown);
    t.branch_start_y = t.ground_y + cfg.blocks_for_span(height * fork);
    true
}
