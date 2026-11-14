//! Headless planet lab: render and measure generated Planet terrain without
//! the game. One process solves a planet once and can run many views.
//!
//! ```text
//! planet_lab <command> [--option value]...
//!   patch   top-down window            --side 512 --px 768 --layer shaded|cover|slope|height|detail
//!   view    eye-level perspective      --alt 1.7 --yaw 0 --pitch -5 --fov 70 --size 960x540
//!   globe   whole planet, orthographic --lon 0 --lat 0 --size 900 --phase 30
//!                                      --layer shaded|height|tectonics|flow|sediment|climate
//!   stats   JSON metrics for a window  --side 512 --spacing 1
//!   where   print named places (spawn, coast, confluence) as chart metres
//!   probe   height, slope, measured and tile-reported gradient at --at
//!   find    places by cover and form   --block Sand --min-h 3 --max-h 14 --min-slope 0 --max-slope 9
//!                                      --side 24000 --spacing 120 --limit 8   (heights above sea)
//!   batch   run one command per line of a file, sharing the solve
//! common:   --seed N  --gen A[,B]  --scale 1|0.5|0.25  --at x,z|spawn|coast|confluence
//!           --out path-prefix  --sun azimuth,elevation  --exposure 1.0
//! ```
//!
//! `--gen A,B` renders both generators and writes them side by side (A left),
//! plus a height-difference image for `patch`. Env `VOXEL_HISTORY_MYR=20`
//! shortens the plate history for a fast loop (not save compatible).
//! Docs: tools/axiom-dev/PLANET_LAB.md.
mod field;
mod png;
mod render;
mod stats;

use anyhow::{Context, bail, ensure};
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, planetary};
use std::collections::HashMap;
use std::path::PathBuf;

struct Options(HashMap<String, String>);

impl Options {
    fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut map = HashMap::new();
        let mut it = args.iter();
        while let Some(key) = it.next() {
            let name = key.strip_prefix("--").with_context(|| format!("expected --option, got {key}"))?;
            map.insert(name.to_owned(), it.next().with_context(|| format!("--{name} needs a value"))?.clone());
        }
        Ok(Self(map))
    }
    fn text(&self, key: &str, default: &str) -> String { self.0.get(key).cloned().unwrap_or_else(|| default.to_owned()) }
    fn number<T: std::str::FromStr>(&self, key: &str, default: T) -> anyhow::Result<T> {
        match self.0.get(key) {
            Some(v) => v.parse().map_err(|_| anyhow::anyhow!("--{key} {v} is not a valid number")),
            None => Ok(default),
        }
    }
    fn pair(&self, key: &str, default: (f64, f64), sep: char) -> anyhow::Result<(f64, f64)> {
        let Some(v) = self.0.get(key) else { return Ok(default); };
        let (a, b) = v.split_once(sep).with_context(|| format!("--{key} wants a{sep}b"))?;
        Ok((a.trim().parse()?, b.trim().parse()?))
    }
}

fn configs(opts: &Options) -> anyhow::Result<Vec<WorldSampleConfig>> {
    let seed = opts.number("seed", 2_738_958_700_u32)?;
    let voxel_scale = match opts.text("scale", "1").as_str() {
        "1" => VoxelScalePreset::OneMeter,
        "0.5" => VoxelScalePreset::HalfMeter,
        "0.25" => VoxelScalePreset::QuarterMeter,
        other => bail!("--scale {other}: use 1, 0.5 or 0.25"),
    };
    let gens = opts.text("gen", &engine_voxel::PLANET_GENERATOR_VERSION.to_string());
    let list = gens.split(',').map(|g| -> anyhow::Result<WorldSampleConfig> {
        let cfg = WorldSampleConfig { planet: None, seed, generator_version: g.trim().parse()?,
            world_type: WorldType::Planet, voxel_scale };
        ensure!(planetary::active(&cfg), "generator {g} has no spherical Planet terrain");
        Ok(cfg)
    }).collect::<anyhow::Result<Vec<_>>>()?;
    ensure!((1..=2).contains(&list.len()), "--gen takes one generator or two for A/B");
    Ok(list)
}

