//! Sub-voxel leaf sprays on exposed crown faces. Leaf voxels carry the crown's
//! occupancy; these crossed cards add the leaf-group scale the grid cannot
//! hold. Visual only: they never affect edits, light or collision. Density
//! follows the grid so a one-metre world draws none and a quarter-metre world
//! draws about one spray per three exposed faces.
use super::{ChunkMesh, GeneratedChunk, Vertex3};
use crate::world::BlockId as B;
use glam::{IVec3, Vec3};
use std::sync::atomic::{AtomicBool, Ordering};

const KIND: f32 = crate::material::terrain_family::TUFT as f32;
/// Leaf card half-size in metres.
const HALF_M: f32 = 0.17;
const MAX_INDICES: usize = 9_000 * 6;
/// Fraction of exposed faces that carry a spray per voxel size.
fn share(scale: f32) -> f32 {
    if scale > 0.6 { 0.0 } else if scale > 0.3 { 0.18 } else { 0.34 }
}

const FACES: [IVec3; 5] = [IVec3::Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

struct Spray {
    hash: u32,
    p: Vec3,
    normal: Vec3,
    color: [f32; 3],
}

pub(super) struct SprayPlan {
    items: Vec<Spray>,
}

fn hash(p: IVec3, face: usize) -> u32 {
    let mut h = (p.x as u32).wrapping_mul(0x9e37_79b9)
        ^ (p.y as u32).wrapping_mul(0x85eb_ca6b)
        ^ (p.z as u32).wrapping_mul(0xc2b2_ae35)
        ^ (face as u32 + 1).wrapping_mul(0x27d4_eb2f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 12)
}

fn unit(h: u32, shift: u32) -> f32 {
    ((h >> shift) & 255) as f32 / 255.0
}

impl SprayPlan {
    pub fn empty_plan() -> Self {
        Self { items: Vec::new() }
    }

    pub fn new(chunk: &GeneratedChunk, cancel: &AtomicBool) -> Option<Self> {
        let scale = chunk.voxel_meters;
        let share = share(scale);
        if share <= 0.0 {
            return Some(Self::empty_plan());
        }
        let mut items = Vec::new();
        for z in 0..32 {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            for y in 0..32 {
                for x in 0..32 {
                    let p = chunk.chunk * 32 + IVec3::new(x, y, z);
                    if chunk.get_world(p) != B::Leaves {
                        continue;
                    }
                    for (i, face) in FACES.iter().enumerate() {
                        if chunk.get_world(p + *face) != B::Air {
                            continue;
                        }
                        let h = hash(p, i);
                        if (h & 1023) as f32 / 1023.0 > share {
                            continue;
                        }
                        // Sideways sprays sit a little under the face centre so
                        // they hang from the block edge instead of floating.
                        let centre = p.as_vec3() + Vec3::splat(0.5) + face.as_vec3() * 0.52
                            + Vec3::new(unit(h, 8) - 0.5, (unit(h, 16) - 0.5) * 0.6 - if face.y == 0 { 0.15 } else { 0.0 }, unit(h, 24) - 0.5) * 0.5;
                        let tint = 0.88 + unit(h, 4) * 0.26;
                        items.push(Spray {
                            hash: h,
                            p: centre,
                            normal: (face.as_vec3() + Vec3::Y * 0.6).normalize(),
                            color: B::Leaves.color(p).map(|c| c * tint),
                        });
                    }
                }
            }
        }
        items.sort_unstable_by_key(|s| s.hash);
        items.truncate(MAX_INDICES / 12);
        Some(Self { items })
    }

    pub fn empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn vertices(&self) -> usize {
        self.items.len() * 8
    }

    pub fn indices(&self) -> usize {
        self.items.len() * 12
    }

    /// Two crossed cards per spray, double sided, lit by a softened face normal.
    pub fn append(&self, mesh: &mut ChunkMesh, scale: f32, lod: u8) {
        if lod > 0 {
            return;
        }
        let half = HALF_M / scale;
        for s in &self.items {
            let yaw = unit(s.hash, 12) * std::f32::consts::TAU;
            let tilt = 0.35 + unit(s.hash, 20) * 0.5;
            let a = Vec3::new(yaw.cos(), 0.0, yaw.sin());
            let b = Vec3::new(-yaw.sin(), 0.0, yaw.cos());
            // Cards lean away from the face so they read as a leaf group, not a
            // flat sticker on the block.
            let lean = s.normal * tilt;
            for (u, v) in [(a, b), (b, a)] {
                let base = mesh.vertices.len() as u32;
                let corners = [
                    s.p - u * half - (v * 0.6 - lean) * half,
                    s.p + u * half - (v * 0.6 - lean) * half,
                    s.p + u * half * 0.7 + (v * 0.6 + lean) * half,
                    s.p - u * half * 0.7 + (v * 0.6 + lean) * half,
                ];
                let shade = [0.86, 0.86, 1.0, 1.0];
                for (k, c) in corners.iter().enumerate() {
                    mesh.vertices.push(Vertex3 {
                        pos: c.to_array(),
                        color: s.color.map(|x| x * shade[k]),
                        normal: s.normal.to_array(),
                        // surface.y is the tuft wind height: a short reach so
                        // the spray stirs with the canopy instead of a grass blade.
                        surface: [0.92, 0.12 * shade[k], KIND, 0.0],
                        ao_strength: 0.9,
                    });
                }
                mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                mesh.indices.extend([base + 2, base + 1, base, base + 3, base + 2, base]);
            }
        }
    }
}
