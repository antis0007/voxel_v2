//! Compare the shared-corner emitter with the original face-by-face geometry.
use super::{Model, Vertex3};
use glam::{Quat, Vec3};

fn reference_triangle(model: &mut Model, mut points: [Vec3; 3], normal: Vec3,
    color: [f32; 3], glow: f32) {
    if (points[1] - points[0]).cross(points[2] - points[0]).dot(normal) < 0.0 {
        points.swap(1, 2);
    }
    for point in points {
        model.vertices.push(Vertex3 {
            pos: (point / model.scale).to_array(), normal: normal.to_array(), color,
            surface: [
                if glow > 0.0 && model.alpha < 0.995 {
                    -model.alpha.clamp(0.06, 0.98)
                } else { 0.48 },
                if model.dissolve > 0.0 { -model.dissolve.clamp(0.0, 1.0) } else { 0.42 },
                if glow > 0.0 { 40.0 } else { model.surface_kind }, glow,
            ],
            ao_strength: 1.0,
        });
    }
}

fn reference_cuboid(model: &mut Model, center: Vec3, size: Vec3, rotation: Quat,
    color: [f32; 3], glow: f32) {
    for (normal, u, v) in [
        (Vec3::X, Vec3::Y, Vec3::Z), (-Vec3::X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::Z, Vec3::X), (-Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y), (-Vec3::Z, Vec3::Y, Vec3::X),
    ] {
        let face = normal * size * 0.5;
        let u = u * size * 0.5;
        let v = v * size * 0.5;
        let points = [
            center + rotation * (face - u - v), center + rotation * (face + u - v),
            center + rotation * (face + u + v), center + rotation * (face - u + v),
        ];
        reference_triangle(model, [points[0], points[1], points[2]], rotation * normal, color, glow);
        reference_triangle(model, [points[0], points[2], points[3]], rotation * normal, color, glow);
    }
}

fn same_vertices(expected: &Model, actual: &Model) {
    assert_eq!(expected.vertices.len(), actual.vertices.len());
    for (index, (expected, actual)) in expected.vertices.iter().zip(&actual.vertices).enumerate() {
        assert_eq!(expected.pos, actual.pos, "position at vertex {index}");
        assert_eq!(expected.normal, actual.normal, "normal at vertex {index}");
        assert_eq!(expected.color, actual.color, "color at vertex {index}");
        assert_eq!(expected.surface, actual.surface, "material at vertex {index}");
        assert_eq!(expected.ao_strength, actual.ao_strength, "AO at vertex {index}");
    }
}

#[test]
fn shared_corners_preserve_cuboid_vertices_and_winding() {
    for scale in [1.0, 0.5, 0.25, 0.125, -1.5] {
        let mut expected = Model::with_vertices(scale, Vec::new());
        let mut actual = Model::with_vertices(scale, Vec::new());
        for center in [Vec3::ZERO, Vec3::new(0.01, -0.57, 19.25), Vec3::new(49_000.0, -100.125, 100_000.0)] {
            for size in [
                Vec3::new(0.01, 0.9, 2.0), Vec3::ZERO, Vec3::new(-0.0, 0.1, 0.3),
                Vec3::splat(-1.0), Vec3::new(1.0, -2.0, 3.0), Vec3::new(-1.0, 2.0, -3.0),
                Vec3::new(1e-12, 1e-8, 1e-4),
            ] {
                for angle in [0.0, 0.1, -0.3, 0.7, 1.0, 2.0, std::f32::consts::PI] {
                    for (alpha, dissolve, kind, glow) in [
                        (1.0, 0.0, 39.0, 0.0), (0.3, 0.5, 69.0, 0.8),
                        (0.997, 1.1, 71.0, 0.0), (-0.1, -0.5, 41.0, 1.0),
                    ] {
                        for model in [&mut expected, &mut actual] {
                            model.vertices.clear();
                            model.alpha = alpha;
                            model.dissolve = dissolve;
                            model.surface_kind = kind;
                        }
                        let rotation = Quat::from_euler(glam::EulerRot::YXZ, angle, angle * 0.7, angle * 0.3);
                        reference_cuboid(&mut expected, center, size, rotation, [0.23, 0.51, 0.87], glow);
                        actual.cuboid(center, size, rotation, [0.23, 0.51, 0.87], glow);
                        assert_eq!(actual.vertices.len(), 36);
                        same_vertices(&expected, &actual);
                    }
                }
            }
        }
    }
}

#[test]
fn shared_surface_preserves_triangle_materials() {
    let mut expected = Model::with_vertices(0.25, Vec::new());
    let mut actual = Model::with_vertices(0.25, Vec::new());
    for alpha in [-1.0, 0.0, 0.3, 0.98, 0.995, 1.0] {
        for dissolve in [-0.5, 0.0, 0.5, 1.1] {
            for glow in [0.0, 0.8] {
                expected.alpha = alpha;
                actual.alpha = alpha;
                expected.dissolve = dissolve;
                actual.dissolve = dissolve;
                let points = [Vec3::ZERO, Vec3::Z, Vec3::X];
                reference_triangle(&mut expected, points, Vec3::Y, [0.1, 0.3, 0.5], glow);
                actual.triangle(points, Vec3::Y, [0.1, 0.3, 0.5], glow);
            }
        }
    }
    same_vertices(&expected, &actual);
}
