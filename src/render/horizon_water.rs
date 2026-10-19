//! Clip distant water to its bed instead of flooding whole shoreline triangles.
use crate::{mesh::{ChunkMesh, Vertex3}, world::{BlockId, WorldSampleConfig}};
use glam::{IVec3, Vec3};

pub(super) fn append(mesh: &mut ChunkMesh, cfg: &WorldSampleConfig) {
    let scale = cfg.voxel_meters();
    let sea = cfg.water_level() as f32 + 1.0;
    // Match the recessed resident water surface so water lies below edge blocks
    // and transitions seamlessly between resident chunks and the horizon mesh.
    let surface = sea - crate::mesh::WATER_SURFACE_INSET;
    let count = mesh.indices.len();
    let material = BlockId::Water.surface();
    for offset in (0..count).step_by(3) {
        let triangle = [0, 1, 2].map(|i| Vec3::from_array(
            mesh.vertices[mesh.indices[offset + i] as usize].pos));
        if triangle.iter().all(|p| p.y >= surface) { continue; }
        let mut polygon = [Vec3::ZERO; 4];
        let mut len = 0;
        for i in 0..3 {
            let a = triangle[i];
            let b = triangle[(i + 1) % 3];
            let wet_a = a.y < surface;
            let wet_b = b.y < surface;
            if wet_a { polygon[len] = a; len += 1; }
            if wet_a != wet_b {
                polygon[len] = a.lerp(b, (surface - a.y) / (b.y - a.y));
                len += 1;
            }
        }
        let base = mesh.vertices.len() as u32;
        for p in &polygon[..len] {
            mesh.vertices.push(Vertex3 {
                pos: [p.x, surface, p.z], normal: [0.0, 1.0, 0.0],
                color: BlockId::Water.color(IVec3::ZERO),
                surface: [material.roughness, material.specular, 8.0,
                    ((sea - p.y) * scale).clamp(0.0, 48.0) / 16.0],
                ao_strength: -1.0,
            });
        }
        for i in 1..len.saturating_sub(1) {
            mesh.indices.extend_from_slice(&[base, base + i as u32, base + i as u32 + 1]);
        }
    }
}
