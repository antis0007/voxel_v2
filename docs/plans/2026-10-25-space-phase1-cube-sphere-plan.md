# Space phase 1: cube-sphere coordinates, implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A pure, exhaustively tested cube-sphere coordinate module in `engine_core` that every later planet system (worldgen, CA, meshing, rendering, space handoff) builds on.

**Architecture:** `engine_core::cube_sphere` holds six faces with integer right-handed bases, cell and chunk stepping across face edges derived from those bases (no hand-written tables, unlike SoA's commented-out ones), three face-to-sphere projections with exact f64 inverses and analytic Jacobians, and a distortion measurement that picks the projection by numbers. Topology is model R (decision 0025): columns carry straight across an edge, height never changes face, corners have valence 3.

**Tech Stack:** Rust 2024, glam 0.32 (`IVec3`, `DVec3`, `DMat3`), criterion via the existing `just bench` setup.

**Spec:** [space track design](2026-10-24-soa-space-port-design.md) section 4.3 and phase 1 of section 6; [decision 0025](../decisions/0025-space-track-soa-port.md) (model R, corners treated as solid, worldgen places corners under ocean or ice).

## Global Constraints

- Phase 1 exit: exhaustive edge tests; area-ratio table for each candidate; round-trip error bounds.
- Model R: unfolded column topology, `y` is height along the column and never crosses a face; corner valence 3.
- Neighbour and rotation tables are derived from the face bases, never hand-written.
- Face pick on the inverse is exact (largest absolute component, deterministic tie order), in f64; no epsilon band.
- The `PosY` face basis equals today's flat world axes (`u = +X`, `n = +Y`, `v = +Z`), so a one-face planet near the top centre reads like the current world.
- `engine_core` stays a leaf crate: no new dependency (glam and serde are already there).
- No `unwrap`/`expect` outside tests; files under 300 lines (split into `cube_sphere/` submodules); doc comments open with what the item does in under 12 words; comments state why or an invariant; no em-dash asides; banned words: seamless, robust, leverage, delve, elevate, comprehensive, crucial, ensure, streamline, powerful, elegant.
- No proper nouns: faces are named by axis (`PosX` ... `NegZ`).

## Review Focus

1. Cells exactly on the last row/column of a face (`x = H-1`, `z = -H`): stepping off and back returns the same cell and identity rotation (Task 2 round-trip over every boundary cell of a small face).
2. Corners: three faces meet; stepping `+u` then `+v` vs `+v` then `+u` from a corner cell lands on different faces, and neither panics (Task 2 corner test).
3. Directions exactly on cube edges and corners (`|x| == |y|`, `|x| == |y| == |z|`) in the inverse: deterministic face pick and in-range `s, t` (Task 3 tie test).
4. Negative and large coordinates: `H = 64_000` (32 km face at 0.25 m) with cells near both ends of the range (Task 2 large-H test).
5. Chunk stepping where `H` is not a multiple of `CHUNK_SIZE`: rejected at construction, not silently wrong (Task 2).

---

### Task 1: Faces, bases and face cells

**Files:**
- Create: `crates/core/src/cube_sphere/mod.rs` (declare `pub mod cube_sphere;` in `crates/core/src/lib.rs`; add one line to the coordinate-spaces list in the lib.rs doc comment)
- Test: same file, `#[cfg(test)] mod tests`

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)] #[repr(u8)] pub enum Face { PosX, NegX, PosY, NegY, PosZ, NegZ }` with `pub const ALL: [Face; 6]` in that order.
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct FaceBasis { pub u: IVec3, pub n: IVec3, pub v: IVec3 }` and `impl Face { pub const fn basis(self) -> FaceBasis; pub fn from_normal(n: IVec3) -> Option<Face>; }`.
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct FaceCell { pub face: Face, pub cell: IVec3 }` (`cell.x` along `u`, `cell.y` height along `n`, `cell.z` along `v`).
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct FaceGrid { half: i32 }` with `pub fn new(half_cells: i32) -> Option<FaceGrid>` (`None` unless `half_cells > 0` and a multiple of `CHUNK_SIZE`), `pub fn half(self) -> i32`, `pub fn contains(self, cell: IVec3) -> bool` (`-half <= x,z < half`, any `y`).

Bases are right-handed (`u × n == v`, matching world `x × y == z`). `PosY` is `u=(1,0,0) n=(0,1,0) v=(0,0,1)`; every other basis is the image of `PosY`'s under a proper rotation (determinant +1) that maps `(0,1,0)` to that face's normal. The builder picks those rotations; the tests pin the properties, not a table.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn bases_are_right_handed_unit_and_distinct() {
    let mut normals = std::collections::BTreeSet::new();
    for f in Face::ALL {
        let b = f.basis();
        assert_eq!(b.u.cross(b.n), b.v, "{f:?}");
        for a in [b.u, b.n, b.v] { assert_eq!(a.abs().element_sum(), 1, "{f:?}"); }
        assert_eq!(Face::from_normal(b.n), Some(f));
        normals.insert((b.n.x, b.n.y, b.n.z));
    }
    assert_eq!(normals.len(), 6);
}

#[test]
fn top_face_matches_flat_world_axes() {
    let b = Face::PosY.basis();
    assert_eq!((b.u, b.n, b.v), (IVec3::X, IVec3::Y, IVec3::Z));
}

#[test]
fn face_grid_rejects_half_not_multiple_of_chunk() {
    assert!(FaceGrid::new(16_000).is_some());
    assert!(FaceGrid::new(16_001).is_none());
    assert!(FaceGrid::new(0).is_none());
    let g = FaceGrid::new(64).unwrap();
    assert!(g.contains(IVec3::new(-64, 999, 63)));
    assert!(!g.contains(IVec3::new(64, 0, 0)));
}
```

