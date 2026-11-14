use super::*;

const HEIGHTS: [f64; 3] = [-50.0, 0.0, 120.0];

/// Local coordinates on a face, including the band just past each edge that a
/// crossing point lands in before `from_body` moves it to the neighbour.
fn samples() -> impl Iterator<Item = DVec3> {
    let extent = HALF_M * 1.3;
    (0..=12).flat_map(move |i| {
        (0..=12).flat_map(move |k| {
            let x = -extent + 2.0 * extent * i as f64 / 12.0;
            let z = -extent + 2.0 * extent * k as f64 / 12.0;
            HEIGHTS.map(move |y| DVec3::new(x, y, z))
        })
    })
}

#[test]
fn body_position_keeps_the_requested_height_above_the_sphere() {
    for face in Face::ALL {
        for local in samples() {
            let body = to_body(join(face, local));
            assert!((body.length() - (RADIUS_M + local.y)).abs() < 1e-6, "{face:?} {local}");
        }
    }
}

#[test]
fn from_body_round_trips_inside_each_face() {
    for face in Face::ALL {
        for local in samples().filter(|p| p.x.abs() <= HALF_M && p.z.abs() <= HALF_M) {
            let p = join(face, local);
            let back = from_body(to_body(p));
            assert!((back - p).length() < 1e-3, "{face:?} {local}: {back} vs {p}");
        }
    }
}

#[test]
fn crossing_an_edge_keeps_the_body_position() {
    // The gap past an edge maps to the same sphere point whichever face owns it,
    // so a crossing moves the player without a jump.
    for face in Face::ALL {
        for local in samples() {
            let p = join(face, local);
            let moved = from_body(to_body(p));
            assert!((to_body(moved) - to_body(p)).length() < 1e-3, "{face:?} {local}");
        }
    }
}

#[test]
fn frame_is_orthonormal_on_every_face() {
    for face in Face::ALL {
        for local in samples() {
            let f = frame(join(face, local));
            assert!((f * f.transpose() - DMat3::IDENTITY).to_cols_array().iter().all(|v| v.abs() < 1e-9));
        }
    }
}
