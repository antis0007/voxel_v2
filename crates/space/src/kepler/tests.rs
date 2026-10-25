use super::*;
use std::f64::consts::{PI, TAU};

const ECCENTRICITIES: [f64; 9] = [0.0, 0.1, 0.5, 0.8, 0.9, 0.97, 0.99, 0.995, 0.999];

fn samples() -> [Elements; 6] {
    [
        Elements::from_period(86_400.0, 3.986e14, 0.3, 0.4, 1.1, 0.7),
        Elements::from_period(5_400.0, 3.986e14, 0.0, 0.0, 0.0, 0.0),
        Elements::from_period(1_000.0, 1e12, 0.2, 0.0, 0.3, 0.5),
        Elements::from_period(40_000.0, 3.986e14, 0.9, 1.0, 5.0, 2.5),
        Elements::from_period(20_000.0, 1e13, 0.6, 2.6, 3.0, 4.0),
        Elements::from_period(30_000.0, 1e13, 0.95, 1.5, 0.2, 6.0),
    ]
}

fn relative(value: DVec3, reference: DVec3) -> f64 {
    (value - reference).length() / reference.length()
}

fn rel_err(value: f64, reference: f64) -> f64 {
    (value - reference).abs() / reference.abs()
}

fn kepler_residual(ea: f64, e: f64, m: f64) -> f64 {
    let r = (ea - e * ea.sin() - m).rem_euclid(TAU);
    r.min(TAU - r)
}

#[test]
fn kepler_residual_is_tiny_across_eccentricities() {
    for e in [0.0, 0.1, 0.5, 0.9, 0.97, 0.999] {
        for k in 0..=720 {
            let m = k as f64 * TAU / 720.0 - 1e-12 * (k == 720) as i32 as f64;
            let ea = solve_kepler(m, e);
            assert!(ea.is_finite());
            let r = kepler_residual(ea, e, m.rem_euclid(TAU));
            assert!(r < 1e-12, "e={e} m={m} residual={r}");
        }
    }
}

#[test]
fn residual_stays_tiny_on_fine_grid_and_near_the_ends() {
    let edges = [1e-15, 1e-12, 1e-9, 1e-6, 1e-3];
    for e in ECCENTRICITIES {
        let ends = edges
            .iter()
            .flat_map(|d| [*d, PI - d, PI + d, TAU - d])
            .collect::<Vec<_>>();
        let grid = (0..=4000).map(|k| k as f64 * TAU / 4000.0);
        for m in ends.into_iter().chain(grid) {
            let m = m.min(TAU - 1e-15);
            let ea = solve_kepler(m, e);
            assert!((0.0..=TAU).contains(&ea), "e={e} m={m} ea={ea}");
            let r = kepler_residual(ea, e, m);
            assert!(r < 1e-12, "e={e} m={m} residual={r}");
        }
    }
}

#[test]
fn newton_converges_inside_the_iteration_cap_and_bisection_agrees() {
    // Starting from π at e = 0.999 needs 15 of the 16 steps near m = 6e-6.
    let tiny = (0..=400).map(|p| 10f64.powf(-18.0 + p as f64 * 0.04));
    for e in ECCENTRICITIES {
        let grid = (0..=2000).map(|k| k as f64 * PI / 2000.0);
        for m in grid.chain(tiny.clone()) {
            let n = newton(m, e).unwrap_or_else(|| panic!("newton gave up: e={e} m={m}"));
            let b = bisect(m, e);
            assert!(kepler_residual(b, e, m) < 1e-12, "bisect e={e} m={m}");
            assert!((n - b).abs() < 1e-10, "e={e} m={m} newton={n} bisect={b}");
        }
    }
}

#[test]
fn anomaly_is_periodic_and_accepts_negative_input() {
    for e in [0.0, 0.4, 0.9, 0.999] {
        for m in [0.0, 0.3, 1.0, 3.0, 4.5, 6.0] {
            let base = solve_kepler(m, e);
            assert!(
                (solve_kepler(m - TAU, e) - base).abs() < 1e-12,
                "e={e} m={m}"
            );
            assert!(
                (solve_kepler(m + 3.0 * TAU, e) - base).abs() < 1e-9,
                "e={e} m={m}"
            );
        }
    }
}

