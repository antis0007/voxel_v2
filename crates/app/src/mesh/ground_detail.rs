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
/// Texture kind of a grass tuft (vertex colour, foliage lighting) and of a loose
/// stone. Must match the kind 34 / 35 branches in `world_shader.wgsl`.
const TUFT_KIND: f32 = crate::material::terrain_family::TUFT as f32;
const STONE_KIND: f32 = crate::material::terrain_family::PEBBLE as f32;
/// Blades per tuft and the per-item mesh reservation they need.
const BLADES: u32 = 6;
/// Smooth 0..1 field over blocks (bilinear hashes on a `PATCH_BLOCKS` lattice)
/// that gathers tufts into loose patches with open gaps between them. Pure
/// function of world column, so chunk reloads and remeshes keep placement.
const PATCH_METRES: f32 = 5.0;
fn patch_field(x: i32, z: i32) -> f32 {
    patch_field_metres(x as f32, z as f32)
}
fn patch_field_metres(x: f32, z: f32) -> f32 {
    let (px, pz) = (x / PATCH_METRES, z / PATCH_METRES);
    let (gx, gz) = (px.floor() as i32, pz.floor() as i32);
    let (fx, fz) = (px - px.floor(), pz - pz.floor());
    let v = |i: i32, k: i32| (hash(IVec3::new(gx + i, 7919, gz + k)) & 65535) as f32 / 65535.0;
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let a = v(0, 0) + (v(1, 0) - v(0, 0)) * sx;
    let b = v(0, 1) + (v(1, 1) - v(0, 1)) * sx;
    a + (b - a) * sz
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
                    let density = if b == B::Sand {
                        threshold / 4
                    } else if grass {
                        // Loose patches: sparse in the gaps, about twice the
                        // old density at a patch heart. Mean stays near 1x.
                        let patch = patch_field_metres(p.x as f32 * scale, p.z as f32 * scale);
                        (threshold as f32 * (0.15 + 2.2 * patch * patch)) as u32
                    } else {
                        threshold
                    };
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
                    let meadow_pebble = grass && (h >> 12).is_multiple_of(7);
                    let grass = grass && !meadow_pebble;
                    let mut color = b.color(p);
                    if meadow_pebble {
                        let shade = 0.82 + ((h >> 8) & 255) as f32 / 255.0 * 0.24;
                        color = [0.29 * shade, 0.27 * shade, 0.22 * shade];
                    } else if grass {
                        // Root colour: the shader's matted-turf green, so the
                        // tuft base sits in the ground texture. Tips lighten
                        // in `append`; a few tufts go dry at the tips.
                        let v = ((h >> 8) & 255) as f32 / 255.0;
                        color = [0.105 + v * 0.02, 0.190 + v * 0.03, 0.060];
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
        self.items.len() * (BLADES as usize * 4).max(20)
    }
    pub fn indices(&self) -> usize {
        self.items.len() * (BLADES as usize * 12).max(60)
    }
    pub fn append(&self, chunk: &GeneratedChunk, mesh: &mut ChunkMesh, lod: u8) {
        if lod > 1 {
            return;
        }
        let scale = chunk.voxel_meters;
        for d in &self.items {
            let size = (0.065 + (d.hash & 255) as f32 / 255.0 * 0.10) / scale;
            if d.grass {
                let dry_tuft = (d.hash >> 20) % 5 == 0;
                for blade in 0..BLADES {
                    let a = blade as f32 * 2.39996 + (d.hash % 100) as f32 * 0.0628;
                    let lateral = Vec3::new(a.cos(), 0.0, a.sin());
                    let variation = ((d.hash.rotate_left(blade * 5) >> 24) & 255) as f32 / 255.0;
                    let height = size * (1.1 + variation * 1.5);
                    let lean = lateral * size * (0.20 + variation * 0.45);
                    // Blades fan out from one root clump; base 2-4 cm wide.
                    let p = d.p + lateral * size * 0.22;
                    let across = Vec3::new(-a.sin(), 0.0, a.cos());
                    let half = size * (0.20 + variation * 0.08);
                    let points = [
                        p - across * half,
                        p + across * half,
                        p + lean + Vec3::Y * height + across * half * 0.18,
                        p + lean + Vec3::Y * height - across * half * 0.18,
                    ];
                    let normal = (lateral * 0.35 + Vec3::Y).normalize();
                    let tip = if dry_tuft && variation > 0.45 {
                        [d.color[0] * 3.1, d.color[1] * 1.75, d.color[2] * 1.4]
                    } else {
                        [d.color[0] * 1.45, d.color[1] * 1.55, d.color[2] * 1.3]
                    };
                    let base = mesh.vertices.len() as u32;
                    for (i, p) in points.into_iter().enumerate() {
                        mesh.vertices.push(Vertex3 {
                            pos: p.to_array(),
                            color: if i < 2 { d.color } else { tip },
                            normal: normal.to_array(),
                            surface: [0.95, if i < 2 { 0.0 } else { height * scale }, TUFT_KIND, 0.0],
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
                        let y = if ring == 0 {
                            -0.012 / scale
                        } else {
                            size * height * (0.8 + variation * 0.2)
                        };
                        let p = d.p + radial * size * radius + Vec3::Y * y;
                        let normal =
                            (radial + Vec3::Y * if ring == 0 { 0.45 } else { 1.8 }).normalize();
                        mesh.vertices.push(Vertex3 {
                            pos: p.to_array(),
                            color: d.color,
                            normal: normal.to_array(),
                            surface: [0.93, 0.03, STONE_KIND, 0.0],
                            ao_strength: 0.9,
                        });
                    }
                }
                mesh.vertices.push(Vertex3 {
                    pos: (d.p + Vec3::Y * size * height).to_array(),
                    color: d.color,
                    normal: Vec3::Y.to_array(),
                    surface: [0.93, 0.03, STONE_KIND, 0.0],
                    ao_strength: 0.9,
                });
                for i in 0..5 {
                    let next = (i + 1) % 5;
                    mesh.indices.extend([
                        base + i,
                        base + i + 5,
                        base + next,
                        base + next,
                        base + i + 5,
                        base + next + 5,
                        base + 10,
                        base + next + 5,
                        base + i + 5,
                    ]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::patch_field;

    #[test]
    fn tuft_patch_field_is_bounded_and_continuous_across_chunk_and_sign_boundaries() {
        for x in -70..70 {
            for z in [-33, -32, -1, 0, 31, 32] {
                let v = patch_field(x, z);
                assert!((0.0..=1.0).contains(&v), "patch {v} at {x},{z}");
                // Neighbouring columns differ by at most one lattice step's
                // worth of smoothstep slope, so placement never jumps at a
                // chunk edge or across zero.
                let dx = (patch_field(x + 1, z) - v).abs();
                assert!(dx <= 0.31, "jump {dx} at {x},{z}");
                assert_eq!(v, patch_field(x, z));
            }
        }
    }
}
