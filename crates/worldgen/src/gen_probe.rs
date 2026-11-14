//! Stage-isolated generation timing for the v15/v16 terrain cost regression.
//!
//! Each stage runs as its own whole pass over the same 80 chunks (a hillside
//! the `terrain` bench also samples), in the
//! real call order and with no per-voxel timers, so one stage cannot warm a
//! cache for another. Run:
//! `cargo test gen_probe -- --ignored --nocapture --test-threads=1`
use super::*;
use std::time::Instant;

fn chunks() -> impl Iterator<Item = IVec3> {
    (6..10).flat_map(|cx| (-2..2).flat_map(move |cz| (-2..3).map(move |cy| IVec3::new(cx, cy, cz))))
}

fn cfg(version: u32) -> WorldSampleConfig {
    WorldSampleConfig {
        planet: None,
        generator_version: version,
        seed: 424242,
        world_type: WorldType::Hills,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter,
    }
}

/// Warm once, then best of three.
fn best(mut pass: impl FnMut() -> usize) -> (f64, usize) {
    let sink = pass();
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        std::hint::black_box(pass());
        best = best.min(t.elapsed().as_secs_f64());
    }
    (best, sink)
}

fn each_voxel(c: &WorldSampleConfig, mut f: impl FnMut(IVec3, ColumnSample, &RegionProfiles) -> bool) -> usize {
    let size = engine_core::PADDED_CHUNK_SIZE;
    let mut n = 0;
    for coord in chunks() {
        let origin = coord * CHUNK_SIZE - IVec3::ONE;
        let profiles = RegionProfiles::new(c, origin.x, origin.z, size);
        for z in 0..size {
            for x in 0..size {
                let column = sample_column(c, origin.x + x, origin.z + z);
                for y in 0..size {
                    n += f(origin + IVec3::new(x, y, z), column, &profiles) as usize;
                }
            }
        }
    }
    n
}

