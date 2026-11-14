use super::*;
use crate::{WorldSampleConfig, WorldType};
use engine_core::VoxelScalePreset;
use engine_core::cube_sphere::atlas;


fn at_latitude(axis: &PlanetAxis, lat_deg: f64, lon_deg: f64) -> DVec3 {
    let east = axis.north.any_orthonormal_vector();
    let north_of_east = axis.north.cross(east);
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    (east * lon.cos() + north_of_east * lon.sin()) * lat.cos() + axis.north * lat.sin()
}

fn cfg(generator_version: u32) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version, seed: crate::test_world::seed(), world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter }
}

/// Voxel column under a body direction, skipping the solved spawn face.
fn shell_column(cfg: &WorldSampleConfig, dir: DVec3) -> Option<(i32, i32)> {
    let p = atlas::from_body(dir.normalize() * atlas::RADIUS_M);
    let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
    (!crate::basin::covers(cfg, x, z)).then_some((x, z))
}

#[test]
fn both_poles_are_polar_and_the_equator_is_warm() {
    let axis = PlanetAxis::EARTHLIKE;
    for lon in [0.0, 120.0, 240.0] {
        let north = at(crate::test_world::seed(), &axis, at_latitude(&axis, 89.0, lon), 0.0);
        let south = at(crate::test_world::seed(), &axis, at_latitude(&axis, -89.0, lon), 0.0);
        let equator = at(crate::test_world::seed(), &axis, at_latitude(&axis, 0.0, lon), 0.0);
        assert!(north.annual_c < -8.0 && south.annual_c < -8.0, "{north:?} {south:?}");
        assert!(equator.annual_c > 18.0 && equator.winter_c > 10.0, "{equator:?}");
    }
}

#[test]
fn hemispheres_have_their_coldest_season_half_a_year_apart() {
    let axis = PlanetAxis::EARTHLIKE;
    let coldest = |lat: f64| {
        let dir = at_latitude(&axis, lat, 0.0);
        (0..48)
            .map(|i: i32| (i, MODEL.seasonal_c(&axis, dir, 0.0, f64::from(i) / 48.0)))
            .fold((0, f64::MAX), |a, b| if b.1 < a.1 { b } else { a })
            .0
    };
    assert_eq!((coldest(60.0) - coldest(-60.0)).rem_euclid(48), 24);
    assert!((winter_anomaly(50f64.to_radians()) - winter_anomaly(-50f64.to_radians())).abs() < 1e-9);
}

#[test]
fn height_cools_a_midlatitude_column_into_snow() {
    let axis = PlanetAxis::EARTHLIKE;
    let c = Climate { wet: 0.6, ..at(crate::test_world::seed(), &axis, at_latitude(&axis, 40.0, 30.0), 0.0) };
    assert!(c.lifted(400.0).annual_c < c.lifted(20.0).annual_c - 10.0);
    let (low, _, _) = cover(c, 22.0, 2.0);
    let (high, _, _) = cover(c, 420.0, 2.0);
    assert_eq!(low, BlockId::Grass);
    assert!(matches!(high, BlockId::SnowGrass | BlockId::Ice), "{high:?}");
}

#[test]
fn a_dry_polar_column_is_desert_and_a_snowy_one_is_ice_sheet() {
    let cold = |wet| Climate { annual_c: -20.0, winter_c: -30.0, wet };
    assert_eq!(cover(cold(0.10), 40.0, 2.0).0, BlockId::Gravel);
    assert_eq!(cover(cold(0.50), 40.0, 2.0).0, BlockId::Ice);
}

#[test]
fn polar_seas_freeze_and_warm_seas_stay_open() {
    let axis = PlanetAxis::EARTHLIKE;
    assert!(sea_ice(at(crate::test_world::seed(), &axis, at_latitude(&axis, 85.0, 10.0), 0.0)));
    assert!(sea_ice(at(crate::test_world::seed(), &axis, at_latitude(&axis, -85.0, 10.0), 0.0)));
    assert!(!sea_ice(at(crate::test_world::seed(), &axis, at_latitude(&axis, 10.0, 10.0), 0.0)));
}

