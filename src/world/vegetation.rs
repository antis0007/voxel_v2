//! V5 vegetation: coherent groves, varied age and overlapping branch crowns.
//! All dimensions are metres; legacy saved worlds retain their original trees.
use super::{BlockId, IVec3, TreeProfile, WorldSampleConfig, hash2, hash3, landscape, tree_trunk_center};

pub(super) fn shape_profile(cfg: &WorldSampleConfig, tree: &mut TreeProfile) {
    let s = landscape::sample(cfg, tree.anchor_x, tree.anchor_z);
    let age = hash2(tree.cell_x, tree.cell_z, cfg.seed.wrapping_add(9413));
    let species = hash2(tree.cell_x.div_euclid(3), tree.cell_z.div_euclid(3), cfg.seed.wrapping_add(9421));
    tree.conifer = s.temperature < 0.31 || (s.moisture > 0.48 && species < 0.24);
    if tree.conifer {
        tree.trunk_h = cfg.blocks_for_span(9.0 + age * 12.0);
        tree.canopy_radius = cfg.blocks_for_span(2.5 + age * 2.3);
        tree.canopy_height = cfg.blocks_for_span(1.5);
    } else {
        let slender = species > 0.72;
        tree.trunk_h = cfg.blocks_for_span(if slender { 6.0 + age * 6.0 } else { 4.0 + age * 5.0 });
        tree.canopy_radius = cfg.blocks_for_span(if slender { 2.1 + age * 1.5 } else { 3.0 + age * 2.4 });
        tree.canopy_height = cfg.blocks_for_span(if slender { 6.0 + age * 3.0 } else { 3.5 + age * 2.5 });
        tree.branch_start_y = tree.ground_y + tree.trunk_h - cfg.blocks_for_span(2.0 + age * 2.0);
        tree.branch_len = (tree.canopy_radius as f32 * 0.62).round() as i32;
        tree.branch_count = 4 + (age * 3.0) as i32;
    }
}

pub(super) fn tree_block(cfg: &WorldSampleConfig, tree: TreeProfile, p: IVec3) -> BlockId {
    let scale = cfg.voxel_meters();
    let dy = (p.y - tree.ground_y) as f32 * scale;
    let height = tree.trunk_h as f32 * scale;
    let radius = tree.canopy_radius as f32 * scale;
    let crown_h = tree.canopy_height as f32 * scale;
    let x = (p.x - tree.anchor_x) as f32 * scale;
    let z = (p.z - tree.anchor_z) as f32 * scale;
    // Reject other cells before evaluating crown lobes. The 12 m cell search
    // already includes every possible crown overhang (under 10 m).
    if dy <= 0.0 || dy > height + crown_h + scale || x.abs() > radius * 1.7 + 2.0 || z.abs() > radius * 1.7 + 2.0 {
        return BlockId::Air;
    }
    if tree.conifer {
        let top = height + 1.5;
        let crown_base = height * 0.14;
        let t = ((dy - crown_base) / (top - crown_base)).clamp(0.0, 1.0);
        let tier = ((dy + hash2(tree.cell_x, tree.cell_z, cfg.seed) * 2.0) / 2.1).fract();
        let r = radius * (1.0 - t).powf(0.72) * (0.72 + 0.28 * (1.0 - tier));
        // Leave two metres of needles above the woody trunk. A minimum apex
        // radius keeps the highest voxel leafy even at the coarse 1 m scale.
        if dy <= height * 0.72 && x.abs() < (scale * 0.45).max(0.24) && z.abs() < (scale * 0.45).max(0.24) {
            return BlockId::Log;
        }
        let edge = hash3(p.x, p.y, p.z, cfg.seed.wrapping_add(9437));
        if dy >= crown_base && dy <= top && x * x + z * z <= (r.max(scale * 0.55) * (0.92 + edge * 0.12)).powi(2) {
            return BlockId::PineLeaves;
        }
        return BlockId::Air;
    }
    let top_y = tree.ground_y + tree.trunk_h;
    let (tx, tz) = tree_trunk_center(tree, p.y.min(top_y));
    if p.y <= top_y && (p.x - tx).abs() <= cfg.blocks_for_height(0.24) && (p.z - tz).abs() <= cfg.blocks_for_height(0.24) {
        return BlockId::Log;
    }
    let (cx, cz) = tree_trunk_center(tree, top_y);
    let dx = (p.x - cx) as f32 * scale;
    let dz = (p.z - cz) as f32 * scale;
    let cy = height + crown_h * 0.22;
    let ry = crown_h * 0.66;
    let mut field = (dx / (radius * 0.78)).powi(2) + ((dy - cy) / ry).powi(2) + (dz / (radius * 0.78)).powi(2);
    let rotation = hash2(tree.cell_x, tree.cell_z, cfg.seed.wrapping_add(9461)) * std::f32::consts::TAU;
    for branch in 0..tree.branch_count {
        let variation = hash3(tree.cell_x, branch, tree.cell_z, cfg.seed.wrapping_add(9473));
        let angle = rotation + branch as f32 * 2.399963;
        let (sz, sx) = angle.sin_cos();
        let reach = radius * (0.48 + variation * 0.30);
        let center_y = height - 0.6 + variation * crown_h * 0.42;
        let lobe_r = radius * (0.56 + variation * 0.18);
        let lobe_y = ry * (0.70 + variation * 0.22);
        let f = ((dx - sx * reach) / lobe_r).powi(2) + ((dy - center_y) / lobe_y).powi(2) + ((dz - sz * reach) / lobe_r).powi(2);
        field = field.min(f);
        // Connected rising boughs stop inside the crown, never at its surface.
        let start = (tree.branch_start_y - tree.ground_y) as f32 * scale;
        let along = (dx * sx + dz * sz).clamp(0.0, reach);
        let by = start + (center_y - start) * along / reach.max(0.1);
        if along > 0.0 && along < reach * 0.80 && (dx - sx * along).abs() <= scale * 0.5
            && (dz - sz * along).abs() <= scale * 0.5 && (dy - by).abs() <= scale * 0.5 {
            return BlockId::Log;
        }
    }
    let edge = hash3(p.x, p.y, p.z, cfg.seed.wrapping_add(9491));
    if field < 0.91 + edge * 0.16 { BlockId::Leaves } else { BlockId::Air }
}
