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
fn is_water_vertex(vertex: &Vertex3) -> bool {
    vertex.surface[2] % 64.0 == 8.0
}

/// Decode these two values in the vertex shader, before interpolation. Preserve
/// roof occlusion for cave water. Every face meeting a water corner samples
/// optics as the top face would, so the side wall and the surface agree on depth
/// and bed colour at their shared edge. Vertices on a cell boundary (integer y)
/// belong to the column above them.
fn encode_water_vertex(chunk: &GeneratedChunk, vertex: &mut Vertex3) {
    let pos = glam::Vec3::from_array(vertex.pos);
    let normal = glam::Vec3::from_array(vertex.normal);
    let at_surface = pos.y - pos.y.floor() > 0.002;
    let sample = if normal.y > 0.5 || at_surface {
        pos
    } else {
        pos + glam::Vec3::Y * 0.02
    };
    let (depth, bed_color) = chunk.water_optics(sample, glam::Vec3::Y);
    vertex.color = bed_color;
    vertex.surface[3] = pack_water(chunk, sample, pos, normal, depth);
}

/// Water cell corners that touch a non-water cell at the surface row: the water
/// edge against a bank, a wall, or the lower side of a flow step. The shader
/// holds these still so waves never pull the surface off its neighbours. Only
/// the four cells around the corner are read, so both chunks that share a border
/// vertex (each with a one-voxel halo) agree.
fn pinned_corner(chunk: &GeneratedChunk, sample: glam::Vec3) -> bool {
    let row = (sample.y - 0.01).floor() as i32;
    let (x, z) = (sample.x.floor() as i32, sample.z.floor() as i32);
    [(-1, -1), (0, -1), (-1, 0), (0, 0)].into_iter().any(|(dx, dz)| {
        chunk.get_world(glam::IVec3::new(x + dx, row, z + dz)) != crate::world::BlockId::Water
    })
}

/// `surface.w` for a water vertex: `8 + k + sky/2`, with `k = depth*4 + 256*pinned`.
/// Depth saturates at 48 m (192 steps), so the pin bit never collides with it.
fn pack_water(
    chunk: &GeneratedChunk,
    sample: glam::Vec3,
    pos: glam::Vec3,
    normal: glam::Vec3,
    depth: f32,
) -> f32 {
    let pinned = if pinned_corner(chunk, sample) { 256.0 } else { 0.0 };
    8.0 + (depth.min(48.0) * 4.0).ceil() + pinned + 0.5 * chunk.sky_visibility(pos, normal)
}

/// Coarse solid meshes retain exact water surfaces. Encode their optics at
/// those actual vertices with the same rule as LOD 0, including banks and rims.
pub fn encode_water(chunk: &GeneratedChunk, mesh: &mut ChunkMesh) {
    for vertex in &mut mesh.vertices {
        if !is_water_vertex(vertex) {
            continue;
        }
        encode_water_vertex(chunk, vertex);
    }
}

impl MeshPlan {
    pub fn for_chunk(chunk: &GeneratedChunk, cancelled: &AtomicBool) -> Option<Self> {
        let plan = SolidPlan::new(chunk, cancelled)?;
        let detail = super::ground_detail::DetailPlan::new(chunk, cancelled)?;
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
    pub fn build_lod(&self, chunk: &GeneratedChunk, lod: u8) -> ChunkMesh {
        let mut mesh = self.plan.build(
            chunk,
            ChunkMesh {
                vertices: Vec::with_capacity(self.vertices),
                indices: Vec::with_capacity(self.indices),
            },
            lod,
        );
        self.detail.append(chunk, &mut mesh, lod);
        for vertex in &mut mesh.vertices {
            // Negative detail is torch geometry; kind 40 carries fire emission.
            if is_water_vertex(vertex) {
                encode_water_vertex(chunk, vertex);
            } else if vertex.surface[3] >= 0.0 && vertex.surface[2] % 64.0 != 40.0 {
                vertex.surface[3] = 4.0
                    + chunk.sky_visibility(
                        glam::Vec3::from_array(vertex.pos),
                        glam::Vec3::from_array(vertex.normal),
                    );
            }
        }
        mesh
    }
}
