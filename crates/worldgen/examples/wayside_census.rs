//! Generator 320 wayside finds around a point: counts, kinds, caches and
//! waymarks, plus a voxel side elevation of the first find of each kind.
//! `cargo run -p engine_worldgen --example wayside_census --profile release-fast -- [x z radius gen scale]`
use engine_content::BlockId;
use engine_worldgen::{WorldSampleConfig, WorldType, planetary, sample_block, wayside};
use engine_core::VoxelScalePreset;
use glam::{IVec3, Vec2};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let num = |i: usize, d: f32| args.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let generator = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(wayside::WAYSIDE_GENERATOR);
    let voxel_scale = match args.get(4).map(String::as_str) {
        Some("0.5") => VoxelScalePreset::HalfMeter,
        Some("0.25") => VoxelScalePreset::QuarterMeter,
        _ => VoxelScalePreset::OneMeter,
    };
    let cfg = WorldSampleConfig { planet: None, seed: 2_738_958_700, generator_version: generator, world_type: WorldType::Planet, voxel_scale };
    let spawn = planetary::spawn_point(&cfg);
    let centre = if args.len() >= 2 { Vec2::new(num(0, 0.0), num(1, 0.0)) } else { Vec2::new(spawn.x as f32, spawn.z as f32) };
    let radius = num(2, 200.0);
    let finds = wayside::census(&cfg, centre, radius);
    println!("gen={generator} scale={} centre=({:.0},{:.0}) radius={radius} finds={}", cfg.voxel_meters(), centre.x, centre.y, finds.len());
    for f in &finds {
        println!("  {:6} at ({:.1},{:.1}) d={:5.1} floor={:.1} cache={} waymark={} heading={:.0}",
            f.kind, f.centre.x, f.centre.y, f.centre.distance(centre), f.floor_m, f.cache, f.waymark, f.heading_deg);
        if let Some(door) = f.door {
            let to = door - f.centre;
            println!("         -> door ({:.0},{:.0}) {:.0} m, bearing {:.0}", door.x, door.y, to.length(), to.x.atan2(to.y).to_degrees().rem_euclid(360.0));
        }
    }
    let v = cfg.voxel_meters();
    let mut seen = Vec::new();
    for f in &finds {
        if seen.contains(&f.kind) { continue; }
        seen.push(f.kind);
        // Plan, north up: the highest wayside block in each column within
        // the find's height band; '.' bare ground.
        println!("
{} at ({:.0},{:.0}) plan, {v} m voxels:", f.kind, f.centre.x, f.centre.y);
        let b = |m: f32| (m / v).floor() as i32;
        let (x0, x1, z0, z1) = (b(f.centre.x - 4.5), b(f.centre.x + 4.5), b(f.centre.y - 4.5), b(f.centre.y + 4.5));
        let (y0, y1) = (b(f.floor_m - 1.0), b(f.floor_m + 4.0));
        for z in (z0..=z1).rev() {
            let row: String = (x0..=x1).map(|x| {
                for y in (y0..=y1).rev() {
                    let ch = match sample_block(&cfg, IVec3::new(x, y, z)) {
                        BlockId::Cobblestone => 'c', BlockId::Stone => 'S', BlockId::StoneBricks => 'B',
                        BlockId::Log => 'L', BlockId::Ash => 'a',
                        BlockId::SurveyCache => 'V', BlockId::SalvageCrate => 'X', BlockId::CultivatorCache => 'G',
                        _ => continue,
                    };
                    return ch;
                }
                '.'
            }).collect();
            println!("  {row}");
        }
    }
}
