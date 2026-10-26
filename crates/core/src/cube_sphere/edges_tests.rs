use super::*;
use crate::cube_sphere::MAX_HALF;
use glam::IVec2;
use std::collections::BTreeSet;

const STEPS: [Step; 4] = [Step::PosU, Step::NegU, Step::PosV, Step::NegV];

fn step_from_delta(d: IVec2) -> Step {
    match (d.x, d.y) {
        (1, 0) => Step::PosU,
        (-1, 0) => Step::NegU,
        (0, 1) => Step::PosV,
        (0, -1) => Step::NegV,
        _ => panic!("not a unit step: {d}"),
    }
}

/// Every edge cell of every face, with the step that leaves the face.
fn all_boundary_cells(g: FaceGrid) -> impl Iterator<Item = (FaceCell, Step)> {
    let h = g.half();
    Face::ALL.into_iter().flat_map(move |face| {
        (-h..h).flat_map(move |i| {
            let at = |x, z| FaceCell { face, cell: IVec3::new(x, i, z) };
            [
                (at(h - 1, i), Step::PosU),
                (at(-h, i), Step::NegU),
                (at(i, h - 1), Step::PosV),
                (at(i, -h), Step::NegV),
            ]
        })
    })
}

/// The destination-face step pointing opposite to `step` carried through `turn`.
fn reverse_in_destination(step: Step, turn: QuarterTurns) -> Step {
    step_from_delta(-turn.apply(step.delta()))
}

/// The same heading after crossing an edge with `turn`, in destination-face axes.
fn carry(step: Step, turn: QuarterTurns) -> Step {
    step_from_delta(turn.apply(step.delta()))
}

/// Steps `first`, then `second` re-expressed in the destination face's axes.
fn two_steps(g: FaceGrid, at: FaceCell, first: Step, second: Step) -> FaceCell {
    let (mid, turn) = g.step_cell(at, first);
    g.step_cell(mid, carry(second, turn)).0
}

fn edge_cell(g: FaceGrid, face: Face, s: Step) -> FaceCell {
    let h = g.half();
    let (x, z) = match s {
        Step::PosU => (h - 1, 0),
        Step::NegU => (-h, 0),
        Step::PosV => (0, h - 1),
        Step::NegV => (0, -h),
    };
    FaceCell { face, cell: IVec3::new(x, 0, z) }
}

fn opposite(f: Face) -> Face {
    Face::from_normal(-f.basis().n).unwrap()
}

fn neighbour_faces(g: FaceGrid, f: Face) -> BTreeSet<Face> {
    STEPS.into_iter().map(|s| g.step_cell(edge_cell(g, f, s), s).0.face).collect()
}

/// Doubled cube-surface centre, computed independently of the implementation.
fn doubled(g: FaceGrid, c: FaceCell) -> IVec3 {
    let b = c.face.basis();
    2 * g.half() * b.n + (2 * c.cell.x + 1) * b.u + (2 * c.cell.z + 1) * b.v
}

#[test]
fn stepping_off_an_edge_and_back_is_identity() {
    let g = FaceGrid::new(64).unwrap();
    for (start, step) in all_boundary_cells(g) {
        let (there, turn) = g.step_cell(start, step);
        assert_ne!(there.face, start.face);
        assert!(g.contains(there.cell));
        let back_step = reverse_in_destination(step, turn);
        let (home, turn_back) = g.step_cell(there, back_step);
        assert_eq!(home, start);
        assert_eq!(turn.compose(turn_back), QuarterTurns::IDENTITY);
    }
}

#[test]
fn every_face_has_four_distinct_neighbours_and_twelve_edges_total() {
    let g = FaceGrid::new(64).unwrap();
    let mut edges = BTreeSet::new();
    for f in Face::ALL {
        let mut seen = BTreeSet::new();
        for s in STEPS {
            let (to, _) = g.step_cell(edge_cell(g, f, s), s);
            seen.insert(to.face);
            edges.insert(if f < to.face { (f, to.face) } else { (to.face, f) });
        }
        assert_eq!(seen.len(), 4);
        assert!(!seen.contains(&f) && !seen.contains(&opposite(f)));
    }
    assert_eq!(edges.len(), 12);
}

