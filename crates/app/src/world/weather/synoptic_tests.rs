use super::*;
use crate::world::DayNightCycle;

const SEED: u32 = 74;

/// Ground direction of cyclone `i`'s eye on weather day `days`.
fn eye(i: u32, days: f64) -> (DVec3, f32) {
    let (c, strength) = cyclone_state(SEED, i, days);
    (rotate_about_north(c, (days * steering()).rem_euclid(std::f64::consts::TAU)), strength)
}

/// Unit direction at latitude and longitude, radians, in the body frame.
fn at(lat: f64, lon: f64) -> DVec3 {
    let east = DVec3::X;
    let up = NORTH.cross(east).normalize();
    (east * lon.cos() + up * lon.sin()) * lat.cos() + NORTH * lat.sin()
}

/// Area-uniform directions from a golden spiral.
fn sphere(n: u32) -> impl Iterator<Item = DVec3> {
    (0..n).map(move |k| {
        let z = 1.0 - 2.0 * (f64::from(k) + 0.5) / f64::from(n);
        let lon = f64::from(k) * 2.399_963_229_728_653;
        at(z.asin(), lon)
    })
}

#[test]
fn belts_are_wet_at_the_equator_dry_in_the_subtropics_and_wet_on_the_tracks() {
    assert!(zonal(0.0) > 0.1);
    assert!(zonal(DRY_BELT_LAT) < -0.15);
    assert!(zonal(STORM_TRACK_LAT) > 0.15);
    assert!((zonal(0.7) - zonal(-0.7)).abs() < 1e-6, "belts mirror across the equator");
}

#[test]
fn cover_bias_averages_out_over_the_planet() {
    // The day's dial stays the planet mean, so a clear day is clear from orbit too.
    let n = 4096;
    let mean = sphere(n).map(|d| regional(d, 3.25, SEED).cover).sum::<f32>() / n as f32;
    assert!(mean.abs() < 0.04, "mean cover bias {mean}");
}

#[test]
fn weather_is_deterministic_and_bounded_everywhere() {
    for d in sphere(512) {
        let a = regional(d, 12.5, SEED);
        assert_eq!(a, regional(d, 12.5, SEED));
        assert!(a.cover.is_finite() && (-1.0..=1.2).contains(&a.cover), "{a:?}");
        assert!((0.0..=1.0).contains(&a.storm));
    }
}

#[test]
fn both_poles_are_continuous_and_finite() {
    for pole in [NORTH, -NORTH] {
        let centre = regional(pole, 2.0, SEED);
        assert!(centre.cover.is_finite());
        // A ring 30 m from the pole: every longitude agrees with the pole itself.
        let lat = std::f64::consts::FRAC_PI_2 - 30.0 / atlas::RADIUS_M;
        let lat = if pole == NORTH { lat } else { -lat };
        for k in 0..16 {
            let r = regional(at(lat, f64::from(k) * std::f64::consts::TAU / 16.0), 2.0, SEED);
            assert!((r.cover - centre.cover).abs() < 0.02, "pole {pole:?} lon {k}: {} vs {}", r.cover, centre.cover);
        }
    }
}

#[test]
fn longitude_wraps_without_a_seam() {
    for lat in [-1.2, -0.4, 0.0, 0.6, 1.1] {
        let west = regional(at(lat, std::f64::consts::PI - 1e-6), 5.0, SEED);
        let east = regional(at(lat, -std::f64::consts::PI + 1e-6), 5.0, SEED);
        assert!((west.cover - east.cover).abs() < 1e-3, "lat {lat}");
    }
}

#[test]
fn cube_face_seams_read_one_weather() {
    // Chart points on two faces that meet on the sphere get the same controls.
    let base = CloudControls::default();
    for dir in [DVec3::new(1.0, 1.0, 0.3), DVec3::new(-0.2, 1.0, -1.0), DVec3::new(1.0, -0.4, 1.0)] {
        let on_edge = dir.normalize();
        let tangent = on_edge.cross(DVec3::new(0.3, 0.5, 0.8)).normalize();
        let (a, b) = ((on_edge + tangent * 2e-6).normalize(), (on_edge - tangent * 2e-6).normalize());
        let (ca, cb) = (atlas::from_body(a * atlas::RADIUS_M), atlas::from_body(b * atlas::RADIUS_M));
        let ga = controls_at(base, None, DVec2::new(ca.x, ca.z), 7.0, SEED);
        let gb = controls_at(base, None, DVec2::new(cb.x, cb.z), 7.0, SEED);
        assert!((ga.coverage - gb.coverage).abs() < 1e-3, "{dir:?}: {} vs {}", ga.coverage, gb.coverage);
    }
}