- [ ] **Step 2: Run, expect FAIL**: `just test cube_sphere` gives compile error, module missing.
- [ ] **Step 3: Implement per Interfaces.**
- [ ] **Step 4: Run, expect PASS**: `just test cube_sphere` gives 3 passed.
- [ ] **Step 5: Commit** `core: cube-sphere faces, bases and face cells`.

### Task 2: Derived edge stepping for cells and chunks

**Files:**
- Create: `crates/core/src/cube_sphere/edges.rs`
- Test: same file

**Interfaces:**
- Consumes: `Face`, `FaceBasis`, `FaceCell`, `FaceGrid` (Task 1).
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum Step { PosU, NegU, PosV, NegV }`
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct QuarterTurns(pub u8)` (0-3, counter-clockwise about the face normal, mapping the source face's tangent axes onto the destination's) with `pub fn apply(self, d: IVec2) -> IVec2` and `pub fn compose(self, other: QuarterTurns) -> QuarterTurns`.
  - `impl FaceGrid { pub fn step_cell(self, at: FaceCell, step: Step) -> (FaceCell, QuarterTurns); pub fn step_chunk(self, at: FaceCell, step: Step) -> (FaceCell, QuarterTurns); }` (`step_chunk` treats `at.cell` as a chunk coordinate on a grid of half `half / CHUNK_SIZE`). Inputs outside the grid are a caller error: return the input unchanged with `QuarterTurns(0)` and `debug_assert!`.

Algorithm (the one thing the signature does not determine). Work in doubled integer coordinates so cell centres are integers. For a cell `(x, z)` on face `A` with half `H`, its doubled cube-surface centre is `P = 2H·n + (2x+1)·u + (2z+1)·v`. A step inside the face is plain `±1`. A step off the `+u` edge (`x = H-1`) goes to face `B` with `n_B = u_A`; the new doubled centre is `P' = 2H·u_A + (2z+1)·v_A + (2H-1)·(-n_A)` (the walk continues "down" the neighbour face). Recover `x' = (P'·u_B - 1)/2`, `z' = (P'·v_B - 1)/2`; `y` is unchanged. The other three edges follow by symmetry (`-u`: `n_B = -u_A`, `±v` likewise). The rotation is the quarter-turn count that maps `A`'s tangent pair `(u_A, v_A)`, after the edge fold, onto `(u_B, v_B)`: the step direction `+u_A` becomes `-n_A` on `B`, and `v_A` stays `v_A`; express both in `B`'s `(u_B, v_B)` and read off the turn.

