//! Versioned tree forms with tapered woody structure and connected crown masses.
use super::{BlockId, IVec3, TreeProfile, WorldSampleConfig, hash2, hash3, landscape};
use glam::Vec3Swizzles;

pub(super) fn profile(cfg: &WorldSampleConfig, tree: &mut TreeProfile) {
    let climate = landscape::sample(cfg, tree.anchor_x, tree.anchor_z);
    let age = hash2(tree.cell_x, tree.cell_z, cfg.seed.wrapping_add(12011));
    let species = hash2(tree.cell_x.div_euclid(3), tree.cell_z.div_euclid(3), cfg.seed.wrapping_add(12037));
    let individual_form = hash2(tree.cell_x, tree.cell_z, cfg.seed.wrapping_add(12073));
    tree.conifer = climate.temperature < 0.32 || (climate.moisture > 0.55 && species < 0.28);
    if cfg.generator_version >= 7 {
        if tree.conifer {
            // Taller spruce and fir silhouettes with enough breadth to read as
            // woodland crowns instead of thin voxel spikes at one-metre scale.
            tree.trunk_h = cfg.blocks_for_span(12.0 + age * 15.0);
            tree.canopy_radius = cfg.blocks_for_span(3.2 + age * 2.6);
            tree.canopy_height = cfg.blocks_for_span(1.0);
            tree.branch_start_y = tree.ground_y + cfg.blocks_for_span(3.2 + age * 2.0);
        } else {
            // Columnar, spreading and irregular broadleaf forms occupy distinct
            // canopy proportions while keeping the existing low-cost branch loop.
            let form_seed = if cfg.generator_version >= 8 { individual_form } else { species };
            let form = if form_seed > 0.76 { 0 } else if form_seed < 0.27 { 1 } else { 2 };
            tree.trunk_h = cfg.blocks_for_span(match form {
                0 => 10.0 + age * 8.0,
                1 => 6.0 + age * 4.0,
                _ => 8.0 + age * 9.0,
            });
            tree.canopy_radius = cfg.blocks_for_span(match form {
                0 => 2.8 + age * 1.8,
                1 => 4.8 + age * 2.8,
                _ => 3.5 + age * 2.8,
            });
            tree.canopy_height = cfg.blocks_for_span(match form {
                0 => 6.0 + age * 3.2,
                1 => 4.2 + age * 3.0,
                _ => 5.4 + age * 4.0,
            });
            tree.branch_start_y = tree.ground_y + cfg.blocks_for_span(3.0 + age * 1.8);
        }
        tree.branch_count = if cfg.generator_version >= 8 {
            4 + (individual_form * 5.0) as i32
        } else {
            5 + (age * 4.0) as i32
        };
        tree.branch_len = (tree.canopy_radius as f32 * 0.76) as i32;
        if cfg.generator_version >= 9 {
            tree.trunk_h = (tree.trunk_h as f32 * 0.72).round().max(3.0) as i32;
            tree.canopy_radius = (tree.canopy_radius as f32 * 0.82).round().max(2.0) as i32;
            tree.canopy_height = (tree.canopy_height as f32 * 0.78).round().max(1.0) as i32;
            tree.branch_len = (tree.canopy_radius as f32 * 0.76) as i32;
        }
        if cfg.generator_version >= 10 {
            use landscape::Biome;
            match climate.biome {
                Biome::Savanna => {
                    tree.conifer = false;
                    tree.trunk_h = cfg.blocks_for_span(5.0 + age * 3.0);
                    tree.canopy_radius = cfg.blocks_for_span(4.0 + age * 2.0);
                    tree.canopy_height = cfg.blocks_for_span(2.0 + age);
                }
                Biome::Rainforest => {
                    tree.conifer = false;
                    tree.trunk_h = cfg.blocks_for_span(11.0 + age * 7.0);
                    tree.canopy_radius = cfg.blocks_for_span(4.0 + age * 2.0);
                    tree.canopy_height = cfg.blocks_for_span(5.0 + age * 3.0);
                }
                Biome::Heathland => {
                    tree.trunk_h = cfg.blocks_for_span(2.5 + age * 2.0);
                    tree.canopy_radius = cfg.blocks_for_span(1.5 + age);
                    tree.canopy_height = cfg.blocks_for_span(2.0 + age);
                }
                Biome::Fen => {
                    tree.conifer = false;
                    tree.trunk_h = cfg.blocks_for_span(4.0 + age * 3.0);
                    tree.canopy_radius = cfg.blocks_for_span(3.0 + age * 1.5);
                    tree.canopy_height = cfg.blocks_for_span(3.5 + age * 2.0);
                }
                _ => {}
            }
            let branch_fraction = if cfg.generator_version >= 11 && tree.conifer { 0.18 } else { 0.55 };
            tree.branch_start_y = tree.ground_y + (tree.trunk_h as f32 * branch_fraction) as i32;
            tree.branch_len = (tree.canopy_radius as f32 * 0.76) as i32;
        }
        return;
    }
    if tree.conifer {
        tree.trunk_h = cfg.blocks_for_span(10.0 + age * 13.0);
        tree.canopy_radius = cfg.blocks_for_span(2.4 + age * 2.1);
        tree.canopy_height = cfg.blocks_for_span(1.0);
        tree.branch_start_y = tree.ground_y + cfg.blocks_for_span(2.2 + age * 1.5);
    } else {
        // Three stable broadleaf silhouettes: narrow, high crowns; spreading
        // crowns with a short trunk; and irregular, taller woodland crowns.
        let form = if species > 0.72 { 0 } else if species < 0.30 { 1 } else { 2 };
        tree.trunk_h = cfg.blocks_for_span(match form {
            0 => 8.0 + age * 6.0,
            1 => 5.0 + age * 3.5,
            _ => 6.5 + age * 7.0,
        });
        tree.canopy_radius = cfg.blocks_for_span(match form {
            0 => 2.0 + age * 1.1,
            1 => 3.8 + age * 2.0,
            _ => 2.8 + age * 2.3,
        });
        tree.canopy_height = cfg.blocks_for_span(match form {
            0 => 5.2 + age * 2.1,
            1 => 3.2 + age * 1.8,
            _ => 4.0 + age * 3.0,
        });
        tree.branch_start_y = tree.ground_y + cfg.blocks_for_span(2.5 + age * 1.4);
    }
    tree.branch_count = 5 + (age * 4.0) as i32;
    tree.branch_len = (tree.canopy_radius as f32 * 0.72) as i32;
}

