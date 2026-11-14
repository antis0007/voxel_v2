use engine_worldgen::{WorldSampleConfig, WorldType, civilization};
use glam::Vec3;

fn main() {
    let seed = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(74);
    let cfg = WorldSampleConfig { seed, generator_version: 243, world_type: WorldType::Planet,
        planet: None, voxel_scale: engine_core::VoxelScalePreset::OneMeter };
    for s in civilization::settlements_near(&cfg, Vec3::ZERO, 1600.0) {
        let h = s.history;
        println!("{},{},{},{},{},{},{},{},{:?},{:?},{:?},{},{}", s.id,s.center.x,s.center.y,s.center.z,
            h.founded,h.peak_year,h.peak_households,h.households,h.abandoned,h.flood_year,h.rockfall_year,
            h.refugees_received,h.burial.description());
    }
}
