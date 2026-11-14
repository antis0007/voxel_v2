//! Generator 33 tree species that differ in silhouette, not only in size:
//! savanna and emergent umbrellas, waterside weeping crowns, high-crowned dry
//! pines, birch groves and timberline krummholz. Older generator versions keep
//! `TreeForm::Classic` and generate bit-identically.
use super::foliage::{self, Relief};
use super::{BlockId, IVec3, TreeProfile, WorldSampleConfig, distance_to_segment, hash3, landscape};
use glam::{Vec3, Vec3Swizzles};
use std::f32::consts::TAU;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TreeForm {
    /// The pre-33 broadleaf or whorled conifer (also birch and krummholz,
    /// which only change its proportions).
    #[default]
    Classic,
    /// Forked trunk under flat leaf pads: acacia, rainforest emergents.
    Umbrella,
    /// Rounded crown with hanging leaf curtains: willows by water.
    Weeping,
    /// Tall bare stem under an irregular needle crown: dry-ground pines.
    HighCrown,
    /// Generator 52 landmark broadleaf (see `giant.rs`).
    Giant,
    Rainforest,
    RainforestPalm,
    RainforestFig,
    OldOak,
    CloudCrown,
    PlateauCrown,
    Aspen,
    Cypress,
    Acacia,
    Cycad,
    Fungal,
    Redwood,
}

impl TreeForm {
    pub(crate) fn is_rainforest(self) -> bool {
        matches!(self, Self::Rainforest | Self::RainforestPalm | Self::RainforestFig)
    }
}

/// Same leaf surface as open crowns: `field < 0.83 + edge * 0.24`.
const THRESHOLD_REACH: f32 = 1.034_408; // sqrt(1.07)

/// Everything `block()` already derived for this tree and voxel.
pub(super) struct Frame {
    pub q: Vec3,
    pub lean: Vec3,
    pub stem: f32,
    pub height: f32,
    pub radius: f32,
    pub crown_h: f32,
    pub rotation: f32,
    pub scale: f32,
    pub relief: Option<Relief>,
}

pub(super) fn choose(
    cfg: &WorldSampleConfig,
    tree: &mut TreeProfile,
    climate: &landscape::Landscape,
    age: f32,
    grove: f32,
    individual: f32,
) {
    use landscape::Biome;
    let span = |m: f32| cfg.blocks_for_span(m);
    let above_water = (tree.ground_y - cfg.water_level()) as f32 * cfg.voxel_meters();
    let waterside = climate.moisture > 0.60 && above_water < 5.5;
    let (form, conifer, trunk, radius, crown) = match climate.biome {
        Biome::Savanna if cfg.generator_version >= 47 => {
            (TreeForm::Umbrella, false, 4.0 + age * 3.0, 4.5 + age * 2.5, 2.6 + age * 1.4)
        }
        Biome::Rainforest if cfg.generator_version >= 47 && individual > 0.78 => {
            (TreeForm::Umbrella, false, 14.0 + age * 6.0, 5.5 + age * 2.5, 5.0 + age * 3.0)
        }
        Biome::Savanna => (TreeForm::Umbrella, false, 4.0 + age * 3.0, 4.5 + age * 2.5, 1.6 + age * 0.8),
        Biome::Rainforest if individual > 0.78 => {
            (TreeForm::Umbrella, false, 17.0 + age * 8.0, 5.5 + age * 2.5, 2.6 + age * 1.4)
        }
        Biome::Fen | Biome::Marsh => (TreeForm::Weeping, false, 5.0 + age * 3.0, 3.6 + age * 1.8, 3.4 + age * 2.0),
        _ if !tree.conifer && waterside => {
            (TreeForm::Weeping, false, 5.0 + age * 3.0, 3.6 + age * 1.8, 3.4 + age * 2.0)
        }
        // Wind-stunted conifers near the timberline: low, wide, skirted to the ground.
        Biome::Alpine | Biome::Tundra | Biome::Heathland if tree.conifer => {
            (TreeForm::Classic, true, 2.2 + age * 1.8, 1.8 + age * 1.2, 1.0)
        }
        _ if tree.conifer && climate.moisture < 0.42 => {
            (TreeForm::HighCrown, true, 10.0 + age * 7.0, 2.4 + age * 1.4, 2.6 + age * 1.6)
        }
        // Pale slender groves; `grove` is shared by 3x3 tree cells.
        _ if !tree.conifer && grove < 0.24 && climate.temperature < 0.52 => {
            tree.branch_count = 3 + (individual * 3.0) as i32;
            (TreeForm::Classic, false, 8.5 + age * 5.0, 1.7 + age * 0.9, 4.0 + age * 2.5)
        }
        _ => return,
    };
    tree.form = form;
    tree.conifer = conifer;
    tree.trunk_h = span(trunk);
    tree.canopy_radius = span(radius);
    tree.canopy_height = span(crown);
    let branch_fraction = match form {
        TreeForm::HighCrown => 0.62,
        _ if conifer => 0.12,
        _ => 0.55,
    };
    tree.branch_start_y = tree.ground_y + (tree.trunk_h as f32 * branch_fraction) as i32;
    tree.branch_len = (tree.canopy_radius as f32 * 0.76) as i32;
}

