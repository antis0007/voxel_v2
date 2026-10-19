//! Exact voxel surfaces replace only the overlapping procedural backdrop.
//! All region IO happens on the horizon worker against an immutable snapshot.
use crate::{
    mesh::{ChunkMesh, Vertex3},
    world::{BlockId as B, ChunkData, WorldSnapshot, chunk_coord_from_world_block as chunk},
};
use glam::{IVec3, Vec3};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
fn clip(poly: &[Vertex3], axis: usize, bound: f32, less: bool) -> Vec<Vertex3> {
    let mut result = Vec::new();
    if poly.is_empty() {
        return result;
    }
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let inside = |v: Vertex3| {
            if less {
                v.pos[axis] <= bound
            } else {
                v.pos[axis] >= bound
            }
        };
        if inside(a) {
            result.push(a);
        }
        if inside(a) != inside(b) {
            let t = (bound - a.pos[axis]) / (b.pos[axis] - a.pos[axis]);
            let mut v = a;
            for j in 0..3 {
                v.pos[j] = a.pos[j] + (b.pos[j] - a.pos[j]) * t;
                v.normal[j] = a.normal[j] + (b.normal[j] - a.normal[j]) * t;
                v.color[j] = a.color[j] + (b.color[j] - a.color[j]) * t;
            }
            result.push(v);
        }
    }
    result
}
fn append(mesh: &mut ChunkMesh, p: &[Vertex3]) {
    if p.len() < 3 {
        return;
    }
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(p);
    for i in 1..p.len() - 1 {
        let a = Vec3::from_array(p[0].pos);
        let b = Vec3::from_array(p[i].pos);
        let c = Vec3::from_array(p[i + 1].pos);
        if (b - a).cross(c - a).length_squared() > 1e-12 {
            mesh.indices
                .extend([base, base + i as u32, base + i as u32 + 1]);
        }
    }
}
pub(super) fn apply(
    mesh: &mut ChunkMesh,
    snapshot: &WorldSnapshot,
    center: (i32, i32),
    cancel: &AtomicBool,
) -> Option<()> {
    let scale = snapshot.cfg.voxel_meters();
    let c = chunk(IVec3::new(
        (center.0 as f32 / scale) as i32,
        0,
        (center.1 as f32 / scale) as i32,
    ));
    let radius = (17000.0 / (32.0 * scale)).ceil() as i32;
    let mut coords: Vec<_> = snapshot
        .edited_coords_near(c, radius)
        .filter(|q| (q.x - c.x).abs().max((q.z - c.z).abs()) <= radius)
        .collect();
    coords.sort_by_key(|q| (q.x, q.z, q.y));
    coords.dedup();
    if coords.is_empty() {
        return Some(());
    }
    let mut columns: BTreeMap<(i32, i32), Vec<i32>> = BTreeMap::new();
    for &q in &coords {
        columns.entry((q.x, q.z)).or_default().push(q.y);
    }
    let old_indices = std::mem::take(&mut mesh.indices);
    // Keep shared source vertices. A tiny edit previously expanded every
    // untouched triangle into three unique vertices, exceeding the GPU buffer
    // limit in dense woodland. Only intersected polygons need new vertices.
    for tri in old_indices.chunks_exact(3) {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let triangle = [mesh.vertices[tri[0] as usize], mesh.vertices[tri[1] as usize], mesh.vertices[tri[2] as usize]];
        let min = triangle.iter().fold(Vec3::splat(f32::INFINITY), |a, v| {
            a.min(Vec3::from_array(v.pos))
        });
        let max = triangle
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |a, v| {
                a.max(Vec3::from_array(v.pos))
            });
        let xmin = (min.x.floor() as i32).div_euclid(32);
        let xmax = (max.x.ceil() as i32).div_euclid(32);
        let intersects = columns.range((xmin, i32::MIN)..=(xmax, i32::MAX)).any(|(&(x,z),ys)| {
            min.x <= (x*32+32) as f32 && max.x >= (x*32) as f32
                && min.z <= (z*32+32) as f32 && max.z >= (z*32) as f32
                && ys.iter().any(|&y| min.y <= (y*32+32) as f32 && max.y >= (y*32) as f32)
        });
        if !intersects {
            mesh.indices.extend_from_slice(tri);
            continue;
        }
        let mut polygons = vec![triangle.to_vec()];
        for (&(x, z), ys) in columns.range((xmin, i32::MIN)..=(xmax, i32::MAX)) {
            if (z * 32) as f32 > max.z || ((z * 32 + 32) as f32) < min.z {
                continue;
            }
            for &y in ys {
                if max.y < (y * 32) as f32 || min.y > (y * 32 + 32) as f32 {
                    continue;
                }
                let mut next = Vec::new();
                for poly in polygons {
                    let mut inside = poly;
                    // Subtract the exact 3D edit volume, retaining slope portions
                    // above/below it and all neighbouring triangles.
                    for (axis, bound, less) in [
                        (0, (x * 32) as f32, true),
                        (0, (x * 32 + 32) as f32, false),
                        (2, (z * 32) as f32, true),
                        (2, (z * 32 + 32) as f32, false),
                        (1, (y * 32) as f32, true),
                        (1, (y * 32 + 32) as f32, false),
                    ] {
                        let outside = clip(&inside, axis, bound, less);
                        if outside.len() >= 3
                            && outside.iter().any(|v| {
                                if less {
                                    v.pos[axis] < bound
                                } else {
                                    v.pos[axis] > bound
                                }
                            })
                        {
                            next.push(outside);
                        }
                        inside = clip(&inside, axis, bound, !less);
                    }
                }
                polygons = next;
            }
        }
        for poly in polygons {
            append(mesh, &poly);
        }
    }
    for q in coords {
        let mut data: HashMap<IVec3, Arc<ChunkData>> = HashMap::new();
        for d in [
            IVec3::ZERO,
            IVec3::X,
            IVec3::NEG_X,
            IVec3::Y,
            IVec3::NEG_Y,
            IVec3::Z,
            IVec3::NEG_Z,
        ] {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            let k = q + d;
            if let Some(value) = snapshot.chunks.get(&k) {
                data.insert(k, value.clone());
            } else if snapshot.regions.records.contains_key(&k) {
                data.insert(k, snapshot.regions.load(k).ok()??.chunk);
            }
        }
        let read = |p: IVec3| {
            data.get(&chunk(p)).map_or_else(
                || crate::world::sample_block(&snapshot.cfg, p),
                |d| d.blocks[ChunkData::local_index(p.x, p.y, p.z)],
            )
        };
        let base = q * 32;
        let cells = data.get(&q)?;
        for axis in 0..3 {
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            for positive in [false, true] {
                let mut normal = IVec3::ZERO;
                normal[axis] = if positive { 1 } else { -1 };
                for plane in 0..32 {
                    if cancel.load(Ordering::Relaxed) {
                        return None;
                    }
                    let mut mask = [B::Air; 1024];
                    for j in 0..32 {
                        for i in 0..32 {
                            let mut p = base;
                            p[axis] += plane;
                            p[u] += i;
                            p[v] += j;
                            let b = cells.blocks[ChunkData::local_index(p.x, p.y, p.z)];
                            if b.is_empty() {
                                continue;
                            }
                            let n = read(p + normal);
                            if n.is_opaque() || n == b {
                                continue;
                            }
                            mask[(j * 32 + i) as usize] = b;
                        }
                    }
                    for j in 0..32usize {
                        let mut i = 0;
                        while i < 32 {
                            let b = mask[j * 32 + i];
                            if b == B::Air {
                                i += 1;
                                continue;
                            }
                            let mut width = 1;
                            while i + width < 32 && mask[j * 32 + i + width] == b {
                                width += 1;
                            }
                            let mut height = 1;
                            while j + height < 32
                                && (0..width).all(|k| mask[(j + height) * 32 + i + k] == b)
                            {
                                height += 1;
                            }
                            let mut p = base;
                            p[axis] += plane + i32::from(positive);
                            p[u] += i as i32;
                            p[v] += j as i32;
                            let mut du = IVec3::ZERO;
                            du[u] = width as i32;
                            let mut dv = IVec3::ZERO;
                            dv[v] = height as i32;
                            let points = if positive {
                                [p, p + du, p + du + dv, p + dv]
                            } else {
                                [p, p + dv, p + du + dv, p + du]
                            };
                            let surf = b.surface();
                            let face = points.map(|p| Vertex3 {
                                pos: p.as_vec3().to_array(),
                                normal: normal.as_vec3().to_array(),
                                color: b.color(p),
                                surface: [
                                    surf.roughness,
                                    surf.specular,
                                    surf.shader_key(),
                                    0.0,
                                ],
                                ao_strength: -2.0,
                            });
                            append(mesh, &face);
                            for y in j..j + height {
                                for x in i..i + width {
                                    mask[y * 32 + x] = B::Air;
                                }
                            }
                            i += width;
                        }
                    }
                }
            }
        }
    }
    Some(())
}