fn distance_to_segment(p: glam::Vec3, a: glam::Vec3, b: glam::Vec3) -> (f32, f32) {
    let delta = b - a;
    let t = ((p - a).dot(delta) / delta.length_squared().max(0.001)).clamp(0.0, 1.0);
    ((p - a - delta * t).length(), t)
}

pub(super) fn block(cfg: &WorldSampleConfig, tree: TreeProfile, p: IVec3) -> BlockId {
    use glam::Vec3;
    let scale = cfg.voxel_meters();
    let q = (p - IVec3::new(tree.anchor_x, tree.ground_y, tree.anchor_z)).as_vec3() * scale;
    let height = tree.trunk_h as f32 * scale;
    let radius = tree.canopy_radius as f32 * scale;
    let crown_h = tree.canopy_height as f32 * scale;
    // Bound every lobe and limb before doing branch work, including lean.
    if q.y <= 0.0 || q.y > height + crown_h || q.x.abs().max(q.z.abs()) > radius * 1.55 + 1.0 {
        return BlockId::Air;
    }
    let age = hash2(tree.cell_x, tree.cell_z, cfg.seed.wrapping_add(12011));
    let lean = Vec3::new(tree.lean_x as f32, 0.0, tree.lean_z as f32) * (0.3 + age * 0.45);
    let trunk_center = lean * (q.y / height).clamp(0.0, 1.0).powi(2);
    let stem = 0.23 + age * 0.19;
    let flare = (1.0 - q.y / 1.2).max(0.0) * 0.30;
    let trunk_r = (stem * (1.0 - 0.60 * q.y / height) + flare).max(scale * 0.56);
    // Woody leaders end inside the leafy crown. Letting the trunk run to the
    // mathematical cone tip exposed metres of bare log at coarse voxel sizes.
    let wood_top = if tree.conifer { height * 0.68 } else { height };
    if q.y <= wood_top && (q - trunk_center).xz().length_squared() <= trunk_r * trunk_r {
        return BlockId::Log;
    }
    let rotation = hash2(tree.cell_x, tree.cell_z, cfg.seed.wrapping_add(12049)) * std::f32::consts::TAU;
    // Keep saved v10 worlds stable; v11 opens the crown into branch groups.
    let open_crown = cfg.generator_version >= 11;
    if tree.conifer {
        let base = (tree.branch_start_y - tree.ground_y) as f32 * scale;
        if q.y < base || q.y > height + scale { return BlockId::Air; }
        let t = ((q.y - base) / (height + scale - base)).clamp(0.0, 1.0);
        let spacing = 1.55 + age * 0.45;
        let tier = (q.y - base) / spacing;
        let whorl = tier.floor() as i32;
        let variation = hash3(tree.cell_x, whorl, tree.cell_z, cfg.seed.wrapping_add(12071));
        let shelf = (1.0 - tier.fract()).powf(0.65);
        let angle = q.z.atan2(q.x) + rotation + whorl as f32 * 0.83;
        let scallop = if open_crown {
            // Unequal branch fans break the lathed Christmas-tree silhouette.
            // The dense inner leader stays connected at every voxel scale.
            0.79 + (angle * 5.0).sin() * 0.14 + (angle * 3.0 + variation * 5.0).sin() * 0.10
        } else { 0.92 + (angle * 5.0).sin() * 0.08 };
        let r = radius * (1.0 - t).powf(0.78) * (0.62 + shelf * 0.38)
            * (0.91 + variation * 0.16) * scallop;
        if (q - trunk_center).xz().length() <= r.max(scale * 0.56) {
            return BlockId::PineLeaves;
        }
        return BlockId::Air;
    }
    let crown = Vec3::new(lean.x, height - 0.1, lean.z);
    let core_width = if open_crown { 0.46 } else { 0.70 };
    let core_height = if open_crown { 0.54 } else { 0.68 };
    let mut field = ((q - crown) / Vec3::new(radius * core_width, crown_h * core_height, radius * core_width)).length_squared();
    for limb in 0..tree.branch_count {
        let variation = hash3(tree.cell_x, limb, tree.cell_z, cfg.seed.wrapping_add(12083));
        let angle = rotation + limb as f32 * 2.399963;
        let reach = radius * (0.48 + variation * 0.23);
        let branch_height = limb as f32 / tree.branch_count as f32 * (height * 0.23);
        let vertical_breakup = if cfg.generator_version >= 8 {
            (variation - 0.5) * height * 0.10
        } else {
            0.0
        };
        let start_y = (tree.branch_start_y - tree.ground_y) as f32 * scale
            + branch_height + vertical_breakup;
        let start = Vec3::new(lean.x, 0.0, lean.z) * (start_y / height).powi(2) + Vec3::Y * start_y;
        let tip_height = if open_crown { (variation - 0.55) * crown_h * 0.95 }
            else { (variation - 0.20) * crown_h * 0.48 };
        let tip = crown + Vec3::new(angle.cos() * reach, tip_height, angle.sin() * reach);
        let (distance, along) = distance_to_segment(q, start, tip);
        let wood_r = (stem * (0.65 - along * 0.40)).max(scale * if open_crown {0.53} else {0.73});
        if distance <= wood_r { return BlockId::Log; }
        // Each woody endpoint sits inside a leafy lobe; no detached balls or
        // exposed radiating sticks. Unequal lobes give each crown a silhouette.
        let lobe_variation = if cfg.generator_version >= 8 { 0.24 } else { 0.11 };
        let lobe = if open_crown {
            Vec3::new(radius * (0.32 + variation * 0.17),
                crown_h * (0.31 + variation * 0.19), radius * (0.30 + variation * 0.20))
        } else {
            Vec3::new(radius * (0.43 + variation * lobe_variation),
                crown_h * (0.44 + variation * (lobe_variation + 0.03)),
                radius * (0.40 + variation * (lobe_variation + 0.03)))
        };
        field = field.min(((q - tip - Vec3::Y * crown_h * 0.12) / lobe).length_squared());
    }
    // Low-frequency edge breakup removes spherical surfaces without noisy
    // per-voxel holes throughout the canopy interior.
    let grain = cfg.blocks_for_span(0.85).max(1);
    let edge = hash3(p.x.div_euclid(grain), p.y.div_euclid(grain), p.z.div_euclid(grain), cfg.seed.wrapping_add(12109));
    let threshold = if open_crown { 0.83 + edge * 0.24 } else { 0.94 + edge * 0.12 };
    if field < threshold { BlockId::Leaves } else { BlockId::Air }
}
