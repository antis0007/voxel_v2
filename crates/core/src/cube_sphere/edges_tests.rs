use super::*;
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
        assert_eq!(turn.compose(turn_back), QuarterTurns(0));
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
fn cell_and_doubled_centre_agree_with_cube_point() {
    let g = FaceGrid::new(64).unwrap();
    let two_h = 2 * g.half();
    for (start, step) in all_boundary_cells(g) {
        let (there, _) = g.step_cell(start, step);
        let (n_a, n_b) = (start.face.basis().n, there.face.basis().n);
        let p = doubled(g, start);
        let p2 = doubled(g, there);
        // Folding about the shared edge moves P by its distance to that edge along both normals.
        let d = two_h - p.dot(n_b);
        assert_eq!(d, 1, "{start:?} {step:?}");
        let folded = p + d * n_b + d * n_a;
        assert_eq!((folded - p2).abs().max_element(), 2, "{start:?} {step:?}");
        assert_eq!(p2.dot(n_b), two_h, "{start:?} {step:?}");
        assert_eq!(p2.abs().max_element(), two_h, "{start:?} {step:?}");
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
fn steps_inside_a_face_are_plain_unit_moves() {
    let g = FaceGrid::new(64).unwrap();
    let at = FaceCell { face: Face::PosX, cell: IVec3::new(5, 9, -3) };
    for s in STEPS {
        let (to, turn) = g.step_cell(at, s);
        let d = s.delta();
        assert_eq!(to, FaceCell { face: Face::PosX, cell: at.cell + IVec3::new(d.x, 0, d.y) });
        assert_eq!(turn, QuarterTurns(0));
    }
}

#[test]
fn quarter_turns_rotate_u_toward_v_and_compose_mod_four() {
    assert_eq!(QuarterTurns(1).apply(IVec2::X), IVec2::Y);
    assert_eq!(QuarterTurns(2).apply(IVec2::new(3, -1)), IVec2::new(-3, 1));
    assert_eq!(QuarterTurns(3).compose(QuarterTurns(2)), QuarterTurns(1));
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
