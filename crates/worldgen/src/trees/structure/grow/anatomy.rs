use super::{Builder, Design, TreeForm, TreeProfile, WorldSampleConfig, bezier, direction, fit};
use crate::trees::structure::Buttress;
use glam::{Vec3, Vec3Swizzles};
use std::f32::consts::TAU;

pub(in super::super) fn uses_anatomy(cfg: &WorldSampleConfig, tree: &TreeProfile) -> bool {
    cfg.generator_version >= engine_voxel::TREE_MORPHOLOGY_GENERATOR && match tree.form {
        TreeForm::OldOak | TreeForm::CloudCrown | TreeForm::PlateauCrown
        | TreeForm::Aspen | TreeForm::Cypress | TreeForm::Acacia => true,
        TreeForm::Classic => tree.conifer && cfg.world_y(tree.trunk_h) >= 22.0,
        TreeForm::Redwood => crate::trees::conifer::tall_redwood(cfg, tree),
        _ => false,
    }
}

pub(super) fn grow(b: &mut Builder, cfg: &WorldSampleConfig, tree: &TreeProfile) {
    let height = cfg.world_y(tree.trunk_h);
    let radius = cfg.world_x(tree.canopy_radius);
    let turn = b.rng.next() * TAU;
    if tree.conifer && tree.form != TreeForm::Cypress {
        let fork = cfg.world_y(tree.branch_start_y - tree.ground_y);
        conifer(b, height, radius, fork, turn, tree.form == TreeForm::Redwood);
    } else {
        let site_fork = (crate::trees::habitats::active(cfg)
            && matches!(tree.form, TreeForm::OldOak | TreeForm::Aspen | TreeForm::Cypress | TreeForm::Acacia))
            .then(|| cfg.world_y(tree.branch_start_y - tree.ground_y));
        broadleaf(b, height, radius, cfg.world_y(tree.canopy_height), turn, tree.form, site_fork);
    }
    if cfg.generator_version >= engine_voxel::CANOPY_UNDERSTORY_GENERATOR {
        let factor = match tree.form {
            TreeForm::Redwood => 1.53,
            TreeForm::OldOak | TreeForm::PlateauCrown => 1.42,
            TreeForm::CloudCrown | TreeForm::Cypress | TreeForm::Acacia => 1.36,
            TreeForm::Aspen => 1.12,
            _ => 1.20,
        };
        super::widen(&mut b.s, factor);
    }
}

// The branches interpolate along the same segments used to voxelize the bole.
fn bole(b: &mut Builder, tip: Vec3, mid: Vec3, base_radius: f32, tip_radius: f32) -> [Vec3; 6] {
    let points = std::array::from_fn(|i| bezier(Vec3::NEG_Y, mid, tip, i as f32 / 5.0));
    let radius = |i: usize| tip_radius + (base_radius - tip_radius) * (1.0 - i as f32 / 5.0).powf(1.20);
    for i in 0..5 {
        b.segment(points[i], points[i + 1], radius(i), radius(i + 1));
    }
    points
}

fn station(points: &[Vec3; 6], y: f32) -> Vec3 {
    for pair in points.windows(2) {
        if y <= pair[1].y {
            return pair[0].lerp(pair[1], ((y - pair[0].y) / (pair[1].y - pair[0].y)).clamp(0.0, 1.0));
        }
    }
    points[5]
}

fn roots(b: &mut Builder, radius: f32, turn: f32) {
    b.s.flare = 0.63;
    b.s.flare_h = radius * 2.3;
    for (i, root) in b.s.buttresses.iter_mut().enumerate() {
        let angle = turn + i as f32 * TAU / 5.0 + b.rng.range(-0.29, 0.29);
        *root = Some(Buttress {
            axis: direction(angle, 0.0).xz(),
            length: radius * b.rng.range(2.0, 3.9),
            height: b.s.flare_h * b.rng.range(0.55, 0.95),
            width: radius * b.rng.range(0.32, 0.52),
        });
    }
}

