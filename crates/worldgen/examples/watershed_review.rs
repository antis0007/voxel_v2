//! Print matched review poses for the current spherical Planet generator.
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, planetary};

fn main() {
    let mut args = std::env::args().skip(1);
    let seed = args.next().map(|v| v.parse().expect("integer seed")).unwrap_or(74);
    let version = engine_voxel::PLANET_GENERATOR_VERSION;
    let cfg = WorldSampleConfig { seed, generator_version: version, planet: None,
        world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter };
    let (a, b, area) = planetary::review::confluence(&cfg).expect("no inland confluence on spawn face");
    println!("seed={seed} reach={:.1},{:.1} downstream={:.1},{:.1} runoff_km2={:.2} generator={version}",
        a.x, a.z, b.x, b.z, area / 1e6);
    let downstream = glam::DVec2::new(b.x - a.x, b.z - a.z).normalize();
    let side = downstream.perp();
    let centre = glam::DVec2::new(a.x, a.z);
    for (name, offset, clearance, pitch) in [
        ("orbit", -downstream * 1800.0, 24_000.0, -1.48),
        ("high", -downstream * 900.0, 1000.0, -0.84),
        ("medium", side * 340.0, 200.0, -0.55),
        ("low", side * 85.0, 20.0, -0.22),
        ("ground", side * 38.0 - downstream * 24.0, 1.7, -0.06),
    ] {
        let at = centre + offset;
        let yaw = (-offset.x).atan2(offset.y);
        println!("{name} camera={:.2},{:.2},{clearance},{yaw:.5},{pitch}", at.x, at.y);
    }
    for footprint in [0.25, 1.0, 32.0, 128.0, 512.0] {
        let g = planetary::ground(&cfg, a.x as f32, a.z as f32, footprint);
        println!("footprint_m={footprint} height_m={:.2} water_m={:?} riparian={:.3}",
            g.top_m, g.water_m, g.riparian);
    }
    if let Some(path) = args.next() {
        let baseline = args.next().map(|v| v.parse().expect("integer baseline generator"));
        hillshade(&cfg, centre, &path, baseline).expect("write terrain hillshade");
    }
}

// Same 4 km extent and illumination in both panels. This compares the
// authoritative height field to filtered samples, not to rendered meshes.
fn hillshade(cfg: &WorldSampleConfig, centre: glam::DVec2, path: &str, baseline: Option<u32>) -> std::io::Result<()> {
    use std::io::{BufWriter, Write};
    const SIDE: usize = 251;
    const STEP: f32 = 16.0;
    let sample = |cfg: &WorldSampleConfig, footprint| -> Vec<f32> {
        (0..SIDE * SIDE).map(|i| {
            let x = centre.x as f32 + (i % SIDE) as f32 * STEP - 2000.0;
            let z = centre.y as f32 + (i / SIDE) as f32 * STEP - 2000.0;
            planetary::ground(cfg, x, z, footprint).top_m
        }).collect()
    };
    let panels = if let Some(generator_version) = baseline {
        [sample(&WorldSampleConfig { generator_version, ..*cfg }, 512.0), sample(cfg, 512.0)]
    } else { [sample(cfg, 1.0), sample(cfg, 512.0)] };
    let mut out = BufWriter::new(std::fs::File::create_new(path)?);
    writeln!(out, "P5\n{} {}\n255", 2 * (SIDE - 2), SIDE - 2)?;
    let light = glam::Vec3::new(-0.55, 0.62, -0.55).normalize();
    for z in 1..SIDE - 1 {
        for heights in &panels {
            for x in 1..SIDE - 1 {
                let i = z * SIDE + x;
                let n = glam::Vec3::new(heights[i - 1] - heights[i + 1], 2.0 * STEP,
                    heights[i - SIDE] - heights[i + SIDE]).normalize();
                let shade = 38.0 + 205.0 * n.dot(light).max(0.0);
                out.write_all(&[shade as u8])?;
            }
        }
    }
    out.flush()
}