- [ ] **Step 1: Write the failing tests**

```rust
fn all_boundary_cells(g: FaceGrid) -> impl Iterator<Item = (FaceCell, Step)> { /* every face, every cell on its four edges, the step that leaves the face */ }

#[test]
fn stepping_off_an_edge_and_back_is_identity() {
    let g = FaceGrid::new(64).unwrap();
    for (start, step) in all_boundary_cells(g) {
        let (there, turn) = g.step_cell(start, step);
        assert_ne!(there.face, start.face);
        assert!(g.contains(there.cell));
        let back_step = reverse_in_destination(step, turn); // the step on `there` that points back
        let (home, turn_back) = g.step_cell(there, back_step);
        assert_eq!(home, start);
        assert_eq!(turn.compose(turn_back), QuarterTurns(0));
    }
}

#[test]
fn every_face_has_four_distinct_neighbours_and_twelve_edges_total() {
    let g = FaceGrid::new(64).unwrap();
    let mut edges = std::collections::BTreeSet::new();
    for f in Face::ALL {
        let mut seen = std::collections::BTreeSet::new();
        for s in [Step::PosU, Step::NegU, Step::PosV, Step::NegV] {
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
    for f in Face::ALL { for (x, z) in [(63, 63), (63, -64), (-64, 63), (-64, -64)] {
        let corner = FaceCell { face: f, cell: IVec3::new(x, 0, z) };
        let su = if x > 0 { Step::PosU } else { Step::NegU };
        let sv = if z > 0 { Step::PosV } else { Step::NegV };
        let (a, _) = g.step_cell(corner, su);
        let (b, _) = g.step_cell(corner, sv);
        let faces = std::collections::BTreeSet::from([corner.face, a.face, b.face]);
        assert_eq!(faces.len(), 3, "{f:?} {x} {z}");
        assert!(neighbour_faces(g, a.face).contains(&b.face), "{f:?} {x} {z}");
    }}
}

#[test]
fn cell_and_doubled_centre_agree_with_cube_point() {
    // The doubled centre of the destination cell is one cell-width from the source centre's
    // edge fold: |P - P'|_inf == 2 and P' lies on the destination face plane.
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
fn chunk_stepping_matches_cell_stepping_at_chunk_corners() {
    // For every boundary chunk of a 4-chunk-half grid, stepping the chunk equals the chunk
    // containing the stepped cell of the chunk's matching corner cell.
}
```