fn conifer(b: &mut Builder, height: f32, radius: f32, fork: f32, turn: f32, redwood: bool) {
    let base_radius = if redwood { b.rng.range(1.85, 2.55) } else { b.rng.range(0.65, 0.92) };
    let lean = direction(turn, 0.0) * height * b.rng.range(0.014, 0.045);
    let tip = lean + Vec3::Y * height;
    let mid = Vec3::Y * height * 0.49 + direction(turn + 1.8, 0.0) * lean.length() * 0.68;
    b.begin();
    let leader = tip - Vec3::Y * if redwood { 2.9 } else { 3.1 };
    let stem = bole(b, leader, mid, base_radius, 0.12);
    if redwood {
        roots(b, base_radius, turn);
    }
    b.end();

    // Stagger the boughs along a spiral, so each tree has open sides and gaps.
    for tier in 0..6 {
        b.begin();
        for branch in 0..4 {
            let i = (tier * 4 + branch) as f32;
            let t = (i + b.rng.range(0.15, 0.90)) / 24.0;
            let start = station(&stem, fork + t * (leader.y - fork));
            let angle = turn + i * 2.399_963 + b.rng.range(-0.27, 0.27);
            let out = direction(angle, 0.0);
            let envelope = if redwood {
                (0.62 + 0.38 * (t * std::f32::consts::PI).sin()) * (1.0 - t.powi(3)).powf(0.58)
            } else {
                (1.0 - 0.92 * t).powf(1.08)
            };
            let length = radius * envelope * b.rng.range(0.78, 1.13);
            let bend = if redwood { 0.11 } else { -0.14 };
            let elbow = start + (out + Vec3::Y * bend) * (length * 0.54);
            let end = start + (out + Vec3::Y * bend * 1.8) * length;
            let wood = base_radius * (0.33 - 0.22 * t);
            let long_bough = redwood || t < 0.68;
            b.segment(start, elbow, wood, wood * 0.58);
            if long_bough {
                b.segment(elbow, elbow.lerp(end, 0.71), wood * 0.58, 0.08);
            }

            // Needle fans follow the bough and its side shoots. Their separate
            // tips and the short inner tufts soften the open edges of the crown.
            let depth = if redwood { 0.40 } else { 0.34 };
            let pad = (length * 0.35).max(1.04);
            let width = (length * 0.30).max(1.05);
            let leaf_y = (length * depth).max(1.12);
            let core = if redwood { 1.65 } else { 1.54 } + length * 0.15;
            let inner = if redwood { start + out * 0.42 } else { start + Vec3::Y * 0.58 + out * 0.20 };
            b.cluster(inner, out, Vec3::new(core, core * 1.13, core * 0.91), 0.86);
            b.cluster(elbow + Vec3::Y * 0.36, out, Vec3::new(pad, leaf_y, width), 0.86);
            b.cluster(end, out, Vec3::new(pad * 0.86, leaf_y * 0.90, width * 0.83), 0.91);
            for side in [-1.0, 1.0] {
                let dir = direction(angle + side * b.rng.range(0.48, 0.96), bend);
                let shoot = elbow + dir * length * b.rng.range(0.29, 0.47);
                if long_bough {
                    b.segment(elbow, elbow.lerp(shoot, 0.62), wood * 0.39, 0.06);
                }
                b.cluster(shoot + Vec3::Y * 0.2, dir, Vec3::new(pad * 0.71, leaf_y * 0.75, width * 0.82), 0.78);
            }
        }
        if tier == 5 {
            let crown = if redwood { 1.64 } else { 1.23 };
            b.cluster(tip - Vec3::Y * crown, direction(turn, 0.0), Vec3::new(crown, crown * 1.6, crown), 0.94);
        }
        b.end();
    }
    fit(&mut b.s, radius * 1.40, height + 1.8, fork);
}

fn broadleaf(b: &mut Builder, height: f32, radius: f32, crown: f32, turn: f32, form: TreeForm, site_fork: Option<f32>) {
    let oak = form == TreeForm::OldOak;
    let cloud = form == TreeForm::CloudCrown;
    let cypress = form == TreeForm::Cypress;
    let (fork, lean, trunk, depth, limbs, rise, droop, cluster) = match form {
        TreeForm::OldOak => (0.53, 0.10, 1.85, 0.93, 5, (0.42, 0.69), 0.09, 0.36),
        TreeForm::CloudCrown => (0.70, 0.21, 0.80, 0.77, 4, (0.40, 0.82), 0.16, 0.35),
        TreeForm::Aspen => (0.60, 0.065, 0.58, 1.13, 4, (0.78, 1.13), 0.07, 0.47),
        TreeForm::Cypress => (0.66, 0.055, 1.38, 0.94, 5, (0.31, 0.72), 0.11, 0.43),
        TreeForm::Acacia => (0.74, 0.28, 0.78, 0.58, 4, (0.14, 0.38), 0.08, 0.38),
        _ => (0.82, 0.16, 0.95, 0.67, 5, (0.24, 0.50), 0.09, 0.38),
    };
    let fork = site_fork.unwrap_or(height * fork);
    let trunk_r = trunk * b.rng.range(0.86, 1.12);
    let d = Design {
        height, crown, reach: radius * 1.40, trunk_r, flare: 0.0, fork, limbs,
        rise, droop, cluster: radius * cluster, slender: false, old: oak,
    };
    b.mass_depth = depth;
    b.fill_canopy = true;
    b.trailing_leaves = cloud && matches!(b.leaf_habit, super::LeafHabit::Rounded);
    let lean = direction(turn, 0.0) * (height * lean * b.rng.range(0.67, 1.20));
    b.begin();
    let stem = bole(b, lean + Vec3::Y * fork, lean * -0.22 + Vec3::Y * fork * 0.48, trunk_r, trunk_r * 0.73);
    if oak || cypress {
        roots(b, trunk_r, turn);
    }
    b.end();

    // Several equal forks share the crown. The longest bough changes direction
    // with the individual tree, leaving enough space to read the wood beneath.
    for i in 0..limbs {
        let angle = turn + i as f32 * TAU / limbs as f32 + b.rng.range(-0.30, 0.30);
        let out = direction(angle, 0.0) + lean.normalize_or_zero() * if cloud { 0.52 } else { 0.13 };
        let high_fork = i == 0 || (oak && i == 2);
        let start = station(&stem, fork * if high_fork { 1.0 } else { b.rng.range(0.80, 0.99) });
        let rise = if oak && high_fork { b.rng.range(0.67, 0.88) } else { b.rng.range(rise.0, rise.1) };
        let length = d.reach * if high_fork { b.rng.range(0.89, 1.09) } else { b.rng.range(0.61, 0.98) };
        let width = trunk_r * if high_fork { 0.61 } else { 0.49 };
        b.limb(start, out.z.atan2(out.x), rise, length, width, &d);
    }
    fit(&mut b.s, d.reach, height + crown, fork);
}
