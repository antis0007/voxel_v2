//! All LODs share unit-spaced chunk-edge vertices. Greedy interiors can use
//! larger patches without creating cross-chunk T-junctions or hiding cracks.
use super::{ChunkMesh, FaceMaterial, GeneratedChunk, Vertex3};
use glam::{IVec3, Vec3};

pub(super) fn emit_conforming_face(
    mesh: &mut ChunkMesh,
    chunk: &GeneratedChunk,
    corners: [[i32; 3]; 4],
    steps: [i32; 4],
    material: FaceMaterial,
    normal: [f32; 3],
    ao: [f32; 4],
) -> bool {
    if steps.iter().all(|&count| count == 1) {
        return false;
    }
    let origin = chunk.chunk * crate::world::CHUNK_SIZE;
    let detail = material.block.surface();
    let water_top = material.block == crate::world::BlockId::Water && normal[1] > 0.5;
    let vertex = |mut position: Vec3, occlusion: f32| {
        if water_top {
            position.y -= super::WATER_SURFACE_INSET;
        }
        Vertex3 {
            pos: position.to_array(),
            color: material.color(),
            normal,
            surface: [
                detail.roughness,
                detail.specular,
                detail.shader_key(),
                detail.detail_strength,
            ],
            ao_strength: 1.0 - (1.0 - occlusion) * detail.ao_strength,
        }
    };
    let center = corners
        .iter()
        .map(|&p| IVec3::from_array(p).as_vec3())
        .sum::<Vec3>()
        * 0.25;
    let center_index = mesh.vertices.len() as u32;
    mesh.vertices.push(vertex(
        origin.as_vec3() + center,
        ao.iter().sum::<f32>() * 0.25,
    ));
    let ring_start = mesh.vertices.len() as u32;
    for edge in 0..4 {
        let start = IVec3::from_array(corners[edge]);
        let end = IVec3::from_array(corners[(edge + 1) % 4]);
        let increment = (end - start) / steps[edge];
        for i in 0..steps[edge] {
            // Integer construction matches the adjacent chunk's exact position;
            // interpolating large world-space float endpoints can round differently.
            let position = (origin + start + increment * i).as_vec3();
            let t = i as f32 / steps[edge] as f32;
            mesh.vertices.push(vertex(
                position,
                ao[edge] + (ao[(edge + 1) % 4] - ao[edge]) * t,
            ));
        }
    }
    let count = mesh.vertices.len() as u32 - ring_start;
    for i in 0..count {
        mesh.indices.extend_from_slice(&[
            center_index,
            ring_start + i,
            ring_start + (i + 1) % count,
        ]);
    }
    true
}
