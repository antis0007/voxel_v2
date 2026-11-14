//! Field, radius, branch, junction, seed and shell-continuity checks for the corkscrew strand.
use super::*;
use crate::WorldType;
use engine_core::VoxelScalePreset;

const SCALES: [VoxelScalePreset; 3] =
    [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter];

fn cfg(seed: u32, scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig {
        planet: None,
        generator_version: VERSION,
        seed,
        world_type: WorldType::Manyfold,
        voxel_scale: scale,
    }
}

/// Perpendicular reference at a polyline edge running from `v` to `next`, the
/// same frame the block sampler derives from the segment tangent.
fn outward_along(v: Vec3, next: Vec3) -> Vec3 {
    let t = (next - v).normalize();
    let e1 = (Vec3::Y - t * t.dot(Vec3::Y)).normalize_or_zero();
    if e1.length_squared() < 1e-6 { Vec3::X } else { e1.normalize() }
}

fn cell_of(cfg: &WorldSampleConfig, p: Vec3) -> IVec3 {
    (p / cfg.voxel_meters()).floor().as_ivec3()
}

/// Distance from `p` to the nearest point of any section whose arc length is
/// more than two maximum radii from `s`. Those sections are not the same tube.
fn distance_to_far_sections(p: Vec3, s: f32) -> f32 {
    let mut best = f32::INFINITY;
    for seg in segments().iter().filter(|seg| (seg.s0 - s).abs() > 2.0 * R_MAX) {
        let ab = seg.b - seg.a;
        let t = ((p - seg.a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        best = best.min((p - (seg.a + ab * t)).length());
    }
    best
}

/// Every interior vertex of both strands with its outward reference, skipping
/// vertices whose shell another section of the tube passes through. The
/// corkscrew is self-intersecting (pitch 40 m, tube up to 64 m across), so
/// `nearest` there reports a different section than the one that owns the
/// vertex. The first and last vertices have no neighbour on one side.
fn vertex_frames() -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    for strand in strands() {
        for k in 2..strand.len() - 1 {
            let v = strand[k];
            let s = nearest(v).2;
            if distance_to_far_sections(v, s) > R_MAX + WALL + 1.0 {
                out.push((v, outward_along(v, strand[k + 1])));
            }
        }
    }
    out
}

#[test]
fn field_is_outward_and_vanishes_on_the_axis() {
    let a = &strands()[0];
    for k in 2..a.len() - 1 {
        let v = a[k];
        assert!(acceleration(v).length() < 0.5, "vertex axis gravity must vanish");
        let p = v + outward_along(v, a[k + 1]) * 6.0;
        let acc = acceleration(p);
        assert!(acc.length() > 1.0, "no gravity off the axis");
        let (cc, _, _, _, _) = nearest(p);
        let dir = (p - cc).normalize_or_zero();
        assert!(dir.dot(acc.normalize_or_zero()) > 0.99, "field is not radial");
    }
}

#[test]
fn field_is_continuous_across_the_branch_junction() {
    let [_, b] = strands();
    let ib = (BRANCH_T * N as f32).round() as usize;
    let before = b[ib - 1] + (b[ib] - b[ib - 1]) * 0.5;
    let after = b[ib] + (b[ib + 1] - b[ib]) * 0.5;
    let step = (acceleration(after) - acceleration(before)).length();
    assert!(step < 2.0, "gravity jumps by {step:.2} m/s^2 across the junction");
}

#[test]
fn radius_swells_and_narrows_like_a_strand() {
    let thin = radius(0.0);
    let thick = radius(15.7); // quarter wave of 0.1 * s
    assert!(thick > thin + 4.0, "radius does not swell along the strand");
    assert!((R_MIN..=R_MAX).contains(&thin) && (R_MIN..=R_MAX).contains(&thick));
}

#[test]
fn radius_stays_within_bounds_over_the_whole_arc() {
    let total = segments().iter().map(|s| s.s0 + (s.b - s.a).length()).fold(0.0, f32::max);
    for i in 0..=400 {
        let r = radius(total * i as f32 / 400.0);
        assert!((R_MIN..=R_MAX).contains(&r), "radius {r} out of bounds at step {i}");
    }
}

#[test]
fn branch_separates_from_the_trunk() {
    let [_a, b] = strands();
    assert!(b.last().unwrap().distance(trunk(1.0)) > BRANCH_LEN * 0.5, "branch does not diverge");
}

#[test]
fn junction_is_a_shared_vertex_with_no_kink() {
    let [a, b] = strands();
    let ib = (BRANCH_T * N as f32).round() as usize;
    assert!((BRANCH_T * N as f32 - ib as f32).abs() < 1e-4, "branch point must fall on a vertex");
    assert!(a[ib].distance(b[ib]) < 1e-3, "strands do not share the junction vertex");
    let leaving = (b[ib + 1] - b[ib]).normalize();
    assert!(trunk_tangent(BRANCH_T).dot(leaving) > 0.99, "branch leaves the trunk at a kink");
}

#[test]
fn block_agrees_with_the_field_radii() {
    let c = cfg(crate::test_world::seed(), VoxelScalePreset::OneMeter);
    let m = c.voxel_meters();
    let a = &strands()[0];
    for k in 2..a.len() - 1 {
        let v = a[k];
        let o = outward_along(v, a[k + 1]);
        for dist in [0.5f32, 3.0, 8.0, 14.0, 20.0, 26.0, 32.0, 40.0, 60.0] {
            let cell = cell_of(&c, v + o * dist);
            let centre = (cell.as_vec3() + Vec3::splat(0.5)) * m;
            let (_, d, _, r, _) = nearest(centre);
            let got = block(&c, cell);
            if d > r + WALL {
                assert_eq!(got, BlockId::Air, "outside must be air at k={k} dist={dist}");
            } else if d > r {
                assert!(matches!(got, BlockId::Stone | BlockId::Bedrock), "rind must be solid at k={k} dist={dist}");
            } else {
                assert!(!matches!(got, BlockId::Stone | BlockId::Bedrock), "interior must not be rind at k={k} dist={dist}");
            }
        }
    }
}

#[test]
fn shell_sandwich_holds_along_both_strands() {
    let c = cfg(crate::test_world::seed(), VoxelScalePreset::OneMeter);
    let frames = vertex_frames();
    assert!(frames.len() > 50,"overlap filter left only {} vertices", frames.len());
    for (v, o) in frames {
        let (_, _, _, r, _) = nearest(v);
        let offsets = [3.0, r + 1.0, R_MAX + WALL + 3.0];
        let probes = offsets.map(|d| v + o * d);
        // On the inner side of a bend a probe can be nearer to another part of
        // the tube than the offset it was placed at, where the shell rule does not apply.
        if probes.iter().zip(offsets).any(|(&p, d)| nearest(p).1 < d - 0.5) {
            continue;
        }
        // Interior is open; the rind is solid; beyond it is open space again.
        assert_eq!(block(&c, cell_of(&c, probes[0])), BlockId::Air, "interior not open at {v}");
        assert!(matches!(
            block(&c, cell_of(&c, probes[1])),
            BlockId::Stone | BlockId::Bedrock
        ), "rind missing at {v}");
        assert_eq!(block(&c, cell_of(&c, probes[2])), BlockId::Air, "outside not open at {v}");
    }
}

#[test]
fn seed_changes_only_the_interior() {
    let (c1, c2) = (cfg(1, VoxelScalePreset::OneMeter), cfg(2, VoxelScalePreset::OneMeter));
    let mut interior_differs = false;
    for (v, o) in vertex_frames() {
        let (_, _, _, r, _) = nearest(v);
        for cell in [cell_of(&c1, v + o * (r + 1.0)), cell_of(&c1, v + o * (R_MAX + WALL + 3.0))] {
            assert_eq!(block(&c1, cell), block(&c2, cell), "rind or outside depends on seed at {cell}");
        }
        for depth in [0.5, 1.0, 2.0] {
            let cell = cell_of(&c1, v + o * (r - depth));
            interior_differs |= block(&c1, cell) != block(&c2, cell);
        }
    }
    assert!(interior_differs, "seed never changes the interior");
}

#[test]
fn spawn_sits_in_tube_air_at_every_scale() {
    for scale in SCALES {
        let c = cfg(crate::test_world::seed(), scale);
        let cell = spawn(&c).as_ivec3();
        let p = (cell.as_vec3() + Vec3::splat(0.5)) * c.voxel_meters();
        let (_, d, _, r, _) = nearest(p);
        assert!(d < r, "spawn {cell} lies outside the tube at {scale:?}");
        assert!(!matches!(block(&c, cell), BlockId::Stone | BlockId::Bedrock), "spawn {cell} is inside the rind");
    }
}

/// Each guide sits at the wall of its own section: `radius(s) - 0.6` from that
/// section's centreline. Measured against `nearest` it would fail where turns
/// overlap, so this checks the placement rule directly.
#[test]
fn guide_lights_sit_at_the_wall_of_their_section() {
    let c = cfg(crate::test_world::seed(), VoxelScalePreset::OneMeter);
    let guides = guide_lights(&c);
    assert_eq!(guides.len(), 24);
    for (i, g) in guides.into_iter().enumerate() {
        let s = i as f32 * 4.0;
        let centre = trunk((s / trunk_length()).clamp(0.0, 1.0));
        let p = (g.as_vec3() + Vec3::splat(0.5)) * c.voxel_meters();
        // Cell rounding moves the centre by at most half a cell diagonal.
        let off = (p - centre).length();
        assert!((off - (radius(s) - 0.6)).abs() < 1.0, "guide {i} off its wall: {off:.2} m from the centreline");
    }
}

#[test]
fn cell_mapping_floors_negative_coordinates() {
    let c = cfg(crate::test_world::seed(), VoxelScalePreset::HalfMeter);
    assert_eq!(cell_of(&c, Vec3::new(-0.1, 0.0, 0.0)).x, -1);
    assert_eq!(cell_of(&c, Vec3::new(-0.5, 0.0, 0.0)).x, -1);
    assert_eq!(cell_of(&c, Vec3::new(-40.0, 0.0, 0.0)).x, -80);
}

#[test]
fn negative_and_far_cells_are_open_air() {
    let c = cfg(crate::test_world::seed(), VoxelScalePreset::OneMeter);
    for cell in [
        IVec3::new(-4096, -1, -4096),
        IVec3::new(-33, -65, 31),
        IVec3::new(1 << 20, 5, -(1 << 20)),
    ] {
        assert_eq!(block(&c, cell), BlockId::Air, "{cell} is not open air");
    }
}

#[test]
fn far_field_is_exactly_zero() {
    for (v, o) in vertex_frames().into_iter().take(8) {
        assert_eq!(acceleration((v + o * 6.0) * 1000.0), Vec3::ZERO, "far field must be exactly zero");
    }
}
