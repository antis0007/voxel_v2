use super::*;
use crate::trees::TreeForm;
use crate::WorldType;
use engine_core::VoxelScalePreset;
use std::collections::{HashSet, VecDeque};

fn cfg(version: u32, scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version: version, seed: 2_738_958_700, world_type: WorldType::Hills, voxel_scale: scale }
}

fn tree(cfg: &WorldSampleConfig, cell: (i32, i32), trunk: f32, radius: f32, crown: f32) -> TreeProfile {
    TreeProfile {
        conifer: false,
        cell_x: cell.0,
        cell_z: cell.1,
        anchor_x: 0,
        anchor_z: 0,
        ground_y: 0,
        trunk_h: cfg.blocks_for_span(trunk),
        canopy_radius: cfg.blocks_for_span(radius),
        canopy_height: cfg.blocks_for_span(crown),
        branch_count: 5,
        branch_len: cfg.blocks_for_span(radius * 0.76),
        branch_start_y: cfg.blocks_for_span(trunk * 0.3),
        lean_x: 0,
        lean_z: 0,
        form: TreeForm::Classic,
    }
}

/// Wood and leaf voxels of one tree, each leaf checked against its bounds.
fn grow(cfg: &WorldSampleConfig, t: &TreeProfile) -> (HashSet<IVec3>, HashSet<IVec3>) {
    let v = cfg.voxel_meters();
    let (reach, top) = bounds(cfg, t);
    let n = (reach / v).ceil() as i32 + 2;
    let (mut wood, mut leaves) = (HashSet::new(), HashSet::new());
    for y in 1..=(top / v).ceil() as i32 + 2 {
        for z in -n..=n {
            for x in -n..=n {
                let q = IVec3::new(x, y, z);
                match block(cfg, t, q) {
                    BlockId::Log => {
                        wood.insert(q);
                    }
                    BlockId::Leaves => {
                        assert!((x as f32).hypot(z as f32) * v <= reach, "leaf outside reach at {v} m");
                        assert!(y as f32 * v <= top, "leaf {} above top {top} at {v} m", y as f32 * v);
                        leaves.insert(q);
                    }
                    _ => {}
                }
            }
        }
    }
    (wood, leaves)
}

const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];

/// Fraction of leaves not 6-connected to wood through other leaves or wood.
fn detached(wood: &HashSet<IVec3>, leaves: &HashSet<IVec3>) -> f32 {
    let mut seen: HashSet<IVec3> = wood.iter().copied().collect();
    let mut queue: VecDeque<IVec3> = wood.iter().copied().collect();
    while let Some(p) = queue.pop_front() {
        for d in SIDES {
            let q = p + d;
            if (leaves.contains(&q) || wood.contains(&q)) && seen.insert(q) {
                queue.push_back(q);
            }
        }
    }
    let attached = leaves.iter().filter(|p| seen.contains(p)).count();
    1.0 - attached as f32 / leaves.len().max(1) as f32
}

#[test]
fn dormant_before_the_gate_and_identical_below_it() {
    let before = cfg(VERSION - 1, VoxelScalePreset::OneMeter);
    let t = tree(&before, (3, -5), 9.0, 4.5, 4.0);
    assert!(!applies(&before, &t));
    let after = cfg(VERSION, VoxelScalePreset::OneMeter);
    assert!(applies(&after, &t));
    let v126 = cfg(126, VoxelScalePreset::OneMeter);
    for p in [IVec3::new(0, 3, 0), IVec3::new(2, 9, 1), IVec3::new(-3, 11, 2)] {
        assert_eq!(crate::trees::block(&v126, t, p), crate::trees::block(&before, t, p));
    }
}

#[test]
fn clusters_hang_from_twigs_with_wood_in_every_group() {
    let c = cfg(VERSION, VoxelScalePreset::OneMeter);
    let t = tree(&c, (3, -5), 9.0, 4.5, 4.0);
    let s = grow::build(&c, &t);
    assert!(s.group_count >= 4, "trunk, leader and limbs: {}", s.group_count);
    assert!(s.cluster_count >= 24, "{} clusters", s.cluster_count);
    for g in &s.groups[..s.group_count] {
        assert!(g.segments.1 > g.segments.0, "a group without wood");
        assert!(g.clusters.1 > g.clusters.0, "a group without leaves");
    }
    // Every cluster overlaps the wood that carries it.
    for l in &s.clusters[..s.cluster_count] {
        let near = s.segments[..s.segment_count]
            .iter()
            .any(|seg| crate::trees::distance_to_segment(l.c, seg.a, seg.b).0 <= l.r.max_element() * 2.0 + 0.3);
        let best = s.segments[..s.segment_count].iter().map(|seg| crate::trees::distance_to_segment(l.c, seg.a, seg.b).0).fold(f32::MAX, f32::min);
        assert!(near, "cluster at {} r={} floats free of wood (nearest wood {best})", l.c, l.r);
    }
}

