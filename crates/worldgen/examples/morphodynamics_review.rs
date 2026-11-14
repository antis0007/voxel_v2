//! Sample the same generated coastal area at all three supported voxel sizes.
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, geo, planetary};
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let seed = std::env::args().nth(1).map_or(Ok(74), |arg| arg.parse())?;
    let version = std::env::args()
        .nth(2)
        .map_or(Ok(engine_voxel::PLANET_GENERATOR_VERSION), |arg| {
            arg.parse()
        })?;
    let cfg = WorldSampleConfig {
        seed,
        world_type: WorldType::Planet,
        generator_version: version,
        voxel_scale: VoxelScalePreset::OneMeter,
        planet: None,
    };
    let started = Instant::now();
    let Some(centre) = planetary::review::coastal_origin(&cfg) else {
        println!("seed={seed} has no eligible coastal mouth");
        return Ok(());
    };
    let checksum = geo::planet_for(&cfg).checksum;
    if cfg.generator_version == planetary::ESTUARY_OUTLOOK_GENERATOR {
        if let Some([approach, outlook, outlet]) = planetary::review::coastal_outlook(&cfg) {
            for (label, from, to) in [
                ("approach", approach, outlook),
                ("outlook", outlook, outlet),
            ] {
                let heading = to - from;
                let yaw = heading.x.atan2(-heading.z);
                println!(
                    "{label}_camera={:.2},{:.2},0.3,{yaw:.5},-0.10",
                    from.x, from.z
                );
            }
        } else {
            println!("seed={seed} has no dry outlook with an eligible approach");
        }
    }
    println!(
        "seed={seed} generator={} checksum={checksum:016x} coast_x={:.1} coast_z={:.1} load_ms={:.2}",
        cfg.generator_version,
        centre.x,
        centre.z,
        started.elapsed().as_secs_f64() * 1000.0
    );
    for voxel_scale in [
        VoxelScalePreset::OneMeter,
        VoxelScalePreset::HalfMeter,
        VoxelScalePreset::QuarterMeter,
    ] {
        let cfg = WorldSampleConfig { voxel_scale, ..cfg };
        let metres = cfg.voxel_meters();
        let mut terrain_hash = 0xcbf29ce484222325;
        let mut voxel_hash = terrain_hash;
        let mut wet = 0;
        let mut deposited = 0;
        let started = Instant::now();
        for z in -64..=64 {
            for x in -64..=64 {
                let wx = centre.x.round() as f32 + x as f32 * 4.0;
                let wz = centre.z.round() as f32 + z as f32 * 4.0;
                let g = planetary::ground(&cfg, wx, wz, 1.0);
                for value in [g.top_m, g.water_m.unwrap_or(f32::MIN), g.delta, g.delta_mud] {
                    terrain_hash = hash(terrain_hash, u64::from(value.to_bits()));
                }
                wet += usize::from(g.water_m.is_some());
                deposited += usize::from(g.delta > 0.35);
                let (bx, bz) = ((wx / metres) as i32, (wz / metres) as i32);
                let (column, water) = planetary::column_with_water(&cfg, bx, bz, metres);
                for value in [
                    column.height as u64,
                    column.surface as u64,
                    u64::from(water.unwrap_or(f32::MIN).to_bits()),
                ] {
                    voxel_hash = hash(voxel_hash, value);
                }
            }
        }
        println!(
            "voxel_m={metres:.2} physical_hash={terrain_hash:016x} voxel_hash={voxel_hash:016x} wet={wet} deposited={deposited} sample_ms={:.2}",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
    Ok(())
}

fn hash(hash: u64, value: u64) -> u64 {
    (hash ^ value).wrapping_mul(0x100000001b3)
}
