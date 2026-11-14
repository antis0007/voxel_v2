//! Geometry checks: the roof never breaks the surface, passages keep their
//! air where they meet caverns, and the timing of a full sample.
use super::layout::surface;
use super::*;
use crate::WorldType;
use crate::undercrown::plan::Plan;
use engine_core::VoxelScalePreset;

pub(super) fn cfg_at(seed: u32, voxel_scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version: super::tests_grid::generator(), seed, world_type: WorldType::Hills, voxel_scale }
}

fn layout(seed: u32) -> (WorldSampleConfig, Layout) {
    let c = cfg_at(seed, VoxelScalePreset::OneMeter);
    let plan = Plan::for_config(&c);
    let l = *plan.v2.unwrap();
    (c, l)
}

/// Shafts are meant to open to the sky: the well and the observatory stair.
fn in_shaft(l: &Layout, x: f32, z: f32) -> bool {
    l.spaces.iter().any(|s| matches!(s.shape, Shape::Shaft { x: sx, z: sz, r, .. } if (x - sx).hypot(z - sz) < r + 2.0))
}

#[test]
fn the_roof_never_breaks_the_surface() {
    // Rock left between the highest open cell and the ground above it.
    const MARGIN: f32 = 3.0;
    let mut worst = (f32::MAX, 0, Vec3::ZERO);
    for seed in [424242u32, 7, 99, 1, 2, 3, 1234, 555, 11, 12, 13, 14, 2024, 31337, 8, 600] {
        let (c, l) = layout(seed);
        let mut x = l.min.x;
        while x < l.max.x {
            let mut z = l.min.z;
            while z < l.max.z {
                if !in_shaft(&l, x, z) {
                    let ground = surface(&c, x, z);
                    let mut y = l.max.y.min(ground + 2.0);
                    while y > l.min.y {
                        let m = Vec3::new(x, y, z);
                        if l.sample(&c, m, Mech::ALL).is_some_and(|b| !b.is_solid()) {
                            let cover = ground - y;
                            if std::env::var("UC_ROOF").is_ok() && cover < MARGIN { println!("seed {seed} rel {:?} cover {cover:.1} anchor {:?}", m - l.anchor, l.anchor); }
                            if cover < worst.0 {
                                worst = (cover, seed, m);
                            }
                            break;
                        }
                        y -= 1.0;
                    }
                }
                z += 1.5;
            }
            x += 1.5;
        }
    }
    if std::env::var("UC_ROOF").is_ok() { println!("{:?}", worst); }
    assert!(worst.0 >= MARGIN, "seed {}: open cell at {:?} has only {:.1} m of rock over it", worst.1, worst.2, worst.0);
}

#[test]
fn passages_keep_their_air_through_cavern_edges() {
    for seed in [424242u32, 7, 99] {
        let (c, l) = layout(seed);
        for (i, t) in l.tunnels.iter().enumerate() {
            let len = (t.b - t.a).length();
            let steps = (len / 0.5).ceil() as i32;
            for k in 0..=steps {
                let p = t.a + (t.b - t.a) * (k as f32 / steps as f32);
                for dy in [0.5, 1.5] {
                    let m = p + Vec3::Y * dy;
                    let b = l.sample(&c, m, Mech::ALL);
                    // A feature built across a passage (a ramp, a stair) is fine
                    // as long as it is walkable: something open sits on it.
                    let open = |b: Option<B>| b.is_some_and(|b| !b.is_solid());
                    if !open(b) {
                        let over = (1..=3).any(|j| open(l.sample(&c, m + Vec3::Y * j as f32, Mech::ALL)) && open(l.sample(&c, m + Vec3::Y * (j as f32 + 1.0), Mech::ALL)));
                        assert!(over, "seed {seed}: passage {i} is blocked by {b:?} at {m:?}");
                    }
                }
            }
        }
    }
}

