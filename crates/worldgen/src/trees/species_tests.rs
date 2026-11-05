//! Generator 130 leaf relief on every non-skeleton crown.
use super::*;
use crate::WorldType;
use engine_core::VoxelScalePreset;
use std::collections::HashSet;

const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];

/// (form, conifer, trunk, crown radius, crown height in metres, needles or fronds only).
const SPECIES: [(TreeForm, bool, f32, f32, f32, bool); 6] = [
    (TreeForm::Classic, false, 3.0, 2.6, 3.0, false),
    (TreeForm::HighCrown, true, 12.0, 3.0, 3.0, false),
    (TreeForm::Umbrella, false, 5.0, 5.0, 3.0, false),
    (TreeForm::Weeping, false, 6.0, 4.0, 4.0, false),
    (TreeForm::Classic, true, 3.5, 2.2, 1.0, true),
    (TreeForm::Classic, true, 14.0, 4.5, 1.0, true),
];

fn tree(cfg: &WorldSampleConfig, (form, conifer, trunk, radius, crown, _): (TreeForm, bool, f32, f32, f32, bool)) -> TreeProfile {
    TreeProfile { conifer, cell_x: -7, cell_z: 11, anchor_x: 0, anchor_z: 0, ground_y: 0,
        trunk_h: cfg.blocks_for_span(trunk), canopy_radius: cfg.blocks_for_span(radius),
        canopy_height: cfg.blocks_for_span(crown), branch_count: 5,
        branch_len: cfg.blocks_for_span(radius * 0.76),
        branch_start_y: cfg.blocks_for_span(trunk * 0.3), lean_x: 0, lean_z: 0, form }
}

/// Wood and leaf voxels, each leaf checked against the tree's LOD bounds.
fn grow(cfg: &WorldSampleConfig, t: TreeProfile) -> (HashSet<IVec3>, HashSet<IVec3>) {
    let v = cfg.voxel_meters();
    let (radius, top) = (crown_reach(cfg, &t), crown_top(cfg, &t));
    let n = (radius / v).ceil() as i32 + 2;
    let (mut wood, mut leaves) = (HashSet::new(), HashSet::new());
    for y in 1..=(top / v).ceil() as i32 + 2 {
        for z in -n..=n { for x in -n..=n {
            let q = IVec3::new(x, y, z);
            match block(cfg, t, q) {
                BlockId::Log | BlockId::MushroomStem => { wood.insert(q); }
                BlockId::Air => {}
                _ => {
                    assert!((x as f32).hypot(z as f32) * v <= radius, "{:?} leaf outside reach at {v} m", t.form);
                    assert!(y as f32 * v <= top, "{:?} conifer={} leaf {} above top {top} at {v} m, v{}",
                        t.form, t.conifer, y as f32 * v, cfg.generator_version);
                    leaves.insert(q);
                }
            }
        }}
    }
    (wood, leaves)
}

/// Exposed leaf faces over leaf count^(2/3).
fn roughness((wood, leaves): &(HashSet<IVec3>, HashSet<IVec3>)) -> f32 {
    let open = leaves.iter()
        .flat_map(|p| SIDES.iter().map(move |d| *p + *d))
        .filter(|n| !leaves.contains(n) && !wood.contains(n))
        .count();
    open as f32 / (leaves.len() as f32).powf(2.0 / 3.0)
}

/// Leaf voxels relief added or removed, per surface voxel of the plain crown.
fn moved(plain: &(HashSet<IVec3>, HashSet<IVec3>), relief: &(HashSet<IVec3>, HashSet<IVec3>)) -> f32 {
    let surface = plain.1.iter()
        .filter(|p| SIDES.iter().any(|d| !plain.1.contains(&(**p + *d)) && !plain.0.contains(&(**p + *d))))
        .count();
    plain.1.symmetric_difference(&relief.1).count() as f32 / surface.max(1) as f32
}

/// Leaf voxels with no 6-connected path through leaves or wood to the ground row.
fn floaters((wood, leaves): &(HashSet<IVec3>, HashSet<IVec3>)) -> usize {
    let solid = |p: &IVec3| wood.contains(p) || leaves.contains(p);
    let mut reached: HashSet<IVec3> = wood.iter().chain(leaves).filter(|p| p.y == 1).copied().collect();
    let mut queue: std::collections::VecDeque<_> = reached.iter().copied().collect();
    while let Some(p) = queue.pop_front() {
        for d in SIDES {
            let n = p + d;
            if solid(&n) && reached.insert(n) { queue.push_back(n); }
        }
    }
    leaves.iter().filter(|p| !reached.contains(p)).count()
}

#[test]
fn every_crown_is_bounded_and_rougher_only_where_resolvable() {
    for species in SPECIES {
        let (mut gain, mut shift) = (Vec::new(), Vec::new());
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
            let [plain, relief] = [129, foliage::VERSION].map(|generator_version| {
                let cfg = WorldSampleConfig { seed: 2738958700, generator_version,
                    world_type: WorldType::Hills, voxel_scale: scale };
                grow(&cfg, tree(&cfg, species))
            });
            assert!(!relief.1.is_empty(), "{:?} crown missing at {scale:?}", species.0);
            assert_eq!(plain.0, relief.0, "{:?} wood changed at {scale:?}", species.0);
            // Thin fronds already float on coarse grids before relief, so compare shares.
            let before = floaters(&plain) as f32 / plain.1.len().max(1) as f32;
            let after = floaters(&relief) as f32 / relief.1.len().max(1) as f32;
            let one = 1.0 / relief.1.len().max(1) as f32;
            assert!(after <= before + 0.02 + one, "{:?} relief detached leaves: share {before} -> {after} at {scale:?}", species.0);
            if scale == VoxelScalePreset::OneMeter && species.5 {
                // No band resolves on a 1 m grid and these surfaces have no broad band.
                assert_eq!(plain.1, relief.1, "{:?} changed at 1 m", species.0);
            }
            let mass = relief.1.len() as f32 / plain.1.len() as f32;
            assert!((0.7..1.4).contains(&mass), "{:?} leaf mass x{mass} at {scale:?}", species.0);
            gain.push(roughness(&relief) / roughness(&plain));
            shift.push(moved(&plain, &relief));
        }
        println!("{:?} conifer={} roughness gain {gain:?} surface moved {shift:?}", species.0, species.1);
        // Faceted hash cubes already count as rough on fine grids, so the
        // face metric only has to rise; the moved surface shows the relief.
        assert!(gain[2] > 1.0 && gain[2] > gain[0], "{:?} quarter-metre crown not rougher: {gain:?}", species.0);
        assert!(shift[2] > 0.2 && shift[2] > shift[0] + 0.15, "{:?} relief not resolution-gated: {shift:?}", species.0);
    }
}

