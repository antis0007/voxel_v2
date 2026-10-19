//! Plan actual surface patches before admitting mesh output allocations.
use super::{ChunkMesh, GeneratedChunk, Vertex3, solid::SolidPlan};
use std::sync::atomic::AtomicBool;
pub const PLANNING_BYTES: usize = 24 * 1024 * 1024;
pub struct MeshPlan {
    pub vertices: usize,
    pub indices: usize,
    plan: SolidPlan,
    detail: super::ground_detail::DetailPlan,
}
impl MeshPlan {
    pub fn for_chunk(chunk: &GeneratedChunk, cancelled: &AtomicBool) -> Option<Self> {
        let plan = SolidPlan::new(chunk, cancelled)?;
        let detail=super::ground_detail::DetailPlan::new(chunk,cancelled)?;
        Some(Self {
            vertices: plan.vertices + detail.vertices(),
            indices: plan.indices + detail.indices(),
            plan,
            detail,
        })
    }
    pub fn lod_invariant(&self) -> bool {
        self.plan.lod_invariant() && self.detail.empty()
    }
    pub fn bytes(&self) -> usize {
        self.vertices * std::mem::size_of::<Vertex3>() + self.indices * 4 + PLANNING_BYTES
    }
    pub fn build(&self, chunk: &GeneratedChunk) -> ChunkMesh {
        self.build_lod(chunk, 0)
    }
    pub fn build_lod(&self, chunk: &GeneratedChunk, lod: u8) -> ChunkMesh {
        let mut mesh = self.plan.build(
            chunk,
            ChunkMesh {
                vertices: Vec::with_capacity(self.vertices),
                indices: Vec::with_capacity(self.indices),
            },
            lod,
        );
        self.detail.append(chunk,&mut mesh,lod);
        for vertex in &mut mesh.vertices {
            // Negative detail is torch geometry; kind 40 carries fire emission.
            if vertex.surface[2] % 64.0 == 8.0 {
                let pos = glam::Vec3::from_array(vertex.pos);
                let normal = glam::Vec3::from_array(vertex.normal);
                // Decode these two values in the vertex shader, before
                // interpolation. Preserve roof occlusion for cave water.
                let (depth, bed_color) = chunk.water_optics(pos, normal);
                if normal.y > 0.5 { vertex.color = bed_color; }
                vertex.surface[3] = 8.0 + (depth.min(48.0) * 4.0).ceil()
                    + 0.5 * chunk.sky_visibility(pos, normal);
            } else if vertex.surface[3] >= 0.0 && vertex.surface[2] % 64.0 != 40.0 {
                vertex.surface[3] = 4.0 + chunk.sky_visibility(
                    glam::Vec3::from_array(vertex.pos), glam::Vec3::from_array(vertex.normal));
            }
        }
        mesh
    }
}
