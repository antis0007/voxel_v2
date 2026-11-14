//! Location catalogue for native visual review of generated public access.
use engine_worldgen::{WorldSampleConfig, WorldType, ruins};
use glam::Vec3;

fn main() {
    let cfg = WorldSampleConfig {
        seed: 74, generator_version: 272, world_type: WorldType::Planet,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter, planet: None,
    };
    for district in ruins::near(&cfg, Vec3::ZERO, 1800.0) {
        let stations = district.stations();
        println!("district={:?} centre={:.0},{:.0} datum={:.0} stations={}",
            district.kind, district.centre.x, district.centre.y, district.datum, stations.len());
        for station in stations {
            println!("  entrance={:.1?} platform={:.1?}", station.entrance, station.platform);
        }
    }
}
