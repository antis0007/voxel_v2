//! Sections and plans of the elevator earthport as PPM images, three pixels per metre.
//!
//! `cargo run --release -p engine_worldgen --example earthport_section -- out_dir [generator] [seed] [hills|planet]`
//! Run once at generator 300 and once at 301 to compare the monument with and without its earthport.
//! Also prints the member census, the torch count and the sampling cost per voxel.
use engine_content::BlockId as B;
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, meridian_terminal as terminal, sample_block, sample_column};
use glam::{IVec3, Vec3};
use std::collections::BTreeMap;
use std::path::Path;

const HALF: i32 = 132;
const LOW: i32 = -80;
const HIGH: i32 = 264;
const ZOOM: usize = 3;

fn colour(block: B) -> [u8; 3] {
    match block {
        B::Air => [198, 214, 230],
        B::Water => [52, 104, 176],
        B::Grass | B::GroveMoss => [96, 132, 70],
        B::Dirt | B::Mud | B::Clay => [112, 86, 60],
        B::Stone | B::Cobblestone | B::Gravel => [104, 104, 106],
        B::Basalt => [44, 46, 58],
        B::StoneBricks => [140, 140, 146],
        B::WeatheredBricks => [150, 104, 84],
        B::DressedStone => [226, 220, 204],
        B::Concrete => [172, 172, 168],
        B::SteelPlate => [96, 120, 146],
        B::RustedSteel => [150, 84, 46],
        B::CopperBlock => [214, 128, 66],
        B::HazardPaint => [232, 196, 40],
        B::Torch => [255, 60, 220],
        B::Workbench | B::Smelter => [240, 60, 60],
        B::Limestone | B::Sandstone | B::Sand => [190, 180, 150],
        _ => [86, 82, 78],
    }
}

fn write(path: &Path, width: i32, height: i32, at: impl Fn(i32, i32) -> B) {
    let (w, h) = (width as usize * ZOOM, height as usize * ZOOM);
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    let mut row = Vec::with_capacity(w * 3);
    for v in 0..height {
        row.clear();
        for u in 0..width {
            let rgb = colour(at(u, v));
            for _ in 0..ZOOM {
                row.extend_from_slice(&rgb);
            }
        }
        for _ in 0..ZOOM {
            out.extend_from_slice(&row);
        }
    }
    if let Err(error) = std::fs::write(path, out) {
        eprintln!("{}: {error}", path.display());
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(dir) = args.next() else {
        eprintln!("usage: earthport_section out_dir [generator] [seed] [hills|planet]");
        return;
    };
    let generator_version = args.next().and_then(|s| s.parse().ok()).unwrap_or(terminal::earthport::VERSION);
    let seed = args.next().and_then(|s| s.parse().ok()).unwrap_or(74);
    let world_type = match args.next().as_deref() {
        Some("planet") => WorldType::Planet,
        _ => WorldType::Hills,
    };
    let cfg = WorldSampleConfig { seed, generator_version, world_type, voxel_scale: VoxelScalePreset::OneMeter, planet: None };
    let dir = Path::new(&dir);
    if let Err(error) = std::fs::create_dir_all(dir) {
        eprintln!("{}: {error}", dir.display());
        return;
    }
    let datum = terminal::floor(&cfg);
    let origin = Vec3::new(terminal::X, datum, terminal::Z);
    let cell = |local: Vec3| (origin + local).floor().as_ivec3();
    println!("generator {generator_version} seed {seed} datum {datum:.1} m, world floor {:.0} m below it", datum - cfg.world_y(cfg.min_world_y()));

    // Vertical sections along a bearing through the tether axis.
    for (name, degrees) in [("section-000", 0.0_f32), ("section-030", 30.0), ("section-090", 90.0), ("section-160", 160.0)] {
        let (s, c) = degrees.to_radians().sin_cos();
        write(&dir.join(format!("{name}.ppm")), HALF * 2, HIGH - LOW, |u, v| {
            let along = (u - HALF) as f32 + 0.5;
            sample_block(&cfg, cell(Vec3::new(along * c, (HIGH - 1 - v) as f32 + 0.5, along * s)))
        });
    }
    // Plans; +z (the arrival road) is up.
    for (name, y) in [("plan-terrace", 1.5_f32), ("plan-kerbs", 0.5), ("plan-cut", -1.5), ("plan-substation", -20.5), ("plan-gallery", -43.5), ("plan-vault", -62.5)] {
        write(&dir.join(format!("{name}.ppm")), HALF * 2, HALF * 2, |u, v| {
            sample_block(&cfg, cell(Vec3::new((u - HALF) as f32 + 0.5, y, (HALF - 1 - v) as f32 + 0.5)))
        });
    }

    // Census and cost of the terminal sampler alone over its whole box.
    let base = cell(Vec3::new(-(HALF as f32), LOW as f32, -(HALF as f32)));
    let mut census: BTreeMap<String, u64> = BTreeMap::new();
    let mut heights = Vec::with_capacity((HALF * HALF * 4) as usize);
    for z in 0..HALF * 2 {
        for x in 0..HALF * 2 {
            heights.push(sample_column(&cfg, base.x + x, base.z + z).height);
        }
    }
    for z in 0..HALF * 2 {
        for x in 0..HALF * 2 {
            let height = heights[(z * HALF * 2 + x) as usize];
            for y in 0..HIGH - LOW {
                if let Some(block) = terminal::block(&cfg, base + IVec3::new(x, y, z), height)
                    && block != B::Air
                {
                    *census.entry(format!("{block:?}")).or_default() += 1;
                }
            }
        }
    }
    // Interleaved passes, best of three per side: peer builds on this machine skew single runs.
    let monument = WorldSampleConfig { generator_version: terminal::earthport::VERSION - 1, ..cfg };
    let pass = |cfg: &WorldSampleConfig, shift: i32, low: i32, high: i32| {
        let started = std::time::Instant::now();
        let mut solid = 0u64;
        for z in 0..HALF * 2 {
            for x in 0..HALF * 2 {
                let height = heights[(z * HALF * 2 + x) as usize];
                for y in low - LOW..high - LOW {
                    solid += u64::from(terminal::block(cfg, base + IVec3::new(x + shift, y, z), height).is_some());
                }
            }
        }
        std::hint::black_box(solid);
        started.elapsed().as_nanos() as f64 / ((HALF * HALF * 4) as f64 * (high - low) as f64)
    };
    for (label, shift, low, high) in [("below datum", 0, LOW, 0), ("above datum", 0, 0, HIGH), ("1 km away", 1000, LOW, HIGH)] {
        let mut best = [f64::MAX; 2];
        for _ in 0..3 {
            best[0] = best[0].min(pass(&monument, shift, low, high));
            best[1] = best[1].min(pass(&cfg, shift, low, high));
        }
        println!("terminal sampler {label}: monument {:.1} ns/voxel, generator {generator_version} {:.1} ns/voxel", best[0], best[1]);
    }
    for (block, count) in census {
        println!("  {block}: {count}");
    }
}
