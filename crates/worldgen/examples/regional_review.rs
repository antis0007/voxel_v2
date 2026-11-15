use std::{fmt::Write, fs, path::Path};

use engine_core::VoxelScalePreset;
use engine_voxel::{WorldSampleConfig, WorldType};
use engine_worldgen::{civilization::regional, sample_column};
use glam::{IVec3, Vec2, Vec3};

const SIZE: i32 = 128;
const SEED: u32 = 424242;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let output = args.next().ok_or_else(|| anyhow::anyhow!("usage: regional_review <output directory>"))?;
    fs::create_dir_all(&output)?;
    for (label, kind, previous) in [
        ("hills", WorldType::Hills, 241),
        ("ridged", WorldType::Ridged, 145),
        ("planet", WorldType::Planet, 290),
    ] {
        let mut cfg = WorldSampleConfig {
            world_type: kind, generator_version: previous, seed: SEED,
            voxel_scale: VoxelScalePreset::OneMeter, planet: None,
        };
        let origin = if kind == WorldType::Planet {
            let at = engine_worldgen::planetary::spawn_point(&cfg);
            Vec2::new(at.x as f32, at.z as f32)
        } else {
            Vec2::ZERO
        };
        for version in [previous, engine_voxel::SHARED_SURFACE_GENERATOR, engine_voxel::REGIONAL_CIVILIZATION_GENERATOR] {
            cfg.generator_version = version;
            let path = Path::new(&output).join(format!("{label}-{version}.svg"));
            capture(&cfg, &path, origin, label)?;
        }
    }
    Ok(())
}

fn capture(cfg: &WorldSampleConfig, path: &Path, origin: Vec2, label: &str) -> anyhow::Result<()> {
    let step = regional::RADIUS_M * 2.0 / SIZE as f32;
    let project = |p: Vec2| (p - origin) / step + Vec2::splat(SIZE as f32 / 2.0);
    let mut svg = String::from("<svg xmlns='http://www.w3.org/2000/svg' width='768' height='816' viewBox='0 0 128 136'><rect width='128' height='136' fill='#172124'/><svg width='128' height='128'>");
    for z in 0..SIZE {
        for x in 0..SIZE {
            let at = origin + Vec2::new(x as f32 - 63.5, z as f32 - 63.5) * step;
            let column = sample_column(cfg, at.x.floor() as i32, at.y.floor() as i32);
            let rgb = if column.height < cfg.water_level() {
                [25, 65, 106]
            } else {
                column.surface.color(IVec3::ZERO).map(|c| (c * 255.0) as u8)
            };
            writeln!(svg, "<rect x='{x}' y='{z}' width='1' height='1' fill='#{:02x}{:02x}{:02x}'/>", rgb[0], rgb[1], rgb[2])?;
        }
    }
    let survey = regional::survey(cfg, Vec3::new(origin.x, 0.0, origin.y));
    for road in &survey.roads {
        svg.push_str("<polyline fill='none' stroke='#deb380' stroke-width='.35' points='");
        for &at in road {
            let p = project(at);
            write!(svg, "{:.2},{:.2} ", p.x, p.y)?;
        }
        svg.push_str("'/>");
    }
    for &city in &survey.cities {
        let p = project(city);
        writeln!(svg, "<rect x='{:.2}' y='{:.2}' width='2' height='2' fill='none' stroke='#ffb94d' stroke-width='.5'/>", p.x - 1.0, p.y - 1.0)?;
    }
    for settlement in &survey.settlements {
        let p = project(Vec2::new(settlement.center.x, settlement.center.z));
        let colour = if settlement.history.households > 0 { "#9fffbd" } else { "#c99c6e" };
        writeln!(svg, "<circle cx='{:.2}' cy='{:.2}' r='.7' fill='{colour}'/>", p.x, p.y)?;
    }
    let living = survey.settlements.iter().filter(|site| site.history.households > 0).count();
    writeln!(svg, "</svg><text x='3' y='133' fill='white' font-size='2.5' font-family='sans-serif'>{label} / {} / seed {SEED} / {living} living, {} roads, {} city ruins</text></svg>", cfg.generator_version, survey.roads.len(), survey.cities.len())?;
    fs::write(path, svg)?;
    println!("{}  living={living} roads={} city_ruins={}", path.display(), survey.roads.len(), survey.cities.len());
    Ok(())
}
