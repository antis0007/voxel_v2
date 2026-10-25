use super::*;
use crate::cube_sphere::{FaceCell, Step};
use glam::IVec3;

const ALL: [Projection; 3] = [Projection::Gnomonic, Projection::Tangent, Projection::Spherified];
const STEPS: [Step; 4] = [Step::PosU, Step::NegU, Step::PosV, Step::NegV];

/// `n x n` points over `[-1, 1]^2`, both ends included exactly.
fn grid(n: i32) -> impl Iterator<Item = (f64, f64)> {
    let at = move |i: i32| -1.0 + 2.0 * f64::from(i) / f64::from(n - 1);
    (0..n).flat_map(move |i| (0..n).map(move |j| (at(i), at(j))))
}

/// Angle between two directions, accurate near zero unlike an `acos` of the dot.
fn angle(a: DVec3, b: DVec3) -> f64 {
    a.cross(b).length().atan2(a.dot(b))
}

#[test]
fn round_trip_is_exact_to_1e12_everywhere_including_edges() {
    let mut worst = [0.0f64; 3];
    for (k, p) in ALL.into_iter().enumerate() {
        for f in Face::ALL {
            for (s, t) in grid(41) {
                let d = p.to_sphere(f, s, t);
                assert!((d.length() - 1.0).abs() < 1e-14, "{p:?} {f:?} {s} {t}");
                let (f2, s2, t2) = p.from_sphere(d);
                let d2 = p.to_sphere(f2, s2, t2);
                assert!(angle(d, d2) < 1e-12, "{p:?} {f:?} {s} {t}");
                if s.abs() < 1.0 && t.abs() < 1.0 {
                    assert_eq!(f2, f, "{p:?} {s} {t}");
                    assert!(
                        (s - s2).abs() < 1e-12 && (t - t2).abs() < 1e-12,
                        "{p:?} {f:?} {s} {t}"
                    );
                    worst[k] = worst[k].max((s - s2).abs()).max((t - t2).abs());
                }
            }
        }
    }
    println!("max interior round-trip error (gnomonic, tangent, spherified): {worst:?}");
}

#[test]
fn face_pick_on_edges_and_corners_is_deterministic() {
    let dirs = [
        DVec3::new(1.0, 1.0, 0.0),
        DVec3::new(-1.0, 1.0, 1.0),
        DVec3::new(0.0, -1.0, -1.0),
        DVec3::ONE,
    ];
    for d in dirs {
        let f = pick_face(d.normalize());
        assert_eq!(f, pick_face(d.normalize()));
        for p in ALL {
            let (_, s, t) = p.from_sphere(d.normalize());
            assert!(s.abs() <= 1.0 && t.abs() <= 1.0, "{p:?} {d}");
        }
    }
    assert_eq!(pick_face(DVec3::new(1.0, 1.0, 0.0).normalize()), Face::PosX);
    assert_eq!(pick_face(DVec3::new(-1.0, 1.0, 1.0).normalize()), Face::NegX);
    assert_eq!(pick_face(DVec3::new(0.0, -1.0, -1.0).normalize()), Face::NegY);
    assert_eq!(pick_face(DVec3::ONE.normalize()), Face::PosX);
    for f in Face::ALL {
        assert_eq!(pick_face(f.basis().n.as_dvec3()), f);
    }
}

#[test]
fn analytic_tangents_match_central_differences() {
    let h = 1e-6;
    for p in ALL {
        for f in Face::ALL {
            for (s, t) in grid(9) {
                let (s, t) = (s.clamp(-1.0 + h, 1.0 - h), t.clamp(-1.0 + h, 1.0 - h));
                let (ds, dt) = p.tangents(f, s, t);
                let ns = (p.to_sphere(f, s + h, t) - p.to_sphere(f, s - h, t)) / (2.0 * h);
                let nt = (p.to_sphere(f, s, t + h) - p.to_sphere(f, s, t - h)) / (2.0 * h);
                assert!((ds - ns).length() < 1e-6, "{p:?} {f:?} {s} {t}: {ds} vs {ns}");
                assert!((dt - nt).length() < 1e-6, "{p:?} {f:?} {s} {t}: {dt} vs {nt}");
            }
        }
    }
}

#[test]
fn neighbouring_faces_agree_on_their_shared_edge() {
    let g = FaceGrid::new(32).unwrap();
    let h = g.half();
    let half_cell = 0.5 / f64::from(h);
    for face in Face::ALL {
        for step in STEPS {
            for i in -h..h {
                let d = step.delta();
                let edge = |k: i32| match k {
                    1 => h - 1,
                    -1 => -h,
                    _ => i,
                };
                let a = FaceCell { face, cell: IVec3::new(edge(d.x), 7, edge(d.y)) };
                let (b, turn) = g.step_cell(a, step);
                assert_ne!(b.face, a.face);
                // Each cell centre sits half a cell inside the shared edge.
                let (sa, ta) = g.cell_face_coords(a.cell);
                let (sa, ta) = (sa + half_cell * f64::from(d.x), ta + half_cell * f64::from(d.y));
                let travel = turn.apply(d);
                let (sb, tb) = g.cell_face_coords(b.cell);
                let (sb, tb) =
                    (sb - half_cell * f64::from(travel.x), tb - half_cell * f64::from(travel.y));
                for p in ALL {
                    let da = p.to_sphere(a.face, sa, ta);
                    let db = p.to_sphere(b.face, sb, tb);
                    assert!(angle(da, db) < 1e-12, "{p:?} {a:?} {step:?} -> {b:?}");
                }
            }
        }
    }
}

#[test]
fn local_frame_is_orthonormal_right_handed_and_radial() {
    for p in ALL {
        for f in Face::ALL {
            for (s, t) in grid(9) {
                let m = local_frame(p, f, s, t);
                let (east, up, north) = (m.x_axis, m.y_axis, m.z_axis);
                assert_eq!(up, p.to_sphere(f, s, t), "{p:?} {f:?} {s} {t}");
                for a in [east, up, north] {
                    assert!((a.length() - 1.0).abs() < 1e-12);
                }
                for (a, b) in [(east, up), (up, north), (north, east)] {
                    assert!(a.dot(b).abs() < 1e-12);
                }
                assert!((m.determinant() - 1.0).abs() < 1e-12, "{p:?} {f:?} {s} {t}");
                // East follows +s and north follows +t, as u and v do on the face.
                let (ds, dt) = p.tangents(f, s, t);
                assert!(east.dot(ds) > 0.0 && north.dot(dt) > 0.0, "{p:?} {f:?} {s} {t}");
            }
        }
    }
}

#[test]
fn cell_face_coords_are_cell_centres() {
    let g = FaceGrid::new(64).unwrap();
    assert_eq!(g.cell_face_coords(IVec3::new(-64, 5, 63)), (-63.5 / 64.0, 63.5 / 64.0));
    assert_eq!(g.cell_face_coords(IVec3::new(0, 0, -1)), (0.5 / 64.0, -0.5 / 64.0));
}
