use super::*;
use crate::test_world::World;
use engine_core::VoxelScalePreset as S;
use glam::IVec3;

const SEEDS: [u32; 3] = [7, 424_242, 2_738_958_700];

fn cfg(seed: u32, scale: S) -> WorldSampleConfig {
    World::with_seed(WorldType::Hills, scale, seed).sample_config()
}

fn solid(b: B) -> bool {
    b.is_solid() && b != B::Water
}

/// Voxel at metre position `w` (x, z) and metre height `y`.
fn cell(cfg: &WorldSampleConfig, x: f32, y: f32, z: f32) -> IVec3 {
    (Vec3::new(x, y, z) * cfg.inv_voxel_meters()).floor().as_ivec3()
}

/// Ground height (top surface, metres) of the column under (x, z).
fn ground(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    let c = cell(cfg, x, 0.0, z);
    let column = crate::sample_column(cfg, c.x, c.z);
    cfg.world_y(column.height) + cfg.voxel_meters()
}

fn grove(cfg: &WorldSampleConfig) -> Arc<Plan> {
    plan(cfg).unwrap_or_else(|| panic!("seed {} has no grove", cfg.seed))
}

#[test]
fn every_test_seed_places_one_stable_grove() {
    for seed in SEEDS {
        let c = cfg(seed, S::OneMeter);
        let (a, f) = anchor(&c).unwrap_or_else(|| panic!("seed {seed}: no grove"));
        let r = a.x.hypot(a.z);
        assert!((300.0..900.0).contains(&r), "seed {seed}: grove {r} m from spawn");
        assert!(a.y > crate::WATER_LEVEL_METERS + 3.0, "seed {seed}: floor {}", a.y);
        assert!((f.length() - 1.0).abs() < 1e-4);
        // Another scale shares the landscape and so the same site.
        let (b, _) = anchor(&cfg(seed, S::HalfMeter)).expect("half-metre grove");
        assert!((a - b).length() < 1.0, "seed {seed}: {a} vs {b}");
    }
    let mut old = cfg(7, S::OneMeter);
    old.generator_version = VERSION - 1;
    assert!(anchor(&old).is_none(), "older generators must not grow a grove");
}

#[test]
fn approach_spur_and_clearing_are_walkable() {
    for seed in SEEDS {
        let c = cfg(seed, S::OneMeter);
        let p = grove(&c);
        let mut route: Vec<Vec2> = (0..46).map(|i| -98.0 + i as f32 * 2.0).map(|u| Vec2::new(u, p.path_v(u))).collect();
        route.extend((0..=20).map(|i| p.door * (i as f32 / 20.0)));
        for q in route {
            let w = p.world(q.x, q.y);
            let g = ground(&c, w.x, w.y);
            for k in 0..3 {
                let at = cell(&c, w.x, g + 0.5 + k as f32, w.y);
                let b = crate::sample_block(&c, at);
                assert!(!solid(b), "seed {seed}: {b:?} blocks the route at local {q} (+{k} m)");
            }
        }
    }
}

#[test]
fn hollow_stair_reaches_a_lit_undercroft() {
    for seed in SEEDS {
        let c = cfg(seed, S::OneMeter);
        let p = grove(&c);
        let (centre, g0, dir) = dress::stair(&p).expect("hollow tree");
        for i in 0..=24 {
            let along = 1.0 + i as f32 * 0.5;
            let floor = g0 - along.min(12.0) * 0.72;
            let w = centre + dir * along;
            for k in 0..2 {
                // Voxels are carved where their centre clears the sloped floor.
                let at = cell(&c, w.x, floor + 1.0 + k as f32, w.y);
                let b = crate::sample_block(&c, at);
                assert!(!solid(b), "seed {seed}: {b:?} in the stair at {along} m (+{k})");
            }
        }
        let (cc, floor) = dress::chamber(&p).expect("chamber");
        assert_eq!(crate::sample_block(&c, cell(&c, cc.x, floor + 1.5, cc.y)), B::Air);
        assert_eq!(location(&c, Vec3::new(cc.x, floor + 1.5, cc.y)), Some("Amberroot Undercroft"));
        // Lumen fibre to harvest: glowcaps on the undercroft walls.
        let mut glow = 0;
        for dz in -8..=8 {
            for dx in -8..=8 {
                for dy in 0..=5 {
                    let at = cell(&c, cc.x + dx as f32, floor + dy as f32, cc.y + dz as f32);
                    glow += (crate::sample_block(&c, at) == B::GlowCap) as u32;
                }
            }
        }
        assert!(glow >= 4, "seed {seed}: undercroft holds {glow} glowcaps");
        // The chamber roof stays under the hollow floor.
        assert!(ground(&c, cc.x, cc.y) > floor + 4.4 + 1.0, "seed {seed}: thin undercroft roof");
    }
}

#[test]
fn pools_are_contained() {
    for seed in SEEDS {
        let c = cfg(seed, S::OneMeter);
        let p = grove(&c);
        for pool in &p.pools {
            let mut water = 0;
            let w = p.world(pool.c.x, pool.c.y);
            let base = cell(&c, w.x, pool.level, w.y);
            let r = pool.r as i32 + 2;
            for dz in -r..=r {
                for dx in -r..=r {
                    for dy in -3..=1 {
                        let at = base + IVec3::new(dx, dy, dz);
                        if crate::sample_block(&c, at) != B::Water {
                            continue;
                        }
                        water += 1;
                        for n in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::NEG_Y] {
                            let b = crate::sample_block(&c, at + n);
                            assert!(b == B::Water || solid(b), "seed {seed}: pool leaks into {b:?} at {:?}", at + n);
                        }
                    }
                }
            }
            assert!(water > 4, "seed {seed}: pool at {} holds {water} voxels", pool.c);
        }
    }
}

