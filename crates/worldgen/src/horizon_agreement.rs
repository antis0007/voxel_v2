//! How closely the horizon heightfield matches the voxel surface chunks draw,
//! in the band where the horizon's 8 m fine grid meets resident terrain
//! (320 m to 1536 m from the camera). Two independent errors:
//!
//! - source divergence: `distant_surface` against the top solid cell that
//!   `sample_block` produces for the same column. Positive means a feature
//!   above the heightfield (trees, structures); negative means carving
//!   (caves, rivers) that the heightfield does not see.
//! - interpolation: `distant_surface` sampled at knots and interpolated over
//!   the horizon lattice's triangles, against `distant_surface` at the column.
//!   The diagonal matches `render/horizon/fine.rs`: cell corners
//!   a=(i,k) b=(i+1,k) c=(i,k+1) d=(i+1,k+1), triangles (a,c,d) and (a,d,b),
//!   so the split runs from a to d.
//!
//! `horizon_agreement_report` prints the full table (ignored, slow);
//! `horizon_agreement_stays_within_measured_bounds` pins the 8 m numbers.
use super::{CURRENT_GENERATOR_VERSION, WorldSampleConfig, WorldType, distant_surface, sample_block};
use engine_content::BlockId;
use engine_core::VoxelScalePreset;
use glam::IVec3;

const INNER_M: f32 = 320.0;
const OUTER_M: f32 = 1536.0;
/// Cells scanned above and below the heightfield top to find the voxel surface.
const SCAN_VOXELS: i32 = 48;

fn config(world_type: WorldType, voxel_scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig { generator_version: CURRENT_GENERATOR_VERSION, seed: 424242, world_type, voxel_scale }
}

/// Columns (voxels) on a `spacing_m` grid in the annulus around the origin,
/// thinned to at most `limit` by an even stride. All four quadrants are
/// covered, so half the samples have a negative x or z.
fn annulus_columns(cfg: &WorldSampleConfig, spacing_m: f32, limit: usize) -> Vec<(i32, i32)> {
    let m = cfg.voxel_meters();
    let steps = (OUTER_M / spacing_m) as i32;
    let mut columns = Vec::new();
    for kz in -steps..=steps {
        for kx in -steps..=steps {
            let (x, z) = (kx as f32 * spacing_m, kz as f32 * spacing_m);
            let r = (x * x + z * z).sqrt();
            if (INNER_M..=OUTER_M).contains(&r) {
                columns.push(((x / m).floor() as i32, (z / m).floor() as i32));
            }
        }
    }
    let stride = columns.len().div_ceil(limit.max(1)).max(1);
    columns.into_iter().step_by(stride).collect()
}

/// Top non-Air, non-Water cell + 1 near the heightfield top `h`, in voxels.
fn voxel_top(cfg: &WorldSampleConfig, x: i32, z: i32, h: i32) -> Option<i32> {
    ((h - SCAN_VOXELS)..=(h + SCAN_VOXELS)).rev().find_map(|y| {
        let block = sample_block(cfg, IVec3::new(x, y, z));
        (block != BlockId::Air && block != BlockId::Water).then_some(y + 1)
    })
}

/// `distant_surface` interpolated from knots every `step_m` metres over the
/// horizon lattice's triangles (diagonal a to d, see the module doc).
fn interpolated(cfg: &WorldSampleConfig, x: i32, z: i32, step_m: f32) -> f32 {
    let step = step_m / cfg.voxel_meters();
    let knot = |i: i32| (i as f32 * step).floor() as i32;
    let (gx, gz) = (x as f32 / step, z as f32 / step);
    let (i, k) = (gx.floor() as i32, gz.floor() as i32);
    let (x0, x1, z0, z1) = (knot(i), knot(i + 1), knot(k), knot(k + 1));
    let u = (x - x0) as f32 / (x1 - x0).max(1) as f32;
    let v = (z - z0) as f32 / (z1 - z0).max(1) as f32;
    let h = |x: i32, z: i32| distant_surface(cfg, x, z).0;
    let (a, b, c, d) = (h(x0, z0), h(x1, z0), h(x0, z1), h(x1, z1));
    if v >= u { a + v * (c - a) + u * (d - c) } else { a + u * (b - a) + v * (d - b) }
}

fn percentile(values: &mut [f32], q: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f32::total_cmp);
    values[((values.len() - 1) as f32 * q).round() as usize]
}

#[derive(Debug)]
struct Divergence {
    columns: usize,
    exact: f32,
    above: f32,
    below: f32,
    p95: f32,
    max: f32,
}

