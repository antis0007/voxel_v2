use super::*;
use glam::Vec3;

/// Last chunk inside face 0 on +X/+Z.
const EDGE: i32 = (atlas::HALF_M as i32) / CHUNK_SIZE - 1;
/// `far_m` for the default 28-chunk view at one metre per voxel.
const FAR_M: f64 = 28.0 * 32.0 * 2.0;

fn eye_at_chunk_center(chunk: IVec3) -> (RenderOrigin, DVec3) {
    let center = (chunk * CHUNK_SIZE).as_dvec3() + DVec3::splat(f64::from(CHUNK_SIZE) * 0.5);
    let origin = RenderOrigin::follow(None, center, 0);
    (origin, center)
}

#[test]
fn chunk_center_stays_on_the_eye_in_the_origin_frame() {
    let chunk = IVec3::new(0, 1, 0);
    let (origin, eye) = eye_at_chunk_center(chunk);
    let draw = chunk_draw(origin, chunk, &Eye::new(eye, FAR_M), 1.0);
    let placed = apply(draw, Vec3::splat(16.0));
    let eye_origin = eye - origin.voxel.as_dvec3();
    let err = (placed - eye_origin).length();
    assert!(err < 1e-3, "center drifted {err} m");
    assert!((draw.axis_x[3] - origin.chunk_offset(chunk)[0]).abs() < 1e-6);
}

#[test]
fn axes_stay_orthogonal_and_unit_at_a_face_corner() {
    // Last in-face chunk on +X/+Z of face 0. The tan map's tangents meet
    // near 60 degrees here; the draw axes must not.
    let chunk = IVec3::new(EDGE, 1, EDGE);
    let (origin, eye) = eye_at_chunk_center(chunk);
    let draw = chunk_draw(origin, chunk, &Eye::new(eye, FAR_M), 1.0);
    let x = Vec3::from_slice(&draw.axis_x[..3]);
    let y = Vec3::from_slice(&draw.axis_y[..3]);
    let z = Vec3::from_slice(&draw.axis_z[..3]);
    assert!(x.dot(y).abs() < 1e-4, "xy {}", x.dot(y));
    assert!(y.dot(z).abs() < 1e-4, "yz {}", y.dot(z));
    assert!(z.dot(x).abs() < 1e-4, "zx {}", z.dot(x));
    for (name, a) in [("x", x), ("y", y), ("z", z)] {
        assert!((a.length() - 1.0).abs() < 1e-3, "|{name}| {}", a.length());
    }
}

#[test]
fn voxels_near_the_eye_stay_square_at_a_corner_and_an_edge() {
    // Corner: the chart axes meet at 60 degrees on the sphere. Edge
    // midpoint: one chart axis is squashed to 0.72.
    for chunk in [IVec3::new(EDGE, 1, EDGE), IVec3::new(EDGE, 1, 0), IVec3::new(-EDGE - 1, 1, EDGE)] {
        let (origin, eye) = eye_at_chunk_center(chunk);
        let draw = chunk_draw(origin, chunk, &Eye::new(eye, FAR_M), 1.0);
        let at = |p: Vec3| placed(draw, p);
        let base = at(Vec3::new(0.0, 16.0, 0.0));
        let along_x = at(Vec3::new(32.0, 16.0, 0.0)) - base;
        let along_z = at(Vec3::new(0.0, 16.0, 32.0)) - base;
        let cos = along_x.normalize().dot(along_z.normalize());
        assert!(cos.abs() < 0.01, "chunk {chunk}: cos {cos}");
        for edge in [along_x, along_z] {
            assert!((edge.length() - 32.0).abs() < 0.1, "chunk {chunk}: edge {}", edge.length());
        }
    }
}