#[test]
fn the_top_water_cell_of_a_polar_ocean_is_ice() {
    let cfg = cfg(VERSION);
    let axis = PlanetAxis::EARTHLIKE;
    let mut frozen = 0;
    let mut open = 0;
    for lon in (0..360).step_by(15) {
        for (lat, want) in [(84.0, BlockId::Ice), (-84.0, BlockId::Ice), (5.0, BlockId::Water)] {
            let Some((x, z)) = shell_column(&cfg, at_latitude(&axis, lat, f64::from(lon))) else { continue };
            let column = crate::sample_column(&cfg, x, z);
            if column.height >= cfg.water_level() {
                continue;
            }
            let top = crate::planet::block(&cfg, glam::IVec3::new(x, cfg.water_level(), z), column);
            assert_eq!(top, want, "lat {lat} lon {lon}");
            if want == BlockId::Ice { frozen += 1 } else { open += 1 }
        }
    }
    assert!(frozen > 0 && open > 0, "frozen {frozen} open {open}");
}

#[test]
fn climate_is_continuous_across_a_cube_face_edge() {
    let axis = PlanetAxis::EARTHLIKE;
    for t in [-0.8, -0.3, 0.0, 0.4, 0.9] {
        // The PosX/PosY edge is the line x == y; step across it on the sphere.
        let edge = DVec3::new(1.0, 1.0, t);
        let a = at(crate::test_world::seed(), &axis, (edge + DVec3::new(1e-6, -1e-6, 0.0)).normalize(), 0.2);
        let b = at(crate::test_world::seed(), &axis, (edge + DVec3::new(-1e-6, 1e-6, 0.0)).normalize(), 0.2);
        assert!((a.annual_c - b.annual_c).abs() < 0.01 && (a.wet - b.wet).abs() < 0.001, "{a:?} {b:?}");
        let pa = atlas::from_body(a_dir(edge, 1e-4) * atlas::RADIUS_M);
        let pb = atlas::from_body(a_dir(edge, -1e-4) * atlas::RADIUS_M);
        assert_ne!(atlas::split(pa).0, atlas::split(pb).0, "probe did not cross a face edge");
    }
}

fn a_dir(edge: DVec3, d: f64) -> DVec3 {
    (edge + DVec3::new(d, -d, 0.0)).normalize()
}

#[test]
fn a_rotated_axis_rotates_the_belts() {
    let tilted = PlanetAxis::new(DVec3::X, PlanetAxis::EARTHLIKE.obliquity);
    let pole = at(crate::test_world::seed(), &tilted, DVec3::X, 0.0);
    let equator = at(crate::test_world::seed(), &tilted, DVec3::Y, 0.0);
    assert!(pole.annual_c < -8.0 && equator.annual_c > 18.0, "{pole:?} {equator:?}");
}

#[test]
fn polar_cover_grows_toward_both_poles() {
    let cfg = cfg(VERSION);
    let axis = PlanetAxis::EARTHLIKE;
    let mut cold_share = Vec::new();
    for band in [0.0, 30.0, 55.0, 80.0] {
        let (mut land, mut cold) = (0, 0);
        for sign in [1.0, -1.0] {
            for lon in (0..360).step_by(6) {
                let Some((x, z)) = shell_column(&cfg, at_latitude(&axis, band * sign, f64::from(lon))) else { continue };
                let c = crate::sample_column(&cfg, x, z);
                if c.height <= cfg.water_level() {
                    continue;
                }
                land += 1;
                cold += u32::from(matches!(c.surface, BlockId::Ice | BlockId::SnowGrass | BlockId::Gravel));
            }
        }
        cold_share.push(f64::from(cold) / f64::from(land.max(1)));
    }
    assert!(cold_share[0] < 0.1 && cold_share[3] > 0.8, "{cold_share:?}");
    assert!(cold_share.windows(2).all(|w| w[0] <= w[1] + 0.05), "{cold_share:?}");
}

