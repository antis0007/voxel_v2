//! Grove site search: survey ruin-cell centres around spawn for gentle,
//! well-watered woodland ground, best score first. Probes read uncarved
//! terrain (the caller sets `PROBING`).
use super::OUTER_R;
use super::plan::{Plan, layout};
use crate::{WATER_LEVEL_METERS, WorldSampleConfig, landscape};
use engine_core::VoxelScalePreset;
use glam::Vec2;
use std::f32::consts::TAU;

const RING_MIN: f32 = 300.0;
const RING_MAX: f32 = 900.0;
const SCALES: [VoxelScalePreset; 3] =
    [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter];

fn relief(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    let column = crate::sample_column_uncached(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z));
    cfg.world_y(column.height)
}

struct Candidate {
    o: Vec2,
    floor: f32,
    score: f32,
}

fn survey(cfg: &WorldSampleConfig, o: Vec2) -> Option<Candidate> {
    let mut inner = vec![relief(cfg, o.x, o.y)];
    let mut outer = Vec::with_capacity(8);
    for i in 0..8 {
        let a = i as f32 * TAU / 8.0;
        let dir = Vec2::new(a.cos(), a.sin());
        inner.push(relief(cfg, o.x + dir.x * 55.0, o.y + dir.y * 55.0));
        outer.push(relief(cfg, o.x + dir.x * 100.0, o.y + dir.y * 100.0));
    }
    let lo = inner.iter().copied().fold(f32::MAX, f32::min);
    let hi = inner.iter().copied().fold(f32::MIN, f32::max);
    if lo < WATER_LEVEL_METERS + 3.0 || hi - lo > 26.0 || hi > 100.0 {
        return None;
    }
    let biome = landscape::sample(cfg, cfg.blocks_for_height(o.x), cfg.blocks_for_height(o.y));
    use landscape::Biome as Bi;
    if !matches!(biome.biome, Bi::Forest | Bi::Rainforest | Bi::Taiga | Bi::Meadow | Bi::Fen | Bi::Heathland) {
        return None;
    }
    inner.sort_by(f32::total_cmp);
    let floor = inner[inner.len() / 2];
    let shelter = outer.iter().sum::<f32>() / outer.len() as f32 - floor;
    Some(Candidate { o, floor, score: (hi - lo) - shelter.clamp(-6.0, 10.0) * 0.6 - biome.moisture * 10.0 })
}

/// The valley keeps its own ground. Ruins need no query: candidates sit at
/// ruin-cell centres, at least 114 m from any possible ruin. Nothing here may
/// touch the site, homestead, dungeon or Undercrown caches: they read terrain
/// (and each other), so consulting them while this plan is built would either
/// re-enter their caches or freeze uncarved ground into them. Homesteads and
/// wayside shrines yield to the grove instead (`reserves`).
fn unclaimed(cfg: &WorldSampleConfig, o: Vec2) -> bool {
    let step = 16.0;
    let n = (OUTER_R / step) as i32;
    for iz in -n..=n {
        for ix in -n..=n {
            let p = o + Vec2::new(ix as f32, iz as f32) * step;
            // Valley plans are per voxel scale; the grove must clear all of them.
            if (p - o).length() <= OUTER_R
                && SCALES.iter().any(|&voxel_scale| {
                    crate::valley::reserves(&WorldSampleConfig { voxel_scale, ..*cfg }, p.x, p.y)
                })
            {
                return false;
            }
        }
    }
    true
}

pub(super) fn build(cfg: &WorldSampleConfig) -> Option<Plan> {
    let seed = cfg.seed.wrapping_add(35_000);
    // Candidates sit at ruin-cell centres, as far from every ruin as the grid allows.
    let span = crate::sites::region_span(cfg);
    let reach = (RING_MAX / span).ceil() as i32;
    let mut found = Vec::new();
    for rz in -reach..=reach {
        for rx in -reach..=reach {
            let o = Vec2::new(rx as f32 + 0.5, rz as f32 + 0.5) * span;
            if (RING_MIN..RING_MAX).contains(&o.length())
                && let Some(c) = survey(cfg, o)
            {
                found.push(c);
            }
        }
    }
    found.sort_by(|a, b| a.score.total_cmp(&b.score).then(a.o.x.total_cmp(&b.o.x)).then(a.o.y.total_cmp(&b.o.y)));
    let best = found.into_iter().take(8).find(|c| unclaimed(cfg, c.o))?;
    Some(layout(cfg, best.o, best.floor.max(WATER_LEVEL_METERS + 3.5), seed))
}