#[test]
fn the_blend_band_never_folds_or_crushes_the_ground() {
    // Undoing a 30 degree shear over too short a band folds terrain back
    // over itself. Past an edge the extended chart is distorted on the true
    // sphere too, so the bound is relative to the sphere's own conditioning.
    for eye in [
        DVec3::new(atlas::HALF_M - 400.0, 40.0, -(atlas::HALF_M - 150.0)),
        DVec3::new(atlas::HALF_M - 100.0, 40.0, 0.0),
    ] {
        let frame = Eye::new(eye, FAR_M);
        for step in 1..=60 {
            let r = FAR_M * 1.2 * f64::from(step) / 60.0;
            for k in 0..16 {
                let a = f64::from(k) * std::f64::consts::TAU / 16.0;
                let p = eye + DVec3::new(a.cos(), 0.0, a.sin()) * r;
                let conditioning = |map: &dyn Fn(DVec3) -> DVec3| {
                    let d = |v: DVec3| (map(p + v) - map(p - v)) * 0.5;
                    let (dx, dz) = (d(DVec3::X), d(DVec3::Z));
                    (dx.x * dz.z - dx.z * dz.x) / dx.length().max(dz.length())
                };
                let blended = conditioning(&|q| frame.relative(q));
                let sphere = conditioning(&|q| frame.inverse * (place(q).0 - frame.body));
                let bound = 0.7 * sphere.min(1.0);
                assert!(blended > bound, "eye {eye} r {r:.0} angle {a:.2}: {blended:.3}, sphere {sphere:.3}");
            }
        }
    }
}

#[test]
fn near_ground_follows_the_chart_axes_like_props_and_the_camera() {
    // world_shader.wgsl draws props at their chart offset minus the
    // curvature drop, and the camera yaw is a chart angle; terrain near
    // the eye must line up with both, at a corner and at an edge.
    for eye in [
        DVec3::new(atlas::HALF_M - 400.0, 40.0, -(atlas::HALF_M - 150.0)),
        DVec3::new(atlas::HALF_M - 100.0, 40.0, 0.0),
        DVec3::new(0.0, 40.0, 0.0),
    ] {
        let frame = Eye::new(eye, FAR_M);
        for offset in [DVec3::new(20.0, 0.0, 0.0), DVec3::new(0.0, 3.0, -20.0), DVec3::new(-14.0, -2.0, 14.0)] {
            let drop = (offset.x * offset.x + offset.z * offset.z) / (2.0 * atlas::RADIUS_M);
            let props = offset - DVec3::new(0.0, drop, 0.0);
            let off = (frame.relative(eye + offset) - props).length();
            assert!(off < 0.01, "eye {eye} offset {offset}: {off} m from the props path");
        }
    }
}

#[test]
fn far_chunks_return_to_the_true_sphere() {
    let eye = DVec3::new(f64::from(EDGE) * 32.0, 40.0, f64::from(EDGE) * 32.0);
    let frame = Eye::new(eye, FAR_M);
    let (eye_body, rot) = place(eye);
    for p in [eye - DVec3::new(FAR_M, 0.0, 0.0), eye - DVec3::new(FAR_M * 0.8, 10.0, FAR_M * 0.8)] {
        let sphere = rot.transpose() * (place(p).0 - eye_body);
        let gap = (frame.relative(p) - sphere).length();
        assert!(gap < 1e-6, "{p}: {gap} m off the sphere");
    }
}

#[test]
fn placement_is_continuous_across_the_face_edge() {
    // Past +X of face 0 the same chart extends; the next face's own chart
    // reads through the extended tan chart, so both agree at the seam.
    let eye = DVec3::new(atlas::HALF_M - 40.0, 30.0, 0.0);
    let frame = Eye::new(eye, FAR_M);
    let inside = DVec3::new(atlas::HALF_M - 0.01, 30.0, 0.0);
    let outside = DVec3::new(atlas::HALF_M + 0.01, 30.0, 0.0);
    let gap = (frame.relative(inside) - frame.relative(outside)).length();
    assert!(gap < 0.05, "edge gap {gap} m");
}

#[test]
fn adjacent_chunks_share_their_face_within_a_centimetre() {
    let eye = DVec3::new(16.0, 48.0, 16.0);
    let origin = RenderOrigin::follow(None, eye, 0);
    let a = chunk_draw(origin, IVec3::new(0, 1, 0), &Eye::new(eye, FAR_M), 1.0);
    let b = chunk_draw(origin, IVec3::new(1, 1, 0), &Eye::new(eye, FAR_M), 1.0);
    let left = apply(a, Vec3::new(32.0, 16.0, 16.0));
    let right = apply(b, Vec3::new(0.0, 16.0, 16.0));
    let gap = (left - right).length();
    assert!(gap < 0.01, "seam gap {gap} m");
}