#[test]
#[ignore = "profiling probe; run explicitly"]
fn gen_probe_stage_costs() {
    let never = std::sync::atomic::AtomicBool::new(false);
    let order: Vec<u32> = std::env::var("GEN_PROBE_ORDER").map(|v| v.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or(vec![15, 16]);
    for version in order {
        let c = cfg(version);
        let full = best(|| {
            chunks()
                .map(|coord| {
                    generate_padded(&c, coord, &never)
                        .unwrap()
                        .blocks
                        .iter()
                        .filter(|b| **b != BlockId::Air)
                        .count()
                })
                .sum()
        });
        let columns = best(|| each_voxel(&c, |_, _, _| false));
        let chain = best(|| {
            each_voxel(&c, |p, col, _| {
                undercrown::block(&c, p, col.height)
                    .or_else(|| sites::block(&c, p, col.height))
                    .or_else(|| sites::cavern(&c, p, col.height))
                    .is_some()
            })
        });
        let undercrown_only = best(|| each_voxel(&c, |p, col, _| undercrown::block(&c, p, col.height).is_some()));
        let sites_block = best(|| each_voxel(&c, |p, col, _| sites::block(&c, p, col.height).is_some()));
        let cavern = best(|| each_voxel(&c, |p, col, _| sites::cavern(&c, p, col.height).is_some()));
        let structure = best(|| {
            each_voxel(&c, |p, col, prof| p.y > col.height && structure_block(&c, p, col, prof) != BlockId::Air)
        });
        let caves = best(|| each_voxel(&c, |p, col, _| p.y <= col.height && cave_carves(&c, p, &col)));
        let per_voxel = best(|| {
            each_voxel(&c, |p, col, prof| sample_block_with(&c, p, col, prof) != BlockId::Air)
        });
        println!(
            "v{version}: generate_padded {:.3}s | columns-only {:.3}s | sample_block_with {:.3}s | \
             chain {:.3}s (undercrown {:.3}s, sites::block {:.3}s, cavern {:.3}s) | \
             structure_block {:.3}s | cave_carves {:.3}s | solid {}",
            full.0, columns.0, per_voxel.0, chain.0, undercrown_only.0, sites_block.0, cavern.0,
            structure.0, caves.0, full.1
        );
    }
}

/// How many voxels get past `cave_carves`' cheap exits into its 3D noise, and
/// the column statistics that decide it.
#[test]
#[ignore = "profiling probe; run explicitly"]
fn gen_probe_cave_noise_reach() {
    for version in [15u32, 16] {
        let c = cfg(version);
        let (mut below, mut noise, mut cols, mut haw_sum, mut depth_sum) = (0u64, 0u64, 0u64, 0f64, 0f64);
        let size = engine_core::PADDED_CHUNK_SIZE;
        for coord in chunks() {
            let origin = coord * CHUNK_SIZE - IVec3::ONE;
            for z in 0..size {
                for x in 0..size {
                    let col = sample_column(&c, origin.x + x, origin.z + z);
                    cols += 1;
                    haw_sum += col.cave.height_above_water as f64;
                    for y in 0..size {
                        let p = origin + IVec3::new(x, y, z);
                        if p.y > col.height || p.y <= c.min_world_y() + c.blocks_for_span(0.75) {
                            continue;
                        }
                        below += 1;
                        let depth = (col.height - p.y) as f32 * c.voxel_meters();
                        let haw = col.cave.height_above_water;
                        if (haw < 9.0 && depth < 5.5) || (haw < 18.0 && depth < 2.5) {
                            continue;
                        }
                        noise += 1;
                        depth_sum += depth as f64;
                    }
                }
            }
        }
        println!(
            "v{version}: below-surface voxels {below}, reach cave noise {noise} ({:.1}%), mean height_above_water {:.2} m, mean noise depth {:.1} m",
            100.0 * noise as f64 / below.max(1) as f64,
            haw_sum / cols as f64,
            depth_sum / noise.max(1) as f64
        );
    }
}

/// Time `generate_padded` for one version, best of three after a warm-up.
fn time_generation(version: u32) -> f64 {
    let never = std::sync::atomic::AtomicBool::new(false);
    let c = cfg(version);
    best(|| {
        chunks()
            .map(|coord| generate_padded(&c, coord, &never).unwrap().blocks.len())
            .sum()
    })
    .0
}

/// Separate thread-local cache state from global state: the same sequence of
/// versions run on one thread, then each version on its own fresh thread.
#[test]
#[ignore = "profiling probe; run explicitly"]
fn gen_probe_thread_state() {
    let same: Vec<f64> = [15u32, 16, 15, 16].iter().map(|&v| time_generation(v)).collect();
    println!("same thread   v15 {:.3} v16 {:.3} v15 {:.3} v16 {:.3}", same[0], same[1], same[2], same[3]);
    let fresh: Vec<f64> = [15u32, 16, 15, 16]
        .iter()
        .map(|&v| std::thread::spawn(move || time_generation(v)).join().unwrap())
        .collect();
    println!("fresh threads v15 {:.3} v16 {:.3} v15 {:.3} v16 {:.3}", fresh[0], fresh[1], fresh[2], fresh[3]);
}
    #[test]
    #[ignore = "historical HEAD timing method, full workload"]
    fn gen_probe_original_order() {
        use super::BlockId as B;
        use std::sync::atomic::AtomicBool;
        let never = AtomicBool::new(false);
        let time = |version: u32| {
            let c = WorldSampleConfig {
                planet: None,
                generator_version: version,
                seed: 424242,
                world_type: WorldType::Hills,
                voxel_scale: VoxelScalePreset::OneMeter,
            };
            let start = std::time::Instant::now();
            let mut solid = 0usize;
            for cx in 6..10 {
                for cz in -2..2 {
                    for cy in -2..3 {
                        let chunk =
                            crate::generate_padded(&c, glam::IVec3::new(cx, cy, cz), &never)
                                .unwrap();
                        solid += chunk.blocks.iter().filter(|b| **b != B::Air).count();
                    }
                }
            }
            (start.elapsed().as_secs_f64(), solid)
        };
        // Each version is timed in its own block. Alternating versions made them
        // evict each other from the shared landscape tile cache, which measured
        // cache thrash instead of generator cost. Best of three per version
        // keeps background load from favouring either side.
        let best = |version| {
            let _ = time(version);
            (0..3).map(|_| time(version).0).fold(f64::MAX, f64::min)
        };
        let old = best(15);
        let new = best(16);
        println!(
            "terrain generation: v15 {old:.3}s, v16 {new:.3}s ({:.2}x)",
            new / old
        );
        assert!(
            new < old * 1.35 + 0.05,
            "v16 generation is {:.2}x slower than v15",
            new / old
        );
    }

/// Per-stage cost of the hills chain on one all-air chunk above the terrain.
#[test]
#[ignore = "profiling probe; run explicitly"]
fn gen_probe_air_chunk_stages() {
    let c = cfg(CURRENT_GENERATOR_VERSION);
    let size = engine_core::PADDED_CHUNK_SIZE;
    let coord = std::env::var("GEN_PROBE_CHUNK").ok().and_then(|v| {
        let n: Vec<i32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
        (n.len() == 3).then(|| IVec3::new(n[0], n[1], n[2]))
    }).unwrap_or(IVec3::new(8, 10, 0));
    println!("chunk {coord}");
    let origin = coord * CHUNK_SIZE - IVec3::ONE;
    let profiles = RegionProfiles::new(&c, origin.x, origin.z, size);
    let cols: Vec<ColumnSample> = (0..size * size).map(|i| sample_column(&c, origin.x + i % size, origin.z + i / size)).collect();
    let each = |f: &dyn Fn(IVec3, ColumnSample) -> bool| {
        best(|| {
            let mut n = 0;
            for z in 0..size { for x in 0..size {
                let col = cols[(z * size + x) as usize];
                for y in 0..size { n += f(origin + IVec3::new(x, y, z), col) as usize; }
            }}
            n
        })
    };
    let m = |p: IVec3| (p.as_vec3() + Vec3::splat(0.5)) * c.voxel_meters();
    let rows: [(&str, &dyn Fn(IVec3, ColumnSample) -> bool); 21] = [
        ("whole sample_block_with", &|p, col| sample_block_with(&c, p, col, &profiles) != BlockId::Air),
        ("basin::covers", &|p, _| basin::covers(&c, p.x, p.z)),
        ("vistas::priority_block", &|p, col| vistas::priority_block(&c, p, col).is_some()),
        ("fernwake::sample", &|p, _| fernwake::sample(&c, m(p)).is_some()),
        ("households::sample", &|p, _| households::sample(&c, m(p)).is_some()),
        ("meridian_terminal::block", &|p, col| meridian_terminal::block(&c, p, col.height).is_some()),
        ("undercrown::block", &|p, col| undercrown::block(&c, p, col.height).is_some()),
        ("amberroot::block", &|p, col| amberroot::block(&c, p, col.height).is_some()),
        ("sites::block", &|p, col| sites::block(&c, p, col.height).is_some()),
        ("sites::cavern", &|p, col| sites::cavern(&c, p, col.height).is_some()),
        ("vistas::block", &|p, col| vistas::block(&c, p, col).is_some()),
        ("structure_block", &|p, col| structure_block(&c, p, col, &profiles) != BlockId::Air),
        ("river_top", &|p, _| profiles.river_top(&c, p.x, p.z).is_some()),
        ("cave_carves (below top)", &|p, col| p.y <= col.height && cave_carves(&c, p, &col)),
        ("geo::subsurface (below)", &|p, col| p.y <= col.height && geo::active(&c)
            && geo::subsurface(&c, c.world_x(p.x), c.world_z(p.z), (col.height - p.y) as f32 * c.voxel_meters()).is_some()),
        ("placer::vein (below)", &|p, col| p.y <= col.height && placer::vein(&c, (p.as_vec3() + Vec3::splat(0.5)) * c.voxel_meters())),
        ("aquifer_pocket (carved)", &|p, col| p.y <= col.height && cave_carves(&c, p, &col) && aquifer_pocket(&c, p, &col) != BlockId::Air),
        ("bedrock top", &|p, _| p.y <= bedrock::ceiling(&c) && p.y <= bedrock::top(&c, p.x, p.z)),
        ("water_table", &|p, _| p.y <= water_table(&c, p.x, p.z)),
        ("authored_block total", &|p, col| authored_block(&c, p, col, true).is_some()),
        ("generate_padded / 34^3", &|p, _| p == origin && generate_padded(&c, coord, &std::sync::atomic::AtomicBool::new(false)).is_some()),
    ];
    for (name, f) in rows {
        let (secs, hits) = each(f);
        println!("{name:28} {:8.2} ms  hits {hits}", secs * 1e3);
    }
}

/// Tree lookups vs tree shape tests inside `structure_block`, one air chunk.
#[test]
#[ignore = "profiling probe; run explicitly"]
fn gen_probe_tree_split() {
    let c = cfg(CURRENT_GENERATOR_VERSION);
    let size = engine_core::PADDED_CHUNK_SIZE;
    for coord in [IVec3::new(8, 10, 0), IVec3::new(8, 2, 0), IVec3::new(-12, 1, 16)] {
        let origin = coord * CHUNK_SIZE - IVec3::ONE;
        let profiles = RegionProfiles::new(&c, origin.x, origin.z, size);
        let cell = c.blocks_for_span(12.0);
        let voxels = || (0..size).flat_map(move |z| (0..size).flat_map(move |x| (0..size).map(move |y| origin + IVec3::new(x, y, z))));
        let lookups = best(|| voxels().map(|p| {
            let (bx, bz) = (floor_div(p.x, cell), floor_div(p.z, cell));
            (bz - 2..=bz + 2).flat_map(|cz| (bx - 2..=bx + 2).map(move |cx| (cx, cz)))
                .filter(|&(cx, cz)| profiles.tree(&c, cx, cz).is_some()).count()
        }).sum());
        let shapes = best(|| voxels().map(|p| {
            let (bx, bz) = (floor_div(p.x, cell), floor_div(p.z, cell));
            (bz - 2..=bz + 2).flat_map(|cz| (bx - 2..=bx + 2).map(move |cx| (cx, cz)))
                .filter_map(|(cx, cz)| profiles.tree(&c, cx, cz))
                .filter(|&t| tree_block(&c, t, p) != BlockId::Air).count()
        }).sum());
        let whole = best(|| voxels().filter(|&p| structure_block(&c, p, sample_column(&c, p.x, p.z), &profiles) != BlockId::Air).count());
        println!("{coord}: lookups {:.2} ms ({} tree tests) | lookups+tree_block {:.2} ms ({} hits) | structure_block {:.2} ms",
            lookups.0 * 1e3, lookups.1, shapes.0 * 1e3, shapes.1, whole.0 * 1e3);
    }
}

/// Tail of `structure_block` after the tree loop, one air chunk.
#[test]
#[ignore = "profiling probe; run explicitly"]
fn gen_probe_structure_tail() {
    let c = cfg(CURRENT_GENERATOR_VERSION);
    let size = engine_core::PADDED_CHUNK_SIZE;
    let origin = IVec3::new(8, 10, 0) * CHUNK_SIZE - IVec3::ONE;
    let profiles = RegionProfiles::new(&c, origin.x, origin.z, size);
    let cols: Vec<ColumnSample> = (0..size * size).map(|i| sample_column(&c, origin.x + i % size, origin.z + i / size)).collect();
    let each = |f: &dyn Fn(IVec3, ColumnSample) -> bool| best(|| {
        let mut n = 0;
        for z in 0..size { for x in 0..size { let col = cols[(z * size + x) as usize];
            for y in 0..size { n += f(origin + IVec3::new(x, y, z), col) as usize; } } }
        n
    });
    let bush = c.blocks_for_span(8.0);
    let rows: [(&str, &dyn Fn(IVec3, ColumnSample) -> bool); 6] = [
        ("trail_reserves", &|p, col| col.surface == BlockId::Gravel && p.y - col.height <= c.blocks_for_span(2.2)
            && planetary::trail_reserves(&c, c.world_x(p.x), c.world_z(p.z), 1.8)),
        ("deadwood::block", &|p, col| deadwood::block(&c, p, col) != BlockId::Air),
        ("bush cells", &|p, _| { let (bx, bz) = (floor_div(p.x, bush), floor_div(p.z, bush));
            (bz - 1..=bz + 1).any(|cz| (bx - 1..=bx + 1).any(|cx| profiles.bush(&c, cx, cz).is_some_and(|b| bush_contains(&c, b, p)))) }),
        ("plants::block_in_column", &|p, col| plants::active(&c) && plants::block_in_column(&c, p, col) != BlockId::Air),
        ("flora::block", &|p, col| flora::active(&c) && flora::block(&c, p, col) != BlockId::Air),
        ("trail_clears", &|p, _| surface::trail_clears(&c, p.as_vec3() * c.voxel_meters())),
    ];
    for (name, f) in rows { let (s, n) = each(f); println!("{name:26} {:6.2} ms hits {n}", s * 1e3); }
}

/// Actual reach of every tree's voxels against its profile fields, by form.
#[test]
#[ignore = "profiling probe; run explicitly"]
fn gen_probe_tree_extents() {
    use std::collections::BTreeMap;
    let radius: i32 = std::env::var("GEN_PROBE_CELLS").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
    let configs = [(6u32, VoxelScalePreset::OneMeter), (33, VoxelScalePreset::OneMeter), (53, VoxelScalePreset::OneMeter),
        (130, VoxelScalePreset::OneMeter), (CURRENT_GENERATOR_VERSION, VoxelScalePreset::OneMeter),
        (CURRENT_GENERATOR_VERSION, VoxelScalePreset::HalfMeter)];
    for (version, scale) in configs {
        for world_type in [WorldType::Hills, WorldType::Ridged] {
            let c = WorldSampleConfig { planet: None, generator_version: version, seed: 424242, world_type, voxel_scale: scale };
            let cell = c.blocks_for_span(12.0);
            let trees: Vec<TreeProfile> = (-radius..=radius).flat_map(|z| (-radius..=radius).map(move |x| (x, z)))
                .filter_map(|(x, z)| tree_profile_for_cell(&c, x, z)).collect();
            let next = std::sync::atomic::AtomicUsize::new(0);
            // form -> (trees, worst dx - H, worst top - V, lowest dy, worst dx, worst top)
            let worst = std::sync::Mutex::new(BTreeMap::<String, (usize, i32, i32, i32, i32, i32)>::new());
            std::thread::scope(|s| { for _ in 0..8 { s.spawn(|| loop {
                let Some(&t) = trees.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) else { break };
                let v = t.trunk_h + t.canopy_height + t.branch_len;
                let (mut dx, mut top, mut low) = (0i32, i32::MIN, i32::MAX);
                let (x0, x1) = ((t.cell_x - 2) * cell, (t.cell_x + 3) * cell);
                let (z0, z1) = ((t.cell_z - 2) * cell, (t.cell_z + 3) * cell);
                for x in x0..x1 { for z in z0..z1 {
                    for y in t.ground_y - 4 * cell..t.ground_y + 3 * v + 4 * cell {
                        if tree_block(&c, t, IVec3::new(x, y, z)) != BlockId::Air {
                            dx = dx.max((x - t.anchor_x).abs().max((z - t.anchor_z).abs()));
                            top = top.max(y - t.ground_y);
                            low = low.min(y - t.ground_y);
                        }
                    }
                }}
                if top == i32::MIN { continue; }
                let h = t.canopy_radius + t.branch_len + t.lean_x.abs() + t.lean_z.abs();
                let mut w = worst.lock().expect("probe lock");
                let e = w.entry(format!("{:?}", t.form)).or_insert((0, i32::MIN, i32::MIN, i32::MAX, 0, 0));
                e.0 += 1; e.1 = e.1.max(dx - h); e.2 = e.2.max(top - v); e.3 = e.3.min(low); e.4 = e.4.max(dx); e.5 = e.5.max(top);
            }); } });
            for (form, (n, ex, ey, low, dx, top)) in worst.into_inner().expect("probe lock") {
                println!("v{version} {scale:?} {world_type:?} {form:14} trees {n:4} dx-H {ex:4} top-V {ey:4} low {low:3} | max dx {dx:3} top {top:3}");
            }
        }
    }
}