The helper functions in the tests are test-only and written by the builder: `all_boundary_cells` (every face, every cell on its four edges, with the step that leaves the face), `reverse_in_destination(step, turn)` (the destination-face step whose direction is the opposite of `step`'s direction after applying `turn`), `edge_cell(g, f, s)` (any cell on face `f`'s edge in direction `s`), `opposite(f)` (the face with the negated normal), `neighbour_faces(g, f)` (the four faces reached by stepping off `f`'s edges). The last two test bodies are described in comments; the builder writes their assertions exactly as described.

- [ ] **Step 2: Run, expect FAIL**: `just test cube_sphere` gives compile error.
- [ ] **Step 3: Implement `step_cell`, `step_chunk`, `QuarterTurns` per the algorithm.**
- [ ] **Step 4: Run, expect PASS**: `just test cube_sphere`.
- [ ] **Step 5: Commit** `core: cube-sphere edge stepping derived from face bases`.

### Task 3: Projections with exact inverses and Jacobians

**Files:**
- Create: `crates/core/src/cube_sphere/projection.rs`
- Test: same file (split tests into `projection_tests.rs` if the file passes 300 lines)

**Interfaces:**
- Consumes: `Face`, `FaceBasis`, `FaceGrid`, `FaceCell` (Task 1).
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)] pub enum Projection { Gnomonic, Tangent, Spherified }`
  - `impl Projection { pub fn to_sphere(self, face: Face, s: f64, t: f64) -> DVec3; pub fn from_sphere(self, dir: DVec3) -> (Face, f64, f64); pub fn tangents(self, face: Face, s: f64, t: f64) -> (DVec3, DVec3); }` where `s, t ∈ [-1, 1]` are face coordinates (`s` along `u`, `t` along `v`), `to_sphere` returns a unit vector, `tangents` returns `(∂dir/∂s, ∂dir/∂t)` analytically.
  - `pub fn pick_face(dir: DVec3) -> Face`: largest `|component|`; ties resolved in the fixed order x, then y, then z; sign picks Pos/Neg.
  - `impl FaceGrid { pub fn cell_face_coords(self, cell: IVec3) -> (f64, f64); }` (cell centre: `((x + 0.5)/H, (z + 0.5)/H)`).
  - `pub fn local_frame(p: Projection, face: Face, s: f64, t: f64) -> DMat3`: columns `(east, up, north)` orthonormal, `up == to_sphere(..)` exactly, `east` the normalized `s` tangent with its `up` component removed, `north = east × up` negated as needed so the frame is right-handed like `(u, n, v)`.

Maps, with cube point `c = n + s'·u + t'·v`:
- Gnomonic: `s' = s`, `dir = normalize(c)`. Inverse: `s = (dir·u)/(dir·n)`.
- Tangent: `s' = tan(πs/4)`. Inverse: `s = atan((dir·u)/(dir·n)) · 4/π`.
- Spherified: take the 3D unit-cube point `c = n + s·u + t·v` (its normal-axis component is ±1). For each world axis `i` with the other two axes `j, k`: `dir_i = c_i · sqrt(1 - c_j²/2 - c_k²/2 + c_j²·c_k²/3)`. The result is already unit length (the analytic cube-to-sphere map); `to_sphere` still normalizes to absorb rounding. The inverse has no closed form: Newton in `(s, t)` from the gnomonic estimate, iterate until `|Δ| < 1e-15` with a cap of 32 iterations, then clamp to `[-1, 1]`.

- [ ] **Step 1: Write the failing tests**

```rust
const ALL: [Projection; 3] = [Projection::Gnomonic, Projection::Tangent, Projection::Spherified];
fn grid(n: i32) -> impl Iterator<Item = (f64, f64)> { /* n x n points over [-1,1]^2 including ±1 */ }

#[test]
fn round_trip_is_exact_to_1e12_everywhere_including_edges() {
    for p in ALL { for f in Face::ALL { for (s, t) in grid(41) {
        let d = p.to_sphere(f, s, t);
        assert!((d.length() - 1.0).abs() < 1e-14);
        let (f2, s2, t2) = p.from_sphere(d);
        let d2 = p.to_sphere(f2, s2, t2);
        assert!(d.angle_between(d2) < 1e-12, "{p:?} {f:?} {s} {t}");
        if s.abs() < 1.0 && t.abs() < 1.0 {
            assert_eq!(f2, f);
            assert!((s - s2).abs() < 1e-12 && (t - t2).abs() < 1e-12);
        }
    }}}
}

#[test]
fn face_pick_on_edges_and_corners_is_deterministic() {
    for d in [DVec3::new(1.0, 1.0, 0.0), DVec3::new(-1.0, 1.0, 1.0), DVec3::new(0.0, -1.0, -1.0), DVec3::ONE] {
        let f = pick_face(d.normalize());
        assert_eq!(f, pick_face(d.normalize()));
        for p in ALL { let (_, s, t) = p.from_sphere(d.normalize()); assert!(s.abs() <= 1.0 && t.abs() <= 1.0); }
    }
    assert_eq!(pick_face(DVec3::new(1.0, 1.0, 0.0).normalize()), Face::PosX);
}

#[test]
fn analytic_tangents_match_central_differences() {
    let h = 1e-6;
    for p in ALL { for f in Face::ALL { for (s, t) in grid(9) {
        let (ds, dt) = p.tangents(f, s.clamp(-1.0 + h, 1.0 - h), t.clamp(-1.0 + h, 1.0 - h));
        // compare with (to_sphere(s+h)-to_sphere(s-h))/(2h) and likewise for t, tolerance 1e-6
    }}}
}

#[test]
fn neighbouring_faces_agree_on_their_shared_edge() {
    // For every face and step, points on the shared edge (s = 1 on face A) map to the same
    // direction as the matching edge points of the neighbour face from Task 2's stepping.
}

#[test]
fn local_frame_is_orthonormal_right_handed_and_radial() { /* det == 1, up == to_sphere */ }
```