/// Order-independent identity of edited chunks whose bounds can be visible.
pub(super) fn visible_fingerprint(snapshot: &WorldSnapshot, eye: Vec3, hidden: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let scale = snapshot.cfg.voxel_meters();
    let mut result = 0u64;
    for q in snapshot.chunks.keys().chain(snapshot.regions.records.keys().filter(|q| !snapshot.chunks.contains_key(q))) {
        let lo = q.as_vec3() * (32.0 * scale);
        let hi = lo + Vec3::splat(32.0 * scale);
        if (lo.x - eye.x).abs().max((lo.z - eye.z).abs()) > 17512.0 + 32.0 * scale { continue; }
        let dx = (eye.x-lo.x).abs().max((eye.x-hi.x).abs());
        let dz = (eye.z-lo.z).abs().max((eye.z-hi.z).abs());
        // Two metres cover the one-metre camera key and face-neighbour changes.
        if dx.hypot(dz) + 2.0 < hidden { continue; }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        q.hash(&mut h);
        if let Some(data) = snapshot.chunks.get(q) {
            (Arc::as_ptr(data) as usize).hash(&mut h);
        } else if let Some(record) = snapshot.regions.records.get(q) {
            record.version.hash(&mut h);
            record.checksum.hash(&mut h);
        }
        result ^= h.finish();
    }
    result
}
