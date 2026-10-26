use super::*;

#[test]
fn gnomonic_area_ratio_is_three_root_three() {
    let d = measure(Projection::Gnomonic, 513);
    assert!((d.area_ratio - 3.0 * 3f64.sqrt()).abs() < 3e-2, "{d:?}");
}

#[test]
fn tangent_area_ratio_is_root_two() {
    let d = measure(Projection::Tangent, 513);
    assert!((d.area_ratio - 2f64.sqrt()).abs() < 1e-2, "{d:?}");
}

#[test]
fn spherified_is_better_than_gnomonic() {
    let g = measure(Projection::Gnomonic, 257);
    let s = measure(Projection::Spherified, 257);
    assert!(s.area_ratio < g.area_ratio && s.max_angle_deviation_deg < g.max_angle_deviation_deg);
}

const ALL: [Projection; 3] = [Projection::Gnomonic, Projection::Tangent, Projection::Spherified];

#[test]
fn every_face_measures_the_same() {
    for p in ALL {
        let reference = measure_face(p, Face::PosY, 65);
        for f in Face::ALL {
            let d = measure_face(p, f, 65);
            assert!((d.area_ratio - reference.area_ratio).abs() < 1e-9, "{p:?} {f:?}");
            assert!((d.max_edge_length_ratio - reference.max_edge_length_ratio).abs() < 1e-9);
            assert!(
                (d.max_angle_deviation_deg - reference.max_angle_deviation_deg).abs() < 1e-9,
                "{p:?} {f:?}"
            );
        }
    }
}

#[test]
fn finer_sampling_approaches_the_limit_from_inside() {
    let limits = [3.0 * 3f64.sqrt(), 2f64.sqrt()];
    for (p, limit) in [Projection::Gnomonic, Projection::Tangent].into_iter().zip(limits) {
        let (coarse, fine) = (measure(p, 129), measure(p, 1025));
        assert!(coarse.area_ratio <= fine.area_ratio, "{p:?}");
        assert!(fine.area_ratio <= limit + 1e-12, "{p:?} {fine:?}");
    }
}

#[test]
fn spherified_has_the_smallest_area_spread_then_tangent_then_gnomonic() {
    let [g, t, s] = ALL.map(|p| measure(p, 513));
    println!("gnomonic {g:?}
tangent {t:?}
spherified {s:?}");
    assert!(s.area_ratio < t.area_ratio && t.area_ratio < g.area_ratio);
}

/// Three faces meet at a cube corner, so the two tangents there are 60 or 120 degrees apart on any map.
#[test]
fn angle_deviation_is_pinned_near_thirty_degrees_by_the_corner() {
    for p in ALL {
        let d = measure(p, 513).max_angle_deviation_deg;
        assert!((29.5..=30.0).contains(&d), "{p:?} {d}");
    }
}
