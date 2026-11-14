//! Bedrock outcrops on forest and meadow slopes (generator 81, dormant).
//!
//! Older generators grey a slope with a value-noise gravel patch that has no
//! relation to the rock below. Here the stratigraphic bed under the column
//! decides. Steep slopes lose their soil outright; on gentler slopes the bed
//! breaks through only on convex shoulders, where soil sheds, and stands proud
//! as a knob. Either way the surface is the rock a pick finds beneath it.
use crate::landscape::{TectonicSample, sample_stratigraphic_bed, sample_tectonics};
use crate::{BlockId, WorldSampleConfig, smoothstep, value_noise2};

/// First generator with geology-driven outcrops; claimed in GENERATOR_GATES.md.
pub const OUTCROP_GENERATOR: u32 = crate::gates::OUTCROPS_AND_WEAVE_GENERATOR;

/// Height a shoulder knob stands above its soil, in metres (at least one voxel).
pub const LEDGE_M: f32 = 0.5;

/// Radius of the convexity stencil, in metres.
pub const CONVEXITY_RADIUS_M: f32 = 4.0;

/// No bed exposes below this slope, so flatter columns skip the tectonic sample.
const MIN_EXPOSURE_SLOPE: f32 = 0.30;

/// Slope band, above `exposure_slope`, over which exposure goes from shoulders
/// only to the whole slope.
const SHOULDER_BAND: f32 = 0.18;

/// Convexity a shoulder needs at the gentle and steep ends of the band, in
/// metres above the stencil mean. About the 85th percentile of forest slopes.
const SHOULDER_CONVEXITY: (f32, f32) = (0.12, 0.04);

/// Slope where a bed's soil starts to fail. Trial values: crystalline and
/// volcanic rock sheds soil earliest, sandstone weathers back to soil.
fn exposure_slope(bed: BlockId) -> f32 {
    match bed {
        BlockId::Stone => 0.36,
        BlockId::Basalt => 0.38,
        BlockId::Limestone => 0.42,
        BlockId::Sandstone => 0.48,
        _ => 0.56,
    }
}

/// What a slope column's substrate is, from the rock under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ground {
    /// Bare rock: surface and filler are this bed. A knob stands proud.
    Outcrop { bed: BlockId, knob: bool },
    /// Soil over rock; the factor thins vegetation as the soil thins.
    Soil(f32),
}

/// Classifies a column at `height_m` metres with the given slope.
/// `convexity` is the height above the mean of the four stencil neighbours;
/// it is evaluated only inside the shoulder band.
pub fn classify(
    cfg: &WorldSampleConfig,
    wx: f32,
    wz: f32,
    height_m: f32,
    slope: f32,
    convexity: impl FnOnce() -> f32,
) -> Ground {
    if slope < MIN_EXPOSURE_SLOPE {
        return Ground::Soil(1.0);
    }
    let tect = sample_tectonics(wx, wz, cfg.seed, cfg.generator_version);
    classify_with(cfg, &tect, wx, wz, height_m, slope, convexity)
}

fn classify_with(
    cfg: &WorldSampleConfig,
    tect: &TectonicSample,
    wx: f32,
    wz: f32,
    height_m: f32,
    slope: f32,
    convexity: impl FnOnce() -> f32,
) -> Ground {
    let bed = sample_stratigraphic_bed(wx, height_m, wz, cfg.seed, tect, cfg.generator_version);
    // A metre-scale wobble keeps the soil line from tracing slope contours.
    let wobble = (value_noise2(wx / 9.0, wz / 9.0, cfg.seed.wrapping_add(81_131)) - 0.5) * 0.10;
    decide(bed, slope, exposure_slope(bed) + wobble, convexity)
}

fn decide(bed: BlockId, slope: f32, start: f32, convexity: impl FnOnce() -> f32) -> Ground {
    if slope >= start + SHOULDER_BAND {
        return Ground::Outcrop { bed, knob: false };
    }
    if slope >= start {
        let t = (slope - start) / SHOULDER_BAND;
        let need = SHOULDER_CONVEXITY.0 + (SHOULDER_CONVEXITY.1 - SHOULDER_CONVEXITY.0) * t;
        if convexity() > need {
            return Ground::Outcrop { bed, knob: true };
        }
    }
    Ground::Soil(1.0 - 0.6 * smoothstep(start - 0.10, start + SHOULDER_BAND, slope))
}

