//! Low, fractured river stones. Real face normals carry broad light planes;
//! the upper cleavage plane replaces the old smooth little dome.
use super::{ChunkMesh, Detail, STONE_KIND, Vec3, Vertex3};

pub(super) const VERTICES: usize = 25;
pub(super) const INDICES: usize = 39;

pub(super) fn append(mesh: &mut ChunkMesh, d: &Detail, scale: f32, size: f32) {
    let rotation = (d.hash >> 16) as f32 * (std::f32::consts::TAU / 65536.0);
    let height = size * (0.22 + ((d.hash >> 8) & 255) as f32 / 255.0 * 0.16);
    let tilt = Vec3::new(rotation.cos() * 0.07, 0.0, rotation.sin() * 0.07);
    let lower: [Vec3; 5] = std::array::from_fn(|i| {
        let a = rotation + i as f32 * std::f32::consts::TAU / 5.0;
        let variation = (d.hash.rotate_left(i as u32 * 5) & 255) as f32 / 255.0;
        Vec3::new(a.cos(), 0.0, a.sin() * 0.72) * size * (0.72 + variation * 0.25)
            - Vec3::Y * (0.012 / scale)
    });
    let upper: [Vec3; 5] = lower.map(|p| {
        let q = Vec3::new(p.x, 0.0, p.z) * 0.62 + tilt * size * 1.4;
        q + Vec3::Y * (height + tilt.dot(q))
    });
    let base = mesh.vertices.len() as u32;
    let mut vertex = |p: Vec3, normal: Vec3, gain: f32| {
        mesh.vertices.push(Vertex3 {
            pos: (d.p + p).to_array(),
            color: d.color.map(|c| c * gain),
            normal: normal.to_array(),
            surface: [0.95, 0.03, STONE_KIND, 0.0],
            ao_strength: if p.y < 0.0 { 0.74 } else { 0.94 },
        });
    };
    for i in 0..5 {
        let next = (i + 1) % 5;
        let normal = (upper[i] - lower[i]).cross(lower[next] - lower[i]).normalize();
        let gain = 0.96 + ((d.hash.rotate_left(i as u32 * 3) >> 24) & 255) as f32 / 255.0 * 0.08;
        for p in [lower[i], upper[i], upper[next], lower[next]] {
            vertex(p, normal, gain);
        }
    }
    let normal = Vec3::new(-tilt.x, 1.0, -tilt.z).normalize();
    for p in upper { vertex(p, normal, 1.0); }
    for i in 0..5 {
        let b = base + i * 4;
        mesh.indices.extend([b, b+1, b+2, b, b+2, b+3]);
    }
    for i in 1..4 { mesh.indices.extend([base+20, base+21+i, base+20+i]); }
}