/// Every non-Air voxel of every tree lies inside `TreeProfile::reach`.
#[test]
#[ignore = "wide validation sweep; run explicitly"]
fn gen_probe_tree_reach_holds() {
    let radius: i32 = std::env::var("GEN_PROBE_CELLS").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
    let versions: Vec<u32> = std::env::var("GEN_PROBE_VERSIONS").ok()
        .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or(vec![6, 11, 33, 53, 100, 130, 160, 200, CURRENT_GENERATOR_VERSION]);
    let mut violations = 0usize;
    let mut checked = 0usize;
    for &version in &versions {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
            for (world_type, seed) in [(WorldType::Hills, 424242u32), (WorldType::Hills, 7), (WorldType::Ridged, 2738958700), (WorldType::Hills, 91)] {
                let c = WorldSampleConfig { planet: None, generator_version: version, seed, world_type, voxel_scale: scale };
                let cell = c.blocks_for_span(12.0);
                let span = if scale == VoxelScalePreset::OneMeter { radius } else { radius / 2 };
                let trees: Vec<TreeProfile> = (-span..=span).flat_map(|z| (-span..=span).map(move |x| (x, z)))
                    .filter_map(|(x, z)| tree_profile_for_cell(&c, x, z)).collect();
                let next = std::sync::atomic::AtomicUsize::new(0);
                let bad = std::sync::atomic::AtomicUsize::new(0);
                std::thread::scope(|s| { for _ in 0..8 { s.spawn(|| loop {
                    let Some(&t) = trees.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) else { break };
                    let v = t.trunk_h + t.canopy_height + t.branch_len;
                    let (x0, x1) = ((t.cell_x - 2) * cell, (t.cell_x + 3) * cell);
                    let (z0, z1) = ((t.cell_z - 2) * cell, (t.cell_z + 3) * cell);
                    for x in x0..x1 { for z in z0..z1 {
                        for y in t.ground_y - 4 * cell..t.ground_y + 3 * v + 4 * cell {
                            let p = IVec3::new(x, y, z);
                            if !t.may_fill(&c, p) && tree_block(&c, t, p) != BlockId::Air {
                                if bad.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 3 {
                                    println!("VIOLATION v{version} {scale:?} {world_type:?} seed {seed} {:?} at {p} anchor ({},{}) ground {} reach {:?}",
                                        t.form, t.anchor_x, t.anchor_z, t.ground_y, t.reach(&c));
                                }
                            }
                        }
                    }}
                }); } });
                checked += trees.len();
                violations += bad.into_inner();
            }
        }
        println!("v{version}: {checked} trees checked so far, {violations} violating voxels");
    }
    assert_eq!(violations, 0);
}