/// Extra voxels a shoulder knob rises above the landscape height.
pub fn ledge_blocks(cfg: &WorldSampleConfig) -> i32 {
    cfg.blocks_for_span(LEDGE_M).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::landscape::{Biome, sample};
    use crate::{sample_block, sample_column};
    use engine_core::VoxelScalePreset;
    use engine_voxel::WorldType;
    use glam::IVec3;

    const SEEDS: [u32; 3] = [424242, 1, 7777];

    fn cfg(version: u32, seed: u32, scale: VoxelScalePreset) -> WorldSampleConfig {
        WorldSampleConfig { planet: None, generator_version: version, seed, world_type: WorldType::Hills, voxel_scale: scale }
    }

    /// Forest and meadow columns outside the spawn exemption, on a coarse grid.
    fn slope_columns(c: &WorldSampleConfig) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        let step = c.blocks_for_span(23.0).max(1);
        let reach = c.blocks_for_span(3000.0);
        for x in (-reach..reach).step_by(step as usize) {
            for z in (-reach..reach).step_by(step as usize + 6) {
                let s = sample(c, x, z);
                if matches!(s.biome, Biome::Forest | Biome::Meadow)
                    && s.slope > 0.3
                    && c.world_x(x).hypot(c.world_z(z)) > 110.0
                {
                    out.push((x, z));
                }
            }
        }
        out
    }

    /// The same classification `column_from_landscape` makes for this column.
    fn ground(c: &WorldSampleConfig, x: i32, z: i32) -> Ground {
        let s = sample(c, x, z);
        let convexity = || {
            let r = c.blocks_for_span(CONVEXITY_RADIUS_M).max(1);
            let ring = [(r, 0), (-r, 0), (0, r), (0, -r)].map(|(dx, dz)| sample(c, x + dx, z + dz).height);
            s.height - ring.iter().sum::<f32>() / 4.0
        };
        classify(c, c.world_x(x), c.world_z(z), s.height, s.slope, convexity)
    }

    /// True when the structure chain in `sample_block_with` answers for `p`.
    fn structure_owns(c: &WorldSampleConfig, p: IVec3, height: i32) -> bool {
        let m = (p.as_vec3() + glam::Vec3::splat(0.5)) * c.voxel_meters();
        crate::fernwake::sample(c, m).is_some()
            || crate::households::sample(c, m).is_some()
            || crate::meridian_terminal::block(c, p, height).is_some()
            || crate::undercrown::block(c, p, height).is_some()
            || crate::amberroot::block(c, p, height).is_some()
            || crate::sites::block(c, p, height).is_some()
            || crate::sites::cavern(c, p, height).is_some()
    }

    #[test]
    fn lithology_and_shoulders_decide_exposure() {
        let flat = || 0.0;
        let at = exposure_slope;
        // Above the band every bed is bare.
        let steep = at(BlockId::Stone) + SHOULDER_BAND + 0.01;
        assert_eq!(decide(BlockId::Stone, steep, at(BlockId::Stone), flat), Ground::Outcrop { bed: BlockId::Stone, knob: false });
        // The same slope keeps soil over sandstone unless it is a shoulder.
        assert!(matches!(decide(BlockId::Sandstone, steep, at(BlockId::Sandstone), flat), Ground::Soil(_)));
        assert_eq!(
            decide(BlockId::Sandstone, steep, at(BlockId::Sandstone), || 0.2),
            Ground::Outcrop { bed: BlockId::Sandstone, knob: true }
        );
        // Inside the band only a shoulder breaks through; a hollow keeps soil.
        let mid = at(BlockId::Stone) + 0.05;
        assert!(matches!(decide(BlockId::Stone, mid, at(BlockId::Stone), || -0.2), Ground::Soil(_)));
        assert_eq!(decide(BlockId::Stone, mid, at(BlockId::Stone), || 0.2), Ground::Outcrop { bed: BlockId::Stone, knob: true });
        // Below the band the stencil is never sampled.
        let gentle = decide(BlockId::Stone, at(BlockId::Stone) - 0.01, at(BlockId::Stone), || unreachable!());
        assert!(matches!(gentle, Ground::Soil(f) if f > 0.4));
    }

    #[test]
    fn outcrops_show_the_rock_beneath_them() {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
            for seed in SEEDS {
                let c = cfg(OUTCROP_GENERATOR, seed, scale);
                let (mut outcrops, mut knobs, mut voids) = (0, 0, 0);
                for (x, z) in slope_columns(&c) {
                    let col = sample_column(&c, x, z);
                    let Ground::Outcrop { bed, knob } = ground(&c, x, z) else {
                        continue;
                    };
                    if col.surface != bed {
                        continue; // a later pass (bank, lake, snow) owns this column
                    }
                    let top = IVec3::new(x, col.height, z);
                    if structure_owns(&c, top, col.height) {
                        continue; // trails, plots and sites grade their own ground
                    }
                    outcrops += 1;
                    knobs += u32::from(knob);
                    assert_eq!(col.filler, bed, "{seed} {x},{z}");
                    assert_eq!(col.vegetation_density, 0.0, "{seed} {x},{z}");
                    assert_eq!(sample_block(&c, top), bed, "surface voxel at {top}");
                    // Caves and openings may reach the surface; they stay open.
                    match sample_block(&c, top - IVec3::Y) {
                        BlockId::Air | BlockId::Water => voids += 1,
                        under => assert_eq!(under, bed, "voxel under {top}"),
                    }
                    let base = c.clamp_height(c.blocks_for_height(sample(&c, x, z).height));
                    let lift = if knob { ledge_blocks(&c) } else { 0 };
                    assert_eq!(col.height, base + lift, "{seed} {x},{z}");
                }
                assert!(outcrops > 10 && knobs > 3, "seed {seed} {scale:?}: {outcrops} outcrops, {knobs} knobs");
                assert!(voids * 10 < outcrops, "seed {seed} {scale:?}: {voids} of {outcrops} undercut");
            }
        }
    }

    #[test]
    fn grey_ground_does_not_spread_over_forest_slopes() {
        // Native review of the first draft: a whole hillside went grey. Gate 81
        // may move bare ground onto shoulders but not add grey area overall.
        let rock = |b: BlockId| {
            matches!(b, BlockId::Stone | BlockId::Basalt | BlockId::Limestone | BlockId::Sandstone | BlockId::Gravel)
        };
        for seed in SEEDS {
            let old = cfg(OUTCROP_GENERATOR - 1, seed, VoxelScalePreset::OneMeter);
            let new = cfg(OUTCROP_GENERATOR, seed, VoxelScalePreset::OneMeter);
            let (mut before, mut after) = (0u32, 0u32);
            for (x, z) in slope_columns(&old) {
                before += u32::from(rock(sample_column(&old, x, z).surface));
                after += u32::from(rock(sample_column(&new, x, z).surface));
            }
            assert!(after <= before + before / 10, "seed {seed}: grey ground {before} -> {after}");
        }
    }

    #[test]
    fn noise_gravel_patches_are_gone() {
        // Earlier generators grey about a third of 0.36-0.54 slopes with gravel
        // from a 19 m noise patch. From 81 the only slope gravel left is caused:
        // a drainage bank or a fault scarp.
        for seed in SEEDS {
            let old = cfg(OUTCROP_GENERATOR - 1, seed, VoxelScalePreset::OneMeter);
            let new = cfg(OUTCROP_GENERATOR, seed, VoxelScalePreset::OneMeter);
            let (mut old_gravel, mut stray) = (0, Vec::new());
            for (x, z) in slope_columns(&old) {
                let s = sample(&old, x, z);
                let (wx, wz) = (new.world_x(x), new.world_z(z));
                if !(0.36..0.54).contains(&s.slope)
                    || s.height < 20.0
                    || crate::drainage::sample(&new, wx, wz).bank > 0.5
                    || sample_tectonics(wx, wz, seed, new.generator_version).fault_plane > 0.62
                {
                    continue;
                }
                old_gravel += u32::from(sample_column(&old, x, z).surface == BlockId::Gravel);
                if sample_column(&new, x, z).surface == BlockId::Gravel {
                    stray.push((x, z));
                }
            }
            assert!(old_gravel > 20, "seed {seed}: fixture lost its noise patches ({old_gravel})");
            assert!(stray.is_empty(), "seed {seed}: uncaused gravel at {stray:?}");
        }
    }
}