fn divergence(cfg: &WorldSampleConfig, columns: &[(i32, i32)]) -> Divergence {
    let deltas: Vec<i32> = columns
        .iter()
        .filter_map(|&(x, z)| {
            let h = distant_surface(cfg, x, z).0 as i32;
            voxel_top(cfg, x, z, h).map(|top| top - h)
        })
        .collect();
    let n = deltas.len().max(1) as f32;
    let share = |f: fn(i32) -> bool| deltas.iter().filter(|d| f(**d)).count() as f32 / n;
    let mut magnitude: Vec<f32> = deltas.iter().map(|d| d.unsigned_abs() as f32).collect();
    Divergence {
        columns: deltas.len(),
        exact: share(|d| d == 0),
        above: share(|d| d > 0),
        below: share(|d| d < 0),
        p95: percentile(&mut magnitude, 0.95),
        max: magnitude.iter().copied().fold(0.0, f32::max),
    }
}

/// (p50, p95, max) interpolation error in voxels.
fn interpolation_error(cfg: &WorldSampleConfig, columns: &[(i32, i32)], step_m: f32) -> (f32, f32, f32) {
    let mut errors: Vec<f32> = columns
        .iter()
        .map(|&(x, z)| (distant_surface(cfg, x, z).0 - interpolated(cfg, x, z, step_m)).abs())
        .collect();
    let max = errors.iter().copied().fold(0.0, f32::max);
    (percentile(&mut errors, 0.5), percentile(&mut errors, 0.95), max)
}

#[test]
fn interpolation_is_exact_at_knots_and_on_planes() {
    let cfg = config(WorldType::Hills, VoxelScalePreset::OneMeter);
    for (x, z) in [(0, 0), (-8, 16), (1536, -1528)] {
        assert_eq!(interpolated(&cfg, x, z, 8.0), distant_surface(&cfg, x, z).0, "knot ({x},{z})");
    }
    let flat = config(WorldType::Superflat, VoxelScalePreset::OneMeter);
    for (x, z) in [(3, 5), (-3, -5), (7, -1)] {
        assert_eq!(interpolated(&flat, x, z, 8.0), distant_surface(&flat, x, z).0, "superflat ({x},{z})");
    }
}

#[test]
fn annulus_covers_all_quadrants_within_the_band() {
    let cfg = config(WorldType::Hills, VoxelScalePreset::QuarterMeter);
    let columns = annulus_columns(&cfg, 16.0, 4096);
    assert!(columns.len() <= 4096 && columns.len() > 1000, "{}", columns.len());
    for quadrant in [(1, 1), (-1, 1), (-1, -1), (1, -1)] {
        assert!(columns.iter().any(|&(x, z)| x.signum() == quadrant.0 && z.signum() == quadrant.1));
    }
    let m = cfg.voxel_meters();
    for &(x, z) in &columns {
        let r = ((x as f32 * m).powi(2) + (z as f32 * m).powi(2)).sqrt();
        assert!((INNER_M - 1.0..=OUTER_M + 1.0).contains(&r), "({x},{z}) at {r} m");
    }
}

/// Measured 2026-10-22 on this test's own sample (Hills, seed 424242,
/// OneMeter, 4094 columns): divergence p95 9 voxels, 8 m interpolation p95
/// 1.75 voxels. Bounds are the measurement x1.1 + 0.5 voxel. The full table is
/// in the horizon-representation ADR.
const MEASURED_DIVERGENCE_P95: f32 = 9.0;
const MEASURED_INTERPOLATION_8M_P95: f32 = 1.75;

#[test]
fn horizon_agreement_stays_within_measured_bounds() {
    let cfg = config(WorldType::Hills, VoxelScalePreset::OneMeter);
    let columns = annulus_columns(&cfg, 2.0, 4096);
    let source = divergence(&cfg, &columns);
    let (_, p95, _) = interpolation_error(&cfg, &columns, 8.0);
    assert!(source.p95 <= MEASURED_DIVERGENCE_P95 * 1.1 + 0.5, "{source:?}");
    assert!(p95 <= MEASURED_INTERPOLATION_8M_P95 * 1.1 + 0.5, "interpolation p95 {p95}");
}

#[test]
#[ignore = "slow report; run with --ignored --nocapture --test-threads=1"]
fn horizon_agreement_report() {
    for world_type in [WorldType::Hills, WorldType::Ridged] {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
            let cfg = config(world_type, scale);
            let m = cfg.voxel_meters();
            let columns = annulus_columns(&cfg, 2.0, 16384);
            let s = divergence(&cfg, &columns[..columns.len().min(4096)]);
            println!(
                "{world_type:?} {scale:?} source: columns={} exact={:.3} above={:.3} below={:.3} p95={} max={} (voxels)",
                s.columns, s.exact, s.above, s.below, s.p95, s.max
            );
            for step_m in [8.0, 16.0, 32.0, 64.0] {
                let (p50, p95, max) = interpolation_error(&cfg, &columns, step_m);
                println!(
                    "{world_type:?} {scale:?} interp {step_m:>2} m: p50={p50:.2} p95={p95:.2} max={max:.2} voxels \
                     (p95 {:.2} m)",
                    p95 * m
                );
            }
        }
    }
}
