//! Numbers for one window: relief per scale band, slope distribution, cover
//! census, sampling cost and a determinism hash. Printed as one JSON object so
//! two generators can be compared without looking at a picture.
use crate::field::{self, Grid};
use engine_worldgen::WorldSampleConfig;
use std::collections::BTreeMap;
use std::fmt::Write;

fn percentile(sorted: &[f32], p: f64) -> f32 {
    if sorted.is_empty() { return 0.0; }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

pub fn window(cfg: &WorldSampleConfig, cx: f64, cz: f64, side_m: f64, spacing: f64) -> String {
    let n = ((side_m / spacing).round() as usize).clamp(16, 1536);
    let started = std::time::Instant::now();
    let grid = Grid::build(cfg, cx, cz, spacing, n);
    let build_ms = started.elapsed().as_secs_f64() * 1000.0;
    let land = |i: usize, j: usize| !grid.at(i, j).wet();
    let top = |i: usize, j: usize| f64::from(grid.at(i, j).top);

    let mut slopes = Vec::with_capacity(n * n);
    // Slope per land cell, for breaks of slope between neighbours.
    let mut slope_at = vec![f32::NAN; n * n];
    let (mut low, mut high, mut dry) = (f32::MAX, f32::MIN, 0usize);
    let mut cover: BTreeMap<String, usize> = BTreeMap::new();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for j in 0..n { for i in 0..n {
        let cell = grid.at(i, j);
        hash = (hash ^ u64::from(cell.top.to_bits())).wrapping_mul(0x0100_0000_01b3);
        *cover.entry(if cell.wet() { "Water".to_owned() } else { format!("{:?}", cell.surface) }).or_default() += 1;
        if !land(i, j) { continue; }
        dry += 1;
        low = low.min(cell.top);
        high = high.max(cell.top);
        if i + 1 < n && j + 1 < n && i > 0 && j > 0 {
            let dx = (top(i + 1, j) - top(i - 1, j)) / (2.0 * spacing);
            let dz = (top(i, j + 1) - top(i, j - 1)) / (2.0 * spacing);
            slopes.push(dx.hypot(dz) as f32);
            slope_at[j * n + i] = dx.hypot(dz) as f32;
        }
    }}
    slopes.sort_unstable_by(f32::total_cmp);
    // Risers: land steeper than 1 and 2 (45 and 63 degrees). Breaks: cells
    // whose slope differs from the next cell east or south by more than 1,
    // per hectare: a cliff lip or foot, not a smooth swell.
    let steep = |limit: f32| if slopes.is_empty() { 0.0 }
        else { slopes.iter().filter(|s| **s > limit).count() as f64 / slopes.len() as f64 };
    let mut breaks = 0usize;
    for j in 0..n - 1 { for i in 0..n - 1 {
        let here = slope_at[j * n + i];
        for there in [slope_at[j * n + i + 1], slope_at[(j + 1) * n + i]] {
            if (here - there).abs() > 1.0 { breaks += 1; }
        }
    }}
    let hectares = (n as f64 * spacing).powi(2) / 10_000.0;

    // RMS height left after removing the mean of four neighbours `s` cells
    // away: relief energy near wavelength 4 * s * spacing.
    let mut bands = String::new();
    let mut s = 1usize;
    while s * 4 < n {
        let (mut sum, mut count) = (0.0f64, 0usize);
        for j in (s..n - s).step_by(2) { for i in (s..n - s).step_by(2) {
            if !(land(i, j) && land(i - s, j) && land(i + s, j) && land(i, j - s) && land(i, j + s)) { continue; }
            let r = top(i, j) - 0.25 * (top(i - s, j) + top(i + s, j) + top(i, j - s) + top(i, j + s));
            sum += r * r;
            count += 1;
        }}
        if count > 0 {
            if !bands.is_empty() { bands.push(','); }
            let _ = write!(bands, "\"{}\":{:.4}", 4.0 * s as f64 * spacing, (sum / count as f64).sqrt());
        }
        s *= 2;
    }

    // Single-thread cost per query at three footprints.
    let mut cost = String::new();
    for footprint in [spacing as f32, 16.0, 256.0] {
        let started = std::time::Instant::now();
        let count = 4000usize;
        let mut sink = 0.0f32;
        for k in 0..count {
            let (i, j) = (k * 7919 % n, k * 104_729 % n);
            sink += field::sample(cfg, grid.x0 + i as f64 * spacing + 0.37, grid.z0 + j as f64 * spacing + 0.61, footprint).top;
        }
        std::hint::black_box(sink);
        if !cost.is_empty() { cost.push(','); }
        let _ = write!(cost, "\"{footprint}\":{:.2}", started.elapsed().as_secs_f64() * 1e6 / count as f64);
    }

    let total = (n * n) as f64;
    let census = cover.iter().map(|(name, count)| format!("\"{name}\":{:.4}", *count as f64 / total))
        .collect::<Vec<_>>().join(",");
    format!(
        "{{\"generator\":{},\"seed\":{},\"centre\":[{cx:.1},{cz:.1}],\"side_m\":{:.1},\"spacing_m\":{spacing},\"cells\":{},\
         \"build_ms\":{build_ms:.1},\"us_per_cell_parallel\":{:.3},\"hash\":\"{hash:016x}\",\"land_fraction\":{:.4},\
         \"height_m\":[{low:.2},{high:.2}],\"slope\":{{\"p50\":{:.4},\"p90\":{:.4},\"p99\":{:.4},\"max\":{:.4}}},\
         \"riser_fraction\":{{\"slope_gt_1\":{:.4},\"slope_gt_2\":{:.4}}},\"break_of_slope_per_ha\":{:.1},\
         \"relief_rms_m_by_wavelength_m\":{{{bands}}},\"us_per_query_by_footprint_m\":{{{cost}}},\"cover\":{{{census}}}}}",
        cfg.generator_version, cfg.seed, n as f64 * spacing, n * n, build_ms * 1000.0 / total, dry as f64 / total,
        percentile(&slopes, 0.5), percentile(&slopes, 0.9), percentile(&slopes, 0.99), slopes.last().copied().unwrap_or(0.0),
        steep(1.0), steep(2.0), breaks as f64 / hectares,
    )
}