fn limb_variation(cfg: &WorldSampleConfig, tree: &TreeProfile, limb: i32) -> f32 {
    hash3(tree.cell_x, limb, tree.cell_z, cfg.seed.wrapping_add(12083))
}

fn umbrella_limbs(tree: &TreeProfile) -> i32 {
    2 + tree.branch_count.rem_euclid(3)
}

/// `field` is the nearest lobe's squared normalised distance; `size` its smallest radius.
fn leaf_or_air(cfg: &WorldSampleConfig, p: IVec3, f: &Frame, field: f32, size: f32) -> BlockId {
    if let Some(relief) = f.relief {
        let leaf = relief.leaf(f.q, f.scale, field, size, foliage::OPEN);
        return if leaf { BlockId::Leaves } else { BlockId::Air };
    }
    let grain = cfg.blocks_for_span(0.85).max(1);
    let edge = hash3(
        p.x.div_euclid(grain),
        p.y.div_euclid(grain),
        p.z.div_euclid(grain),
        cfg.seed.wrapping_add(12109),
    );
    if field < 0.83 + edge * 0.24 {
        BlockId::Leaves
    } else {
        BlockId::Air
    }
}

/// Forked trunk carrying flat, overlapping leaf pads.
pub(super) fn umbrella(cfg: &WorldSampleConfig, tree: &TreeProfile, p: IVec3, f: &Frame) -> BlockId {
    let fork = Vec3::new(f.lean.x * 0.30, f.height * 0.55, f.lean.z * 0.30);
    let crown_y = f.height + f.crown_h * 0.1;
    let centre = Vec3::new(f.lean.x, crown_y, f.lean.z);
    let mut field = ((f.q - centre) / Vec3::new(f.radius * 0.55, f.crown_h * 0.5, f.radius * 0.55))
        .length_squared();
    let mut size = (f.radius * 0.55).min(f.crown_h * 0.5);
    let limbs = umbrella_limbs(tree);
    for limb in 0..limbs {
        let variation = limb_variation(cfg, tree, limb);
        let angle = f.rotation + limb as f32 * TAU / limbs as f32 + (variation - 0.5) * 0.9;
        let reach = f.radius * (0.45 + variation * 0.25);
        let rise = if cfg.generator_version >= 47 { (variation - 0.5) * f.crown_h * 0.35 } else { 0.0 };
        let tip = centre + Vec3::new(angle.cos() * reach, rise, angle.sin() * reach);
        let (distance, along) = distance_to_segment(f.q, fork, tip);
        let minimum = if cfg.generator_version >= 47 { 0.82 } else { 0.53 };
        if distance <= (f.stem * (0.62 - along * 0.30)).max(f.scale * minimum) {
            return BlockId::Log;
        }
        let pad = f.radius * (0.45 + variation * 0.15);
        let lobe = ((f.q - tip) / Vec3::new(pad, f.crown_h * 0.5, pad)).length_squared();
        if lobe < field {
            field = lobe;
            size = pad.min(f.crown_h * 0.5);
        }
    }
    leaf_or_air(cfg, p, f, field, size)
}

