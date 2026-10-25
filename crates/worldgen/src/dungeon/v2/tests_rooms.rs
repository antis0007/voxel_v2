//! Containment, seals, starry halls, clefts, caches.
use super::*;

#[test]
fn every_complex_stays_inside_the_cell_that_samples_it() {
    let (mut seen, mut rooms) = (0, 0);
    for seed in 0u32..20 {
        let c = cfg(seed.wrapping_mul(2246822519) ^ 99, VoxelScalePreset::OneMeter);
        for ((ix, iz), d) in complexes(&c, 6) {
            let half = crate::dungeon::CELL * 0.5;
            let (ox, oz) = (ix as f32 * crate::dungeon::CELL, iz as f32 * crate::dungeon::CELL);
            assert!(
                d.min.x >= ox - half && d.max.x <= ox + half && d.min.z >= oz - half && d.max.z <= oz + half,
                "seed {seed} cell ({ix},{iz}): x {}..{} z {}..{} outside {}..{} / {}..{}",
                d.min.x, d.max.x, d.min.z, d.max.z, ox - half, ox + half, oz - half, oz + half
            );
            seen += 1;
            rooms += d.rooms.len();
        }
    }
    // Generator 39 siting rejects most hill cells; the bound is on what is found.
    assert!(seen >= 60, "{seen}");
    // Fitting must not shrink complexes much: footprints hold six to twelve
    // rooms, and siting (39+) turns away more of the large ones, which span
    // more uneven ground.
    assert!(rooms as f32 / seen as f32 >= 6.0, "mean rooms {}", rooms as f32 / seen as f32);
}

#[test]
fn the_core_shell_is_tombstone_all_round() {
    let mut cores = 0;
    for (c, cell, d) in many(VoxelScalePreset::OneMeter, 6, 24) {
        let height = c.blocks_for_height(1000.0);
        let Some(o) = d.objective else { continue };
        cores += 1;
        let r = &d.rooms[o];
        let (mut y, top) = (r.floor - r.sub() - 1.0 - WALL + 0.5, r.floor + r.top() + WALL);
        while y < top {
            let mut z = r.z0 - WALL + 0.5;
            while z < r.z1 + WALL {
                let mut x = r.x0 - WALL + 0.5;
                while x < r.x1 + WALL {
                    let m = Vec3::new(x, y, z);
                    if !r.inside(m) {
                        let b = d.sample(&c, m, height);
                        assert!(matches!(b, Some(B::TombStone) | Some(B::Air) | Some(B::Torch)), "cell {cell:?} core shell at {m} is {b:?}");
                    }
                    x += 1.0;
                }
                z += 1.0;
            }
            y += 1.0;
        }
    }
    assert!(cores >= 12, "{cores}");
}

/// Wall-clock for rasterising whole complexes at 1 m; best of three passes.
#[test]
#[ignore = "timing"]
fn time_sampling() {
    let c = cfg(424242, VoxelScalePreset::OneMeter);
    let found = complexes(&c, 12);
    let mut best = f64::MAX;
    let mut cells = 0usize;
    for _ in 0..3 {
        let t = std::time::Instant::now();
        cells = found.iter().map(|(_, d)| Grid::new(&c, d).blocks.len()).sum();
        best = best.min(t.elapsed().as_secs_f64());
    }
    println!("{} complexes, {cells} cells, {:.1} ms, {:.1} ns/cell", found.len(), best * 1e3, best * 1e9 / cells as f64);
}

