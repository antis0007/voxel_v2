//! Top-down PPM of the Anchor district nearest the origin, one pixel per metre.
//!
//! `cargo run --release -p engine_worldgen --example outskirts_preview -- out.ppm [generator] [seed] [nth]`
//! Run once at generator 298 and once at 299 to compare a city with and without its outskirts.
use engine_core::VoxelScalePreset;
use engine_content::BlockId as B;
use engine_worldgen::{WorldSampleConfig, WorldType, ruins, sample_column};
use glam::{IVec3, Vec3};

fn colour(block: B) -> [u8; 3] {
    match block {
        B::Grass => [96, 132, 70],
        B::GroveMoss => [74, 112, 66],
        B::Basalt => [52, 54, 62],
        B::Gravel => [128, 124, 116],
        B::Concrete => [168, 168, 164],
        B::WeatheredBricks => [150, 104, 84],
        B::StoneBricks => [136, 136, 140],
        B::DressedStone => [186, 182, 170],
        B::RustedSteel => [142, 82, 48],
        B::SteelPlate => [118, 130, 142],
        B::Log => [104, 78, 48],
        B::Leaves => [52, 98, 50],
        B::HazardPaint => [222, 190, 60],
        B::GlazedTiles => [90, 150, 170],
        B::SalvageCrate | B::CultivatorCache | B::SurveyCache => [255, 40, 200],
        B::Water => [60, 96, 150],
        B::Sand => [200, 188, 150],
        _ => [110, 110, 100],
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else { eprintln!("usage: outskirts_preview out.ppm [generator] [seed] [nth]"); return };
    let generator_version = args.next().and_then(|s| s.parse().ok()).unwrap_or(engine_voxel::ANCHOR_OUTSKIRTS_GENERATOR);
    let seed = args.next().and_then(|s| s.parse().ok()).unwrap_or(424_242);
    let nth: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cfg = WorldSampleConfig {
        seed, generator_version, world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter, planet: None,
    };
    let mut found = ruins::near(&cfg, Vec3::ZERO, 4000.0);
    found.sort_by(|a, b| a.centre.length_squared().total_cmp(&b.centre.length_squared()));
    let Some(district) = found.get(nth) else { eprintln!("no district"); return };
    let half = 520i32;
    let size = (half * 2) as usize;
    let origin = district.centre.floor().as_ivec2() - half;
    let mut out = format!("P6\n{size} {size}\n255\n").into_bytes();
    let mut stashes = 0;
    for z in 0..size as i32 {
        for x in 0..size as i32 {
            let (wx, wz) = (origin.x + x, origin.y + z);
            let column = sample_column(&cfg, wx, wz);
            let terrain = cfg.world_y(column.height + 1);
            let mut top = (column.surface, 0);
            for y in (column.height - 2..=column.height + 9).rev() {
                let p = IVec3::new(wx, y, wz);
                match ruins::block(&cfg, p, p.as_vec3() + Vec3::splat(0.5), terrain) {
                    Some(B::Air) | None => {}
                    Some(block) => { top = (block, y - column.height); break; }
                }
            }
            if matches!(top.0, B::SalvageCrate | B::CultivatorCache | B::SurveyCache) { stashes += 1; }
            let lift = 1.0 + 0.07 * top.1.clamp(-2, 9) as f32;
            let relief = 0.85 + 0.15 * ((column.height as f32 * 0.35).sin() * 0.5 + 0.5);
            out.extend(colour(top.0).map(|c| (f32::from(c) * lift * relief).min(255.0) as u8));
        }
    }
    if let Err(e) = std::fs::write(&path, out) { eprintln!("{e}"); return; }
    println!("generator={generator_version} centre={:.0},{:.0} kind={:?} caches_on_top={stashes} -> {path}",
        district.centre.x, district.centre.y, district.kind);
}
