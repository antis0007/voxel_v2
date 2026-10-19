//! Packed occlusion masks and exact greedy-patch allocation planning.
use super::{ChunkMesh, FaceMaterial, GeneratedChunk};
use crate::world::{BlockId, CHUNK_SIZE};
use glam::IVec3;
use std::sync::atomic::{AtomicBool, Ordering};
const N: usize = 32;
const P: usize = super::PADDED_CHUNK_SIZE as usize;
const INTERIOR: u64 = ((1u64 << N) - 1) << 1;
struct Patch {
    axis: usize,
    positive: bool,
    plane: i32,
    u: i32,
    v: i32,
    width: i32,
    height: i32,
    material: FaceMaterial,
}
pub(super) struct SolidPlan {
    patches: Vec<Patch>,
    pub vertices: usize,
    pub indices: usize,
    torches: bool,
    compact_torches: bool,
    partial_water: bool,
}
impl SolidPlan {
    pub fn new(chunk: &GeneratedChunk, cancelled: &AtomicBool) -> Option<Self> {
        let mut opaque = vec![0u64; 3 * P * P];
        let mut renderable = vec![0u64; 3 * P * P];
        let mut water = vec![0u64; 3 * P * P];
        let partial_water = !chunk.water_levels.is_empty();
        let mut torches = 0;
        let mut water_faces = 0;
        for y in 0..P {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            for z in 0..P {
                for x in 0..P {
                    let b = chunk.blocks[(y * P + z) * P + x];
                    let inside =
                        (1..=N).contains(&x) && (1..=N).contains(&y) && (1..=N).contains(&z);
                    if inside && b.is_torch() {
                        torches += 1;
                    }
                    // Six faces are a conservative bound only for cells carrying partial water.
                    if inside && partial_water && b == BlockId::Water {
                        water_faces += 6;
                    }
                    let rows = [z * P + y, P * P + x * P + z, 2 * P * P + y * P + x];
                    let bits = [1u64 << x, 1u64 << y, 1u64 << z];
                    let draw = !b.is_empty()
                        && !b.is_torch()
                        && !b.is_model()
                        && (!partial_water || b != BlockId::Water);
                    for axis in 0..3 {
                        if b.is_opaque() {
                            opaque[rows[axis]] |= bits[axis];
                        }
                        if b == BlockId::Water {
                            water[rows[axis]] |= bits[axis];
                        }
                        if draw {
                            renderable[rows[axis]] |= bits[axis];
                        }
                    }
                }
            }
        }
        let compact_torches = torches > 1024;
        let torch_vertices = if compact_torches { 48 } else { 320 };
        let mut plan = Self {
            patches: Vec::new(),
            vertices: torches * torch_vertices + water_faces * 4,
            indices: torches * (torch_vertices / 4 * 6) + water_faces * 6,
            torches: torches > 0,
            compact_torches,
            partial_water,
        };
        let mut masks = vec![None; (N + 1) * N * N];
        for axis in 0..3 {
            let ua = (axis + 1) % 3;
            let va = (axis + 2) % 3;
            for positive in [false, true] {
                if cancelled.load(Ordering::Relaxed) {
                    return None;
                }
                masks.fill(None);
                let mut planes = 0u64;
                for v in 0..N {
                    for u in 0..N {
                        let row = axis * P * P + (v + 1) * P + u + 1;
                        let neighbor = if positive {
                            opaque[row] >> 1
                        } else {
                            opaque[row] << 1
                        };
                        // Internal water faces were previously visited one voxel
                        // at a time even in completely submerged ocean volumes.
                        let adjacent_water = if positive {
                            water[row] >> 1
                        } else {
                            water[row] << 1
                        };
                        let mut visible =
                            ((renderable[row] & !neighbor & !(water[row] & adjacent_water))
                                & INTERIOR)
                                >> 1;
                        while visible != 0 {
                            let depth = visible.trailing_zeros() as usize;
                            visible &= visible - 1;
                            let mut p = [0; 3];
                            p[axis] = depth as i32;
                            p[ua] = u as i32;
                            p[va] = v as i32;
                            let mut q = p;
                            q[axis] += if positive { 1 } else { -1 };
                            let b = super::mesh_block_at(chunk, p);
                            if !super::face_visible(b, super::mesh_block_at(chunk, q)) {
                                continue;
                            }
                            let plane = depth + usize::from(positive);
                            planes |= 1 << plane;
                            let world_p = chunk.chunk * CHUNK_SIZE + IVec3::from_array(p);
                            let water_bucket = if b == BlockId::Water && axis == 1 && positive {
                                let mut cells = 0u8;
                                let mut sample_p = world_p;
                                while cells < 6
                                    && sample_p.y >= chunk.origin().y
                                    && chunk.get_world(sample_p) == BlockId::Water
                                {
                                    cells += 1;
                                    sample_p.y -= 1;
                                }
                                if sample_p.y < chunk.origin().y {
                                    6
                                } else {
                                    cells
                                }
                            } else {
                                0
                            };
                            masks[plane * N * N + v * N + u] = Some(
                                FaceMaterial::from_block_with_water_bucket(b, world_p, water_bucket),
                            );
                        }
                    }
                }
                while planes != 0 {
                    let plane = planes.trailing_zeros() as usize;
                    planes &= planes - 1;
                    let mask = &mut masks[plane * N * N..(plane + 1) * N * N];
                    let mut i = 0;
                    while i < N * N {
                        let Some(material) = mask[i] else {
                            i += 1;
                            continue;
                        };
                        let u = i % N;
                        let v = i / N;
                        let is_water_top =
                            material.block == BlockId::Water && axis == 1 && positive;
                        let max_water_span = if !is_water_top {
                            N
                        } else if material.water_depth_bucket <= 2 {
                            1
                        } else {
                            2
                        };
                        let mut w = 1;
                        while u + w < N
                            && w < max_water_span
                            && (!is_water_top || (u + w) % 2 == 1)
                            && mask[i + w] == Some(material)
                        {
                            w += 1;
                        }
                        let mut h = 1;
                        'grow: while v + h < N
                            && h < max_water_span
                            && (!is_water_top || (v + h) % 2 == 1)
                        {
                            for x in 0..w {
                                if mask[(v + h) * N + u + x] != Some(material) {
                                    break 'grow;
                                }
                            }
                            h += 1;
                        }
                        let border = |p| p == 0 || p == N;
                        let rim = {
                            (if border(v) { w } else { 1 })
                                + (if border(v + h) { w } else { 1 })
                                + (if border(u) { h } else { 1 })
                                + (if border(u + w) { h } else { 1 })
                        };
                        plan.vertices += rim + usize::from(rim > 4);
                        plan.indices += if rim > 4 { rim * 3 } else { 6 };
                        plan.patches.push(Patch {
                            axis,
                            positive,
                            plane: plane as i32,
                            u: u as i32,
                            v: v as i32,
                            width: w as i32,
                            height: h as i32,
                            material,
                        });
                        for y in v..v + h {
                            mask[y * N + u..y * N + u + w].fill(None);
                        }
                        i += w;
                    }
                }
            }
        }
        Some(plan)
    }
    pub fn lod_invariant(&self) -> bool {
        !self.torches
    }
    pub fn build(&self, chunk: &GeneratedChunk, mut mesh: ChunkMesh, lod: u8) -> ChunkMesh {
        for p in &self.patches {
            super::emit_greedy_face(
                &mut mesh, chunk, p.axis, p.positive, p.plane, p.u, p.v, p.width, p.height,
                p.material,
            );
        }
        if self.torches {
            super::append_torch_mesh(&mut mesh, chunk, lod, self.compact_torches);
        }
        if self.partial_water {
            super::append_water_mesh(&mut mesh, chunk);
        }
        mesh
    }
}