/// Prints every room's kind and outline for a seed.
#[test]
#[ignore = "debug dump"]
fn dump_survey() {
    let seed = std::env::var("V2_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let c = cfg(seed, VoxelScalePreset::OneMeter);
    for (cell, d) in complexes(&c, 8) {
        for (i, r) in d.rooms.iter().enumerate() {
            let plan = (r.kind == Kind::Orrery).then(|| starry::plan(r));
            println!("{cell:?} room {i} {:?} {:?} {:?} {}x{} {plan:?}", r.kind, r.outline, r.damage, r.w(), r.d());
        }
    }
}

/// Orrery complexes, with their orrery rooms, over many seeds.
fn orreries(scale: VoxelScalePreset, want: usize) -> Vec<(WorldSampleConfig, std::rc::Rc<Complex>)> {
    let mut out = Vec::new();
    for seed in 0u32..400 {
        let c = cfg(seed.wrapping_mul(747796405) ^ 31, scale);
        for (_, d) in planned(&c, 3) {
            if d.rooms.iter().any(|r| r.kind == Kind::Orrery) {
                out.push((c, d));
                if out.len() >= want {
                    return out;
                }
            }
        }
    }
    out
}

#[test]
fn orrery_rooms_come_in_three_plans_and_are_walkable() {
    let mut plans = HashSet::new();
    let mut islands = (0, 0);
    let mut stairs = 0;
    for (scale, want) in [(VoxelScalePreset::OneMeter, 14), (VoxelScalePreset::HalfMeter, 3)] {
        for (c, d) in orreries(scale, want) {
            let grid = Grid::new(&c, &d);
            let seen = grid.reach(start(&d, grid.v));
            for r in d.rooms.iter().filter(|r| r.kind == Kind::Orrery) {
                plans.insert(starry::plan(r));
                assert!(reached(&grid, &seen, r), "{:?} orrery {:?} unreachable at {}", d.job, starry::plan(r), grid.v);
                // Whoever falls into the void climbs back to the bridges.
                if starry::plan(r) == starry::Plan::Islands {
                    islands.0 += 1;
                    stairs += usize::from(r.stair != super::super::escape::NONE);
                    let any = (0..r.w() as i32).any(|i| (0..r.d() as i32).any(|k| starry::island(r, i as f32 + 0.5, k as f32 + 0.5).is_some()));
                    islands.1 += usize::from(any);
                    let (x, z) = r.centre();
                    let y = ((r.floor - starry::depth(r) + grid.v * 0.5) / grid.v).floor() as i32;
                    let (x0, z0) = ((r.x0 / grid.v) as i32, (r.z0 / grid.v) as i32);
                    let (x1, z1) = ((r.x1 / grid.v) as i32, (r.z1 / grid.v) as i32);
                    let tall = (1.8 / grid.v).ceil() as i32;
                    let pits: Vec<IVec3> = (z0..z1).flat_map(|z| (x0..x1).map(move |x| IVec3::new(x, y, z))).filter(|p| grid.stand(*p, tall)).collect();
                    assert!(!pits.is_empty(), "islands room without a void floor");
                    let pit = pits[pits.len() / 2];
                    let out = grid.reach(pit);
                    let top = ((r.floor + 0.5) / grid.v).floor() as i32;
                    assert!(out.iter().any(|p| p.y >= top), "no way out of the void at {x},{z}");
                }
            }
        }
    }
    assert_eq!(plans.len(), 3, "{plans:?}");
    // Most island rooms keep a deep void with a stair; the rest fall back to
    // a shallow one.
    assert!(stairs * 2 >= islands.0, "{stairs} stairs in {} island rooms", islands.0);
    assert!(islands.1 * 4 >= islands.0 * 3, "islands in only {} of {} island rooms", islands.1, islands.0);
}

#[test]
fn a_fall_into_the_cleft_climbs_back_out() {
    let mut checked = 0;
    for (scale, want) in [(VoxelScalePreset::OneMeter, 10), (VoxelScalePreset::HalfMeter, 3)] {
        let mut n = 0;
        'seeds: for seed in 0u32..300 {
            let c = cfg(seed.wrapping_mul(2891336453) ^ 7, scale);
            for (cell, d) in planned(&c, 3) {
                if !d.rooms.iter().any(|r| r.kind == Kind::Chasm) {
                    continue;
                }
                let grid = Grid::new(&c, &d);
                let seen = grid.reach(start(&d, grid.v));
                let tall = (1.8 / grid.v).ceil() as i32;
                for r in d.rooms.iter().filter(|r| r.kind == Kind::Chasm) {
                    assert!(reached(&grid, &seen, r), "cell {cell:?} cleft unreachable at {}", grid.v);
                    // Stand in the pool and find the way back to the floor.
                    let y = ((r.floor - r.rift - 1.0 + grid.v * 0.5) / grid.v).floor() as i32;
                    let (x0, z0, x1, z1) = ((r.x0 / grid.v) as i32, (r.z0 / grid.v) as i32, (r.x1 / grid.v) as i32, (r.z1 / grid.v) as i32);
                    let pool: Vec<IVec3> = (z0..z1).flat_map(|z| (x0..x1).map(move |x| IVec3::new(x, y, z))).filter(|p| grid.get(*p) == B::Water && grid.stand(*p, tall)).collect();
                    assert!(pool.len() > 4, "cell {cell:?}: cleft without a pool");
                    let out = grid.reach(pool[pool.len() / 2]);
                    let top = ((r.floor + grid.v * 0.5) / grid.v).floor() as i32;
                    assert!(out.iter().any(|p| p.y >= top), "cell {cell:?} at {}: no way out of the cleft", grid.v);
                    checked += 1;
                }
                n += 1;
                if n >= want {
                    break 'seeds;
                }
            }
        }
    }
    assert!(checked >= 10, "only {checked} clefts");
}