#[test]
fn corner_chunks_share_their_face() {
    let chunk_a = IVec3::new(EDGE - 1, 1, EDGE);
    let eye = (chunk_a * CHUNK_SIZE).as_dvec3() + DVec3::splat(f64::from(CHUNK_SIZE) * 0.5);
    let origin = RenderOrigin::follow(None, eye, 0);
    let a = chunk_draw(origin, chunk_a, &Eye::new(eye, FAR_M), 1.0);
    let b = chunk_draw(origin, IVec3::new(EDGE, 1, EDGE), &Eye::new(eye, FAR_M), 1.0);
    let left = placed(a, Vec3::new(32.0, 16.0, 16.0));
    let right = placed(b, Vec3::new(0.0, 16.0, 16.0));
    let gap = (left - right).length();
    assert!(gap < 0.05, "corner seam gap {gap} m");
    // The trilinear correction must also track the blended placement inside.
    let frame = Eye::new(eye, FAR_M);
    let eye_origin = eye - origin.voxel.as_dvec3();
    let corner = (IVec3::new(EDGE, 1, EDGE) * CHUNK_SIZE).as_dvec3();
    for p in [Vec3::new(16.0, 16.0, 16.0), Vec3::new(8.0, 3.0, 27.0)] {
        let exact = frame.relative(corner + p.as_dvec3()) + eye_origin;
        let off = (placed(b, p) - exact).length();
        assert!(off < 0.05, "interior {p}: {off} m");
    }
}

/// The shader's trilinear placement from shared corners.
fn placed(draw: DrawTransform, pos: Vec3) -> DVec3 {
    let t = (pos / CHUNK_SIZE as f32).as_dvec3();
    let mut out = DVec3::ZERO;
    for i in 0..8 {
        let pick = |bit: usize, v: f64| if (i >> bit) & 1 == 1 { v } else { 1.0 - v };
        let w = pick(0, t.x) * pick(1, t.y) * pick(2, t.z);
        let lane = |axis: usize| f64::from(draw.seam[axis * 2 + i / 4][i % 4]);
        out += DVec3::new(lane(0), lane(1), lane(2)) * w;
    }
    out
}

#[test]
fn seam_correction_closes_shared_faces_far_from_the_eye() {
    // Gaps grow with distance from the eye; check far and near a face corner,
    // across x, z and negative-coordinate neighbours.
    for (eye_chunk, a) in [
        (IVec3::new(0, 1, 0), IVec3::new(9, 1, -7)),
        (IVec3::new(EDGE - 9, 1, EDGE - 9), IVec3::new(EDGE - 1, 1, EDGE)),
        (IVec3::new(-40, 1, -40), IVec3::new(-49, 1, -33)),
    ] {
        let (origin, eye) = eye_at_chunk_center(eye_chunk);
        let draw_a = chunk_draw(origin, a, &Eye::new(eye, FAR_M), 1.0);
        for (step, shared) in [(IVec3::X, Vec3::X), (IVec3::Z, Vec3::Z)] {
            let draw_b = chunk_draw(origin, a + step, &Eye::new(eye, FAR_M), 1.0);
            for (u, v) in [(0.0, 0.0), (7.0, 25.0), (32.0, 32.0), (16.0, 3.0)] {
                let other = if shared == Vec3::X { Vec3::new(0.0, u, v) } else { Vec3::new(u, v, 0.0) };
                let on_a = other + shared * CHUNK_SIZE as f32;
                let gap = (placed(draw_a, on_a) - placed(draw_b, other)).length();
                assert!(gap < 2e-3, "chunk {a} step {step} at {other}: gap {gap} m");
            }
        }
    }
}

#[test]
fn a_chunk_past_the_face_edge_stays_finite() {
    let chunk = IVec3::new(EDGE + 21, 2, -3);
    let (origin, eye) = eye_at_chunk_center(IVec3::new(EDGE, 2, 0));
    let draw = chunk_draw(origin, chunk, &Eye::new(eye, FAR_M), 1.0);
    let placed = apply(draw, Vec3::new(8.0, 4.0, 8.0));
    assert!(placed.is_finite(), "{placed}");
    assert_eq!(draw.offset[3], 1.0);
}

#[test]
fn past_the_edge_chunks_sit_where_physics_and_worldgen_put_them() {
    // cross_face and worldgen read past-edge chart points through the
    // unclamped tan map; drawing them elsewhere pops the view at the crossing.
    for p in [
        DVec3::new(atlas::HALF_M + 800.0, 50.0, 300.0),
        DVec3::new(atlas::HALF_M + 300.0, 10.0, -(atlas::HALF_M + 300.0)),
        DVec3::new(-(atlas::HALF_M + 1500.0), 0.0, 0.0),
    ] {
        let gap = (place(p).0 - atlas::to_body(p)).length();
        assert!(gap < 1e-6, "{p}: drawn {gap} m from its body position");
    }
}
