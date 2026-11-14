//! Every structural cell of the surveyed settlement must join the ground by
//! faces. A roof whose steps only meet at edges reads as floating tiles.
use super::*;
use crate::floating_check;

#[test]
fn settlement_has_no_floating_cells() {
    for scale in [engine_core::VoxelScalePreset::OneMeter, engine_core::VoxelScalePreset::HalfMeter] {
        let cfg = WorldSampleConfig {
            planet: None,
            seed: 424242,
            generator_version: crate::CURRENT_GENERATOR_VERSION,
            world_type: WorldType::Hills,
            voxel_scale: scale,
        };
        let p = plan(&cfg).unwrap();
        assert!(p.hamlet_suitable, "{scale:?}: no hamlet for this seed");
        let v = cfg.voxel_meters();
        let centre = (p.hamlet / v).floor().as_ivec3();
        let half = IVec3::new((40.0 / v) as i32, (16.0 / v) as i32, (40.0 / v) as i32);
        let found = floating_check::islands(&cfg, centre - half, centre + half);
        let local: Vec<_> = found
            .iter()
            .map(|g| {
                let m = (g[0].0.as_vec3() + Vec3::splat(0.5)) * v;
                village_axes(&p, m.x, m.z)
            })
            .collect();
        assert!(
            found.is_empty(),
            "{scale:?}: {} floating islands at village (a, b) {local:?}\n{}",
            found.len(),
            floating_check::describe(&found)
        );
    }
}
