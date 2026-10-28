use super::planet::{build, Planet, Progress, Stage};
use super::*;

/// The home planet at an eighth of the resolution, so tests stay fast.
const SMALL: PlanetParams = PlanetParams { cells: 128, ..HOME };

fn small(seed: u32) -> Planet {
    build(seed, &SMALL, &Progress::default())
}

fn land_cells(p: &Planet) -> impl Iterator<Item = usize> + '_ {
    (0..p.grid.len()).filter(|&i| p.land[i] && !p.grid.on_border(i))
}

#[test]
fn the_same_seed_solves_the_same_planet_bit_for_bit() {
    let (a, b) = (small(7), small(7));
    assert_eq!(a.checksum, b.checksum);
    assert_ne!(a.checksum, small(8).checksum);
}

#[test]
fn every_land_cell_drains_to_the_sea_and_no_dry_pit_traps_water() {
    let p = small(3);
    let len = p.grid.len();
    for i in land_cells(&p) {
        let mut c = i;
        for _ in 0..len {
            let r = p.receiver[c] as usize;
            if r == c {
                break;
            }
            c = r;
        }
        assert!(!p.land[c] || p.grid.on_border(c), "cell {i} ends at land cell {c}");
        if p.lake[i] > f32::MIN {
            continue;
        }
        let lowest = p.grid.neighbours(i).all(|(j, _)| p.height[j] > p.height[i]);
        assert!(!lowest, "dry pit at cell {i}");
    }
}

#[test]
fn ranges_feed_large_catchments_with_few_isolated_summits() {
    let p = small(11);
    let km2_per_cell = p.grid.cell_m * p.grid.cell_m / 1.0e6;
    let land = land_cells(&p).count() as f64 * km2_per_cell;
    let largest = land_cells(&p).map(|i| p.discharge[i] as f64 / 1.0e6).fold(0.0, f64::max);
    // The noise generator's catchments stayed under 6 kmÂ² (docs/plans/2026-10-24-worldgen-spec-gap.md).
    assert!(largest > 20.0, "largest catchment {largest:.1} kmÂ² of {land:.0} kmÂ² land");

    // Isolated peaks per land kmÂ² on the same 256 m lattice, against generator 65's far field.
    let summits = |h: &[f32], land: &dyn Fn(usize) -> bool| {
        let (mut peaks, mut cells) = (0.0, 0.0);
        for i in 0..p.grid.len() {
            if !land(i) || p.grid.on_border(i) {
                continue;
            }
            cells += 1.0;
            if h[i] > SEA_M as f32 + 60.0 && p.grid.neighbours(i).all(|(j, _)| h[j] < h[i]) {
                peaks += 1.0;
            }
        }
        peaks / (cells * km2_per_cell)
    };
    let new = summits(&p.height, &|i| p.land[i]);
    let cfg = WorldSampleConfig {
        generator_version: 65,
        seed: 11,
        world_type: WorldType::Ridged,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter,
    };
    let old_h: Vec<f32> = (0..p.grid.len())
        .map(|i| {
            let [x, z] = p.grid.at(i);
            crate::landscape::preview_landscape_lod(&cfg, x as i32, z as i32, p.grid.cell_m as f32).height
        })
        .collect();
    let old = summits(&old_h, &|i| old_h[i] > SEA_M as f32);
    eprintln!("summits per kmÂ²: solve {new:.3}, generator 65 {old:.3}; largest catchment {largest:.1} kmÂ²");
    assert!(new * 3.0 < old, "summits per kmÂ²: solve {new:.3}, generator 65 {old:.3}");
}