#[test]
fn cyclones_live_on_the_tracks_and_fade_before_they_respawn() {
    for i in 0..hills().cyclones {
        let (c, _) = cyclone_state(SEED, i, 0.0);
        let lat = c.dot(NORTH).asin().abs();
        assert!((lat - f64::from(STORM_TRACK_LAT)).abs() < 0.16, "cyclone {i} at latitude {lat}");
    }
    // Strength is continuous through every epoch change over 60 days.
    for i in 0..hills().cyclones {
        let mut last = cyclone_state(SEED, i, 0.0).1;
        for step in 1..=6000 {
            let s = cyclone_state(SEED, i, f64::from(step) * 0.01).1;
            assert!((s - last).abs() < 0.02, "cyclone {i} jumped at step {step}");
            last = s;
        }
    }
}

#[test]
fn storms_drift_east_on_the_steering_flow() {
    let (i, days) = (0..hills().cyclones).map(|i| (i, 1.0)).max_by(|a, b| eye(a.0, a.1).1.total_cmp(&eye(b.0, b.1).1)).unwrap_or((0, 1.0));
    let (start, _) = eye(i, days);
    let (later, _) = eye(i, days + 0.5);
    let lon = |d: DVec3| (d.dot(NORTH.cross(DVec3::X).normalize())).atan2(d.dot(DVec3::X));
    let moved = (lon(later) - lon(start)).rem_euclid(std::f64::consts::TAU);
    assert!((moved - 0.5 * steering()).abs() < 1e-9, "moved {moved}");
    assert!(regional(start, days, SEED).storm > 0.3);
}

#[test]
fn precipitation_changes_only_inside_cyclone_cores() {
    use crate::simulation::storms::precipitation;
    let day_storm = CloudControls { cumulonimbus: 0.02, ..CloudControls::default() };
    let n = 8192;
    let mut raised = 0u32;
    for d in sphere(n) {
        let r = regional(d, 4.0, SEED);
        let c = apply(day_storm, r);
        assert!(c.cumulonimbus >= day_storm.cumulonimbus);
        if r.storm <= day_storm.cumulonimbus {
            assert_eq!(precipitation(c.cumulonimbus), precipitation(day_storm.cumulonimbus));
        }
        if precipitation(c.cumulonimbus) > 0.0 {
            raised += 1;
            // Rain never falls from a thin deck.
            assert!(c.coverage >= c.cumulonimbus * 0.9 - 1e-6);
        }
    }
    let share = raised as f32 / n as f32;
    assert!(share < 0.03, "cyclone rain covers {share} of the planet");
    // A day storm keeps its rain in a dry belt.
    let stormy = CloudControls { cumulonimbus: 0.85, coverage: 0.9, ..CloudControls::default() };
    let dry = apply(stormy, Regional { cover: -0.5, storm: 0.0 });
    assert_eq!(dry.cumulonimbus, 0.85);
    assert!(dry.coverage >= 0.76);
}

#[test]
fn the_weather_clock_is_the_synced_day_counter() {
    // Peers and loaded saves share day index and time of day, not day length or wall time.
    let mut a = DayNightCycle::new(0.3, 480.0);
    let mut b = DayNightCycle::new(0.9, 1_200.0);
    a.set_day_index(41);
    b.set_day_index(41);
    a.set_time_of_day(0.625);
    b.set_time_of_day(0.625);
    assert_eq!(days(&a), days(&b));
    assert_eq!(days(&a), 41.625);
    let p = DVec2::new(5_300.0, -2_100.0);
    let c = CloudControls::default();
    assert_eq!(controls_at(c, None, p, days(&a), SEED), controls_at(c, None, p, days(&b), SEED));
    // Advancing one day moves the weather without a jump at midnight.
    a.set_time_of_day(0.999_99);
    let before = controls_at(c, None, p, days(&a), SEED).coverage;
    a.advance(480.0 * 2e-5);
    assert_eq!(a.day_index(), 42);
    assert!((controls_at(c, None, p, days(&a), SEED).coverage - before).abs() < 1e-3);
}

#[test]
fn long_clocks_stay_smooth() {
    let d = at(0.3, 1.0);
    let a = regional(d, 1.0e6, SEED);
    let b = regional(d, 1.0e6 + 1e-4, SEED);
    assert!(a.cover.is_finite() && (a.cover - b.cover).abs() < 0.01);
}
