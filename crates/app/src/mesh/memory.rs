//! Plan actual surface patches before admitting mesh output allocations.
use super::{ChunkMesh, GeneratedChunk, Vertex3, solid::SolidPlan};
use std::sync::atomic::AtomicBool;
pub const PLANNING_BYTES: usize = 24 * 1024 * 1024;
pub struct MeshPlan {
    pub vertices: usize,
    pub indices: usize,
    plan: SolidPlan,
    detail: super::ground_detail::DetailPlan,
    sprays: super::leaf_sprays::SprayPlan,
}
fn is_water_vertex(vertex: &Vertex3) -> bool {
    vertex.surface[2] % 64.0 == 8.0
}

/// Decode these two values in the vertex shader, before interpolation. Preserve
/// roof occlusion for cave water. Every face meeting a water corner samples
/// optics as the top face would, so the side wall and the surface agree on depth
/// and bed colour at their shared edge. Vertices on a cell boundary (integer y)
/// belong to the column above them.
type WaterOptics = (f32, [f32; 3], u8);
type WaterOpticsCache = [Option<([u32; 3], WaterOptics)>; 128];

fn encode_water_vertex(chunk: &GeneratedChunk, vertex: &mut Vertex3, cache: &mut WaterOpticsCache) {
    let pos = glam::Vec3::from_array(vertex.pos);
    let normal = glam::Vec3::from_array(vertex.normal);
    let at_surface = pos.y - pos.y.floor() > 0.002;
    let sample = if normal.y > 0.5 || at_surface {
        pos
    } else {
        pos + glam::Vec3::Y * 0.02
    };
    // Greedy faces and adjacent quads often repeat the same corner. Cache
    // the costly column walk without allocating per-chunk hash tables.
    let key = sample.to_array().map(f32::to_bits);
    let hash = (sample.x as i32).wrapping_mul(73)
        ^ (sample.z as i32).wrapping_mul(31)
        ^ (key[1] ^ (key[1] >> 19)) as i32;
    let slot = &mut cache[hash as usize & 127];
    let (depth, bed_color, shore) = match *slot {
        Some((cached_key, optics)) if cached_key == key => optics,
        _ => {
            let optics = chunk.water_surface_optics(sample, glam::Vec3::Y);
            *slot = Some((key, optics));
            optics
        }
    };
    vertex.color = bed_color;
    vertex.surface[3] = pack_water(chunk, sample, pos, normal, depth, shore);
}

/// Water cell corners that touch a non-water cell at the surface row: the water
/// edge against a bank, a wall, or the lower side of a flow step. The shader
/// holds these still so waves never pull the surface off its neighbours. Only
/// the four cells around the corner are read, so both chunks that share a border
/// vertex (each with a one-voxel halo) agree.
fn water_corner_flags(chunk: &GeneratedChunk, sample: glam::Vec3) -> f32 {
    let row = (sample.y - 0.00001).floor() as i32;
    let (x, z) = (sample.x.floor() as i32, sample.z.floor() as i32);
    let mut pin = 0.0;
    for (dx, dz) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
        let block = chunk.get_world(glam::IVec3::new(x + dx, row, z + dz));
        if block != crate::world::BlockId::Water {
            // A lower water step can pin geometry, but only a solid bank
            // gets a meniscus and shoreline foam.
            if block.is_solid() {
                return 4096.0 + 8192.0;
            }
            pin = 4096.0;
        }
    }
    pin
}

/// `surface.w`: `8 + depth*64 + 4096*pinned + 8192*shore + 16384*direction + sky/2`.
/// 1/64 m depth precision keeps films out of the surf band; depth caps at 48 m.
fn pack_water(
    chunk: &GeneratedChunk,
    sample: glam::Vec3,
    pos: glam::Vec3,
    normal: glam::Vec3,
    depth: f32,
    shore_direction: u8,
) -> f32 {
    8.0 + (depth.min(48.0) * 64.0).ceil() + water_corner_flags(chunk, sample)
        + 16384.0 * f32::from(shore_direction)
        + 0.5 * chunk.sky_visibility(pos, normal)
}

/// Coarse solid meshes retain exact water surfaces. Encode their optics at
/// those actual vertices with the same rule as LOD 0, including banks and rims.
pub fn encode_water(chunk: &GeneratedChunk, mesh: &mut ChunkMesh) {
    let mut optics_cache = [None; 128];
    for vertex in &mut mesh.vertices {
        if !is_water_vertex(vertex) {
            continue;
        }
        encode_water_vertex(chunk, vertex, &mut optics_cache);
    }
}

impl MeshPlan {
    pub fn for_chunk(chunk: &GeneratedChunk, cancelled: &AtomicBool, lod: u8) -> Option<Self> {
        let plan = SolidPlan::new(chunk, cancelled)?;
        let detail = if lod > 1 {
            super::ground_detail::DetailPlan::empty_plan()
        } else {
            super::ground_detail::DetailPlan::new(chunk, cancelled)?
        };
        let sprays = if lod > 0 {
            super::leaf_sprays::SprayPlan::empty_plan()
        } else {
            super::leaf_sprays::SprayPlan::new(chunk, cancelled)?
        };
        Some(Self {
            vertices: plan.vertices + detail.vertices() + sprays.vertices(),
            indices: plan.indices + detail.indices() + sprays.indices(),
            plan,
            detail,
            sprays,
        })
    }
    pub fn lod_invariant(&self) -> bool {
        self.plan.lod_invariant() && self.detail.empty() && self.sprays.empty()
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
        self.sprays.append(&mut mesh, chunk.voxel_meters, lod);
        let canopy = super::canopy::Canopy::new(chunk);
        let mut optics_cache = [None; 128];
        for vertex in &mut mesh.vertices {
            // Negative detail is torch geometry; kind 40 carries fire emission.
            if is_water_vertex(vertex) {
                encode_water_vertex(chunk, vertex, &mut optics_cache);
            } else if vertex.surface[3] >= 0.0 && vertex.surface[2] % 64.0 != 40.0 {
                vertex.surface[3] = 4.0
                    + chunk.sky_visibility(
                        glam::Vec3::from_array(vertex.pos),
                        glam::Vec3::from_array(vertex.normal),
                    );
                if !canopy.is_empty() {
                    canopy.apply(vertex);
                }
                if vertex.surface[2] % 64.0 == 10.0 {
                    super::water_motion::encode_liquid_current(chunk, vertex);
                }
            }
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::BlockId;
    use glam::IVec3;

    #[test]
    fn distant_lods_do_not_plan_discarded_ground_cover() {
        let mut chunk = GeneratedChunk::new(
            IVec3::ZERO,
            vec![BlockId::Air; (crate::mesh::PADDED_CHUNK_SIZE as usize).pow(3)],
        );
        for z in 1..=32 {
            for x in 1..=32 {
                chunk.set_world(IVec3::new(x, 1, z), BlockId::Grass);
            }
        }
        let never = AtomicBool::new(false);
        let near = MeshPlan::for_chunk(&chunk, &never, 0).unwrap();
        assert!(!near.detail.empty());
        let far = MeshPlan::for_chunk(&chunk, &never, 2).unwrap();
        assert!(far.detail.empty());
        assert_eq!(far.vertices, far.plan.vertices);
        assert_eq!(far.indices, far.plan.indices);
    }
}
