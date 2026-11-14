//! Staged, pixel-stepped surface fractures for selected mining blocks.
use super::LineVertex;
use glam::{IVec3, Vec3};

const STAGES: usize = 8;
// Negative alpha selects ungraded material damage in the shared overlay shader.
const COLOR: [f32; 4] = [0.075, 0.045, 0.035, -1.0];

pub fn append_fractures(world: &crate::world::World, eye: Vec3, out: &mut Vec<LineVertex>) {
    let scale = world.voxel_scale.meters();
    let reach = 8.0 / scale;
    for (cell, _, damage) in world.mining_fractures.visible()
        .filter(|(p, b, _)| p.as_vec3().distance_squared(eye) < reach * reach && world.get(*p) == *b)
        .take(16)
    {
        append_block(world, cell, damage, 0.0045 / scale, out);
    }
}
const SEGMENTS: [([f32; 2], [f32; 2]); 16] = [
    ([0.48, 0.50], [0.39, 0.40]),
    ([0.48, 0.50], [0.58, 0.41]),
    ([0.39, 0.40], [0.35, 0.27]),
    ([0.58, 0.41], [0.70, 0.34]),
    ([0.48, 0.50], [0.43, 0.63]),
    ([0.48, 0.50], [0.61, 0.58]),
    ([0.35, 0.27], [0.22, 0.20]),
    ([0.70, 0.34], [0.78, 0.20]),
    ([0.43, 0.63], [0.32, 0.74]),
    ([0.61, 0.58], [0.72, 0.70]),
    ([0.39, 0.40], [0.25, 0.46]),
    ([0.58, 0.41], [0.62, 0.24]),
    ([0.32, 0.74], [0.23, 0.84]),
    ([0.72, 0.70], [0.82, 0.79]),
    ([0.25, 0.46], [0.14, 0.55]),
    ([0.61, 0.58], [0.54, 0.79]),
];

/// Damage belongs to an existing cube surface, not the whole selection volume.
/// Large hand selections can include air, fluids and cells already removed by
/// earlier edits. Models and fittings need their own surface-shaped damage.
pub fn append_block(
    world: &crate::world::World,
    cell: IVec3,
    fraction: f32,
    expand: f32,
    out: &mut Vec<LineVertex>,
) {
    let block = world.get(cell);
    if !block.is_ray_pickable()
        || block.is_empty()
        || block.is_liquid()
        || block.is_model()
        || block.is_torch()
        || engine_content::fittings::state(block).is_some()
        || engine_content::thin::is_thin(block)
    {
        return;
    }
    append(cell, fraction, expand, out);
}

/// Appends thick, stepped fissures on all six block faces.
fn append(block: IVec3, fraction: f32, expand: f32, out: &mut Vec<LineVertex>) {
    if !fraction.is_finite() || fraction <= 0.0 || !expand.is_finite() {
        return;
    }
    let stage = (fraction.clamp(0.0, 1.0) * STAGES as f32).ceil() as usize;
    let segments = stage.clamp(1, STAGES) * (SEGMENTS.len() / STAGES);
    let faces = [
        (Vec3::X, Vec3::Z, Vec3::Y),
        (Vec3::NEG_X, Vec3::NEG_Z, Vec3::Y),
        (Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::NEG_Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
    ];
    let size = 1.0 + 2.0 * expand.max(0.0);
    let origin = block.as_vec3() + Vec3::splat(0.5);
    let point = |normal: Vec3, u: Vec3, v: Vec3, uv: [f32; 2]| {
        origin
            + normal * (0.5 + expand.max(0.0))
            + u * ((uv[0] - 0.5) * size)
            + v * ((uv[1] - 0.5) * size)
    };
    for (normal, u, v) in faces {
        for &(a, b) in &SEGMENTS[..segments] {
            for offset in [-0.008, 0.0, 0.008] {
                let snap = |p: [f32; 2]| [
                    (p[0] * 16.0).round() / 16.0,
                    (p[1] * 16.0).round() / 16.0 + offset,
                ];
                let a = snap(a);
                let b = snap(b);
                for uv in [a, [b[0], a[1]], [b[0], a[1]], b] {
                    out.push(LineVertex {
                        pos: point(normal, u, v, uv).to_array(),
                        color: COLOR,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractures_grow_through_eight_bounded_stages() {
        let mut previous = 0;
        for stage in 1..=STAGES {
            let mut lines = Vec::new();
            append(
                IVec3::new(-40, 70, 130),
                stage as f32 / STAGES as f32,
                0.004,
                &mut lines,
            );
            assert_eq!(lines.len(), stage * 144);
            assert!(lines.len() > previous);
            assert!(
                lines
                    .iter()
                    .all(|line| line.pos.into_iter().all(f32::is_finite))
            );
            previous = lines.len();
        }
    }

    #[test]
    fn inactive_or_invalid_progress_draws_nothing() {
        for fraction in [0.0, -0.2, f32::NAN, f32::INFINITY] {
            let mut lines = Vec::new();
            append(IVec3::ZERO, fraction, 0.004, &mut lines);
            assert!(lines.is_empty(), "fraction {fraction}");
        }
    }

    #[test]
    fn fracture_pattern_translates_with_its_block() {
        let mut a = Vec::new();
        let mut b = Vec::new();
        append(IVec3::ZERO, 0.5, 0.004, &mut a);
        append(IVec3::new(3, -2, 7), 0.5, 0.004, &mut b);
        let offset = [3.0, -2.0, 7.0];
        for (a, b) in a.iter().zip(&b) {
            for axis in 0..3 {
                assert!((b.pos[axis] - a.pos[axis] - offset[axis]).abs() < 1e-5);
            }
        }
    }
}
