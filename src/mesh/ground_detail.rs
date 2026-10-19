//! Sparse metre-scale surface detail. Ownership follows the supporting voxel;
//! no moving random seeds, chunk-edge offsets, collision edits or texture sheets.
use super::{ChunkMesh, GeneratedChunk, Vertex3};
use crate::world::BlockId as B;
use glam::{IVec3, Vec3};
use std::sync::atomic::{AtomicBool, Ordering};
pub(super) struct Detail {
    p: Vec3,
    hash: u32,
    grass: bool,
    color: [f32; 3],
}
pub(super) struct DetailPlan {
    items: Vec<Detail>,
}
fn hash(p: IVec3) -> u32 {
    let mut h = (p.x as u32).wrapping_mul(0x9e3779b9)
        ^ (p.z as u32).wrapping_mul(0x85ebca6b)
        ^ (p.y as u32).wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb352d);
    h ^ (h >> 15)
}
impl DetailPlan {
    pub fn new(chunk: &GeneratedChunk, cancel: &AtomicBool) -> Option<Self> {
        let mut items = Vec::new();
        let scale = chunk.voxel_meters;
        let threshold = (scale * scale * 0.40 * 65535.0) as u32;
        for z in 0..32 {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            for x in 0..32 {
                let roof = chunk
                    .sky_ceiling
                    .get(((z + 1) * 34 + x + 1) as usize)
                    .copied();
                let min_y = roof.map_or(0, |h| (h - chunk.chunk.y * 32).clamp(0, 32));
                let max_y = roof.map_or(32, |h| (h - chunk.chunk.y * 32 + 1).clamp(0, 32));
                for y in (min_y..max_y).rev() {
                    let p = chunk.chunk * 32 + IVec3::new(x, y, z);
                    let b = chunk.get_world(p);
                    let grass = matches!(b, B::Grass | B::FungalSoil);
                    if !grass
                        && !matches!(b, B::Gravel | B::Stone | B::Sand | B::Dirt | B::Limestone)
                    {
                        continue;
                    }
                    if chunk.get_world(p + IVec3::Y) != B::Air {
                        continue;
                    }
                    let h = hash(p);
                    // Beaches should read as broad sand, not a regular field
                    // of dark stones. Keep the physical density scale invariant.
                    let density = if b == B::Sand { threshold / 4 } else { threshold };
                    if (h & 65535) > density {
                        continue;
                    }
                    let top = p.as_vec3()
                        + Vec3::new(
                            0.2 + ((h >> 16) & 255) as f32 / 425.0,
                            1.002,
                            0.2 + ((h >> 24) & 255) as f32 / 425.0,
                        );
                    if chunk.sky_visibility(top, Vec3::Y) < 0.7 {
                        continue;
                    }
                    let meadow_pebble = grass && (h >> 12) % 7 == 0;
                    let grass = grass && !meadow_pebble;
                    let mut color = b.color(p);
                    if meadow_pebble {
                        let shade = 0.82 + ((h >> 8) & 255) as f32 / 255.0 * 0.24;
                        color = [0.29 * shade, 0.27 * shade, 0.22 * shade];
                    } else if grass {
                        let dry = ((h >> 8) & 255) as f32 / 255.0;
                        color = [
                            color[0] * (0.78 + dry * 0.18),
                            color[1] * (0.76 + dry * 0.08),
                            color[2] * 0.80,
                        ];
                    } else {
                        color = [color[0] * 0.96, color[1] * 0.97, color[2] * 0.98];
                    }
                    items.push(Detail {
                        p: top,
                        hash: h,
                        grass,
                        color,
                    });
                }
            }
        }
        // Hash priority distributes the bounded budget over the whole chunk.
        items.sort_unstable_by_key(|d| d.hash);
        items.truncate(384);
        Some(Self { items })
    }
    pub fn empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn vertices(&self) -> usize {
        self.items.len() * 20
    }
    pub fn indices(&self) -> usize {
        self.items.len() * 60
    }
    pub fn append(&self, chunk: &GeneratedChunk, mesh: &mut ChunkMesh, lod: u8) {
        if lod > 1 {
            return;
        }
        let scale = chunk.voxel_meters;
        for d in &self.items {
            let size = (0.065 + (d.hash & 255) as f32 / 255.0 * 0.10) / scale;
            if d.grass {
                for blade in 0..5 {
                    let a = blade as f32 * 2.39996 + (d.hash % 100) as f32 * 0.0628;
                    let lateral = Vec3::new(a.cos(), 0.0, a.sin());
                    let variation = ((d.hash.rotate_left(blade * 5) >> 24) & 255) as f32 / 255.0;
                    let height = size * (1.0 + variation * 1.4);
                    let lean = lateral * size * (0.25 + variation * 0.55);
                    let p = d.p + lateral * size * 0.18;
                    let points = [
                        p - lateral * size * 0.16,
                        p + lateral * size * 0.16,
                        p + lean + Vec3::Y * height + lateral * size * 0.035,
                        p + lean + Vec3::Y * height - lateral * size * 0.035,
                    ];
                    let normal = Vec3::new(-a.sin(), 0.4, a.cos()).normalize();
                    let base = mesh.vertices.len() as u32;
                    for (i, p) in points.into_iter().enumerate() {
                        let shade = if i < 2 { 0.72 } else { 1.12 };
                        mesh.vertices.push(Vertex3 {
                            pos: p.to_array(),
                            color: d.color.map(|v| v * shade),
                            normal: normal.to_array(),
                            surface: [0.95, 0.0, 0.0, 0.0],
                            ao_strength: 0.85,
                        });
                    }
                    mesh.indices.extend([
                        base,
                        base + 1,
                        base + 2,
                        base,
                        base + 2,
                        base + 3,
                        base + 2,
                        base + 1,
                        base,
                        base + 3,
                        base + 2,
                        base,
                    ]);
                }
            } else {
                // Two irregular rings and a broad cap replace the identical
                // four-sided pyramids. Eleven shared vertices / 45 indices
                // remain inside the existing 20 / 60 per-item reservation.
                let base = mesh.vertices.len() as u32;
                let rotation = (d.hash >> 16) as f32 * (std::f32::consts::TAU / 65536.0);
                let height = 0.16 + ((d.hash >> 8) & 255) as f32 / 255.0 * 0.18;
                for ring in 0..2 {
                    for i in 0..5 {
                        let a = rotation + i as f32 * std::f32::consts::TAU / 5.0;
                        let variation = (d.hash.rotate_left(i * 5) & 255) as f32 / 255.0;
                        let radial = Vec3::new(a.cos(), 0.0, a.sin() * 0.72);
                        let radius = (0.72 + variation * 0.28) * if ring == 0 { 0.8 } else { 0.43 };
                        let y = if ring == 0 { -0.012 / scale } else { size * height * (0.8 + variation * 0.2) };
                        let p = d.p + radial * size * radius + Vec3::Y * y;
                        let normal = (radial + Vec3::Y * if ring == 0 { 0.45 } else { 1.8 }).normalize();
                        mesh.vertices.push(Vertex3 {
                            pos: p.to_array(),
                            color: d.color,
                            normal: normal.to_array(),
                            surface: [0.93, 0.03, 0.0, 0.0],
                            ao_strength: 0.9,
                        });
                    }
                }
                mesh.vertices.push(Vertex3 {
                    pos: (d.p + Vec3::Y * size * height).to_array(),
                    color: d.color,
                    normal: Vec3::Y.to_array(),
                    surface: [0.93, 0.03, 0.0, 0.0],
                    ao_strength: 0.9,
                });
                for i in 0..5 {
                    let next = (i + 1) % 5;
                    mesh.indices.extend([
                        base + i, base + i + 5, base + next,
                        base + next, base + i + 5, base + next + 5,
                        base + 10, base + next + 5, base + i + 5,
                    ]);
                }
            }
        }
    }
}