/// Tree forms present per world type and version over a wide region.
#[test]
#[ignore = "profiling probe; run explicitly"]
fn gen_probe_tree_form_census() {
    for version in [53u32, 130, 160, 200, CURRENT_GENERATOR_VERSION] {
        for world_type in [WorldType::Hills, WorldType::Ridged, WorldType::Planet] {
            let mut forms = std::collections::BTreeMap::<String, usize>::new();
            for seed in [424242u32, 7, 91, 2738958700, 123456] {
                let c = WorldSampleConfig { planet: None, generator_version: version, seed, world_type, voxel_scale: VoxelScalePreset::OneMeter };
                for z in (-400..400).step_by(7) { for x in (-400..400).step_by(7) {
                    if let Some(t) = tree_profile_for_cell(&c, x, z) { *forms.entry(format!("{:?}", t.form)).or_default() += 1; }
                }}
            }
            println!("v{version} {world_type:?}: {forms:?}");
        }
    }
}

/// `TreeProfile::reach` holds for up to 40 trees of every form found in a
/// wide region, so rare forms are covered too.
#[test]
#[ignore = "wide validation sweep; run explicitly"]
fn gen_probe_tree_reach_every_form() {
    let per_form = 40usize;
    let mut violations = 0usize;
    let versions: Vec<u32> = std::env::var("GEN_PROBE_VERSIONS").ok()
        .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or(vec![53, 130, 160, 200, CURRENT_GENERATOR_VERSION]);
    for version in versions {
        for world_type in [WorldType::Hills, WorldType::Planet] {
            for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
                let mut picked = std::collections::BTreeMap::<String, Vec<(WorldSampleConfig, TreeProfile)>>::new();
                for seed in [424242u32, 7, 91, 2738958700, 123456] {
                    let c = WorldSampleConfig { planet: None, generator_version: version, seed, world_type, voxel_scale: scale };
                    for z in (-400..400).step_by(3) { for x in (-400..400).step_by(3) {
                        if let Some(t) = tree_profile_for_cell(&c, x, z) {
                            let list = picked.entry(format!("{:?}", t.form)).or_default();
                            if list.len() < per_form { list.push((c, t)); }
                        }
                    }}
                }
                let all: Vec<_> = picked.values().flatten().copied().collect();
                let next = std::sync::atomic::AtomicUsize::new(0);
                let bad = std::sync::atomic::AtomicUsize::new(0);
                std::thread::scope(|s| { for _ in 0..8 { s.spawn(|| loop {
                    let Some(&(c, t)) = all.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) else { break };
                    let cell = c.blocks_for_span(12.0);
                    let v = t.trunk_h + t.canopy_height + t.branch_len;
                    for x in (t.cell_x - 2) * cell..(t.cell_x + 3) * cell { for z in (t.cell_z - 2) * cell..(t.cell_z + 3) * cell {
                        for y in t.ground_y - 4 * cell..t.ground_y + 3 * v + 6 * cell {
                            let p = IVec3::new(x, y, z);
                            if !t.may_fill(&c, p) && tree_block(&c, t, p) != BlockId::Air
                                && bad.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 4 {
                                println!("VIOLATION v{version} {scale:?} {world_type:?} seed {} {:?} at {p} anchor ({},{}) ground {} dy {} reach {:?}",
                                    c.seed, t.form, t.anchor_x, t.anchor_z, t.ground_y, p.y - t.ground_y, t.reach(&c));
                            }
                        }
                    }}
                }); } });
                let n = bad.into_inner();
                violations += n;
                let forms: Vec<String> = picked.iter().map(|(k, v)| format!("{k}:{}", v.len())).collect();
                println!("v{version} {world_type:?} {scale:?}: {} trees, {n} bad | {}", all.len(), forms.join(" "));
            }
        }
    }
    assert_eq!(violations, 0);
}
