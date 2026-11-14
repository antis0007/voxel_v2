//! Texture refiner: renders the game's own surface shaders onto a labelled sheet.
//!
//! Shaders are read from disk on every render, so with `--watch` an edit to any
//! world shader file produces a new PNG in about a second, with no game restart
//! or rebuild. Each tile has four bands: albedo, diffuse lit with the shader's
//! relief normal, and gloss (GGX specular plus sky reflection using the
//! per-texel roughness and reflectance), and highlights alone. Usage: `just art --help`.
mod font;
mod gpu;
mod tiles;

#[cfg(test)]
mod tests;

#[path = "../../app/src/world_shader_files.rs"]
mod world_shader_files;

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const HELP: &str = "art_refiner [options]
  --tiles <list>     preset (stones ground ores wood metals industry kinds) or block names, comma separated [stones]
  --faces <list>     up, side, x; one tile per face [side]
  --zoom <metres>    metres across one tile [1.2]
  --dist <factor>    footprint multiplier, 1 = close, 8 = far; shows distance filtering [1]
  --tile <px>        tile size in pixels [192]
  --cols <n>         tiles per row [4]
  --origin x,y,z     world position of the tile corner in metres [0.3,0.3,0.3]
  --light x,y,z      sun direction for the lit rows [-0.35,0.45,0.82]
  --view x,y,z       eye direction in the face frame: x right, y up, z out of the face [0.35,-0.05,0.94]
  --seed <n>         scene seed, as cam.scene_seed.x [0]
  --out <png>        output path [target/art/refine.png]
  --watch            re-render whenever a shader file changes
  --no-labels        leave captions off";

struct Options {
    tiles: String,
    faces: Vec<u32>,
    zoom: f32,
    dist: f32,
    tile_px: u32,
    cols: u32,
    origin: [f32; 3],
    light: [f32; 3],
    view: [f32; 3],
    seed: u32,
    out: PathBuf,
    watch: bool,
    labels: bool,
}

fn triple(text: &str) -> Result<[f32; 3]> {
    let v: Vec<f32> = text.split(',').map(|p| p.trim().parse()).collect::<Result<_, _>>()?;
    match v[..] {
        [x, y, z] => Ok([x, y, z]),
        _ => bail!("expected x,y,z, got {text:?}"),
    }
}

fn parse() -> Result<Options> {
    let mut o = Options {
        tiles: "stones".into(), faces: vec![1], zoom: 1.2, dist: 1.0, tile_px: 192, cols: 4,
        origin: [0.3; 3], light: [-0.35, 0.45, 0.82], view: [0.35, -0.05, 0.94], seed: 0,
        out: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/art/refine.png"),
        watch: false, labels: true,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().with_context(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--tiles" => o.tiles = value()?,
            "--faces" => {
                o.faces = value()?.split(',').map(|f| match f.trim() {
                    "up" => Ok(0), "side" => Ok(1), "x" => Ok(2), other => bail!("face {other:?}: up, side or x"),
                }).collect::<Result<_>>()?
            }
            "--zoom" => o.zoom = value()?.parse()?,
            "--dist" => o.dist = value()?.parse()?,
            "--tile" => o.tile_px = value()?.parse()?,
            "--cols" => o.cols = value()?.parse::<u32>()?.max(1),
            "--origin" => o.origin = triple(&value()?)?,
            "--light" => o.light = triple(&value()?)?,
            "--view" => o.view = triple(&value()?)?,
            "--seed" => o.seed = value()?.parse()?,
            "--out" => o.out = PathBuf::from(value()?),
            "--watch" => o.watch = true,
            "--no-labels" => o.labels = false,
            _ => bail!("{HELP}"),
        }
    }
    Ok(o)
}

fn shader_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/src/shaders")
}

fn watched() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = world_shader_files::WORLD_SHADER_FILES.iter().map(|f| shader_dir().join(f)).collect();
    files.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("harness.wgsl"));
    files
}

fn stamp() -> Vec<Option<SystemTime>> {
    watched().iter().map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok()).collect()
}

fn compose() -> Result<String> {
    let parts: Vec<String> = watched().iter()
        .map(|f| std::fs::read_to_string(f).with_context(|| f.display().to_string()))
        .collect::<Result<_>>()?;
    let (harness, world) = parts.split_last().context("no shader files")?;
    Ok(format!("{}\n{harness}", world.join("\n")))
}

fn build_scene(o: &Options) -> Result<(gpu::Scene, Vec<String>)> {
    let specs = tiles::resolve(&o.tiles)?;
    let mut scene_tiles = Vec::new();
    let mut names = Vec::new();
    for s in &specs {
        for &face in &o.faces {
            scene_tiles.push(gpu::Tile { kind: s.kind, block: s.block, face, base: s.linear, rough: s.rough, refl: s.refl, metallic: s.metallic });
            let suffix = if o.faces.len() > 1 { ["_UP", "_SIDE", "_X"][face as usize] } else { "" };
            names.push(format!("{}{suffix} K{}", s.name, s.kind));
        }
    }
    if scene_tiles.is_empty() || scene_tiles.len() > 128 {
        bail!("{} tiles; the sheet holds 1 to 128", scene_tiles.len());
    }
    let cols = o.cols.min(scene_tiles.len() as u32);
    let scene = gpu::Scene {
        tile_px: o.tile_px, cols, tiles: scene_tiles, span_m: o.zoom, fp_scale: o.dist,
        ambient: 0.35, sun: 0.85, origin: o.origin, light: o.light, view: o.view, seed: o.seed,
    };
    Ok((scene, names))
}

fn label_origin(index: u32, scene: &gpu::Scene) -> (u32, u32) {
    (index % scene.cols * scene.tile_px + 2,
        index / scene.cols * scene.tile_px * gpu::BANDS + 2)
}

fn render_once(gpu: &gpu::Gpu, o: &Options) -> Result<()> {
    let started = Instant::now();
    let pipeline = gpu.compile(&compose()?)?;
    let compiled = started.elapsed();
    let (scene, names) = build_scene(o)?;
    let mut pixels = gpu.render(&pipeline, &scene)?;
    let (w, h) = scene.size();
    if o.labels {
        for (i, name) in names.iter().enumerate() {
            let (x, y) = label_origin(i as u32, &scene);
            font::label(&mut pixels, w, x, y, 2, name);
        }
    }
    if let Some(dir) = o.out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    image::save_buffer(&o.out, &pixels, w, h, image::ColorType::Rgba8)?;
    println!("{} tiles {w}x{h}  compile {:.0} ms  total {:.0} ms  -> {}", names.len(),
        compiled.as_secs_f32() * 1e3, started.elapsed().as_secs_f32() * 1e3, o.out.display());
    Ok(())
}

fn main() -> Result<()> {
    let o = parse()?;
    let gpu = gpu::Gpu::new()?;
    println!("adapter {}", gpu.adapter);
    let mut last = stamp();
    if let Err(e) = render_once(&gpu, &o) {
        eprintln!("{e:#}");
        if !o.watch {
            std::process::exit(1);
        }
    }
    if !o.watch { return Ok(()); }
    loop {
        std::thread::sleep(Duration::from_millis(120));
        let now = stamp();
        if now != last {
            // Editors write in bursts; let the save settle before reading.
            std::thread::sleep(Duration::from_millis(150));
            last = stamp();
            if let Err(e) = render_once(&gpu, &o) {
                eprintln!("{e:#}");
            }
        }
    }
}
