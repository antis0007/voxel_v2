//! Generator 52 ruin and carving layers: they show at fine scales, never
//! close a door-to-door route, keep lights supported and sample the same
//! every time.
use super::*;

/// Rooms the ruin layer dresses most: damp or damaged, not puzzles or seals.
fn dressed(d: &Complex, r: &Room) -> bool {
    !r.sealed && !r.kind.is_puzzle() && (ruin::wet(d, r) > 0.0 || r.damage != Damage::Intact)
}

#[test]
fn ruin_dressing_keeps_every_door_reachable_at_each_scale() {
    let mut rooms = 0;
    for (seed, scale) in [(21u32, VoxelScalePreset::OneMeter), (22, VoxelScalePreset::HalfMeter), (23, VoxelScalePreset::QuarterMeter), (24, VoxelScalePreset::QuarterMeter)] {
        let c = cfg(seed.wrapping_mul(2654435761) ^ 52, scale);
        for (cell, d) in planned(&c, 3) {
            for r in d.rooms.iter().filter(|r| dressed(&d, r)) {
                let lo = Vec3::new(r.x0 - 3.0, r.floor - r.sub() - 2.0, r.z0 - 3.0);
                let hi = Vec3::new(r.x1 + 3.0, r.floor + r.top() + 1.0, r.z1 + 3.0);
                let grid = Grid::within(&c, &d, lo, hi);
                let doors = rooms::doors(&d, r, grid.v);
                if doors.len() < 2 {
                    continue;
                }
                let seen = grid.reach(doors[0]);
                for door in &doors[1..] {
                    let near = seen.iter().any(|p| (*p - *door).abs().max_element() <= (0.6 / grid.v) as i32);
                    assert!(near, "cell {cell:?} {:?} {:?} at {}: door {door} not reached from {}", r.kind, r.damage, grid.v, doors[0]);
                }
                rooms += 1;
            }
        }
    }
    assert!(rooms >= 12, "only {rooms} dressed rooms");
}

/// Counts of ruin blocks inside the rooms of a few planned complexes.
fn census(scale: VoxelScalePreset) -> [usize; 3] {
    let c = cfg(4242, scale);
    let height = c.blocks_for_height(1000.0);
    let v = c.voxel_meters();
    let mut n = [0usize; 3];
    for (_, d) in planned(&c, 3) {
        for r in d.rooms.iter().filter(|r| !r.sealed) {
            let mut y = r.floor - v;
            while y < r.floor + r.h {
                let mut x = r.x0 + v * 0.5;
                while x < r.x1 {
                    for z in [r.z0 + v * 0.5, r.z0 + 1.0 + v * 0.5, r.z1 - v * 0.5] {
                        match d.sample(&c, Vec3::new(x, y + v * 0.5, z), height) {
                            Some(B::GroveMoss) => n[0] += 1,
                            Some(B::Leaves | B::Bush) => n[1] += 1,
                            Some(B::Gravel | B::Cobblestone) => n[2] += 1,
                            _ => {}
                        }
                    }
                    x += v;
                }
                y += v;
            }
        }
    }
    n
}

#[test]
fn ruin_shows_at_fine_scales_and_coarse_worlds_keep_their_shapes() {
    let [_, coarse_vines, _] = census(VoxelScalePreset::OneMeter);
    let [moss, vines, rubble] = census(VoxelScalePreset::QuarterMeter);
    assert_eq!(coarse_vines, 0, "vines at 1 m");
    assert!(moss > 10 && vines > 50 && rubble > 50, "moss {moss}, vines {vines}, rubble {rubble} at 0.25 m");
}

#[test]
fn generator_51_rooms_have_no_ruin_layer() {
    let c = WorldSampleConfig { generator_version: 51, ..cfg(4242, VoxelScalePreset::QuarterMeter) };
    let height = c.blocks_for_height(1000.0);
    for (_, d) in planned(&c, 2) {
        for r in &d.rooms {
            let mut x = r.x0 + 0.125;
            while x < r.x1 {
                for fy in [-0.125f32, 0.125, 0.375, 2.6, r.h - 0.9] {
                    let b = d.sample(&c, Vec3::new(x, r.floor + fy, r.z0 + 0.125), height);
                    assert!(!matches!(b, Some(B::GroveMoss | B::Leaves | B::Bush)), "{:?} {b:?} at generator 51", r.kind);
                }
                x += 0.25;
            }
        }
    }
}

#[test]
fn quarter_metre_sampling_is_deterministic() {
    let c = cfg(91, VoxelScalePreset::QuarterMeter);
    let (_, d) = planned(&c, 1).remove(0);
    let (_, e) = planned(&c, 1).remove(0);
    let height = c.blocks_for_height(1000.0);
    for k in 0..6000 {
        let t = Vec3::new(crate::hash2(k, 1, 5), crate::hash2(k, 2, 5), crate::hash2(k, 3, 5));
        let m = d.min + (d.max - d.min) * t;
        assert_eq!(d.sample(&c, m, height), e.sample(&c, m, height));
    }
}
