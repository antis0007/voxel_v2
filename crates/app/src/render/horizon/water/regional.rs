//! Elevated natural water over the same bed triangles as the distant terrain.
//! This is derived geometry; it never replenishes or edits simulated water.
use super::*;
use engine_worldgen::{CachedProfiles, Profiles};

#[derive(Clone, Copy)]
struct Point {
    bed: Vec3,
    level: f32,
    depth: f32,
    color: [f32; 3],
}

impl Point {
    fn crossing(self, other: Self) -> Self {
        let t = (self.depth / (self.depth - other.depth)).clamp(0.0, 1.0);
        Self {
            bed: self.bed.lerp(other.bed, t),
            level: self.level + (other.level - self.level) * t,
            depth: 0.0,
            color: filtered_bed(self.color, other.color, t),
        }
    }
}

pub(super) fn append(mesh: &mut ChunkMesh, cfg: &WorldSampleConfig, bed_indices: usize, bed_vertices: usize) {
    if cfg.generator_version < 14 || cfg.world_type == crate::world::WorldType::Superflat {
        return;
    }
    let scale = cfg.voxel_meters();
    let sea = cfg.water_level() as f32 + 1.0;
    let geology = engine_worldgen::geo::active(cfg);
    let water = BlockId::Water.surface();
    let water_key = water.shader_key();
    // Lazy, build-local samples: bounded by the already admitted terrain mesh.
    let mut levels: Vec<Option<Option<f32>>> = vec![None; bed_vertices];
    let mut lake_levels = if geology { vec![None; bed_vertices] } else { Vec::new() };
    let mut triangles = 0usize;
    let mut polygon = Vec::with_capacity(4);
    for offset in (0..bed_indices).step_by(3) {
        let ids = [
            mesh.indices[offset],
            mesh.indices[offset + 1],
            mesh.indices[offset + 2],
        ];
        let vertices = ids.map(|i| mesh.vertices[i as usize]);
        let bed = vertices.map(|v| Vec3::from_array(v.pos));
        if bed.iter().all(|p| p.y <= sea) {
            continue;
        }
        // Narrow rivers cannot be inferred across kilometre-sized triangles.
        // Keep those on the ocean path; the fine rings own inland water detail.
        let coarse = (0..3).any(|i| {
            let d = (bed[i] - bed[(i + 1) % 3]) * scale;
            d.x.abs().max(d.z.abs()) > 64.0
        });
        if coarse && !geology {
            continue;
        }
        // Skirts/vertical walls do not own a second water surface.
        if (bed[1] - bed[0]).cross(bed[2] - bed[0]).y <= 0.0 {
            continue;
        }
        let heads = std::array::from_fn::<_, 3, _>(|i| {
            let cache = if coarse { &mut lake_levels } else { &mut levels };
            *cache[ids[i] as usize].get_or_insert_with(|| {
                (if coarse {
                    engine_worldgen::geo::lake_level(cfg, bed[i].x * scale, bed[i].z * scale)
                } else {
                    CachedProfiles.river_top(cfg, bed[i].x.floor() as i32, bed[i].z.floor() as i32)
                })
                    // Generated surface cells are full voxels, just like the
                    // near mesh. Match that top instead of drawing below it.
                    .map(|m| {
                        let h=m/scale;
                        if cfg.generator_version>=79 && h.fract()>0.0 {
                            h.floor()+h.fract()*(1.0-crate::mesh::WATER_SURFACE_INSET)+crate::mesh::WATER_SURFACE_INSET
                        } else { h.ceil() }
                    })
                    .filter(|h| *h > sea)
            })
        });
        let Some(reference) = heads.into_iter().flatten().reduce(f32::min) else {
            continue;
        };
        let points = std::array::from_fn::<_, 3, _>(|i| {
            let level = heads[i].unwrap_or(reference) - crate::mesh::WATER_SURFACE_INSET;
            let delta = level - bed[i].y;
            Point {
                bed: bed[i],
                level,
                color: vertices[i].color,
                // Explicitly dry columns stay dry even in a depressed swale.
                // Borrowing the local head only locates their shoreline edge.
                depth: if heads[i].is_some() {
                    delta
                } else {
                    -delta.abs().max(0.25 / scale)
                },
            }
        });
        polygon.clear();
        for i in 0..3 {
            let (a, b) = (points[i], points[(i + 1) % 3]);
            if a.depth > 0.0 {
                polygon.push(a);
            }
            if (a.depth > 0.0) != (b.depth > 0.0) {
                polygon.push(a.crossing(b));
            }
        }
        if polygon.len() < 3 {
            continue;
        }
        let base = mesh.vertices.len() as u32;
        for p in &polygon {
            mesh.vertices.push(Vertex3 {
                pos: [p.bed.x, p.level, p.bed.z],
                normal: Vec3::Y.to_array(),
                color: p.color,
                surface: [
                    water.roughness,
                    water.specular,
                    water_key,
                    ((p.level - p.bed.y) * scale).clamp(0.0, 48.0) / 16.0,
                ],
                ao_strength: -1.0,
            });
        }
        for i in 1..polygon.len() - 1 {
            mesh.indices
                .extend_from_slice(&[base, base + i as u32, base + i as u32 + 1]);
            triangles += 1;
        }
    }
    println!("[horizon] regional_water_triangles={triangles}");
}
