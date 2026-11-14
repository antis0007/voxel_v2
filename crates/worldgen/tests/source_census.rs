//! Census of padded streaming sources: one-value share and palette widths.
//!
//! Sizes the compact source format before it is written. Run:
//! `cargo test --release -p engine_worldgen --test source_census -- --ignored --nocapture`
use engine_voxel::{CURRENT_GENERATOR_VERSION, WorldSampleConfig, WorldType};
use glam::IVec3;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

const RADIUS: i32 = 28;
const STRIDE: i32 = 4;

/// Index width a palette of `distinct` ids needs, in bits per cell.
fn width(distinct: usize) -> usize {
    match distinct {
        0 | 1 => 0,
        2 => 1,
        3..=4 => 2,
        5..=16 => 4,
        _ => 8,
    }
}

#[derive(Default, Clone, Copy)]
struct Tally {
    sources: usize,
    by_width: [usize; 5],
    packed_bytes: usize,
    dense_bytes: usize,
    nanos: u128,
}

fn slot(bits: usize) -> usize {
    match bits {
        0 => 0,
        1 => 1,
        2 => 2,
        4 => 3,
        _ => 4,
    }
}

fn census(world_type: WorldType) {
    let cfg = WorldSampleConfig {
        planet: None,
        generator_version: CURRENT_GENERATOR_VERSION,
        seed: 424242,
        world_type,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter,
    };
    let size = engine_core::CHUNK_SIZE;
    let (low, high) = (
        engine_core::floor_div(cfg.min_world_y(), size),
        engine_core::floor_div(cfg.max_world_y() - 1, size),
    );
    let mut coords = Vec::new();
    for cz in (-RADIUS..=RADIUS).step_by(STRIDE as usize) {
        for cx in (-RADIUS..=RADIUS).step_by(STRIDE as usize) {
            if cx * cx + cz * cz <= RADIUS * RADIUS {
                coords.extend((low..=high).map(|cy| IVec3::new(cx, cy, cz)));
            }
        }
    }
    let next = AtomicUsize::new(0);
    let never = AtomicBool::new(false);
    let rows = Mutex::new(vec![Tally::default(); (high - low + 1) as usize]);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let Some(&coord) = coords.get(next.fetch_add(1, Ordering::Relaxed)) else { break };
                let start = std::time::Instant::now();
                let terrain = engine_worldgen::generate_padded(&cfg, coord, &never).expect("never cancelled");
                let nanos = start.elapsed().as_nanos();
                let mut seen = [false; 256];
                for &block in &terrain.blocks {
                    seen[block as usize] = true;
                }
                let distinct = seen.iter().filter(|&&s| s).count();
                let bits = width(distinct);
                let cells = terrain.blocks.len();
                let mut rows = rows.lock().expect("census lock");
                let row = &mut rows[(coord.y - low) as usize];
                row.sources += 1;
                row.nanos += nanos;
                row.by_width[slot(bits)] += 1;
                // The streamer never caches a one-value source, so the ratio
                // covers only the sources that reach the cache.
                if bits > 0 {
                    row.dense_bytes += cells;
                    row.packed_bytes += if bits == 8 { cells } else { (cells * bits).div_ceil(8) + distinct };
                }
            });
        }
    });
    let rows = rows.into_inner().expect("census lock");
    println!("{world_type:?} gen {CURRENT_GENERATOR_VERSION}, radius {RADIUS} stride {STRIDE}, chunk rows {low}..={high}");
    println!("  cy  sources  one-value  1bit  2bit  4bit  8bit  packed/dense of cached  ms/chunk");
    let mut total = Tally::default();
    for (i, row) in rows.iter().enumerate() {
        println!(
            "{:4} {:8} {:10} {:5} {:5} {:5} {:5}  {:.3}  {:.2}",
            low + i as i32, row.sources, row.by_width[0], row.by_width[1], row.by_width[2],
            row.by_width[3], row.by_width[4], row.packed_bytes as f64 / row.dense_bytes.max(1) as f64,
            row.nanos as f64 / 1e6 / row.sources.max(1) as f64
        );
        total.sources += row.sources;
        total.packed_bytes += row.packed_bytes;
        total.dense_bytes += row.dense_bytes;
        for k in 0..5 {
            total.by_width[k] += row.by_width[k];
        }
    }
    println!(
        "  all {:8} {:10} {:5} {:5} {:5} {:5}  {:.3}",
        total.sources, total.by_width[0], total.by_width[1], total.by_width[2], total.by_width[3],
        total.by_width[4], total.packed_bytes as f64 / total.dense_bytes.max(1) as f64
    );
}