/// Rounded crown whose rim hangs in ragged curtains towards the ground.
pub(super) fn weeping(cfg: &WorldSampleConfig, tree: &TreeProfile, p: IVec3, f: &Frame) -> BlockId {
    let centre = Vec3::new(f.lean.x, f.height, f.lean.z);
    let mut field = ((f.q - centre) / Vec3::new(f.radius * 0.55, f.crown_h * 0.45, f.radius * 0.55))
        .length_squared();
    let mut size = (f.radius * 0.55).min(f.crown_h * 0.45);
    for limb in 0..tree.branch_count {
        let variation = limb_variation(cfg, tree, limb);
        let angle = f.rotation + limb as f32 * 2.399963;
        let reach = f.radius * (0.55 + variation * 0.20);
        let start_y = f.height * (0.6 + limb as f32 / tree.branch_count as f32 * 0.3);
        let start = f.lean * (start_y / f.height).powi(2) + Vec3::Y * start_y;
        let tip = centre + Vec3::new(angle.cos() * reach, f.crown_h * 0.15, angle.sin() * reach);
        let (distance, along) = distance_to_segment(f.q, start, tip);
        if distance <= (f.stem * (0.60 - along * 0.35)).max(f.scale * 0.53) {
            return BlockId::Log;
        }
        let lobe = Vec3::new(f.radius * 0.28, f.crown_h * 0.30, f.radius * 0.28);
        let l = ((f.q - tip) / lobe).length_squared();
        if l < field {
            field = l;
            size = lobe.min_element();
        }
    }
    let leaf = leaf_or_air(cfg, p, f, field, size);
    if leaf != BlockId::Air || f.q.y > f.height {
        return leaf;
    }
    // Curtains: one strand per angular sector, some missing, ragged lengths.
    let rel = (f.q - f.lean).xz();
    let sector = ((rel.y.atan2(rel.x) - f.rotation).rem_euclid(TAU) / TAU * 22.0) as i32;
    let strand = hash3(tree.cell_x, sector, tree.cell_z, cfg.seed.wrapping_add(12131));
    let outer = f.radius * (0.62 + strand * 0.30);
    let thickness = (f.scale * 1.2).max(0.7);
    let d = rel.length();
    let hangs = strand > 0.3 && f.q.y >= f.height * (0.30 + strand * 0.25);
    if let Some(relief) = f.relief {
        let outside = (d - outer).max(outer - thickness - d);
        let leaf = hangs && relief.shell(f.q, f.scale, outside, thickness * 0.5, foliage::FRONDS);
        return if leaf { BlockId::Leaves } else { BlockId::Air };
    }
    if hangs && d <= outer && d >= outer - thickness {
        BlockId::Leaves
    } else {
        BlockId::Air
    }
}

/// Leaf reach in metres from the anchor for non-classic crowns; mirrors the
/// shapes above. `None` means the caller's classic bound applies.
pub(super) fn crown_reach(cfg: &WorldSampleConfig, tree: &TreeProfile, radius: f32, lean_len: f32) -> Option<f32> {
    let scale = cfg.voxel_meters();
    let crown = match tree.form {
        TreeForm::Cycad | TreeForm::Fungal | TreeForm::Redwood => return Some(radius + scale),
        TreeForm::Classic | TreeForm::HighCrown | TreeForm::Giant
        | TreeForm::Rainforest | TreeForm::RainforestPalm | TreeForm::RainforestFig | TreeForm::OldOak
        | TreeForm::CloudCrown | TreeForm::PlateauCrown | TreeForm::Aspen | TreeForm::Cypress | TreeForm::Acacia => return None,
        TreeForm::Umbrella => {
            let mut reach = radius * 0.55 * THRESHOLD_REACH;
            for limb in 0..umbrella_limbs(tree) {
                let variation = limb_variation(cfg, tree, limb);
                let pad = radius * (0.45 + variation * 0.15);
                reach = reach.max(radius * (0.45 + variation * 0.25) + pad * THRESHOLD_REACH);
            }
            reach
        }
        TreeForm::Weeping => {
            let mut reach = (radius * 0.55 * THRESHOLD_REACH).max(radius * 0.92);
            for limb in 0..tree.branch_count {
                let variation = limb_variation(cfg, tree, limb);
                reach = reach.max(radius * (0.55 + variation * 0.20) + radius * 0.28 * THRESHOLD_REACH);
            }
            reach
        }
    };
    Some(lean_len + crown + scale)
}

/// Highest leaf in metres above the ground voxel for non-classic crowns.
pub(super) fn crown_top(cfg: &WorldSampleConfig, tree: &TreeProfile, height: f32, crown_h: f32) -> Option<f32> {
    let top = match tree.form {
        TreeForm::Cycad | TreeForm::Fungal | TreeForm::Redwood => height + crown_h,
        TreeForm::Classic | TreeForm::HighCrown | TreeForm::Giant
        | TreeForm::Rainforest | TreeForm::RainforestPalm | TreeForm::RainforestFig | TreeForm::OldOak
        | TreeForm::CloudCrown | TreeForm::PlateauCrown | TreeForm::Aspen | TreeForm::Cypress | TreeForm::Acacia => return None,
        TreeForm::Umbrella if cfg.generator_version >= 47 => {
            let mut rise = 0.0f32;
            for limb in 0..umbrella_limbs(tree) {
                rise = rise.max((limb_variation(cfg, tree, limb) - 0.5) * crown_h * 0.35);
            }
            height + crown_h * (0.1 + 0.5 * THRESHOLD_REACH) + rise
        }
        TreeForm::Umbrella => height + crown_h * (0.1 + 0.5 * THRESHOLD_REACH),
        TreeForm::Weeping => {
            height + (crown_h * 0.45 * THRESHOLD_REACH).max(crown_h * (0.15 + 0.30 * THRESHOLD_REACH))
        }
    };
    Some(top + 0.5 * cfg.voxel_meters())
}
