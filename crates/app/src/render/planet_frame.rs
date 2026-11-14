//! Per-chunk planet placement in the eye's tangent frame.
//!
//! The face chart's `tan` map turns a square into a 60° parallelogram near a
//! cube corner, and it rebuilds a 20 km body position in f32. Each chunk is
//! one rigid frame, computed in f64 and stored relative to the render origin.
//!
//! Voxels and collision live in the chart, so drawing the true sphere shears
//! them at the player's feet near a corner. Around the eye the chart's linear
//! distortion is removed, leaving the chart axes that the camera yaw and the
//! props path in world_shader.wgsl use; the true sphere returns by `far_m`.
//! The cover shader applies the same blend, so chunks and cover agree.
use super::origin::{DrawTransform, RenderOrigin};
use engine_core::cube_sphere::{Face, Projection, atlas, local_frame};
use engine_core::CHUNK_SIZE;
use glam::{DMat3, DVec3, IVec3, Vec3};
use std::f64::consts::FRAC_PI_4;

/// Eye-frame placement shared by chunks, outlines and the cover for one frame.
#[derive(Clone, Copy)]
pub struct Eye {
    chart: DVec3,
    face: Face,
    /// Eye position on its face, chart metres.
    local: DVec3,
    body: DVec3,
    inverse: DMat3,
    /// Chart axis minus the chart Jacobian in the eye frame, on x and z.
    undo_x: DVec3,
    undo_z: DVec3,
    near_m: f64,
    far_m: f64,
}

impl Eye {
    /// Builds the eye frame; chart shear is undone inside `far_m` metres.
    pub fn new(eye_m: DVec3, far_m: f64) -> Eye {
        let (body, rot) = place(eye_m);
        let inverse = rot.transpose();
        let jacobian = |axis: DVec3| inverse * (place(eye_m + axis).0 - place(eye_m - axis).0) * 0.5;
        let (face, local) = atlas::split(eye_m);
        let far_m = far_m.max(1.0);
        Eye {
            chart: eye_m,
            face,
            local,
            body,
            inverse,
            // The target is the chart itself, not its nearest rotation: props
            // and the camera yaw already use chart axes, and a rotation turns
            // the ground up to 15 degrees against them at a corner.
            undo_x: DVec3::X - jacobian(DVec3::X),
            undo_z: DVec3::Z - jacobian(DVec3::Z),
            near_m: far_m / 8.0,
            far_m,
        }
    }

    /// Returns the eye's chart position in metres.
    pub fn chart_m(&self) -> DVec3 {
        self.chart
    }

    /// Uniform rows for `planet_flat` in camera_lighting_shared.wgsl.
    pub fn shader_rows(&self) -> [[f32; 4]; 3] {
        [
            self.undo_x.as_vec3().extend(self.near_m as f32).to_array(),
            self.undo_z.as_vec3().extend(self.far_m as f32).to_array(),
            [self.local.x as f32, self.local.z as f32, self.face as u8 as f32, 0.0],
        ]
    }

    /// Position of chart point `p_m` relative to the eye, in the eye frame.
    pub fn relative(&self, p_m: DVec3) -> DVec3 {
        let body = place(p_m).0;
        let sphere = self.inverse * (body - self.body);
        let Some((dx, dz)) = self.chart_offset(body) else { return sphere };
        // Blending in log distance bounds d * dw/dd by 1.5 / ln(far / near);
        // a linear band that undoes a 30 degree shear folds the ground.
        let reach = (dx.hypot(dz).max(self.near_m) / self.near_m).ln() / (self.far_m / self.near_m).ln();
        let w = 1.0 - smoothstep(0.0, 1.0, reach);
        sphere + (self.undo_x * dx + self.undo_z * dz) * w
    }

    /// Horizontal chart offset from the eye, read on the eye's face.
    ///
    /// Reads the body point through the eye face's tan chart, as the cover
    /// shader does; inside the face that is the chart itself, and past an
    /// edge both agree where chunks and cover overlap.
    fn chart_offset(&self, body: DVec3) -> Option<(f64, f64)> {
        let b = self.face.basis();
        let (u, n, v) = (b.u.as_dvec3(), b.n.as_dvec3(), b.v.as_dvec3());
        let dn = body.dot(n);
        if dn <= 0.0 { return None; }
        let chart = |axis: DVec3| (body.dot(axis) / dn).atan() / FRAC_PI_4 * atlas::HALF_M;
        Some((chart(u) - self.local.x, chart(v) - self.local.z))
    }
}

