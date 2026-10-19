//! Shared dual-grid vertices: natural geology relaxes, authored/structural cells pin.
//! The voxel connectivity is retained; no height-only surface or hidden cave fill.
use super::{ChunkMesh, FaceMaterial, GeneratedChunk, Vertex3};
use crate::world::{BlockId as B, CHUNK_SIZE};
use glam::{IVec3, Vec3};
use std::collections::HashMap;

pub(super) fn natural(b: B) -> bool {
    matches!(
        b,
        B::Grass
            | B::Dirt
            | B::Stone
            | B::Sand
            | B::Gravel
            | B::Clay
            | B::Mud
            | B::Basalt
            | B::Sandstone
            | B::SnowGrass
            | B::Limestone
            | B::FungalSoil
            | B::CoalOre
            | B::IronOre
            | B::CopperOre
            | B::TinOre
    )
}
#[derive(Clone, Copy)]
struct Corner {
    offset: Vec3,
    normal: Vec3,
    visibility: f32,
}

pub(super) struct Sculptor {
    lod: u8,
    corners: Vec<Option<Corner>>,
    vertices: HashMap<(u32, u32, u8), u32>,
}
impl Sculptor {
    pub fn new(lod: u8) -> Self {
        Self {
            lod,
            corners: vec![None; 33 * 33 * 33],
            vertices: HashMap::new(),
        }
    }
    fn corner(&mut self, chunk: &GeneratedChunk, p: IVec3) -> Corner {
        let key = ((p.y * 33 + p.z) * 33 + p.x) as usize;
        if let Some(c) = self.corners[key] {
            return c;
        }
        let world = chunk.chunk * CHUNK_SIZE + p;
        let mut solid = 0u8;
        let mut pin = false;
        for i in 0..8 {
            let q = world + IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1) - IVec3::ONE;
            let block = chunk.get_world(q);
            if block.is_opaque() {
                solid |= 1 << i;
            }
            pin |= !natural(block) && !block.is_empty() && !block.is_liquid();
        }
        let mut gradient = Vec3::ZERO;
        let mut intersection = Vec3::ZERO;
        let mut count = 0;
        for i in 0..8usize {
            let a = Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32);
            let inside = (solid >> i) & 1;
            gradient += (Vec3::splat(1.0) - a * 2.0) * inside as f32;
            for axis in 0..3 {
                let bit = 1 << axis;
                if i & bit == 0 && inside != ((solid >> (i | bit)) & 1) {
                    let mut edge = a;
                    edge[axis] = 0.5;
                    intersection += edge;
                    count += 1;
                }
            }
        }
        // Ambiguous checkerboard cells stay on the source grid. Smoothing may
        // not join separate solids or pinch a disconnected air passage closed.
        let hard = pin;
        pin |= disconnected(solid) || disconnected(!solid);
        let offset = if pin || count == 0 {
            Vec3::ZERO
        } else {
            ((intersection / count as f32 - Vec3::splat(0.5)) * 1.35)
                .clamp(Vec3::splat(-0.45), Vec3::splat(0.45))
        };
        let result = Corner {
            offset,
            normal: if hard {
                Vec3::ZERO
            } else {
                gradient.normalize_or_zero()
            },
            visibility: 0.64 + 0.36 * ((8 - solid.count_ones()) as f32 / 4.0).min(1.0),
        };
        self.corners[key] = Some(result);
        result
    }
    fn vertex(
        &mut self,
        mesh: &mut ChunkMesh,
        chunk: &GeneratedChunk,
        p: IVec3,
        material: FaceMaterial,
        fallback: Vec3,
    ) -> (u32, Vec3, Vec3) {
        let c = self.corner(chunk, p);
        let n = if c.normal.length_squared() > 0.1 {
            c.normal
        } else {
            fallback
        };
        let group = if c.normal.length_squared() > 0.1 {
            6
        } else {
            ((fallback.x.abs() < 0.5) as u8 + (fallback.z.abs() > 0.5) as u8) * 2
                + (fallback.min_element() < -0.5) as u8
        };
        let cell = ((p.y * 33 + p.z) * 33 + p.x) as u32;
        let key = (cell, material.block.as_u32(), group);
        let position = (chunk.chunk * CHUNK_SIZE + p).as_vec3() + c.offset;
        if let Some(&i) = self.vertices.get(&key) {
            return (i, position, n);
        }
        let i = mesh.vertices.len() as u32;
        mesh.vertices
            .push(vertex(position, n, material, c.visibility));
        // Deduplication is useful but optional; its scratch cannot grow with
        // adversarial material fragmentation beyond the worker reservation.
        if self.vertices.len() < 16_384 {
            self.vertices.insert(key, i);
        }
        (i, position, n)
    }
    pub fn emit(
        &mut self,
        mesh: &mut ChunkMesh,
        chunk: &GeneratedChunk,
        corners: [IVec3; 4],
        material: FaceMaterial,
        normal: Vec3,
    ) {
        let mut rim = Vec::with_capacity(128);
        for edge in 0..4 {
            let a = corners[edge];
            let b = corners[(edge + 1) % 4];
            let delta = b - a;
            let steps = delta.abs().max_element();
            let increment = delta / steps;
            let boundary =
                (0..3).any(|axis| delta[axis] == 0 && (a[axis] == 0 || a[axis] == CHUNK_SIZE));
            for i in 0..steps {
                let p = a + increment * i;
                if i > 0 && !boundary {
                    let before = self.corner(chunk, p - increment);
                    let center = self.corner(chunk, p);
                    let after = self.corner(chunk, p + increment);
                    // Only remove geometrically collinear vertices. Distance may
                    // relax shading detail, never move a silhouette or chunk seam.
                    let normal_error = [0.001, 0.025, 0.10, 0.30][self.lod.min(3) as usize];
                    let ao_error = [0.01, 0.04, 0.08, 0.16][self.lod.min(3) as usize];
                    if (before.offset + after.offset - center.offset * 2.0).length_squared() < 1e-8
                        && (before.normal + after.normal - center.normal * 2.0).length_squared()
                            < normal_error
                        && (before.visibility + after.visibility - center.visibility * 2.0).abs()
                            < ao_error
                    {
                        continue;
                    }
                }
                rim.push(self.vertex(mesh, chunk, p, material, normal));
            }
        }
        if rim.len() == 4
            && (rim[1].1 - rim[0].1)
                .cross(rim[2].1 - rim[0].1)
                .dot(rim[3].1 - rim[0].1)
                .abs()
                < 1e-5
        {
            mesh.indices
                .extend_from_slice(&[rim[0].0, rim[1].0, rim[2].0, rim[0].0, rim[2].0, rim[3].0]);
            return;
        }
        let center = mesh.vertices.len() as u32;
        // The fan centre belongs to the relaxed surface too. Pinning it to the
        // old block face produces a raised diamond in every non-planar patch.
        let position = rim.iter().map(|p| p.1).sum::<Vec3>() / rim.len() as f32;
        let n = rim.iter().map(|p| p.2).sum::<Vec3>().normalize_or_zero();
        mesh.vertices.push(vertex(
            position,
            if n.length_squared() > 0.1 { n } else { normal },
            material,
            1.0,
        ));
        for i in 0..rim.len() {
            mesh.indices
                .extend_from_slice(&[center, rim[i].0, rim[(i + 1) % rim.len()].0]);
        }
    }
}

fn disconnected(bits: u8) -> bool {
    if bits == 0 {
        return false;
    }
    let mut reached = 1u8 << bits.trailing_zeros();
    loop {
        let next = reached
            | (((reached & 0x55) << 1)
                | ((reached & 0xaa) >> 1)
                | ((reached & 0x33) << 2)
                | ((reached & 0xcc) >> 2)
                | ((reached & 0x0f) << 4)
                | ((reached & 0xf0) >> 4))
                & bits;
        if next == reached {
            return reached != bits;
        }
        reached = next;
    }
}
fn vertex(position: Vec3, normal: Vec3, material: FaceMaterial, visibility: f32) -> Vertex3 {
    let s = material.block.surface();
    Vertex3 {
        pos: position.to_array(),
        color: material.color(),
        normal: normal.to_array(),
        surface: [
            s.roughness,
            s.specular,
            s.shader_key(),
            s.detail_strength,
        ],
        ao_strength: 1.0 - (1.0 - visibility) * s.ao_strength,
    }
}