#[test]
fn dead_ends_hold_a_cache_you_can_reach() {
    let mut caches = 0;
    for seed in 0u32..10 {
        let c = cfg(seed.wrapping_mul(1597334677) ^ 5, VoxelScalePreset::OneMeter);
        for (cell, d) in complexes(&c, 3) {
            let grid = Grid::new(&c, &d);
            let seen = grid.reach(start(&d, grid.v));
            for r in d.rooms.iter().filter(|r| r.cache) {
                let (x, z) = treasure::cache_spot(&d, r);
                let crate_at = (Vec3::new(r.x0 + x, r.floor + 0.5 + 1e-3, r.z0 + z) / grid.v).floor().as_ivec3();
                assert_eq!(grid.get(crate_at), B::SalvageCrate, "cell {cell:?} {:?} {:?}", r.kind, r.damage);
                // The plinth stands on the floor, not over a basin or a void.
                let under = (Vec3::new(r.x0 + x, r.floor - 0.5 * grid.v, r.z0 + z) / grid.v).floor().as_ivec3();
                assert!(grid.get(under).is_solid(), "cell {cell:?}: cache in {:?} floats over {:?}", r.kind, grid.get(under));
                // Someone can stand within arm's reach of it.
                let near = seen.iter().any(|p| (p.as_vec3() - crate_at.as_vec3()).length() * grid.v < 3.0);
                assert!(near, "cell {cell:?}: cache in {:?} out of reach", r.kind);
                caches += 1;
            }
        }
    }
    assert!(caches >= 15, "only {caches} caches");
}

/// At a quarter metre, every door of an orrery or cleft reaches every other.
#[test]
fn quarter_metre_starry_and_cleft_rooms_join_their_doors() {
    let mut rooms = 0;
    for seed in 0u32..200 {
        let c = cfg(seed.wrapping_mul(747796405) ^ 31, VoxelScalePreset::QuarterMeter);
        for (cell, d) in planned(&c, 3) {
            for r in d.rooms.iter().filter(|r| matches!(r.kind, Kind::Orrery | Kind::Chasm) && !r.sealed) {
                let lo = Vec3::new(r.x0 - 3.0, r.floor - r.sub() - 2.0, r.z0 - 3.0);
                let hi = Vec3::new(r.x1 + 3.0, r.floor + 4.0, r.z1 + 3.0);
                let grid = Grid::within(&c, &d, lo, hi);
                // Door mouths: where each touching corridor meets the room box.
                let doors: Vec<IVec3> = d
                    .corrs
                    .iter()
                    .filter(|k| k.x1 > r.x0 - 0.1 && k.x0 < r.x1 + 0.1 && k.z1 > r.z0 - 0.1 && k.z0 < r.z1 + 0.1)
                    .map(|k| {
                        let x = ((k.x0 + k.x1) * 0.5).clamp(r.x0 + 0.6, r.x1 - 0.6);
                        let z = ((k.z0 + k.z1) * 0.5).clamp(r.z0 + 0.6, r.z1 - 0.6);
                        (Vec3::new(x, k.floor_at(x, z) + 0.6, z) / grid.v).floor().as_ivec3()
                    })
                    .collect();
                if doors.len() < 2 {
                    continue;
                }
                let seen = grid.reach(doors[0]);
                for door in &doors[1..] {
                    let near = seen.iter().any(|p| (*p - *door).abs().max_element() <= 2);
                    assert!(near, "cell {cell:?} {:?} {:?}: door {door} not reached from {}", r.kind, (r.kind == Kind::Orrery).then(|| starry::plan(r)), doors[0]);
                }
                rooms += 1;
            }
        }
        if rooms >= 8 {
            break;
        }
    }
    assert!(rooms >= 8, "only {rooms} rooms");
}