/// Radius where the ground is back on the true sphere: twice the view distance.
///
/// The cover shader applies the same blend, so the band may run past the
/// resident ring; a wide band keeps the undo from squashing the ground.
pub fn far_m(voxel_m: f64) -> f64 {
    let chunks = f64::from(super::render_distance_settings().view_distance_chunks);
    chunks * f64::from(CHUNK_SIZE) * voxel_m * 2.0
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Origin-frame affine that keeps one chunk's voxels rectangular.
#[cfg(test)]
pub fn chunk_draw(origin: RenderOrigin, chunk: IVec3, eye: &Eye, voxel_m: f64) -> DrawTransform {
    let eye_origin = eye.chart - origin.voxel.as_dvec3() * voxel_m;
    chunk_draw_with_corners(origin, chunk, eye, voxel_m, |corner| {
        eye.relative(corner.as_dvec3() * f64::from(CHUNK_SIZE) * voxel_m) + eye_origin
    })
}

pub fn chunk_draw_with_corners(
    origin: RenderOrigin, chunk: IVec3, eye: &Eye, voxel_m: f64,
    mut projected_corner: impl FnMut(IVec3) -> DVec3,
) -> DrawTransform {
    let corner = (chunk * CHUNK_SIZE).as_dvec3();
    let size = f64::from(CHUNK_SIZE);
    let half = size * 0.5;
    // Same split the flat path uses: the view matrix removes the f32 residual.
    let eye_origin = eye.chart - origin.voxel.as_dvec3() * voxel_m;
    let exact: [DVec3; 8] = std::array::from_fn(|i| {
        let step = IVec3::new((i & 1) as i32, ((i >> 1) & 1) as i32, ((i >> 2) & 1) as i32);
        projected_corner(chunk + step)
    });
    // Rigid axes average the four chunk edges along each axis, the linear part
    // of the trilinear map through the corners, so corrections stay small.
    let axis = |bit: usize| {
        let edges = (0..8).filter(|i| i & bit == 0).map(|i| exact[i | bit] - exact[i]);
        (edges.sum::<DVec3>() / (4.0 * size)).as_vec3()
    };
    let (axis_x, axis_y, axis_z) = (axis(1), axis(2), axis(4));
    let center_origin = eye.relative((corner + DVec3::splat(half)) * voxel_m) + eye_origin;
    let translation = center_origin
        - axis_x.as_dvec3() * half
        - axis_y.as_dvec3() * half
        - axis_z.as_dvec3() * half;
    let voxels = origin.chunk_offset(chunk);
    let mut draw = DrawTransform {
        offset: [translation.x as f32, translation.y as f32, translation.z as f32, 1.0],
        axis_x: axis_x.extend(voxels[0]).to_array(),
        axis_y: axis_y.extend(voxels[1]).to_array(),
        axis_z: axis_z.extend(voxels[2]).to_array(),
        seam: [[0.0; 4]; 6],
    };
    // Upload the shared positions directly. Reconstructing them by adding a
    // rounded affine transform and a separate residual can round differently
    // on the two owners of the same face.
    for (i, exact) in exact.into_iter().enumerate() {
        for (axis, value) in [exact.x, exact.y, exact.z].into_iter().enumerate() {
            draw.seam[axis * 2 + i / 4][i % 4] = value as f32;
        }
    }
    draw
}

/// Body position of a chart point and its orthonormal frame.
///
/// Past a face edge this is the unclamped tan map, as worldgen and
/// `planet::cross_face` read it, so a chunk does not move when the eye crosses.
/// tan reaches a pole at |s| = 2; beyond `REACH` the leftover chart offset is
/// applied linearly in the last frame to stay finite.
fn place(chart_m: DVec3) -> (DVec3, DMat3) {
    const REACH: f64 = 1.5;
    let (face, local) = atlas::split(chart_m);
    let s = (local.x / atlas::HALF_M).clamp(-REACH, REACH);
    let t = (local.z / atlas::HALF_M).clamp(-REACH, REACH);
    let frame = local_frame(Projection::Tangent, face, s, t);
    let radius = atlas::RADIUS_M + local.y;
    let (ds, dt) = Projection::Tangent.tangents(face, s, t);
    let metric = |d: DVec3| {
        let scale = d.length() * radius / atlas::HALF_M;
        if scale.is_finite() && scale > 1e-4 { scale } else { 1.0 }
    };
    let (scale_x, scale_z) = (metric(ds), metric(dt));
    let clamped = DVec3::new(s * atlas::HALF_M, local.y, t * atlas::HALF_M);
    let extra = local - clamped;
    let body = Projection::Tangent.to_sphere(face, s, t) * radius
        + frame.x_axis * (extra.x * scale_x)
        + frame.y_axis * extra.y
        + frame.z_axis * (extra.z * scale_z);
    (body, frame)
}

#[cfg(test)]
fn apply(draw: DrawTransform, pos: Vec3) -> DVec3 {
    let axis = |a: [f32; 4]| Vec3::new(a[0], a[1], a[2]).as_dvec3();
    axis(draw.axis_x) * f64::from(pos.x)
        + axis(draw.axis_y) * f64::from(pos.y)
        + axis(draw.axis_z) * f64::from(pos.z)
        + Vec3::from_slice(&draw.offset[..3]).as_dvec3()
}

#[cfg(test)]
#[path = "planet_frame_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "planet_frame_gpu_tests.rs"]
mod gpu_tests;