#[test]
fn corner_has_valence_three() {
    let g = FaceGrid::new(64).unwrap();
    for f in Face::ALL {
        for (x, z) in [(63, 63), (63, -64), (-64, 63), (-64, -64)] {
            let corner = FaceCell { face: f, cell: IVec3::new(x, 0, z) };
            let su = if x > 0 { Step::PosU } else { Step::NegU };
            let sv = if z > 0 { Step::PosV } else { Step::NegV };
            let (a, _) = g.step_cell(corner, su);
            let (b, _) = g.step_cell(corner, sv);
            let faces = BTreeSet::from([corner.face, a.face, b.face]);
            assert_eq!(faces.len(), 3, "{f:?} {x} {z}");
            assert!(neighbour_faces(g, a.face).contains(&b.face), "{f:?} {x} {z}");
        }
    }
}

#[test]
fn corner_two_step_orders_land_on_different_faces() {
    let g = FaceGrid::new(64).unwrap();
    let h = g.half();
    for face in Face::ALL {
        for (x, z) in [(h - 1, h - 1), (h - 1, -h), (-h, h - 1), (-h, -h)] {
            let corner = FaceCell { face, cell: IVec3::new(x, 0, z) };
            let su = if x > 0 { Step::PosU } else { Step::NegU };
            let sv = if z > 0 { Step::PosV } else { Step::NegV };
            let (via_u, via_v) = (g.step_cell(corner, su).0.face, g.step_cell(corner, sv).0.face);
            let (uv, vu) = (two_steps(g, corner, su, sv), two_steps(g, corner, sv, su));
            // Each order ends on the face the other order's first step entered.
            assert_eq!(uv.face, via_v, "{face:?} {x} {z}");
            assert_eq!(vu.face, via_u, "{face:?} {x} {z}");
            assert_ne!(uv.face, vu.face, "{face:?} {x} {z}");
            assert_ne!(uv, vu, "{face:?} {x} {z}");
        }
    }
}

#[test]
fn belt_walk_returns_home() {
    let g = FaceGrid::new(64).unwrap();
    let starts = [
        (Face::PosY, IVec3::new(3, 2, -5), Step::PosU),
        (Face::NegX, IVec3::new(-17, 0, 40), Step::PosV),
        (Face::PosZ, IVec3::new(63, 9, 63), Step::NegU),
        (Face::NegY, IVec3::new(-64, -3, -64), Step::NegV),
        (Face::NegZ, IVec3::new(10, 0, -1), Step::PosU),
    ];
    for (face, cell, first) in starts {
        let start = FaceCell { face, cell };
        let (mut at, mut step, mut turned) = (start, first, QuarterTurns::IDENTITY);
        let mut faces = BTreeSet::from([face]);
        // The belt is four faces of 2H cells each.
        for _ in 0..8 * g.half() {
            let (next, turn) = g.step_cell(at, step);
            step = carry(step, turn);
            turned = turned.compose(turn);
            faces.insert(next.face);
            at = next;
        }
        assert_eq!(at, start, "{start:?} {first:?}");
        assert_eq!(step, first, "{start:?} {first:?}");
        assert_eq!(turned, QuarterTurns::IDENTITY, "{start:?} {first:?}");
        assert_eq!(faces.len(), 4, "{start:?} {first:?}");
    }
}

#[test]
fn cell_and_doubled_centre_agree_with_cube_point() {
    let g = FaceGrid::new(64).unwrap();
    for (start, step) in all_boundary_cells(g) {
        let (there, _) = g.step_cell(start, step);
        let (n_a, n_b) = (start.face.basis().n, there.face.basis().n);
        // A doubled unit is half a cell: the centre moves one unit out along
        // the new normal and one unit down the old one.
        assert_eq!(doubled(g, there), doubled(g, start) + n_b - n_a, "{start:?} {step:?}");
    }
}