fn place(cfg: &WorldSampleConfig, name: &str) -> anyhow::Result<(f64, f64)> {
    Ok(match name {
        "spawn" => { let p = planetary::spawn_point(cfg); (p.x, p.z) }
        "coast" => { let p = planetary::review::coastal_origin(cfg).context("no coastal bake on this planet")?; (p.x, p.z) }
        "confluence" => { let p = planetary::review::confluence(cfg).context("no confluence on the spawn face")?.0; (p.x, p.z) }
        other => {
            let (x, z) = other.split_once(',').with_context(|| format!("--at {other}: use x,z or spawn|coast|confluence"))?;
            (x.trim().parse()?, z.trim().parse()?)
        }
    })
}

fn save(path: &std::path::Path, width: usize, height: usize, pixels: &[[f32; 3]], exposure: f32) -> anyhow::Result<()> {
    let bytes: Vec<u8> = pixels.iter().flat_map(|p| png::encode(*p, exposure)).collect();
    png::write(path, width, height, &bytes).with_context(|| format!("writing {}", path.display()))
}

/// Two equal images side by side with a dark gutter.
fn beside(a: &[[f32; 3]], b: &[[f32; 3]], width: usize, height: usize) -> (Vec<[f32; 3]>, usize) {
    let gutter = 6;
    let mut out = Vec::with_capacity((width * 2 + gutter) * height);
    for row in 0..height {
        out.extend_from_slice(&a[row * width..(row + 1) * width]);
        out.extend(std::iter::repeat_n([0.0; 3], gutter));
        out.extend_from_slice(&b[row * width..(row + 1) * width]);
    }
    (out, width * 2 + gutter)
}

