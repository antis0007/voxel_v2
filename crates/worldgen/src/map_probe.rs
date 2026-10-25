//! Top-down world maps for judging terrain without a GPU run. Run:
//! `cargo test -p engine_worldgen --release --lib map_probe -- --ignored --nocapture`
//! with `MAP_SEED`, `MAP_VERSION`, `MAP_SIZE_M`, `MAP_STEP`, `MAP_CX`, `MAP_CZ`
//! and `MAP_OUT` overriding the defaults. Prints coverage statistics too.
use super::{CURRENT_GENERATOR_VERSION, WATER_LEVEL_METERS, WorldSampleConfig, WorldType, drainage, lakes, landscape};
use engine_core::VoxelScalePreset;
use landscape::Biome;

fn env<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[derive(Clone, Copy, Default)]
struct Cell {
    top: f32,
    water: Option<f32>,
    lake: Option<lakes::LakeKind>,
    biome: Option<Biome>,
}

fn biome_rgb(b: Biome) -> [f32; 3] {
    match b {
        Biome::Ocean => [0.20, 0.35, 0.60],
        Biome::Coast => [0.86, 0.80, 0.58],
        Biome::Marsh => [0.36, 0.48, 0.34],
        Biome::Meadow => [0.50, 0.72, 0.36],
        Biome::Forest => [0.22, 0.50, 0.22],
        Biome::Taiga => [0.24, 0.40, 0.32],
        Biome::Desert => [0.90, 0.80, 0.52],
        Biome::Badlands => [0.78, 0.46, 0.30],
        Biome::Tundra => [0.70, 0.74, 0.66],
        Biome::Alpine => [0.62, 0.62, 0.64],
        Biome::Savanna => [0.74, 0.70, 0.38],
        Biome::Rainforest => [0.10, 0.42, 0.18],
        Biome::Heathland => [0.56, 0.52, 0.42],
        Biome::Fen => [0.44, 0.56, 0.40],
    }
}

fn sample(cfg: &WorldSampleConfig, x: i32, z: i32, step: f32) -> Cell {
    let s = landscape::preview_landscape_lod(cfg, x, z, step);
    let top = s.height + 1.0;
    let lake = lakes::sample(cfg, x as f32, z as f32);
    let lake_water = lake.water_m.filter(|&w| w > top);
    let water = if top <= WATER_LEVEL_METERS {
        Some(WATER_LEVEL_METERS)
    } else {
        lake_water.or_else(|| drainage::sample(cfg, x as f32, z as f32).water_m.filter(|&w| w > top))
    };
    Cell { top, water, lake: lake_water.and(lake.kind), biome: Some(s.biome) }
}

#[test]
#[ignore = "map probe; run explicitly"]
fn map_probe() {
    let cfg = WorldSampleConfig {
        generator_version: env("MAP_VERSION", CURRENT_GENERATOR_VERSION),
        seed: env("MAP_SEED", 2_738_958_700u32),
        world_type: WorldType::Hills,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    let (size, step) = (env("MAP_SIZE_M", 8192i32), env("MAP_STEP", 8i32));
    let (cx, cz) = (env("MAP_CX", 0i32), env("MAP_CZ", 0i32));
    let n = (size / step) as usize;
    let mut cells = vec![Cell::default(); n * n];
    let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
    std::thread::scope(|scope| {
        for (t, chunk) in cells.chunks_mut(n * n.div_ceil(threads)).enumerate() {
            let cfg = &cfg;
            scope.spawn(move || {
                for (i, c) in chunk.iter_mut().enumerate() {
                    let k = t * n * n.div_ceil(threads) + i;
                    let (x, z) = (cx - size / 2 + (k % n) as i32 * step, cz - size / 2 + (k / n) as i32 * step);
                    *c = sample(cfg, x, z, step as f32);
                }
            });
        }
    });
    let mut rgb = vec![0u8; n * n * 3];
    let mut biomes = std::collections::BTreeMap::<&str, usize>::new();
    let (mut ocean, mut inland_water) = (0usize, 0usize);
    let mut lake_cells = std::collections::BTreeMap::<String, usize>::new();
    let mut tops: Vec<f32> = cells.iter().map(|c| c.top).collect();
    for z in 0..n {
        for x in 0..n {
            let c = cells[z * n + x];
            let east = cells[z * n + (x + 1).min(n - 1)].top;
            let south = cells[(z + 1).min(n - 1) * n + x].top;
            // Light from the north-west, slopes in metres per sample.
            let shade = (1.0 + ((c.top - east) + (c.top - south)) / step as f32 * 0.9).clamp(0.45, 1.35);
            let color = match (c.water, c.biome) {
                (Some(w), _) if c.top <= WATER_LEVEL_METERS => {
                    ocean += 1;
                    let depth = ((w - c.top) / 40.0).clamp(0.0, 1.0);
                    [0.28 - 0.16 * depth, 0.50 - 0.25 * depth, 0.72 - 0.22 * depth]
                }
                (Some(_), _) => {
                    inland_water += 1;
                    if let Some(k) = c.lake {
                        *lake_cells.entry(format!("{k:?}")).or_default() += 1;
                        [0.10, 0.70, 0.75]
                    } else {
                        [0.15, 0.45, 0.95]
                    }
                }
                (None, Some(b)) => {
                    *biomes.entry(b.label()).or_default() += 1;
                    let base = biome_rgb(b);
                    let lift = ((c.top - 12.0) / 220.0).clamp(0.0, 1.0) * 0.25;
                    [base[0] * shade + lift, base[1] * shade + lift, base[2] * shade + lift]
                }
                _ => [0.0; 3],
            };
            for (j, v) in color.into_iter().enumerate() {
                rgb[(z * n + x) * 3 + j] = (v.clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    }
    tops.sort_by(f32::total_cmp);
    let pct = |p: f32| tops[((tops.len() - 1) as f32 * p) as usize];
    let total = (n * n) as f32;
    println!(
        "map seed={} v{} {}x{} m step {}: ocean {:.1}% inland water {:.2}% top p10 {:.0} p50 {:.0} p90 {:.0} p99 {:.0} max {:.0}",
        cfg.seed, cfg.generator_version, size, size, step,
        ocean as f32 / total * 100.0, inland_water as f32 / total * 100.0,
        pct(0.1), pct(0.5), pct(0.9), pct(0.99), pct(1.0)
    );
    let land = (n * n - ocean - inland_water).max(1) as f32;
    let list: Vec<String> = biomes.iter().map(|(k, v)| format!("{k} {:.1}%", *v as f32 / land * 100.0)).collect();
    println!("map biomes (of land): {}", list.join(", "));
    println!("map lake cells by kind: {lake_cells:?}");
    let out = std::env::var("MAP_OUT").unwrap_or_else(|_| {
        format!("{}/../../target/maps/map-{}-v{}.png", env!("CARGO_MANIFEST_DIR"), cfg.seed, cfg.generator_version)
    });
    if let Some(dir) = std::path::Path::new(&out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&out, png(n as u32, n as u32, &rgb)).expect("write map png");
    println!("map written {out}");
}

/// Minimal RGB PNG with stored (uncompressed) deflate blocks.
fn png(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &b in bytes {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let mut raw = Vec::with_capacity((w as usize * 3 + 1) * h as usize);
    for row in rgb.chunks(w as usize * 3) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, b) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        z.extend_from_slice(&(b.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}
