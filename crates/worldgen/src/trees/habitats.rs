//! Site-driven composition for the woodland habitat preview.
use super::{TreeForm, TreeProfile, WorldSampleConfig, landscape::{Biome, Landscape}};
use crate::{smoothstep, value_noise2};

pub const WOODLAND_HABITAT_PREVIEW: u32 = 253;

pub(super) fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version == WOODLAND_HABITAT_PREVIEW
}

pub(super) fn site(cfg: &WorldSampleConfig, tree: &TreeProfile) -> Landscape {
    // A tree's ground contact still uses its actual voxel scale. Species
    // selection uses one fixed habitat footprint on the same physical site.
    let recipe = WorldSampleConfig { voxel_scale: engine_core::VoxelScalePreset::OneMeter, ..*cfg };
    let (x, z) = crate::tree_anchor(&recipe, tree.cell_x, tree.cell_z);
    super::landscape::preview_landscape_lod(&recipe, x, z, 8.0)
}

pub(super) fn profile(
    cfg: &WorldSampleConfig, tree: &mut TreeProfile, site: &Landscape, age: f32, roll: f32,
) -> bool {
    use Biome::*;
    if !active(cfg) || !matches!(site.biome, Forest | Taiga | Marsh | Fen | Savanna | Heathland) {
        return false;
    }
    let grove = value_noise2(
        cfg.world_x(tree.anchor_x) / 64.0,
        cfg.world_z(tree.anchor_z) / 64.0,
        cfg.seed.wrapping_add(253_019),
    );
    let dry = 1.0 - smoothstep(0.34, 0.57, site.moisture);
    let exposure = smoothstep(0.12, 0.42, site.slope);
    let edge = (1.0 - (2.0 * site.ecotone - 1.0).abs()).clamp(0.0, 1.0);
    // These are dimensions in metres. Each form keeps its own woody structure
    // and supplies bounds to the same voxel and distant tree consumers.
    let (form, conifer, trunk, radius, crown, fork) = match site.biome {
        Forest if roll < 0.06 + 0.46 * dry => {
            (TreeForm::HighCrown, true, 12.0 + age * 13.0, 3.3 + age * 2.2, 3.5 + age * 1.8, 0.67)
        }
        Forest if roll < 0.20 + 0.43 * grove + 0.12 * edge => {
            (TreeForm::Aspen, false, 12.0 + age * 14.0, 2.5 + age * 1.8, 4.0 + age * 2.6, 0.68)
        }
        Forest if age > 0.45 && roll > 0.59 + 0.23 * grove => {
            (TreeForm::OldOak, false, 11.0 + age * 14.0, 7.8 + age * 4.0 + 1.3 * edge,
                6.2 + age * 3.1, 0.49 - 0.12 * edge)
        }
        Forest => return false,
        Taiga if roll < 0.09 + 0.39 * dry + 0.21 * exposure => {
            (TreeForm::HighCrown, true, 10.0 + age * 12.0, 3.6 + age * 2.2, 3.1 + age * 2.0, 0.65)
        }
        Taiga if site.temperature > 0.25 && roll < 0.20 + 0.34 * grove => {
            (TreeForm::Aspen, false, 10.0 + age * 13.0, 2.6 + age * 1.6, 4.4 + age * 1.7, 0.64)
        }
        Taiga => {
            (TreeForm::Classic, true, 15.0 + age * 25.0, 3.6 + age * 2.7, 1.0, 0.22)
        }
        Fen | Marsh if site.temperature > 0.33 && roll > 0.27 + 0.34 * grove - 0.14 * site.riparian => {
            (TreeForm::Cypress, true, 11.0 + age * 16.0, 3.9 + age * 2.7, 4.3 + age * 2.6, 0.61)
        }
        Fen | Marsh => {
            (TreeForm::Weeping, false, 4.8 + age * 5.5, 4.4 + age * 3.4, 3.5 + age * 2.6, 0.48)
        }
        Savanna if roll > 0.24 + 0.36 * grove && age > 0.24 => {
            (TreeForm::Acacia, false, 6.2 + age * 8.3, 5.0 + age * 3.3, 2.2 + age * 1.5, 0.70)
        }
        Savanna => {
            (TreeForm::Umbrella, false, 3.8 + age * 3.6, 2.6 + age * 2.4, 1.6 + age * 1.3, 0.55)
        }
        Heathland if roll < 0.22 + 0.44 * exposure => {
            (TreeForm::Classic, true, 2.4 + age * 1.4, 1.7 + age * 1.6, 0.8, 0.16)
        }
        Heathland => {
            (TreeForm::HighCrown, true, 4.5 + age * 5.5, 2.3 + age * 1.7, 1.8 + age * 1.3, 0.58)
        }
        _ => return false,
    };
    tree.form = form;
    tree.conifer = conifer;
    tree.trunk_h = cfg.blocks_for_span(trunk);
    tree.canopy_radius = cfg.blocks_for_span(radius);
    tree.canopy_height = cfg.blocks_for_span(crown);
    tree.branch_start_y = tree.ground_y + cfg.blocks_for_span(trunk * fork);
    tree.branch_len = cfg.blocks_for_span(radius * 0.76);
    true
}