fn run(command: &str, opts: &Options) -> anyhow::Result<()> {
    let cfgs = configs(opts)?;
    let started = std::time::Instant::now();
    for cfg in &cfgs {
        let (nodes, checksum) = planetary::review::lab::solve_identity(cfg).context("planet solve")?;
        eprintln!("[planet-lab] generator={} seed={} nodes={nodes} checksum={checksum:016x} solve_s={:.1}",
            cfg.generator_version, cfg.seed, started.elapsed().as_secs_f64());
    }
    let at = opts.text("at", "spawn");
    let (x, z) = place(&cfgs[0], &at)?;
    let label = cfgs.iter().map(|c| c.generator_version.to_string()).collect::<Vec<_>>().join("-vs-");
    let out = PathBuf::from(opts.text("out", &format!("target/planet-lab/{command}-{label}")));
    let png_path = |suffix: &str| PathBuf::from(format!("{}{suffix}.png", out.display()));
    let (azimuth, elevation) = opts.pair("sun", (135.0, 38.0), ',')?;
    let light = render::Light::new(azimuth as f32, elevation as f32);
    let exposure = opts.number("exposure", 1.0_f32)?;
    let rendered = std::time::Instant::now();
    match command {
        "where" => {
            for name in ["spawn", "coast", "confluence"] {
                match place(&cfgs[0], name) {
                    Ok((x, z)) => println!("{{\"place\":\"{name}\",\"x\":{x:.1},\"z\":{z:.1}}}"),
                    Err(e) => println!("{{\"place\":\"{name}\",\"error\":\"{e}\"}}"),
                }
            }
        }
        "probe" => {
            // The gradient the tiles report against one measured from heights.
            let step = opts.number("step", 6.0_f32)?;
            for cfg in &cfgs {
                let h = |dx: f32, dz: f32| planetary::ground(cfg, x as f32 + dx, z as f32 + dz, 8.0).top_m;
                let measured = [(h(step, 0.0) - h(-step, 0.0)) / (2.0 * step), (h(0.0, step) - h(0.0, -step)) / (2.0 * step)];
                let g = planetary::ground(cfg, x as f32, z as f32, 1.0);
                let solved = planetary::review::lab::drainage_gradient(cfg, x, z);
                println!("{{\"generator\":{},\"at\":[{x:.1},{z:.1}],\"top_m\":{:.2},\"slope\":{:.3},\"measured_gradient\":[{:.3},{:.3}],\"tile_gradient\":{},\"water\":{},\"block\":\"{:?}\",\"moisture\":{:.2},\"maat_c\":{:.1},\"sediment\":{:.2},\"delta\":{:.2},\"riparian\":{:.2}}}",
                    cfg.generator_version, g.top_m, g.slope, measured[0], measured[1],
                    solved.map_or("null".to_owned(), |(v, w)| format!("{{\"dx\":{:.3},\"dz\":{:.3},\"weight\":{w:.2}}}", v[0], v[1])),
                    g.water_m.is_some(), planetary::column(cfg, x as i32, z as i32, 1.0).surface, g.moisture, g.maat_c, g.sediment, g.delta, g.riparian);
            }
        }
        "find" => {
            let (side, spacing) = (opts.number("side", 24_000.0_f64)?, opts.number("spacing", 120.0_f64)?);
            let n = ((side / spacing) as usize).clamp(8, 1024);
            let block = opts.text("block", "");
            let (min_h, max_h) = (opts.number("min-h", -1.0e9_f32)?, opts.number("max-h", 1.0e9_f32)?);
            let (min_slope, max_slope) = (opts.number("min-slope", 0.0_f32)?, opts.number("max-slope", 1.0e9_f32)?);
            let limit = opts.number("limit", 8_usize)?;
            let grid = field::Grid::build(&cfgs[0], x, z, spacing, n);
            let mut found: Vec<(f64, f64)> = Vec::new();
            // Nearest matches first, spread at least five cells apart.
            let mut order: Vec<usize> = (0..n * n).collect();
            let centre = (n as f64 - 1.0) * 0.5;
            order.sort_by(|a, b| {
                let d = |k: usize| ((k % n) as f64 - centre).hypot((k / n) as f64 - centre);
                d(*a).total_cmp(&d(*b))
            });
            for k in order {
                let cell = &grid.cells[k];
                let slope = grid.slope(k % n, k / n);
                let above = cell.top - field::SEA;
                if cell.wet() || !(min_h..=max_h).contains(&above) || !(min_slope..=max_slope).contains(&slope)
                    || (!block.is_empty() && format!("{:?}", cell.surface) != block) { continue; }
                let (px, pz) = (grid.x0 + (k % n) as f64 * spacing, grid.z0 + (k / n) as f64 * spacing);
                if found.iter().any(|(fx, fz)| (fx - px).hypot(fz - pz) < spacing * 5.0) { continue; }
                println!("{{\"x\":{px:.0},\"z\":{pz:.0},\"above_sea_m\":{above:.1},\"slope\":{:.3},\"block\":\"{:?}\"}}", slope, cell.surface);
                found.push((px, pz));
                if found.len() >= limit { break; }
            }
            if found.is_empty() { println!("{{\"found\":0}}"); }
        }
        "stats" => {
            let (side, spacing) = (opts.number("side", 512.0_f64)?, opts.number("spacing", 1.0_f64)?);
            ensure!(spacing > 0.0 && side >= spacing * 16.0, "window too small for its spacing");
            let mut lines = Vec::new();
            for cfg in &cfgs {
                let line = stats::window(cfg, x, z, side, spacing);
                println!("{line}");
                lines.push(line);
            }
            let path = PathBuf::from(format!("{}.json", out.display()));
            if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
            std::fs::write(&path, format!("[{}]\n", lines.join(",\n")))?;
        }
        "patch" => {
            let (side, px) = (opts.number("side", 512.0_f64)?, opts.number("px", 768_usize)?);
            ensure!((64..=2048).contains(&px) && side > 0.0, "patch wants 64..=2048 px and a positive side");
            let layer = render::Layer::parse(&opts.text("layer", "shaded"))?;
            let grids: Vec<_> = cfgs.iter().map(|cfg| field::Grid::build(cfg, x, z, side / px as f64, px)).collect();
            let tops: Vec<Vec<f32>> = grids.iter().map(|g| g.cells.iter().map(|c| c.top).collect()).collect();
            let images: Vec<_> = grids.into_iter().map(|g| render::patch(g, &layer, &light)).collect();
            if let [a, b] = images.as_slice() {
                let (both, width) = beside(a, b, px, px);
                save(&png_path(""), width, px, &both, exposure)?;
                let (mut sum, mut peak) = (0.0f64, 0.0f32);
                let diff: Vec<[f32; 3]> = (0..px * px).map(|k| {
                    // Rows are stored north-up in the picture, south-first in the grid.
                    let cell = (px - 1 - k / px) * px + k % px;
                    let d = tops[1][cell] - tops[0][cell];
                    sum += f64::from(d.abs());
                    peak = peak.max(d.abs());
                    [0.1 + (d / 8.0).clamp(0.0, 1.0), 0.1, 0.1 + (-d / 8.0).clamp(0.0, 1.0)]
                }).collect();
                save(&png_path("-diff"), px, px, &diff, 1.0)?;
                println!("{{\"diff_mean_abs_m\":{:.3},\"diff_max_abs_m\":{peak:.3},\"red\":\"B higher\",\"blue\":\"B lower\"}}", sum / (px * px) as f64);
            } else {
                save(&png_path(""), px, px, &images[0], exposure)?;
            }
        }
        "view" => {
            let (width, height) = opts.pair("size", (960.0, 540.0), 'x')?;
            let (width, height) = (width as usize, height as usize);
            ensure!((64..=2560).contains(&width) && (64..=1440).contains(&height), "view size out of range");
            let cam = render::Camera { x, z, above_m: opts.number("alt", 1.7_f32)?,
                yaw: opts.number("yaw", 0.0_f32)?.to_radians(), pitch: opts.number("pitch", -5.0_f32)?.to_radians(),
                fov_deg: opts.number("fov", 70.0_f32)? };
            let images: Vec<_> = cfgs.iter().map(|cfg| render::view(cfg, &cam, width, height, &light)).collect();
            if let [a, b] = images.as_slice() {
                let (both, total) = beside(a, b, width, height);
                save(&png_path(""), total, height, &both, exposure)?;
            } else {
                save(&png_path(""), width, height, &images[0], exposure)?;
            }
        }
        "globe" => {
            let size = opts.number("size", 900_usize)?;
            ensure!((128..=2048).contains(&size), "globe size out of range");
            let layer = render::GlobeLayer::parse(&opts.text("layer", "shaded"))?;
            let (lon, lat, phase) = (opts.number("lon", 0.0_f64)?, opts.number("lat", 0.0_f64)?, opts.number("phase", 30.0_f32)?);
            let images: Vec<_> = cfgs.iter().map(|cfg| render::globe(cfg, lon, lat, size, &layer, phase)).collect();
            if let [a, b] = images.as_slice() {
                let (both, total) = beside(a, b, size, size);
                save(&png_path(""), total, size, &both, exposure)?;
            } else {
                save(&png_path(""), size, size, &images[0], exposure)?;
            }
        }
        other => bail!("unknown command {other}; see the header of examples/planet_lab/main.rs"),
    }
    if !matches!(command, "where" | "stats" | "find" | "probe") {
        println!("{{\"command\":\"{command}\",\"generators\":\"{label}\",\"at\":[{x:.1},{z:.1}],\"out\":\"{}\",\"render_ms\":{:.0}}}",
            png_path("").display().to_string().replace('\\', "/"), rendered.elapsed().as_secs_f64() * 1000.0);
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first() else {
        bail!("usage: planet_lab <patch|view|globe|stats|where|find|batch> [--option value]...");
    };
    let cache = std::env::var_os("PLANET_LAB_CACHE").map_or_else(|| PathBuf::from("target/planet-lab/geology"), PathBuf::from);
    if std::env::var_os("VOXEL_HISTORY_MYR").is_none() && std::fs::create_dir_all(&cache).is_ok() {
        planetary::configure_geology_cache(cache);
    }
    if command == "batch" {
        let file = args.get(1).context("batch <file>: one command per line")?;
        for (number, line) in std::fs::read_to_string(file)?.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
            run(&words[0], &Options::parse(&words[1..])?).with_context(|| format!("{file}:{}", number + 1))?;
        }
        return Ok(());
    }
    run(command, &Options::parse(&args[1..])?)
}