#[test]
fn large_face_and_height_are_preserved() {
    let g = FaceGrid::new(64_000).unwrap();
    let start = FaceCell { face: Face::NegZ, cell: IVec3::new(63_999, -417, -64_000) };
    let (to, _) = g.step_cell(start, Step::PosU);
    assert_eq!(to.cell.y, -417);
    assert!(g.contains(to.cell));
}

#[test]
fn largest_grid_steps_without_overflow() {
    let half = MAX_HALF - MAX_HALF % CHUNK_SIZE;
    let g = FaceGrid::new(half).unwrap();
    for (start, step) in [
        (IVec3::new(half - 1, 0, half - 1), Step::PosU),
        (IVec3::new(-half, 0, -half), Step::NegV),
    ] {
        let at = FaceCell { face: Face::PosY, cell: start };
        let (to, _) = g.step_cell(at, step);
        assert_ne!(to.face, at.face);
        assert!(g.contains(to.cell));
        assert_eq!(doubled(g, to), doubled(g, at) + to.face.basis().n - at.face.basis().n);
    }
}

#[test]
fn steps_inside_a_face_are_plain_unit_moves() {
    let g = FaceGrid::new(64).unwrap();
    let at = FaceCell { face: Face::PosX, cell: IVec3::new(5, 9, -3) };
    for s in STEPS {
        let (to, turn) = g.step_cell(at, s);
        let d = s.delta();
        assert_eq!(to, FaceCell { face: Face::PosX, cell: at.cell + IVec3::new(d.x, 0, d.y) });
        assert_eq!(turn, QuarterTurns::IDENTITY);
    }
}

#[test]
fn quarter_turns_rotate_u_toward_v_and_compose_mod_four() {
    let q = QuarterTurns::new;
    assert_eq!(q(1).apply(IVec2::X), IVec2::Y);
    assert_eq!(q(2).apply(IVec2::new(3, -1)), IVec2::new(-3, 1));
    assert_eq!(q(3).compose(q(2)), q(1));
}

#[test]
fn quarter_turns_reduce_mod_four_and_invert() {
    for k in 0..8u8 {
        assert_eq!(QuarterTurns::new(k).get(), k % 4);
    }
    assert_eq!(QuarterTurns::new(4), QuarterTurns::new(0));
    assert_eq!(QuarterTurns::new(0), QuarterTurns::IDENTITY);
    for k in 0..4u8 {
        let t = QuarterTurns::new(k);
        assert_eq!(t.compose(t.inverse()), QuarterTurns::IDENTITY, "{t:?}");
        assert_eq!(t.inverse().compose(t), QuarterTurns::IDENTITY, "{t:?}");
    }
}

#[test]
fn chunk_stepping_matches_cell_stepping_at_chunk_corners() {
    let g = FaceGrid::new(4 * CHUNK_SIZE).unwrap();
    let chunks = FaceGrid { half: 4 };
    let last = CHUNK_SIZE - 1;
    for (chunk, step) in all_boundary_cells(chunks) {
        let base = chunk.cell * CHUNK_SIZE;
        // Both corner cells on the edge the step leaves through.
        let corners: [IVec3; 2] = match step {
            Step::PosU => [IVec3::new(last, 0, 0), IVec3::new(last, 0, last)],
            Step::NegU => [IVec3::new(0, 0, 0), IVec3::new(0, 0, last)],
            Step::PosV => [IVec3::new(0, 0, last), IVec3::new(last, 0, last)],
            Step::NegV => [IVec3::new(0, 0, 0), IVec3::new(last, 0, 0)],
        };
        let (to_chunk, chunk_turn) = g.step_chunk(chunk, step);
        for corner in corners {
            let cell = FaceCell { face: chunk.face, cell: base + corner + IVec3::new(0, 7, 0) };
            let (to_cell, cell_turn) = g.step_cell(cell, step);
            let containing = FaceCell {
                face: to_cell.face,
                cell: to_cell.cell.div_euclid(IVec3::splat(CHUNK_SIZE)),
            };
            assert_eq!(to_chunk, containing, "{chunk:?} {step:?} {corner}");
            assert_eq!(chunk_turn, cell_turn, "{chunk:?} {step:?}");
        }
    }
}
