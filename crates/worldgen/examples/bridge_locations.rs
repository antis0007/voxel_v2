//! Print an actual generated crossing camera; no world edits.
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, surface};
fn main() {
    let seed=std::env::args().nth(1).and_then(|s|s.parse().ok()).unwrap_or(74);
    let cfg=WorldSampleConfig { seed,generator_version:62,world_type:WorldType::Hills,voxel_scale:VoxelScalePreset::HalfMeter };
    println!("seed={seed} bridge={:?}",surface::bridge_review_view(&cfg));
}