#[test]
#[ignore = "measurement probe; run explicitly"]
fn source_census_hills() {
    census(WorldType::Hills);
}

/// Splits one all-air chunk's cost: repeat generation vs per-voxel sampling.
#[test]
#[ignore = "measurement probe; run explicitly"]
fn air_chunk_cost_split() {
    let cfg = WorldSampleConfig {
        planet: None,
        generator_version: CURRENT_GENERATOR_VERSION,
        seed: 424242,
        world_type: WorldType::Hills,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter,
    };
    let never = AtomicBool::new(false);
    let coord = IVec3::new(8, 10, 0);
    let time = |f: &dyn Fn()| {
        f();
        (0..5).map(|_| { let t = std::time::Instant::now(); f(); t.elapsed().as_secs_f64() * 1e3 })
            .fold(f64::MAX, f64::min)
    };
    let full = time(&|| { std::hint::black_box(engine_worldgen::generate_padded(&cfg, coord, &never)); });
    let size = engine_core::PADDED_CHUNK_SIZE;
    let origin = coord * engine_core::CHUNK_SIZE - IVec3::ONE;
    let columns: Vec<_> = (0..size * size)
        .map(|i| engine_worldgen::sample_column(&cfg, origin.x + i % size, origin.z + i / size)).collect();
    let column_ms = time(&|| {
        for i in 0..size * size {
            std::hint::black_box(engine_worldgen::sample_column(&cfg, origin.x + i % size, origin.z + i / size));
        }
    });
    let voxel_ms = time(&|| {
        for z in 0..size { for x in 0..size {
            let column = columns[(z * size + x) as usize];
            for y in 0..size {
                std::hint::black_box(engine_worldgen::sample_block_with(
                    &cfg, origin + IVec3::new(x, y, z), column, &engine_worldgen::CachedProfiles));
            }
        }}
    });
    println!("air chunk {coord}: generate_padded {full:.2} ms (columns cached) | 34^2 sample_column {column_ms:.2} ms | 34^3 sample_block_with {voxel_ms:.2} ms");
}