#[test]
fn spawn_is_temperate_the_north_edge_frozen_and_the_south_edge_warm() {
    let s = climate::sin_latitude(&HOME, 0.0, 0.0);
    assert!((s - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12);
    let north = climate::sin_latitude(&HOME, 0.0, -HOME.face_edge_m * 0.5);
    let south = climate::sin_latitude(&HOME, 0.0, HOME.face_edge_m * 0.5);
    assert!(climate::sea_level_c(north) < PERMAFROST_C);
    assert!((5.0..15.0).contains(&climate::sea_level_c(s)));
    assert!(climate::sea_level_c(south) > 20.0);
}

#[test]
fn spawn_is_dry_land_for_every_seed() {
    for seed in [1, 2, 7, 11, 2738958700] {
        let p = small(seed);
        let h = p.grid.bilinear(&p.height, 0.0, 0.0);
        assert!(h > SEA_M + 1.0, "seed {seed}: origin at {h:.1} m");
        assert!(rivers::lake(&p, 0.0, 0.0).is_none(), "seed {seed}: origin under a lake");
    }
}

#[test]
fn colder_north_holds_permafrost_and_the_south_does_not() {
    let p = small(5);
    let row = |z: usize| (0..p.grid.n).map(move |x| z * p.grid.n + x).filter(|&i| p.land[i]);
    let north: Vec<f32> = row(p.grid.n / 16).map(|i| p.maat[i]).collect();
    let south: Vec<f32> = row(p.grid.n * 15 / 16).map(|i| p.maat[i]).collect();
    if let (Some(n), Some(s)) = (north.iter().cloned().reduce(f32::max), south.iter().cloned().reduce(f32::min)) {
        assert!(n < PERMAFROST_C as f32 && s > 15.0, "north max {n}, south min {s}");
    }
}

#[test]
fn detail_blends_without_a_step_at_tile_seams() {
    let p = small(2);
    for k in -3..3 {
        for z in [-300.0, 40.0, 900.0] {
            let seam = (k as f64 + 0.5) * detail::TILE_M;
            let a = detail::offset(&p, 2, seam - 0.01, z);
            let b = detail::offset(&p, 2, seam + 0.01, z);
            assert!((a - b).abs() < 0.05, "step {a} -> {b} at x={seam}");
        }
    }
}

#[test]
fn detail_never_raises_ground_more_than_a_fifth_of_its_cut() {
    let p = small(9);
    let mut deepest = 0.0f64;
    for i in 0..400 {
        let (x, z) = ((i % 20) as f64 * 97.0 - 900.0, (i / 20) as f64 * 89.0 - 900.0);
        let d = detail::offset(&p, 9, x, z);
        assert!(d <= 0.2 * 36.0 + 1e-6, "raised {d}");
        deepest = deepest.min(d);
    }
    assert!(deepest < -1.0, "no gullies cut: deepest {deepest}");
}

#[test]
fn polygon_ponds_and_troughs_hold_water_below_their_rims() {
    for i in 0..4000 {
        let (x, z) = ((i % 80) as f64 * 0.7, (i / 80) as f64 * 0.7);
        for (maat, wet) in [(-12.0, 0.8), (-3.0, 0.8), (-3.0, 0.2)] {
            let poly = periglacial::polygon(1, x, z, maat, wet);
            // Rims and polygon interiors stay at or above the smooth surface,
            // so water below it is always contained.
            if let Some(w) = poly.water {
                assert!(w <= 0.0, "water above the smooth surface at ({x}, {z})");
            } else {
                assert!(poly.ground >= -2.0 - 1e-9, "unbounded trough at ({x}, {z})");
            }
        }
    }
}

/// Writes a shaded relief map of the home planet to `target/geo-maps/<seed>.ppm`.
#[test]
#[ignore]
fn dump_planet_map() {
    let seed: u32 = std::env::var("GEO_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let t = std::time::Instant::now();
    let p = build(seed, &HOME, &Progress::default());
    eprintln!("solve {:?}", t.elapsed());
    let pct = |mut v: Vec<f32>| {
        v.sort_by(f32::total_cmp);
        [10, 25, 50, 75, 90, 99].map(|q| v[(v.len() - 1) * q / 100])
    };
    let land = |f: &[f32]| (0..f.len()).filter(|&i| p.land[i]).map(|i| f[i]).collect::<Vec<_>>();
    eprintln!("height p10..p99 {:?}", pct(land(&p.height)));
    eprintln!("wet    p10..p99 {:?}", pct(land(&p.wet)));
    eprintln!("maat   p10..p99 {:?}", pct(land(&p.maat)));
    let n = p.grid.n;
    let mut out = format!("P6 {n} {n} 255\n").into_bytes();
    for i in 0..p.grid.len() {
        let (x, z) = (i % n, i / n);
        let h = p.height[i];
        let rgb = if !p.land[i] {
            [30, 60, 110]
        } else if p.lake[i] > f32::MIN {
            [60, 110, 170]
        } else if p.discharge[i] > rivers::SOURCE_AREA_M2 * 3.0 {
            [70, 140, 210]
        } else {
            let e = p.height[(z.saturating_sub(1)) * n + x.saturating_sub(1)];
            let shade = (0.75 + (e - h) / 40.0).clamp(0.25, 1.3);
            let k = ((h - SEA_M as f32) / HOME.relief_m as f32).clamp(0.0, 1.0);
            let base = if p.maat[i] < PERMAFROST_C as f32 { [200.0, 200.0, 190.0] } else {
                let w = p.wet[i];
                [140.0 - 60.0 * w + 90.0 * k, 130.0 + 30.0 * w + 60.0 * k, 80.0 + 90.0 * k]
            };
            base.map(|c: f32| (c * shade).clamp(0.0, 255.0))
                .map(|c| c as u8)
        };
        out.extend_from_slice(&rgb);
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/geo-maps");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{seed}.ppm")), out).unwrap();
}

#[test]
fn progress_reports_every_erosion_step_and_ends_done() {
    let progress = Progress::default();
    build(4, &PlanetParams { cells: 48, ..HOME }, &progress);
    let (done, total) = progress.steps();
    assert_eq!(done, total);
    assert_eq!(progress.stage(), Stage::Done);
    let preview = progress.preview().expect("final preview");
    assert!(preview.wet.is_some() && preview.discharge_km2.is_some());
}