#[test]
fn special_anomalies_match_their_closed_forms() {
    for e in ECCENTRICITIES {
        assert!((0.0..1e-30).contains(&solve_kepler(0.0, e)), "e={e}");
        assert!((solve_kepler(PI, e) - PI).abs() < 1e-15, "e={e}");
    }
    for m in [0.0, 0.7, 2.0, 3.5, 6.2] {
        assert!((solve_kepler(m, 0.0) - m).abs() < 1e-15);
    }
}

#[test]
fn degenerate_anomalies_stay_in_range_or_give_nan() {
    for e in [0.0, 0.5, 0.999] {
        let ea = solve_kepler(-1e-20, e);
        assert!((0.0..TAU).contains(&ea), "e={e} ea={ea}");
    }
    assert!(solve_kepler(f64::NAN, 0.5).is_nan());
    assert!(solve_kepler(f64::INFINITY, 0.5).is_nan());
}

#[test]
fn state_obeys_vis_viva_and_constant_angular_momentum() {
    let el = Elements::from_period(86_400.0, 3.986e14, 0.3, 0.4, 1.1, 0.7);
    let h0 = {
        let (r, v) = el.state_at(0.0);
        r.cross(v)
    };
    for k in 0..360 {
        let (r, v) = el.state_at(k as f64 * TAU / 360.0);
        let vis = el.mu * (2.0 / r.length() - 1.0 / el.a);
        assert!((v.length_squared() - vis).abs() / vis < 1e-10);
        assert!((r.cross(v) - h0).length() / h0.length() < 1e-10);
    }
}

#[test]
fn every_sample_conserves_energy_and_momentum() {
    for el in samples() {
        let (r0, v0) = el.state_at(0.0);
        let h0 = r0.cross(v0);
        let h_expected = (el.mu * el.a * (1.0 - el.e * el.e)).sqrt();
        assert!(rel_err(h0.length(), h_expected) < 1e-10, "{el:?}");
        for k in 0..180 {
            let (r, v) = el.state_at(0.05 + k as f64 * TAU / 180.0);
            let vis = el.mu * (2.0 / r.length() - 1.0 / el.a);
            assert!(rel_err(v.length_squared(), vis) < 1e-10, "{el:?} k={k}");
            assert!(relative(r.cross(v), h0) < 1e-10, "{el:?} k={k}");
        }
    }
}

#[test]
fn zero_inclination_orbits_spin_about_plus_y() {
    let el = Elements::from_period(1000.0, 1e12, 0.2, 0.0, 0.3, 0.5);
    let (r, v) = el.state_at(1.0);
    let h = r.cross(v).normalize();
    assert!((h - DVec3::Y).length() < 1e-12);
}

#[test]
fn orbit_normal_follows_node_and_inclination() {
    for (i, node, arg_peri) in [
        (0.0, 0.0, 0.0),
        (0.4, 1.1, 0.7),
        (1.0, 5.0, 2.5),
        (PI / 2.0, 2.0, 1.0),
        (2.6, 3.0, 4.0),
        (PI, 0.5, 0.5),
    ] {
        let el = Elements::from_period(10_000.0, 1e13, 0.3, i, node, arg_peri);
        let expected = DVec3::new(i.sin() * node.sin(), i.cos(), i.sin() * node.cos());
        for m in [0.0, 1.0, 2.5, 4.0, 5.9] {
            let (r, v) = el.state_at(m);
            let h = r.cross(v).normalize();
            assert!((h - expected).length() < 1e-12, "i={i} node={node} m={m}");
        }
    }
}