/// Cross-section and plan at waist height of the longest passage of the
/// first complex for `V2_SEED`.
#[test]
#[ignore = "debug dump"]
fn dump_passage() {
    let seed = std::env::var("V2_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let c = cfg(seed, VoxelScalePreset::OneMeter);
    let (_, d) = complexes(&c, 1).remove(0);
    let k = d.corrs.iter().max_by(|a, b| (a.a1 - a.a0).total_cmp(&(b.a1 - b.a0))).unwrap();
    let height = c.blocks_for_height(1000.0);
    let glyph = |b: Option<B>| match b {
        None => ' ',
        Some(B::Air) => '.',
        Some(B::Torch) => 't',
        Some(B::Gravel) => ',',
        Some(b) if b.is_solid() => '#',
        Some(_) => '?',
    };
    let (mid, f) = ((k.a0 + k.a1) * 0.5, k.fa.min(k.fb));
    println!("passage {}..{} along_x {} w {} h {}", k.a0, k.a1, k.along_x, if k.along_x { k.z1 - k.z0 } else { k.x1 - k.x0 }, k.h);
    let (c0, c1) = if k.along_x { (k.z0 - 2.0, k.z1 + 2.0) } else { (k.x0 - 2.0, k.x1 + 2.0) };
    for dy in (-2..7).rev() {
        let row: String = (0..((c1 - c0) as i32)).map(|i| {
            let a = c0 + i as f32 + 0.5;
            let m = if k.along_x { Vec3::new(mid, f + dy as f32 + 0.5, a) } else { Vec3::new(a, f + dy as f32 + 0.5, mid) };
            glyph(d.sample(&c, m, height))
        }).collect();
        println!("{dy:>3} {row}");
    }
    println!("-- plan at waist");
    for i in 0..((c1 - c0) as i32) {
        let a = c0 + i as f32 + 0.5;
        let row: String = (0..((k.a1 - k.a0) as i32)).map(|j| {
            let l = k.a0 + j as f32 + 0.5;
            let (x, z) = if k.along_x { (l, a) } else { (a, l) };
            glyph(d.sample(&c, Vec3::new(x, k.floor_at(x, z) + 1.0, z), height))
        }).collect();
        println!("{row}");
    }
}

/// Every gallery of the scan, not the first few: before generator 53 a
/// ceiling beam over the top step stranded some decks, hidden by an early exit.
#[test]
fn galleries_can_be_climbed_to() {
    let mut decks = 0;
    for (seed, scale) in (0u32..60).map(|s| (s, if s % 4 == 3 { VoxelScalePreset::HalfMeter } else { VoxelScalePreset::OneMeter })) {
        let mut c = cfg(seed.wrapping_mul(3266489917) ^ 11, scale);
        c.generator_version = c.generator_version.max(gallery::HEADROOM);
        for (cell, d) in complexes(&c, 3) {
            if !d.rooms.iter().any(|r| gallery::has(&d, r)) {
                continue;
            }
            let grid = Grid::new(&c, &d);
            let seen = grid.reach(start(&d, grid.v));
            for r in d.rooms.iter().filter(|r| gallery::has(&d, r)) {
                // Someone stands on the deck, 3.5 m over the floor, inside the room.
                let deck = ((r.floor + (3.5 / grid.v).round() * grid.v + grid.v * 0.5) / grid.v).floor() as i32;
                let up = seen.iter().any(|p| {
                    let m = (p.as_vec3() + Vec3::splat(0.5)) * grid.v;
                    p.y == deck && m.x > r.x0 && m.x < r.x1 && m.z > r.z0 && m.z < r.z1
                });
                assert!(up, "seed {seed} cell {cell:?}: {:?} gallery out of reach at {}", r.kind, grid.v);
                decks += 1;
            }
        }
    }
    assert!(decks >= 8, "only {decks} galleries");
}

/// Door mouths of a room: where each touching corridor meets the room box.
pub(super) fn doors(d: &Complex, r: &Room, v: f32) -> Vec<IVec3> {
    d.corrs
        .iter()
        .filter(|k| k.x1 > r.x0 - 0.1 && k.x0 < r.x1 + 0.1 && k.z1 > r.z0 - 0.1 && k.z0 < r.z1 + 0.1)
        .map(|k| {
            let x = ((k.x0 + k.x1) * 0.5).clamp(r.x0 + 0.6, r.x1 - 0.6);
            let z = ((k.z0 + k.z1) * 0.5).clamp(r.z0 + 0.6, r.z1 - 0.6);
            (Vec3::new(x, k.floor_at(x, z) + 0.6, z) / v).floor().as_ivec3()
        })
        .collect()
}

/// Fine detail never closes a room: at a quarter metre and a half, every
/// door of every kind of room reaches every other.
#[test]
fn fine_scales_keep_every_room_open_door_to_door() {
    let mut kinds = HashSet::new();
    let mut rooms = 0;
    for (seed, scale) in [(3u32, VoxelScalePreset::QuarterMeter), (5, VoxelScalePreset::QuarterMeter), (8, VoxelScalePreset::HalfMeter)] {
        let c = cfg(seed.wrapping_mul(2654435761) ^ 77, scale);
        for (cell, d) in complexes(&c, 2) {
            for r in d.rooms.iter().filter(|r| !r.sealed && !r.kind.is_puzzle()) {
                let lo = Vec3::new(r.x0 - 3.0, r.floor - r.sub() - 2.0, r.z0 - 3.0);
                let hi = Vec3::new(r.x1 + 3.0, r.floor + r.top() + 1.0, r.z1 + 3.0);
                let grid = Grid::within(&c, &d, lo, hi);
                let doors = doors(&d, r, grid.v);
                if doors.len() < 2 {
                    continue;
                }
                let seen = grid.reach(doors[0]);
                for door in &doors[1..] {
                    let near = seen.iter().any(|p| (*p - *door).abs().max_element() <= (0.6 / grid.v) as i32);
                    assert!(near, "cell {cell:?} {:?} at {}: door {door} not reached from {}", r.kind, grid.v, doors[0]);
                }
                kinds.insert(format!("{:?}", r.kind));
                rooms += 1;
            }
        }
    }
    assert!(rooms >= 12 && kinds.len() >= 5, "{rooms} rooms, kinds {kinds:?}");
}

/// The fine layer shows at fine scales and leaves 1 m worlds as they were.
#[test]
fn fine_detail_resolves_only_where_voxels_are_small() {
    let count = |scale| {
        let c = cfg(424242, scale);
        let (_, d) = complexes(&c, 1).remove(0);
        let height = c.blocks_for_height(1000.0);
        let v = c.voxel_meters();
        let mut accents = 0;
        for r in &d.rooms {
            let mut y = r.floor;
            while y < r.floor + r.h {
                let mut x = r.x0 + v * 0.5;
                while x < r.x1 {
                    for z in [r.z0 + v * 0.5, r.z0 + r.d() * 0.5 + v * 0.5] {
                        accents += usize::from(d.sample(&c, Vec3::new(x, y + v * 0.5, z), height) == Some(d.pal.accent));
                    }
                    x += v;
                }
                y += v;
            }
        }
        accents
    };
    let (coarse, fine) = (count(VoxelScalePreset::OneMeter), count(VoxelScalePreset::QuarterMeter));
    assert!(fine > coarse * 2 + 10, "accent voxels: {coarse} at 1 m, {fine} at 0.25 m");
}

/// Every torch at fine scales stands on, or hangs from, something solid:
/// pier sconces, candles on chandeliers, lamps on rails, niches and caches.
#[test]
fn fine_scale_lights_are_all_supported() {
    let mut torches = 0;
    for (seed, scale) in [(11u32, VoxelScalePreset::HalfMeter), (12, VoxelScalePreset::QuarterMeter)] {
        let c = cfg(seed, scale);
        for (cell, d) in planned(&c, 2) {
            for r in d.rooms.iter().filter(|r| !r.sealed) {
                let lo = Vec3::new(r.x0 - 2.0, r.floor - 1.0, r.z0 - 2.0);
                let hi = Vec3::new(r.x1 + 2.0, r.floor + r.top() + 1.0, r.z1 + 2.0);
                let grid = Grid::within(&c, &d, lo, hi);
                for (i, &b) in grid.blocks.iter().enumerate() {
                    let Some(off) = b.torch_support_offset() else { continue };
                    let (sx, sz) = (grid.size.x as usize, grid.size.z as usize);
                    let p = grid.min + IVec3::new((i % sx) as i32, (i / (sx * sz)) as i32, ((i / sx) % sz) as i32);
                    assert!(grid.get(p + off).is_solid(), "cell {cell:?} {:?} at {}: {b:?} at {p} hangs on {:?}", r.kind, grid.v, grid.get(p + off));
                    torches += 1;
                }
            }
        }
    }
    assert!(torches >= 40, "only {torches} torches");
}
