//! Top-down map of the solved planet face: relief, channels, biomes and every tree crown.
//! `cargo test -p engine_worldgen --release --lib planet_probe -- --ignored --nocapture`
//! with `MAP_SEED`, `MAP_VERSION`, `MAP_SIZE_M`, `MAP_STEP`, `MAP_CX`, `MAP_CZ`, `MAP_OUT`.
use super::{CURRENT_GENERATOR_VERSION, WorldSampleConfig, WorldType, geo, landscape, sample_column, tree_profile_uncached};
use engine_core::VoxelScalePreset;
use std::collections::BTreeMap;

fn env<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[derive(Clone, Copy, Default)]
struct Cell {
    top: f32,
    water: bool,
    vegetation: f32,
    biome: &'static str,
}

#[derive(Default)]
struct Tally {
    columns: usize,
    trees: usize,
}

#[test]
#[ignore = "planet map probe; run explicitly"]
fn planet_probe() {
    let cfg = WorldSampleConfig {
        planet: None,
        generator_version: env("MAP_VERSION", CURRENT_GENERATOR_VERSION),
        seed: env("MAP_SEED", 74u32),
        world_type: WorldType::Planet,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    let (size, step) = (env("MAP_SIZE_M", 3072i32), env("MAP_STEP", 4i32));
    let (cx, cz) = (env("MAP_CX", 0i32), env("MAP_CZ", 0i32));
    let (x0, z0) = (cx - size / 2, cz - size / 2);
    let n = (size / step) as usize;
    let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
    let rows = n.div_ceil(threads);
    let mut cells = vec![Cell::default(); n * n];
    std::thread::scope(|scope| {
        for (t, chunk) in cells.chunks_mut(n * rows).enumerate() {
            let cfg = &cfg;
            scope.spawn(move || {
                for (i, c) in chunk.iter_mut().enumerate() {
                    let k = t * n * rows + i;
                    let (x, z) = (x0 + (k % n) as i32 * step, z0 + (k / n) as i32 * step);
                    let col = sample_column(cfg, x, z);
                    let g = geo::ground(cfg, x as f32, z as f32, 1.0);
                    *c = Cell {
                        top: cfg.world_y(col.height),
                        water: g.water_m.is_some() || col.height < cfg.water_level(),
                        vegetation: col.vegetation_density,
                        biome: landscape::preview_landscape_lod(cfg, x, z, 1.0).biome.label(),
                    };
                }
            });
        }
    });
    // Every 12 m tree cell in the window, evaluated exactly as chunks do.
    let tree_cells = size / 12;
    let (c0x, c0z) = (x0.div_euclid(12), z0.div_euclid(12));
    let mut trees = vec![None; (tree_cells * tree_cells) as usize];
    std::thread::scope(|scope| {
        let per = trees.len().div_ceil(threads);
        for (t, chunk) in trees.chunks_mut(per).enumerate() {
            let cfg = &cfg;
            scope.spawn(move || {
                for (i, slot) in chunk.iter_mut().enumerate() {
                    let k = (t * per + i) as i32;
                    *slot = tree_profile_uncached(cfg, c0x + k % tree_cells, c0z + k / tree_cells)
                        .map(|p| (p.anchor_x, p.anchor_z, p.canopy_radius));
                }
            });
        }
    });

    let mut rgb = vec![0u8; n * n * 3];
    let mut by_biome = BTreeMap::<&str, Tally>::new();
    let (mut land, mut water, mut wooded, mut above_86, mut above_122) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for z in 0..n {
        for x in 0..n {
            let c = cells[z * n + x];
            let east = cells[z * n + (x + 1).min(n - 1)].top;
            let south = cells[(z + 1).min(n - 1) * n + x].top;
            let shade = (1.0 + ((c.top - east) + (c.top - south)) / step as f32 * 0.9).clamp(0.45, 1.35);
            let colour = if c.water {
                water += 1;
                [0.18, 0.42, 0.86]
            } else {
                land += 1;
                by_biome.entry(c.biome).or_default().columns += 1;
                wooded += usize::from(c.vegetation > 0.3);
                above_86 += usize::from(c.top > 86.0);
                above_122 += usize::from(c.top > 122.0);
                let v = c.vegetation.clamp(0.0, 1.2) / 1.2;
                [(0.66 - 0.30 * v) * shade, (0.70 - 0.12 * v) * shade, (0.44 - 0.18 * v) * shade]
            };
            let o = (z * n + x) * 3;
            for (ch, v) in colour.iter().enumerate() {
                rgb[o + ch] = (v.clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    }
    let (mut placed, mut wet_trees) = (0usize, 0usize);
    for &(ax, az, r) in trees.iter().flatten() {
        placed += 1;
        let (px, pz) = ((ax - x0) / step, (az - z0) / step);
        if px < 0 || pz < 0 || px >= n as i32 || pz >= n as i32 {
            continue;
        }
        wet_trees += usize::from(cells[pz as usize * n + px as usize].water);
        if let Some(t) = by_biome.get_mut(cells[pz as usize * n + px as usize].biome) {
            t.trees += 1;
        }
        let rr = (r as f32 / step as f32).max(1.0);
        let ri = rr.ceil() as i32;
        for dz in -ri..=ri {
            for dx in -ri..=ri {
                let (qx, qz) = (px + dx, pz + dz);
                if ((dx * dx + dz * dz) as f32) > rr * rr || qx < 0 || qz < 0 || qx >= n as i32 || qz >= n as i32 {
                    continue;
                }
                let o = (qz as usize * n + qx as usize) * 3;
                rgb[o..o + 3].copy_from_slice(&[22, 70, 30]);
            }
        }
    }
    let km2 = (size as f64 / 1000.0).powi(2);
    let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
    println!("planet_probe seed {} version {} window {size} m at ({cx},{cz}) step {step} m", cfg.seed, cfg.generator_version);
    println!("  land {:.1}%  water {:.1}%  land with density>0.3 {:.1}%", pct(land, n * n), pct(water, n * n), pct(wooded, land));
    println!("  land above 86 m {:.1}%  above 122 m {:.1}%", pct(above_86, land), pct(above_122, land));
    println!("  trees {placed}  ({:.0} per km2 of window), {wet_trees} anchored in water", placed as f64 / km2);
    let column_km2 = (step as f64 / 1000.0).powi(2);
    for (b, t) in &by_biome {
        println!("  {b:<18} {:>5.1}% of land  {:>6.0} trees/km2", pct(t.columns, land), t.trees as f64 / (t.columns as f64 * column_km2).max(1e-9));
    }
    let out = std::env::var("MAP_OUT").unwrap_or_else(|_| format!("planet-{}-v{}.ppm", cfg.seed, cfg.generator_version));
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/maps");
    let _ = std::fs::create_dir_all(&dir);
    let mut bytes = format!("P6\n{n} {n}\n255\n").into_bytes();
    bytes.extend_from_slice(&rgb);
    std::fs::write(dir.join(&out), bytes).ok();
    println!("  wrote target/maps/{out}");
}

#[test]
#[ignore = "planet corridor probe; run explicitly"]
fn corridor_probe() {
    let cfg = |v| WorldSampleConfig { generator_version: v, seed: env("MAP_SEED", 74u32), world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter, planet: None };
    let (old, new) = (cfg(env("MAP_BEFORE", 135u32)), cfg(env("MAP_VERSION", 136u32)));
    let (size, step) = (env("MAP_SIZE_M", 12288i32), env("MAP_STEP", 6i32));
    let n = size / step;
    let mut cut = [0usize; 4];
    let (mut deepest, mut land, mut wet_old, mut wet_new) = (0f32, 0usize, 0usize, 0usize);
    for k in 0..n * n {
        let (x, z) = ((k % n) * step - size / 2, (k / n) * step - size / 2);
        let (a, b) = (geo::ground(&old, x as f32, z as f32, 1.0), geo::ground(&new, x as f32, z as f32, 1.0));
        wet_old += usize::from(a.water_m.is_some());
        wet_new += usize::from(b.water_m.is_some());
        if a.top_m <= 0.0 {
            continue;
        }
        land += 1;
        let d = a.top_m - b.top_m;
        deepest = deepest.max(d);
        for (i, t) in [0.5f32, 2.0, 5.0, 10.0].iter().enumerate() {
            cut[i] += usize::from(d > *t);
        }
    }
    let pct = |c: usize| 100.0 * c as f64 / land.max(1) as f64;
    println!("corridor_probe {size} m step {step}: lowered >0.5 m {:.2}%  >2 m {:.2}%  >5 m {:.2}%  >10 m {:.2}%  deepest {deepest:.1} m",
        pct(cut[0]), pct(cut[1]), pct(cut[2]), pct(cut[3]));
    println!("  water columns before {wet_old} after {wet_new}");
}

#[test]
#[ignore = "solves a planet; run explicitly"]
fn incised_water_never_rises_downstream() {
    let cfg = WorldSampleConfig { planet: None, generator_version: 136, seed: env("MAP_SEED", 74u32), world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter };
    let p = geo::planet_for(&cfg);
    let levels = geo::corridor::incised_levels(&p);
    let (mut rises, mut above, mut cut_nodes, mut deepest) = (0usize, 0usize, 0usize, 0f32);
    for i in 0..levels.len() {
        let r = p.receiver[i] as usize;
        let still = !p.land[i] || p.lake[i] > f32::MIN || r == i;
        if still {
            continue;
        }
        rises += usize::from(levels[i] + 1e-4 < levels[r]);
        let surface = p.height[i];
        above += usize::from(levels[i] > surface + 1e-4);
        let cut = surface - levels[i];
        cut_nodes += usize::from(p.discharge[i] >= 2.5e6 && cut > 1.0);
        deepest = deepest.max(cut);
    }
    println!("incised levels: {rises} rises, {above} above the surface, {cut_nodes} channel nodes cut >1 m, deepest {deepest:.1} m");
    assert_eq!(rises, 0);
}

#[test]
#[ignore = "timing; run explicitly with --test-threads=1"]
fn corridor_generation_cost() {
    let cfg = |v| WorldSampleConfig { planet: None, generator_version: v, seed: env("MAP_SEED", 74u32), world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter };
    let (old, new) = (cfg(env("MAP_BEFORE", 135u32)), cfg(env("MAP_VERSION", 136u32)));
    let _ = (geo::planet_for(&old), geo::corridor::incised_levels(&geo::planet_for(&new)));
    // A 6 x 6 chunk patch over the river south of spawn, surface layer only.
    let chunks: Vec<glam::IVec3> = (0..36).map(|k| glam::IVec3::new(k % 6 - 40, 0, k / 6 + 60)).collect();
    let pass = |c: &WorldSampleConfig| {
        let t = std::time::Instant::now();
        for &p in &chunks {
            let surface = super::sample_column(c, p.x * 32 + 16, p.z * 32 + 16).height.div_euclid(32);
            std::hint::black_box(super::generate(c, glam::IVec3::new(p.x, surface, p.z)));
        }
        t.elapsed().as_secs_f64() * 1000.0 / chunks.len() as f64
    };
    let (mut a, mut b) = (f64::MAX, f64::MAX);
    for _ in 0..4 {
        a = a.min(pass(&old));
        b = b.min(pass(&new));
    }
    println!("chunk generation, best of 4 interleaved: v{} {a:.2} ms  v{} {b:.2} ms  ({:+.1}%)", old.generator_version, new.generator_version, (b / a - 1.0) * 100.0);
}