#[test]
fn state_matches_closed_form_in_true_anomaly() {
    for el in samples() {
        let line_of_nodes = DVec3::new(el.node.cos(), 0.0, -el.node.sin());
        let normal = DVec3::new(
            el.i.sin() * el.node.sin(),
            el.i.cos(),
            el.i.sin() * el.node.cos(),
        );
        let along_orbit_at_node = normal.cross(line_of_nodes);
        let peri = el.arg_peri.cos() * line_of_nodes + el.arg_peri.sin() * along_orbit_at_node;
        let quad = -el.arg_peri.sin() * line_of_nodes + el.arg_peri.cos() * along_orbit_at_node;
        let h = (el.mu * el.a * (1.0 - el.e * el.e)).sqrt();
        for k in 0..60 {
            let m = 0.1 + k as f64 * TAU / 60.0;
            let ea = solve_kepler(m, el.e);
            let nu = ((1.0 - el.e * el.e).sqrt() * ea.sin()).atan2(ea.cos() - el.e);
            let radius = el.a * (1.0 - el.e * ea.cos());
            let r_expected = radius * (nu.cos() * peri + nu.sin() * quad);
            let v_expected = el.mu / h * (-nu.sin() * peri + (el.e + nu.cos()) * quad);
            let (r, v) = el.state_at(m);
            assert!(relative(r, r_expected) < 1e-11, "{el:?} k={k}");
            assert!(relative(v, v_expected) < 1e-11, "{el:?} k={k}");
        }
    }
}

#[test]
fn node_rotates_from_plus_x_toward_minus_z() {
    let el = Elements::from_period(10_000.0, 1e13, 0.0, 0.0, PI / 2.0, 0.0);
    let (r, _) = el.state_at(0.0);
    assert!((r / el.a - DVec3::NEG_Z).length() < 1e-12);
}

#[test]
fn inclined_orbit_rises_through_the_plane_at_the_ascending_node() {
    let el = Elements::from_period(10_000.0, 1e13, 0.2, 0.6, 0.9, 0.0);
    let (r, v) = el.state_at(0.0);
    assert!(r.y.abs() < 1e-6 * r.length());
    assert!(v.y > 0.0);
}

#[test]
fn apsides_sit_at_expected_radius() {
    for el in samples() {
        let (peri, _) = el.state_at(0.0);
        let (apo, _) = el.state_at(PI);
        assert!(
            rel_err(peri.length(), el.a * (1.0 - el.e)) < 1e-12,
            "{el:?}"
        );
        assert!(rel_err(apo.length(), el.a * (1.0 + el.e)) < 1e-12, "{el:?}");
        assert!(peri.dot(apo) < 0.0);
    }
}

#[test]
fn velocity_matches_position_derivative() {
    for el in samples() {
        let dm = el.mean_motion() * 1e-3;
        for k in 0..48 {
            let m = 0.05 + k as f64 * TAU / 48.0;
            let (_, v) = el.state_at(m);
            let (ahead, _) = el.state_at(m + dm);
            let (behind, _) = el.state_at(m - dm);
            let numeric = (ahead - behind) / 2e-3;
            assert!(relative(numeric, v) < 1e-6, "{el:?} k={k}");
        }
    }
}

#[test]
fn full_period_returns_to_start() {
    for el in samples() {
        for k in 0..24 {
            let m = 0.05 + k as f64 * TAU / 24.0;
            let (r, v) = el.state_at(m);
            for turns in [-1.0, 1.0, 5.0] {
                let (r2, v2) = el.state_at(m + turns * TAU);
                assert!(relative(r2, r) < 1e-9, "{el:?} k={k} turns={turns}");
                assert!(relative(v2, v) < 1e-9, "{el:?} k={k} turns={turns}");
            }
        }
    }
}

#[test]
fn period_and_mean_motion_agree() {
    for period in [1_000.0, 86_400.0, 3.1558e7, 4.0e9] {
        let el = Elements::from_period(period, 1.327e20, 0.1, 0.0, 0.0, 0.0);
        assert!(
            rel_err(el.mean_motion() * period, TAU) < 1e-12,
            "T={period}"
        );
    }
}

#[test]
fn earth_period_about_the_sun_gives_one_au() {
    let el = Elements::from_period(31_558_149.8, 1.327_124_400_18e20, 0.0167, 0.0, 0.0, 0.0);
    assert!(rel_err(el.a, 1.495_978_707e11) < 1e-5, "a={}", el.a);
}

#[test]
fn scaled_semi_major_axis_keeps_mean_motion_with_cubed_mu() {
    let base = Elements::from_period(86_400.0, 3.986e14, 0.3, 0.4, 1.1, 0.7);
    let s: f64 = 0.37;
    let scaled = Elements {
        a: base.a * s,
        mu: base.mu * s.powi(3),
        ..base
    };
    assert!(rel_err(scaled.mean_motion(), base.mean_motion()) < 1e-12);
}