#[test]
fn the_same_seed_and_version_generate_the_same_columns() {
    let cfg = cfg(VERSION);
    let axis = PlanetAxis::EARTHLIKE;
    let sweep = || {
        (0..40)
            .filter_map(|i| shell_column(&cfg, at_latitude(&axis, f64::from(i) * 4.4 - 88.0, f64::from(i) * 37.0)))
            .map(|(x, z)| {
                let c = crate::sample_column(&cfg, x, z);
                (c.height, c.surface, c.vegetation_density.to_bits())
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(sweep(), sweep());
}

#[test]
fn planets_before_the_gate_keep_open_polar_seas() {
    let cfg = cfg(VERSION - 1);
    let axis = PlanetAxis::EARTHLIKE;
    for lon in (0..360).step_by(30) {
        if let Some((x, z)) = shell_column(&cfg, at_latitude(&axis, 85.0, f64::from(lon))) {
            assert!(!crate::planet::sea_ice_at(&cfg, x, z));
        }
    }
}

/// Unlit equirectangular cover and temperature maps, north at the top: `cargo test -p engine_worldgen dump_planet_climate_maps -- --ignored`.
#[test]
#[ignore]
fn dump_planet_climate_maps() {
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/reviews/planet-climate");
    std::fs::create_dir_all(&out).ok();
    let axis = PlanetAxis::EARTHLIKE;
    let (w, h) = (360usize, 180usize);
    let colour = |b: BlockId, wet: bool| -> [u8; 3] {
        use BlockId::*;
        match (b, wet) {
            (Ice, _) => [235, 242, 250],
            (_, true) => [24, 52, 104],
            (SnowGrass, _) => [205, 212, 222],
            (Grass, _) => [70, 120, 50],
            (Sand | Sandstone, _) => [214, 196, 140],
            (Gravel, _) => [128, 122, 114],
            (Stone | Basalt, _) => [96, 92, 90],
            (Mud | Clay, _) => [110, 92, 64],
            (GroveMoss, _) => [40, 96, 60],
            _ => [200, 0, 200],
        }
    };
    for version in [VERSION - 1, VERSION, crate::planet_ocean::VERSION] {
        let cfg = cfg(version);
        let mut cover_px = Vec::with_capacity(w * h * 3);
        let mut heat_px = Vec::with_capacity(w * h * 3);
        for j in 0..h {
            for i in 0..w {
                let dir = at_latitude(&axis, 89.5 - j as f64, i as f64 - 180.0);
                let p = atlas::from_body(dir * atlas::RADIUS_M);
                let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
                let c = crate::sample_column(&cfg, x, z);
                let sea = c.height < cfg.water_level();
                let frozen = sea && crate::planet::sea_ice_at(&cfg, x, z);
                let mut px = colour(if frozen { BlockId::Ice } else { c.surface }, sea && !frozen);
                if sea && !frozen {
                    // Shelf turquoise to plain navy, as a bathymetric chart reads.
                    let d = (f64::from(cfg.world_y(cfg.water_level()) - cfg.world_y(c.height)) / 70.0).clamp(0.0, 1.0);
                    let lerp = |a: f64, b: f64| (a + (b - a) * d.sqrt()) as u8;
                    px = [lerp(120.0, 18.0), lerp(200.0, 40.0), lerp(190.0, 96.0)];
                }
                cover_px.extend(px);
                let t = at(crate::test_world::seed(), &axis, dir, 0.0).lifted(f64::from(cfg.world_y(c.height)) - 2.0).annual_c;
                let u = ((t + 30.0) / 60.0).clamp(0.0, 1.0);
                heat_px.extend([(u * 255.0) as u8, 40, ((1.0 - u) * 255.0) as u8]);
            }
        }
        for (name, px) in [("cover", &cover_px), ("heat", &heat_px)] {
            let mut file = format!("P6\n{w} {h}\n255\n").into_bytes();
            file.extend_from_slice(px);
            std::fs::write(out.join(format!("{name}-g{version}.ppm")), file).unwrap();
        }
    }
}

/// Column cost per gate: `cargo test --release -p engine_worldgen --lib time_planet_columns -- --ignored --nocapture`.
#[test]
#[ignore]
fn time_planet_columns() {
    let axis = PlanetAxis::EARTHLIKE;
    let dirs: Vec<DVec3> = (0..4000).map(|i| at_latitude(&axis, f64::from(i % 170) - 85.0, f64::from(i) * 0.37)).collect();
    for round in 0..2 {
        for version in [VERSION - 1, VERSION] {
            let cfg = cfg(version);
            let cols: Vec<(i32, i32)> = dirs.iter().filter_map(|d| shell_column(&cfg, *d)).collect();
            let t = std::time::Instant::now();
            let mut sink = 0i64;
            for (x, z) in &cols {
                sink += i64::from(crate::sample_column(&cfg, *x, *z).height);
            }
            let per = t.elapsed().as_secs_f64() * 1e6 / cols.len() as f64;
            let t = std::time::Instant::now();
            for d in &dirs {
                sink += at(crate::test_world::seed(), &axis, *d, 0.0).annual_c as i64;
            }
            let climate = t.elapsed().as_secs_f64() * 1e6 / dirs.len() as f64;
            eprintln!("round {round} g{version}: column {per:.1} us, climate {climate:.2} us ({sink})");
        }
    }
}