(The last three bodies are described in comments; the builder writes the assertions as described, tolerances as given.)

- [ ] **Step 2: Run, expect FAIL**: `just test cube_sphere`.
- [ ] **Step 3: Implement the three maps, inverse, tangents, `pick_face`, `cell_face_coords`, `local_frame`.**
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Commit** `core: cube-sphere projections with exact inverses and Jacobians`.

### Task 4: Distortion measurement, bench and the measurement record

**Files:**
- Create: `crates/core/src/cube_sphere/distortion.rs`
- Create: `crates/core/benches/cube_sphere.rs` (register in `crates/core/Cargo.toml` with `[[bench]] name = "cube_sphere" harness = false`, following how other crates in the workspace declare criterion benches; check `just bench` picks it up)
- Create: `docs/plans/2026-10-25-cube-sphere-projection-measurements.md`

**Interfaces:**
- Consumes: `Projection` (Task 3).
- Produces: `#[derive(Clone, Copy, Debug)] pub struct Distortion { pub area_ratio: f64, pub max_angle_deviation_deg: f64, pub max_edge_length_ratio: f64 }` and `pub fn measure(p: Projection, samples_per_axis: u32) -> Distortion`: area element `|∂s × ∂t|` and angle between the tangents sampled on a regular grid of cell centres over one face (symmetry makes one face enough); `area_ratio = max/min`; angle deviation from 90°; edge-length ratio of `|∂s|` max/min.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn gnomonic_area_ratio_is_three_root_three() {
    let d = measure(Projection::Gnomonic, 513);
    assert!((d.area_ratio - 3.0 * 3f64.sqrt()).abs() < 2e-2, "{d:?}");
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
```

(Sampling at cell centres never reaches the exact corner, so the expected values are approached from inside; the tolerances allow for that.)

- [ ] **Step 2: Run, expect FAIL.**
- [ ] **Step 3: Implement `measure`.**
- [ ] **Step 4: Run, expect PASS**: `just test cube_sphere`.
- [ ] **Step 5: Bench.** `crates/core/benches/cube_sphere.rs`: per projection, time `to_sphere` for a 32 x 32 column grid (one chunk's columns) and `from_sphere` for 1024 directions. Run `just bench cube_sphere`; do not run `just bench-save`.
- [ ] **Step 6: Measurement record.** Write `docs/plans/2026-10-25-cube-sphere-projection-measurements.md`: one table with the three projections × (`area_ratio`, `max_angle_deviation_deg`, `max_edge_length_ratio`, `to_sphere` ns per column, `from_sphere` ns per call), the exact commands, the machine (CPU name), and one recommendation line for W01's SDD. State that ADR 0024's "area ratio about 1.6 with a plain gnomonic map" is not what the measurement shows (3√3 ≈ 5.2 for area; give the measured length ratio too, which may be the quantity 0024 meant). Do not edit ADR 0024.
- [ ] **Step 7: `just verify`, then commit** `core: cube-sphere distortion measurement and projection record` (code, bench, record).