/// FNV-1a over every block, bed height and sky ceiling of the census region.
fn region_hash(world_type: WorldType, generator_version: u32) -> (u64, f64) {
    let cfg = WorldSampleConfig {
        planet: None, generator_version, seed: 424242, world_type,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter,
    };
    let size = engine_core::CHUNK_SIZE;
    let (low, high) = (engine_core::floor_div(cfg.min_world_y(), size), engine_core::floor_div(cfg.max_world_y() - 1, size));
    let mut coords = Vec::new();
    for cz in (-RADIUS..=RADIUS).step_by(STRIDE as usize) {
        for cx in (-RADIUS..=RADIUS).step_by(STRIDE as usize) {
            if cx * cx + cz * cz <= RADIUS * RADIUS {
                coords.extend((low..=high).map(|cy| IVec3::new(cx, cy, cz)));
            }
        }
    }
    let never = AtomicBool::new(false);
    let next = AtomicUsize::new(0);
    let hashes = Mutex::new(vec![0u64; coords.len()]);
    let start = std::time::Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..std::thread::available_parallelism().map_or(4, |n| n.get().min(8)) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(&coord) = coords.get(i) else { break };
                let t = engine_worldgen::generate_padded(&cfg, coord, &never).expect("never cancelled");
                let mut h = 0xcbf2_9ce4_8422_2325u64;
                let mut eat = |b: u8| { h ^= u64::from(b); h = h.wrapping_mul(0x100_0000_01b3); };
                t.blocks.iter().for_each(|&b| eat(b as u8));
                for v in t.bed_height.iter().chain(t.sky_ceiling.iter()) { v.to_le_bytes().into_iter().for_each(&mut eat); }
                t.bed_frac.iter().for_each(|f| f.to_bits().to_le_bytes().into_iter().for_each(&mut eat));
                for (p, a) in &t.partial_water { [p.x, p.y, p.z].iter().for_each(|v| v.to_le_bytes().into_iter().for_each(&mut eat)); a.to_le_bytes().into_iter().for_each(&mut eat); }
                hashes.lock().expect("hash lock")[i] = h;
            });
        }
    });
    let secs = start.elapsed().as_secs_f64();
    let all = hashes.into_inner().expect("hash lock").iter().fold(0u64, |a, &h| a.rotate_left(5) ^ h);
    (all, secs)
}

#[test]
#[ignore = "equivalence probe; run explicitly"]
fn region_hashes() {
    for (world, version) in [(WorldType::Hills, CURRENT_GENERATOR_VERSION), (WorldType::Hills, 53), (WorldType::Hills, 20), (WorldType::Ridged, CURRENT_GENERATOR_VERSION)] {
        let (h, secs) = region_hash(world, version);
        println!("hash {world:?} v{version} {h:016x} {secs:.2}s");
    }
}

/// Cycles this thread has run, unaffected by time spent switched out.
#[cfg(windows)]
fn thread_cycles() -> u64 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> isize;
        fn QueryThreadCycleTime(thread: isize, cycles: *mut u64) -> i32;
        fn SetThreadAffinityMask(thread: isize, mask: usize) -> usize;
    }
    static PINNED: std::sync::Once = std::sync::Once::new();
    // SAFETY: pins only the calling thread; logical CPU 2 is a P-core thread
    // on this machine, so cycles are not mixed across core types.
    PINNED.call_once(|| unsafe { SetThreadAffinityMask(GetCurrentThread(), 1 << 2); });
    let mut cycles = 0u64;
    // SAFETY: the pseudo handle is always valid for the calling thread and
    // `cycles` outlives the call.
    unsafe { QueryThreadCycleTime(GetCurrentThread(), &mut cycles) };
    cycles
}

/// Single-thread best-of-7 generation cost over fixed chunks in megacycles,
/// warm caches.
#[cfg(windows)]
#[test]
#[ignore = "timing probe; run explicitly"]
fn chunk_timing() {
    let cfg = WorldSampleConfig {
        planet: None, generator_version: CURRENT_GENERATOR_VERSION, seed: 424242, world_type: WorldType::Hills,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter,
    };
    let never = AtomicBool::new(false);
    let chunks = [
        (8, 10, 0), (8, 3, 0), (8, 2, 0), (8, 1, 0), (8, 0, 0), (8, -2, 0),
        (-12, 1, 16), (-12, 0, 16), (20, 1, -20), (20, 0, -20), (-24, 2, -8), (-24, -1, -8),
    ];
    let mut total = 0.0;
    let mut line = String::new();
    for (x, y, z) in chunks {
        let coord = IVec3::new(x, y, z);
        let _ = engine_worldgen::generate_padded(&cfg, coord, &never);
        let best = (0..7).map(|_| {
            let start = thread_cycles();
            std::hint::black_box(engine_worldgen::generate_padded(&cfg, coord, &never));
            (thread_cycles() - start) as f64 / 1e6
        }).fold(f64::MAX, f64::min);
        total += best;
        line += &format!(" {best:.0}");
    }
    println!("timing total {total:.0} Mcycles |{line}");
}