/// Wall-clock cost of sampling the whole box at 1 m. Run with `--release
/// --ignored --nocapture`.
#[test]
#[ignore = "timing"]
fn time_full_sample() {
    let (c, l) = layout(424242);
    let (lo, hi) = (l.min.floor().as_ivec3(), l.max.ceil().as_ivec3());
    let cells = ((hi - lo).as_vec3()).element_product();
    let mut best = f64::MAX;
    let mut checksum = 0u64;
    for _ in 0..3 {
        let t = std::time::Instant::now();
        checksum = 0;
        for y in lo.y..hi.y {
            for z in lo.z..hi.z {
                for x in lo.x..hi.x {
                    let m = IVec3::new(x, y, z).as_vec3() + Vec3::splat(0.5);
                    if let Some(b) = l.sample(&c, m, Mech::default()) {
                        checksum = checksum.wrapping_mul(31).wrapping_add(b as u64 + 1);
                    }
                }
            }
        }
        best = best.min(t.elapsed().as_secs_f64());
    }
    println!("full sample: {cells} cells, best of 3 {:.1} ms, checksum {checksum:x}", best * 1000.0);
}

#[test]
fn the_index_matches_a_full_scan() {
    for (seed, scale) in [(424242u32, VoxelScalePreset::OneMeter), (7, VoxelScalePreset::HalfMeter)] {
        let c = cfg_at(seed, scale);
        let l = *Plan::for_config(&c).v2.unwrap();
        let mut full = l.clone();
        let whole = |n: usize| vec![(l.min, l.max); n];
        full.index = index::Index::build(l.min, l.max, &whole(l.features.len()), &whole(l.spaces.len()), &whole(l.tunnels.len()));
        let v = c.voxel_meters();
        let (lo, hi) = ((l.min / v).floor().as_ivec3(), (l.max / v).ceil().as_ivec3());
        let mut bad = Vec::new();
        for y in (lo.y..hi.y).step_by(if v < 1.0 { 3 } else { 1 }) {
            for z in (lo.z..hi.z).step_by(if v < 1.0 { 3 } else { 1 }) {
                for x in lo.x..hi.x {
                    let m = (IVec3::new(x, y, z).as_vec3() + Vec3::splat(0.5)) * v;
                    for mech in [Mech::default(), Mech::ALL] {
                        let (a, b) = (l.sample(&c, m, mech), full.sample(&c, m, mech));
                        if a != b && bad.len() < 12 {
                            bad.push((m - l.anchor, a, b));
                        }
                    }
                }
            }
        }
        assert!(bad.is_empty(), "seed {seed}: index differs from a full scan at {bad:?}");
    }
}

/// Wall torches inside a cavern space, and whether each has rock behind it.
fn cave_lamps(c: &WorldSampleConfig, l: &Layout, name_room: usize) -> (usize, usize) {
    let v = c.voxel_meters();
    let (mut lamps, mut hung) = (0, 0);
    for s in l.spaces.iter().filter(|s| s.room == name_room) {
        let Shape::Cave { c: cc, rad, .. } = s.shape else { continue };
        let (lo, hi) = (((cc - rad) / v).floor().as_ivec3(), ((cc + rad) / v).ceil().as_ivec3());
        for y in lo.y..hi.y {
            for z in lo.z..hi.z {
                for x in lo.x..hi.x {
                    let m = (IVec3::new(x, y, z).as_vec3() + Vec3::splat(0.5)) * v;
                    let Some(b) = l.sample(c, m, Mech::default()) else { continue };
                    let behind = match b {
                        B::TorchWallWest => Vec3::X,
                        B::TorchWallEast => Vec3::NEG_X,
                        B::TorchWallNorth => Vec3::Z,
                        B::TorchWallSouth => Vec3::NEG_Z,
                        _ => continue,
                    };
                    lamps += 1;
                    hung += usize::from(l.sample(c, m + behind * v, Mech::default()).is_some_and(|b| b.is_solid()));
                }
            }
        }
    }
    (lamps, hung)
}

#[test]
fn generator_36_lights_the_crown_chasm() {
    for seed in [424242u32, 7, 99] {
        let (c, l) = layout(seed);
        let plan = Plan::for_config(&c);
        let chasm = plan.rooms.iter().position(|r| r.name == "Crown Chasm").unwrap();
        let (lamps, hung) = cave_lamps(&c, &l, chasm);
        assert_eq!(lamps, hung, "seed {seed}: a lamp hangs on air");
        let old = WorldSampleConfig { generator_version: 35, ..c };
        let lo = *Plan::for_config(&old).v2.unwrap();
        let before = cave_lamps(&old, &lo, chasm).0;
        assert!(lamps >= before + 8, "seed {seed}: {before} lamps before 36, {lamps} after");
    }
}