#[test]
fn amber_gills_reach_the_light_index() {
    let c = cfg(7, S::OneMeter);
    let (a, _) = anchor(&c).expect("grove");
    let centre = cell(&c, a.x, a.y, a.z).div_euclid(IVec3::splat(crate::CHUNK_SIZE));
    let mut lights = 0;
    for dz in -2..=2 {
        for dx in -2..=2 {
            for dy in -1..=1 {
                let data = crate::generate(&c, centre + IVec3::new(dx, dy, dz));
                lights += data.torches.len();
            }
        }
    }
    assert!(lights > 8, "only {lights} amber lights indexed around the grove");
}



#[test]
#[ignore]
fn amberroot_cost() {
    let never = std::sync::atomic::AtomicBool::new(false);
    let time = |version: u32, coords: &[IVec3]| {
        let mut c = cfg(424_242, S::OneMeter);
        c.generator_version = version;
        let t = std::time::Instant::now();
        for &k in coords {
            std::hint::black_box(crate::generate_padded(&c, k, &never));
        }
        t.elapsed().as_secs_f64() * 1e3
    };
    let (a, _) = anchor(&cfg(424_242, S::OneMeter)).unwrap();
    let centre = cell(&cfg(424_242, S::OneMeter), a.x, a.y, a.z).div_euclid(IVec3::splat(32));
    for (name, base) in [("away", IVec3::new(6, -1, -1)), ("grove", centre - IVec3::new(1, 1, 1))] {
        let coords: Vec<IVec3> = (0..27).map(|i| base + IVec3::new(i % 3, (i / 3) % 3, i / 9)).collect();
        let (mut best34, mut best35) = (f64::MAX, f64::MAX);
        for _ in 0..5 {
            best34 = best34.min(time(34, &coords));
            best35 = best35.min(time(35, &coords));
        }
        println!("[cost] {name}: v34 {best34:.1} ms  v35 {best35:.1} ms  ({:+.1}%)", (best35 / best34 - 1.0) * 100.0);
    }
}

fn digest(c: &WorldSampleConfig, coords: impl Iterator<Item = IVec3>) -> Vec<(IVec3, u64)> {
    let never = std::sync::atomic::AtomicBool::new(false);
    let mut out: Vec<(IVec3, u64)> = coords
        .map(|k| {
            let chunk = crate::generate_padded(c, k, &never).expect("never cancelled");
            let h = chunk.blocks.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100_0000_01b3));
            (k, h)
        })
        .collect();
    out.sort_by_key(|(k, _)| (k.x, k.y, k.z));
    out
}

#[test]
fn generator_34_is_untouched_away_from_the_grove() {
    for seed in SEEDS {
        let new = cfg(seed, S::OneMeter);
        let mut old = new;
        old.generator_version = VERSION - 1;
        // Spawn-side chunks, far outside the grove's reach.
        let coords = || (0..8).map(|i| IVec3::new(i % 4 - 2, i / 4 - 1, (i % 3) - 1));
        assert_eq!(digest(&old, coords()), digest(&new, coords()), "seed {seed}");
    }
}

#[test]
fn every_scale_valley_stays_out_of_the_grove() {
    for seed in SEEDS {
        let p = grove(&cfg(seed, S::OneMeter));
        for scale in [S::OneMeter, S::HalfMeter, S::QuarterMeter] {
            let c = cfg(seed, scale);
            for iz in -6..=6 {
                for ix in -6..=6 {
                    let w = p.o + Vec2::new(ix as f32, iz as f32) * 16.0;
                    if (w - p.o).length() <= OUTER_R {
                        assert!(!crate::valley::reserves(&c, w.x, w.y), "seed {seed} {scale:?}: valley at {w}");
                    }
                }
            }
        }
    }
}

#[test]
fn chunk_order_and_entry_point_do_not_change_the_grove() {
    let c = cfg(7, S::OneMeter);
    let (a, _) = anchor(&c).expect("grove");
    let centre = cell(&c, a.x, a.y, a.z).div_euclid(IVec3::splat(crate::CHUNK_SIZE));
    let coords: Vec<IVec3> = (0..8).map(|i| centre + IVec3::new(i % 2, i / 4, (i / 2) % 2)).collect();
    let reset = || PLANS.get().into_iter().for_each(|m| m.lock().unwrap_or_else(|e| e.into_inner()).clear());
    reset();
    let forward = {
        let coords = coords.clone();
        std::thread::spawn(move || digest(&c, coords.into_iter())).join().expect("thread")
    };
    reset();
    // A cold thread whose first grove query comes from a HUD place-name lookup.
    let backward = std::thread::spawn(move || {
        let _ = crate::sites::location(&c, (a / c.voxel_meters()).floor());
        digest(&c, coords.into_iter().rev())
    })
    .join()
    .expect("thread");
    assert_eq!(forward, backward);
}