#[test]
fn leaves_stay_connected_and_the_crown_is_open() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let c = cfg(VERSION, scale);
        let t = tree(&c, (3, -5), 9.0, 4.5, 4.0);
        let (wood, leaves) = grow(&c, &t);
        assert!(!wood.is_empty() && !leaves.is_empty());
        let loose = detached(&wood, &leaves);
        assert!(loose < 0.04, "{loose:.3} of leaves detached at {} m", c.voxel_meters());
        // The crown envelope is not solid: inside the leaf bounding box a
        // real share of cells is air.
        let (lo, hi) = leaves.iter().fold((IVec3::MAX, IVec3::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let span = hi - lo + IVec3::ONE;
        let cells = (span.x * span.y * span.z) as f32;
        let filled = (leaves.len() + wood.len()) as f32 / cells;
        // Coarse grids cannot show gaps narrower than a voxel, so the bar
        // tightens as the grid refines.
        let bar = 0.34 + 0.22 * c.voxel_meters();
        assert!(filled < bar, "crown box {filled:.2} filled at {} m", c.voxel_meters());
    }
}

#[test]
fn same_tree_at_every_scale() {
    let one = cfg(VERSION, VoxelScalePreset::OneMeter);
    let quarter = cfg(VERSION, VoxelScalePreset::QuarterMeter);
    let a = tree(&one, (3, -5), 9.0, 4.5, 4.0);
    let b = tree(&quarter, (3, -5), 9.0, 4.5, 4.0);
    let (ra, ta) = bounds(&one, &a);
    let (rb, tb) = bounds(&quarter, &b);
    assert!((ra - rb).abs() < 1.6, "reach {ra} vs {rb}");
    assert!((ta - tb).abs() < 1.6, "top {ta} vs {tb}");
    let (_, l, n) = visual(&one, &a);
    assert!(n >= 3);
    assert!(l[..n].iter().all(|l| l.radii.min_element() > 0.3));
}

#[test]
fn one_open_sector_between_primary_limbs() {
    let c = cfg(VERSION, VoxelScalePreset::OneMeter);
    let t = tree(&c, (3, -5), 9.0, 4.5, 4.0);
    let s = grow::build(&c, &t);
    // Limb groups start at index 1; their first segment leaves the trunk.
    let mut azimuths: Vec<f32> = s.groups[1..s.group_count]
        .iter()
        .map(|g| {
            let seg = s.segments[g.segments.0 as usize];
            let d = seg.b - seg.a;
            d.z.atan2(d.x).rem_euclid(std::f32::consts::TAU)
        })
        .collect();
    azimuths.sort_by(f32::total_cmp);
    let mut widest = 0.0f32;
    for i in 0..azimuths.len() {
        let next = azimuths[(i + 1) % azimuths.len()];
        let gap = (next - azimuths[i]).rem_euclid(std::f32::consts::TAU);
        widest = widest.max(gap);
    }
    assert!(widest > grow::OPEN_SECTOR * 0.9, "widest gap {widest}");
}

/// `cargo test -p engine_worldgen --lib trees::structure::tests::print_anchors -- --ignored --nocapture`
/// Structure trees near the forest review pose, in metres.
#[test]
#[ignore]
fn print_anchors() {
    let c = cfg(VERSION, VoxelScalePreset::QuarterMeter);
    let cell_m = c.blocks_for_span(12.0) as f32 * c.voxel_meters();
    let (cx, cz) = ((432.0 / cell_m) as i32, (-4.0 / cell_m) as i32);
    for z in cz - 12..=cz + 12 {
        for x in cx - 12..=cx + 12 {
            let Some(t) = crate::tree_profile_for_cell(&c, x, z) else { continue };
            if !applies(&c, &t) { continue; }
            let v = c.voxel_meters();
            let (reach, top) = bounds(&c, &t);
            println!("anchor_m=({:.1},{:.1},{:.1}) trunk={:.1} radius={:.1} reach={reach:.1} top={top:.1} cell=({x},{z})",
                t.anchor_x as f32 * v, (t.ground_y + 1) as f32 * v, t.anchor_z as f32 * v, t.trunk_h as f32 * v, t.canopy_radius as f32 * v);
        }
    }
}
