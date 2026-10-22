//! Exact voxel surfaces replace only the overlapping procedural backdrop.
//! All region IO happens on the horizon worker against an immutable snapshot.
use crate::{
    mesh::{ChunkMesh, Vertex3},
    world::{BlockId as B, ChunkData, WorldSnapshot, chunk_coord_from_world_block as chunk},
};
use glam::{IVec3, Vec3, Vec3Swizzles};
use super::super::coverage::ResidentCoverage;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub(super) fn clip(poly: &[Vertex3], axis: usize, bound: f32, less: bool) -> Vec<Vertex3> {
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
            // Procedural water stores continuous bed depth here. Exact voxel
            // water packs metadata instead, and terrain stores discrete IDs.
            if is_backdrop_ao(a.ao_strength) && is_backdrop_ao(b.ao_strength)
                && a.surface[2]%64.0 == 8.0 && b.surface[2]%64.0 == 8.0 {
                v.surface[3] = a.surface[3] + (b.surface[3] - a.surface[3]) * t;
            }
            result.push(v);
        }
    }
    result
}
/// True for any procedural backdrop vertex tag: the open range (-1.5, 0) that
/// `terrain_is_backdrop` in world_shader.wgsl accepts. Plain backdrop meshes use
/// -1.0 and distant columns use `columns::COLUMN_AO`; exact edit proxies sit at
/// or below -2. Compare with this, never with `== -1.0`.
pub(super) fn is_backdrop_ao(ao: f32) -> bool {
    ao < 0.0 && ao > -1.5
}

pub(super) fn append(mesh: &mut ChunkMesh, p: &[Vertex3]) {
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
#[derive(Clone, Copy)]
struct Bounds {
    low: IVec3,
    high: IVec3,
    structural: bool,
}
pub(super) type Omitted = Vec<(Vec3, Vec3)>;
fn add_bounds(out: &mut HashMap<IVec3, Bounds>, low: IVec3, high: IVec3) {
    add_kind(out,low,high,false)
}
fn add_kind(out: &mut HashMap<IVec3, Bounds>, low: IVec3, high: IVec3, structural: bool) {
    let first = low.div_euclid(IVec3::splat(8));
    let last = (high - IVec3::ONE).div_euclid(IVec3::splat(8));
    for y in first.y..=last.y {
        for z in first.z..=last.z {
            for x in first.x..=last.x {
                let q = IVec3::new(x, y, z);
                out.entry(q).and_modify(|b|b.structural|=structural)
                    .or_insert(Bounds { low: q * 8, high: (q + IVec3::ONE) * 8, structural });
            }
        }
    }
}
fn compact(mesh: &mut ChunkMesh) {
    let mut remap = vec![u32::MAX; mesh.vertices.len()];
    let mut vertices = Vec::new();
    for index in &mut mesh.indices {
        let value = &mut remap[*index as usize];
        if *value == u32::MAX {
            *value = vertices.len() as u32;
            vertices.push(mesh.vertices[*index as usize]);
        }
        *index = *value;
    }
    mesh.vertices = vertices;
}

// Clip fragments can duplicate complete vertices. Weld only bit-identical
// attributes: normals, material IDs, water metadata and AO remain unchanged.
fn weld(mesh: &mut ChunkMesh) {
    let mut order: Vec<u32> = (0..mesh.vertices.len() as u32).collect();
    let key = |i:u32| bytemuck::cast::<Vertex3,[u32;14]>(mesh.vertices[i as usize]);
    order.sort_unstable_by_key(|&i| key(i));
    let mut remap=vec![0u32;mesh.vertices.len()];
    let mut vertices=Vec::new();
    let mut previous=None;
    for i in order {
        let value=key(i);
        if previous != Some(value) { vertices.push(mesh.vertices[i as usize]); previous=Some(value); }
        remap[i as usize]=vertices.len() as u32-1;
    }
    for i in &mut mesh.indices { *i=remap[*i as usize]; }
    mesh.vertices=vertices;
}

#[cfg(test)]
mod brick_tests {
    use super::*;
    #[test]
    fn surface_and_buried_changes_keep_separate_nonoverlapping_owners() {
        let mut bricks = HashMap::new();
        for p in [IVec3::new(1,0,1), IVec3::new(1,24,1)] {
            add_bounds(&mut bricks, p-IVec3::splat(2), p+IVec3::splat(3));
        }
        assert!(bricks.contains_key(&IVec3::new(0,0,0)));
        assert!(bricks.contains_key(&IVec3::new(0,3,0)));
        assert!(!bricks.contains_key(&IVec3::new(0,1,0)));
        for (key,b) in bricks {
            assert_eq!(b.low, key*8);
            assert_eq!(b.high-b.low, IVec3::splat(8));
            assert_eq!(chunk(b.low), chunk(b.high-IVec3::ONE));
        }
    }
}

pub(super) fn apply(
    mesh: &mut ChunkMesh,
    snapshot: &WorldSnapshot,
    center: (i32, i32),
    eye: Vec3,
    hidden: f32,
    coverage: &ResidentCoverage,
    cancel: &AtomicBool,
    trees: &[super::vegetation::TreeSource],
) -> Option<Omitted> {
    let scale = snapshot.cfg.voxel_meters();
    let c = chunk(IVec3::new(
        (center.0 as f32 / scale) as i32,
        0,
        (center.1 as f32 / scale) as i32,
    ));
    let radius = (super::SOURCE_REACH / (32.0 * scale)).ceil() as i32;
    let mut coords: Vec<_> = snapshot.edited_coords_near(c, radius).collect();
    coords.sort_by_key(|q| (q.x, q.z, q.y));
    coords.dedup();
    let mut water: HashMap<IVec3, Vec<(IVec3, u8)>> = HashMap::new();
    for (&p, &v) in snapshot.water_levels.iter() {
        water.entry(chunk(p)).or_default().push((p, v));
    }
    let mut regions = HashMap::new();
    let mut omitted = Vec::new();
    let mut changed_cells = 0usize;
    for q in coords {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let lo=q.as_vec3()*32.0*scale; let hi=lo+Vec3::splat(32.0*scale);
        let centre=glam::Vec2::new(center.0 as f32,center.1 as f32);
        if centre.distance(centre.clamp(lo.xz(),hi.xz()))>super::SOURCE_REACH { continue; }
        let cells = if let Some(cells) = snapshot.chunks.get(&q) {
            cells.clone()
        } else {
            let loaded = snapshot.regions.load(q).ok()??;
            water.entry(q).or_default().extend(loaded.water.iter().map(|(&p, &v)| (p, v)));
            loaded.chunk
        };
        let procedural = crate::world::generate_padded(&snapshot.cfg, q, cancel)?;
        for y in 0..32 {
            for z in 0..32 {
                for x in 0..32 {
                    let p = q * 32 + IVec3::new(x, y, z);
                    let old=procedural.get_world(p);
                    let new=cells.blocks[ChunkData::local_index(x,y,z)];
                    if new != old {
                        let structural=!matches!(old,B::Air|B::Water)||!matches!(new,B::Air|B::Water);
                        add_kind(&mut regions, p-IVec3::splat(2), p+IVec3::splat(3),structural);
                        changed_cells += 1;
                    }
                }
            }
        }
        if let Some(levels) = water.get(&q) {
            for &(p, v) in levels {
                let original = if procedural.get_world(p) == B::Water {
                    8
                } else {
                    0
                };
                if v != original && !submerged_water(snapshot, p) {
                    add_bounds(&mut regions, p-IVec3::splat(2), p+IVec3::splat(3));
                }
            }
        }
        if !cells.torches.is_empty() {
            for &p in &cells.torches { add_kind(&mut regions,p-IVec3::splat(2),p+IVec3::splat(3),true); }
        }
    }
    // A proxy crown may extend outside the changed voxel's brick, or represent
    // several source placement cells. Remove that whole proxy and rebuild its
    // source footprint before publication; a suppression mask alone is a hole.
    let touched:Vec<_>=trees.iter().filter(|tree| regions.values().any(|b|
        b.structural && b.low.cmplt(tree.high).all() && b.high.cmpgt(tree.low).all())).collect();
    if !touched.is_empty() {
        for tree in &touched { add_kind(&mut regions,tree.low,tree.high,true); }
        let mut at=0usize;
        mesh.indices.retain(|_| { let keep=!touched.iter().any(|tree|tree.indices.contains(&at)); at+=1; keep });
        println!("[horizon] edited_tree_proxies={} replacement_bricks={}",touched.len(),regions.len());
    }
    // Fixed, disjoint 8^3 bricks prevent a surface change and a buried cave
    // change from becoming one tall, falsely visible chunk-sized replacement.
    let mut resident_omissions:HashMap<(i32,i32),(IVec3,IVec3)>=HashMap::new();
    regions.retain(|_, b| {
        if coverage.contains_bounds(b.low.as_vec3()*scale,b.high.as_vec3()*scale,scale) {
            let q=chunk(b.low);
            resident_omissions.entry((q.x,q.z)).and_modify(|(lo,hi)|{*lo=lo.min(b.low);*hi=hi.max(b.high);}).or_insert((b.low,b.high));
            false
        } else if b.structural || bounds_visible(snapshot, b.low.as_vec3(), b.high.as_vec3(), eye, hidden, coverage) {
            true
        } else { omitted.push((b.low.as_vec3(), b.high.as_vec3())); false }
    });
    // All these bricks have the same complete-column owner. Keep one immutable
    // obligation per column instead of rechecking thousands of equivalent GPU
    // ownership bits every simulation frame. Occlusion omissions stay separate.
    omitted.extend(resident_omissions.into_values().map(|(lo,hi)|(lo.as_vec3(),hi.as_vec3())));
    if regions.is_empty() {
        return Some(omitted);
    }
    let mut regions: Vec<_> = regions.into_values().map(|b| (chunk(b.low), b)).collect();
    regions.sort_by_key(|(q,b)| (q.x, q.z, q.y, b.low.y, b.low.z, b.low.x));
    let mut columns: BTreeMap<(i32, i32), Vec<Bounds>> = BTreeMap::new();
    for (q, b) in &regions {
        columns.entry((q.x, q.z)).or_default().push(*b);
    }
    let old_indices = std::mem::take(&mut mesh.indices);
    for tri in old_indices.chunks_exact(3) {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let triangle = tri
            .iter()
            .map(|&i| mesh.vertices[i as usize])
            .collect::<Vec<_>>();
        let min = triangle.iter().fold(Vec3::splat(f32::INFINITY), |v, p| {
            v.min(Vec3::from_array(p.pos))
        });
        let max = triangle
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |v, p| {
                v.max(Vec3::from_array(p.pos))
            });
        let xmin = (min.x.floor() as i32).div_euclid(32);
        let xmax = (max.x.ceil() as i32).div_euclid(32);
        let boxes: Vec<_> = columns
            .range((xmin, i32::MIN)..=(xmax, i32::MAX))
            .flat_map(|(_, b)| b.iter())
            .filter(|b| (b.structural || mesh.vertices[tri[0] as usize].surface[2]%64.0==8.0)
                && min.cmple(b.high.as_vec3()).all() && max.cmpge(b.low.as_vec3()).all())
            .copied()
            .collect();
        if boxes.is_empty() {
            mesh.indices.extend_from_slice(tri);
            continue;
        }
        let mut polygons = vec![triangle];
        for b in boxes {
            let mut next = Vec::new();
            for mut inside in polygons {
                for (axis, bound, less) in [
                    (0, b.low.x as f32, true),
                    (0, b.high.x as f32, false),
                    (2, b.low.z as f32, true),
                    (2, b.high.z as f32, false),
                    (1, b.low.y as f32, true),
                    (1, b.high.y as f32, false),
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
        for poly in polygons {
            append(mesh, &poly);
        }
    }
    compact(mesh);
    println!("[horizon] edited parent_after_clip_bytes={}", super::mesh_bytes(mesh));
    // Set whenever patches are appended to `mesh`; a weld is only useful when
    // duplicates may have been added since the last one.
    let mut parent_dirty = true;
    let mut owner_source = None;
    let mut owner_patch = None;
    let structural_owners:std::collections::HashSet<_>=regions.iter().filter(|(_,b)|b.structural).map(|(q,_)|*q).collect();
    for (q, b) in &regions {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        if owner_source.as_ref().is_none_or(|(owner, _)| owner != q) {
            owner_source = Some((*q, edited_source(snapshot, *q, cancel)?));
            owner_patch = None;
        }
        let source = &owner_source.as_ref()?.1;
        let mut surface = false;
        'cells: for y in b.low.y..b.high.y {
            for z in b.low.z..b.high.z {
                for x in b.low.x..b.high.x {
                    let p = IVec3::new(x, y, z);
                    let value = source.get_world(p);
                    if value.is_empty() || (!b.structural && value!=B::Water) {
                        continue;
                    }
                    if (value == B::Water && source.water_levels.get(&p).is_some_and(|v| *v < 8))
                        || [
                            IVec3::X,
                            IVec3::NEG_X,
                            IVec3::Y,
                            IVec3::NEG_Y,
                            IVec3::Z,
                            IVec3::NEG_Z,
                        ]
                        .iter()
                        .any(|d| {
                            let n = source.get_world(p + *d);
                            !n.is_opaque() && n != value
                        })
                    {
                        surface = true;
                        break 'cells;
                    }
                }
            }
        }
        if !surface {
            continue;
        }
        if owner_patch.is_none() {
            owner_patch = Some(if structural_owners.contains(q) {
                let plan = crate::mesh::MeshPlan::for_chunk(source, cancel)?;
                let raw_bytes = plan.vertices * std::mem::size_of::<Vertex3>() + plan.indices * 4;
                if raw_bytes > 64 * 1024 * 1024 {
                    eprintln!("[horizon] edit scratch output exceeds 64MiB q={q:?} bytes={raw_bytes}");
                    return None;
                }
                plan.build_lod(source, 0)
            } else {
                let admitted=crate::mesh::exact_water_mesh_bytes_upper_bound(source);
                if admitted>64*1024*1024 {
                    eprintln!("[horizon] water edit scratch admission exceeds 64MiB q={q:?} bytes={admitted}"); return None;
                }
                crate::mesh::water_mesh(source)
            });
        }
        let patch = owner_patch.as_ref()?;
        let mut clipped = ChunkMesh::empty();
        let mut remap = vec![u32::MAX; patch.vertices.len()];
        for tri in patch.indices.chunks_exact(3) {
            if !b.structural && patch.vertices[tri[0] as usize].surface[2]%64.0!=8.0 { continue; }
            let points = [tri[0], tri[1], tri[2]].map(|i| Vec3::from_array(patch.vertices[i as usize].pos));
            let lo = points.iter().fold(Vec3::splat(f32::INFINITY), |a, p| a.min(*p));
            let hi = points.iter().fold(Vec3::splat(f32::NEG_INFINITY), |a, p| a.max(*p));
            if hi.cmplt(b.low.as_vec3()).any() || lo.cmpgt(b.high.as_vec3()).any() {
                continue;
            }
            let normal = patch.vertices[tri[0] as usize].normal;
            if (0..3).any(|axis| lo[axis] == hi[axis] &&
                ((lo[axis] == b.low[axis] as f32 && normal[axis] > 0.0) ||
                 (hi[axis] == b.high[axis] as f32 && normal[axis] < 0.0))) { continue; }
            if lo.cmpge(b.low.as_vec3()).all() && hi.cmple(b.high.as_vec3()).all() {
                for &index in tri {
                    let mapped = &mut remap[index as usize];
                    if *mapped == u32::MAX {
                        *mapped = clipped.vertices.len() as u32;
                        clipped.vertices.push(patch.vertices[index as usize]);
                    }
                    clipped.indices.push(*mapped);
                }
                continue;
            }
            let mut poly = tri
                .iter()
                .map(|&i| patch.vertices[i as usize])
                .collect::<Vec<_>>();
            for (axis, bound, less) in [
                (0, b.low.x as f32, false),
                (0, b.high.x as f32, true),
                (2, b.low.z as f32, false),
                (2, b.high.z as f32, true),
                (1, b.low.y as f32, false),
                (1, b.high.y as f32, true),
            ] {
                poly = clip(&poly, axis, bound, less);
            }
            append(&mut clipped, &poly);
        }
        crate::mesh::encode_water(source, &mut clipped);
        for v in &mut clipped.vertices {
            if v.surface[2] % 64.0 != 8.0 && v.surface[2] % 64.0 != 40.0 && v.surface[3] >= 0.0 {
                v.surface[3] = 4.0
                    + source.sky_visibility(Vec3::from_array(v.pos), Vec3::from_array(v.normal));
            }
            v.ao_strength = -2.0 - v.ao_strength;
        }
        if super::mesh_bytes(mesh) + super::mesh_bytes(&clipped) > super::MAX_MESH_BYTES {
            if parent_dirty {
                let before=super::mesh_bytes(mesh);
                weld(mesh);
                println!("[horizon] parent_bitwise_weld before={before} after={}", super::mesh_bytes(mesh));
            }
            weld(&mut clipped);
        }
        if super::mesh_bytes(mesh) + super::mesh_bytes(&clipped) > super::MAX_MESH_BYTES {
            eprintln!(
                "[horizon] edited output budget q={q:?} current={} patch={} limit={}",
                super::mesh_bytes(mesh),
                super::mesh_bytes(&clipped),
                super::MAX_MESH_BYTES
            );
            return None;
        }
        let base = mesh.vertices.len() as u32;
        mesh.vertices.extend(clipped.vertices);
        mesh.indices
            .extend(clipped.indices.into_iter().map(|i| base + i));
        parent_dirty = true;
    }
    println!(
        "[horizon] edit_regions={} changed_cells={changed_cells} final_bytes={}",
        regions.len(),
        super::mesh_bytes(mesh)
    );
    Some(omitted)
}

fn edited_source(
    snapshot: &WorldSnapshot,
    q: IVec3,
    cancel: &AtomicBool,
) -> Option<crate::mesh::GeneratedChunk> {
    // Reuse the resident mesher and its material/water/roof conventions.
    // Load the whole vertical edited column halo so a player roof above the
    // padded volume still shades this exact fallback, including cold saves.
    let mut data: HashMap<IVec3, Arc<ChunkData>> = HashMap::new();
    let mut cold_water = HashMap::new();
    let padded_low = q * 32 - IVec3::ONE;
    let padded_high = padded_low + IVec3::splat(crate::mesh::PADDED_CHUNK_SIZE);
    for k in snapshot.chunks.keys().chain(
        snapshot
            .regions
            .records
            .keys()
            .filter(|k| !snapshot.chunks.contains_key(k)),
    ) {
        if k.x.abs_diff(q.x) > 1 || k.z.abs_diff(q.z) > 1 {
            continue;
        }
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let value = if let Some(value) = snapshot.chunks.get(k) {
            value.clone()
        } else {
            let loaded = snapshot.regions.load(*k).ok()??;
            cold_water.extend(loaded.water.iter()
                .filter(|(p, _)| p.cmpge(padded_low).all() && p.cmplt(padded_high).all())
                .map(|(&p, &v)| (p, v)));
            loaded.chunk
        };
        data.insert(*k, value);
    }
    let read = |p: IVec3| {
        data.get(&chunk(p)).map_or_else(
            || crate::world::sample_block(&snapshot.cfg, p),
            |d| d.blocks[ChunkData::local_index(p.x, p.y, p.z)],
        )
    };
    let mut source = crate::world::generate_padded(&snapshot.cfg, q, cancel)?;
    let origin = source.origin();
    let size = crate::mesh::PADDED_CHUNK_SIZE;
    for z in 0..size {
        for x in 0..size {
            for y in 0..size {
                let p = origin + IVec3::new(x, y, z);
                if data.contains_key(&chunk(p)) {
                    source.set_world(p, read(p));
                }
                // A resident chunk supersedes its saved record, including an
                // empty override map after a partial cell becomes full again.
                if let Some(&level) = snapshot.water_levels.get(&p).or_else(|| cold_water.get(&p)) {
                    source.water_levels.insert(p, level);
                }
            }
        }
    }
    let occludes = |b: B| b.is_opaque() && !matches!(b, B::Leaves | B::PineLeaves | B::Bush);
    for z in 0..size {
        for x in 0..size {
            let wx = origin.x + x;
            let wz = origin.z + z;
            let mut roof = crate::world::terrain_height(&snapshot.cfg, wx, wz);
            // Only walk changed intervals; never scan the entire world height.
            while data.contains_key(&chunk(IVec3::new(wx, roof, wz)))
                && !occludes(read(IVec3::new(wx, roof, wz)))
            {
                roof -= 1;
            }
            for (coord, cells) in &data {
                if coord.x != wx.div_euclid(32) || coord.z != wz.div_euclid(32) {
                    continue;
                }
                if let Some(y) = (0..32)
                    .rev()
                    .find(|&y| occludes(cells.blocks[ChunkData::local_index(wx, y, wz)]))
                {
                    roof = roof.max(coord.y * 32 + y);
                }
            }
            source.sky_ceiling[(z * size + x) as usize] = roof;
        }
    }

    Some(source)
}

fn relevant(snapshot: &WorldSnapshot, q: IVec3, eye: Vec3, hidden: f32) -> bool {
    let scale = snapshot.cfg.voxel_meters();
    let lo = q.as_vec3() * (32.0 * scale);
    let hi = lo + Vec3::splat(32.0 * scale);
    let nearest = eye.xz().clamp(lo.xz(), hi.xz()).distance(eye.xz());
    if nearest > super::SOURCE_REACH {
        return false;
    }
    let dx = (eye.x - lo.x).abs().max((eye.x - hi.x).abs());
    let dz = (eye.z - lo.z).abs().max((eye.z - hi.z).abs());
    // dx/dz already measure the FARTHEST chunk corner. Subtracting another
    // chunk diagonal counted that extent twice and made hidden local physics
    // cancel distant builds. Include only the two-voxel shading neighbour halo.
    // Match the minimum exclusion shared by terrain and foliage in the shader;
    // an eased/requested coverage radius is not proof of resident geometry.
    dx.hypot(dz) + 2.0 * scale >= hidden && !occluded_below(snapshot, q, eye / scale)
}

/// An edit omitted by a proven occluder at build time must be rebuilt before
/// a new view can expose it, even if the underlying saved content is unchanged.
pub(super) fn scope_covers(
    omitted: &Omitted,
    current: &WorldSnapshot,
    eye: Vec3,
    hidden: f32,
) -> bool {
    scope_covers_resident(omitted, current, eye, hidden, &ResidentCoverage::default())
}

pub(super) fn scope_covers_resident(omitted: &Omitted, current: &WorldSnapshot, eye: Vec3, hidden: f32, coverage: &ResidentCoverage) -> bool {
    omitted.iter().all(|&(lo, hi)| {
        let visible=bounds_visible(current,lo,hi,eye,hidden,coverage);
        static TRACES:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
        if visible && std::env::var("VOXEL_HORIZON_TRACE").as_deref()==Ok("1")
            && TRACES.fetch_add(1,Ordering::Relaxed)<64 {
            eprintln!("[horizon-scope] exposed low={lo:?} high={hi:?} eye={eye:?} owned={} omitted={}",coverage.contains_bounds(lo*current.cfg.voxel_meters(),hi*current.cfg.voxel_meters(),current.cfg.voxel_meters()),omitted.len());
        }
        !visible
    })
}

fn bounds_visible(snapshot: &WorldSnapshot, low: Vec3, high: Vec3, eye: Vec3, hidden: f32, coverage: &ResidentCoverage) -> bool {
    let scale = snapshot.cfg.voxel_meters();
    let a = low * scale;
    let b = high * scale;
    if coverage.contains_bounds(a, b, scale) { return false; }
    let hidden = if coverage.header[3] != 0 { 0.0 } else { hidden };
    if eye.xz().distance(eye.xz().clamp(a.xz(), b.xz())) > super::SOURCE_REACH {
        return false;
    }
    let far = (eye.x - a.x).abs().max((eye.x - b.x).abs())
        .hypot((eye.z - a.z).abs().max((eye.z - b.z).abs()));
    far >= hidden && !occluded_bounds(snapshot, low, high, eye / scale)
}

// The water mesher and optics fill a cell with Water above it completely.
// Unknown saved neighbours stay dependencies; this never performs cold IO.
fn submerged_water(snapshot: &WorldSnapshot, p: IVec3) -> bool {
    // The lower source sees this cell in its top halo, whose above neighbour
    // is outside the padded mesh. Keep that conservative dependency until all
    // source builders carry an authoritative second vertical halo.
    if p.y.rem_euclid(32) == 0 { return false; }
    let read = |p: IVec3| {
        let q = chunk(p);
        if let Some(data) = snapshot.chunks.get(&q) {
            Some(data.blocks[ChunkData::local_index(p.x, p.y, p.z)])
        } else if snapshot.regions.records.contains_key(&q) {
            None
        } else {
            Some(crate::world::sample_block(&snapshot.cfg, p))
        }
    };
    read(p) == Some(B::Water) && read(p + IVec3::Y) == Some(B::Water)
}

/// Late dependency validation compares actual changed voxels. A falling cell at
/// y=0 does not make the whole y=0..31 chunk a visible surface dependency.
/// Unknown/cold versions fail conservatively; the retained procedural base lets
/// that edit stage retry without rerunning regional terrain generation.
pub(super) fn changes_visible(
    old: &WorldSnapshot,
    new: &WorldSnapshot,
    eye: Vec3,
    hidden: f32,
) -> bool {
    changes_visible_resident(old, new, eye, hidden, &ResidentCoverage::default())
}

pub(super) fn changes_visible_resident(old: &WorldSnapshot, new: &WorldSnapshot, eye: Vec3, hidden: f32, coverage: &ResidentCoverage) -> bool {
    changes_visible_inner(old,new,eye,hidden,coverage,false)
}

fn changes_visible_inner(old: &WorldSnapshot, new: &WorldSnapshot, eye: Vec3, hidden: f32, coverage: &ResidentCoverage, trace: bool) -> bool {
    let mut bounds: HashMap<IVec3, Bounds> = HashMap::new();
    let mut add = |_q: IVec3, lo: IVec3, hi: IVec3, structural: bool| {
        add_kind(&mut bounds, lo - IVec3::splat(2), hi + IVec3::splat(2),structural);
    };
    for (q, data) in new.chunks.iter() {
        let scale=new.cfg.voxel_meters();
        if coverage.contains_bounds((q.as_vec3()*32.0-Vec3::splat(2.0))*scale,
            ((q.as_vec3()+Vec3::ONE)*32.0+Vec3::splat(2.0))*scale,scale) { continue; }
        if let Some(previous) = old.chunks.get(q) {
            if Arc::ptr_eq(previous, data) {
                continue;
            }
            let (lo,hi)=match (previous.modified_bounds,data.modified_bounds) {
                (None,None)=>(IVec3::ZERO,IVec3::ZERO),
                (Some(a),None)|(None,Some(a))=>a,
                (Some((a,b)),Some((c,d)))=>(a.min(c),b.max(d)),
            };
            for y in lo.y..hi.y { for z in lo.z..hi.z { for x in lo.x..hi.x {
                let i=ChunkData::local_index(x,y,z);
                if previous.blocks[i]!=data.blocks[i] {
                    let p=*q*32+IVec3::new(x,y,z);
                    let structural=!matches!(previous.blocks[i],B::Air|B::Water)||!matches!(data.blocks[i],B::Air|B::Water);
                    add(*q,p,p+IVec3::ONE,structural);
                }
            } } }
            if previous.torches != data.torches {
                add(*q, *q * 32, (*q + IVec3::ONE) * 32,true);
            }
        } else {
            if let Some((lo, hi)) = data.modified_bounds {
                add(*q, *q * 32 + lo, *q * 32 + hi,false);
            }
            if let Some((lo,hi))=data.structural_bounds {
                add(*q,*q*32+lo,*q*32+hi,true);
            }
        }
    }
    for q in old.chunks.keys() {
        if !new.chunks.contains_key(q) {
            add(*q, *q * 32, (*q + IVec3::ONE) * 32,true);
        }
    }
    for q in old.regions.records.keys().chain(new.regions.records.keys()) {
        if new.chunks.contains_key(q) {
            continue;
        }
        let a = old.regions.records.get(q).map(|r| (r.version, r.checksum));
        let b = new.regions.records.get(q).map(|r| (r.version, r.checksum));
        if a != b {
            add(*q, *q * 32, (*q + IVec3::ONE) * 32,true);
        }
    }
    for (q,previous,current) in new.water_levels.changed_chunks(&old.water_levels) {
        let scale=new.cfg.voxel_meters();
        if coverage.contains_bounds((q.as_vec3()*32.0-Vec3::splat(2.0))*scale,
            ((q.as_vec3()+Vec3::ONE)*32.0+Vec3::splat(2.0))*scale,scale) { continue; }
        if let Some(current)=current { for (p,level) in current {
            if previous.and_then(|v|v.get(p))!=Some(level) && !(submerged_water(old,*p) && submerged_water(new,*p)) { add(q,*p,*p+IVec3::ONE,false); }
        } }
        if let Some(previous)=previous { for p in previous.keys() {
            if current.is_none_or(|v| !v.contains_key(p)) && !(submerged_water(old,*p) && submerged_water(new,*p)) { add(q,*p,*p+IVec3::ONE,false); }
        } }
    }
    bounds.into_values().any(|b| {
        let visible=if b.structural {
            let scale=new.cfg.voxel_meters(); let lo=b.low.as_vec3()*scale; let hi=b.high.as_vec3()*scale;
            if coverage.header[3]==0 {
                let far=(eye.x-lo.x).abs().max((eye.x-hi.x).abs()).hypot((eye.z-lo.z).abs().max((eye.z-hi.z).abs()));
                if far<hidden { return false; }
            }
            eye.xz().distance(eye.xz().clamp(lo.xz(),hi.xz()))<=super::SOURCE_REACH
                && !coverage.contains_bounds(lo,hi,scale)
        } else { bounds_visible(new, b.low.as_vec3(), b.high.as_vec3(), eye, hidden, coverage) };
        if visible && trace {
            eprintln!("[horizon-reject-bound] low={:?} high={:?} structural={} coverage_enabled={}",b.low,b.high,b.structural,coverage.header[3]);
            let mut shown=0;
            for y in b.low.y..b.high.y {for z in b.low.z..b.high.z {for x in b.low.x..b.high.x {
                let p=IVec3::new(x,y,z);let q=chunk(p);let index=ChunkData::local_index(x,y,z);
                let before=old.chunks.get(&q).map(|c|c.blocks[index]);let after=new.chunks.get(&q).map(|c|c.blocks[index]);
                if before!=after || old.water_levels.get(&p)!=new.water_levels.get(&p) {
                    eprintln!("[horizon-reject-cell] p={p:?} old_hot={} new_hot={} old_block={before:?} new_block={after:?} old_volume={:?} new_volume={:?} old_record={:?} new_record={:?}",old.chunks.contains_key(&q),new.chunks.contains_key(&q),old.water_levels.get(&p),new.water_levels.get(&p),old.regions.records.get(&q).map(|r|(r.version,r.checksum)),new.regions.records.get(&q).map(|r|(r.version,r.checksum)));
                    shown+=1;if shown==4 {break;}
                }
            }if shown==4 {break;}}if shown==4 {break;}}
        }
        visible
    })
}

pub(super) fn trace_rejection(old:&WorldSnapshot,new:&WorldSnapshot,omitted:&Omitted,eye:Vec3,hidden:f32,coverage:&ResidentCoverage) {
    let changed=changes_visible_inner(old,new,eye,hidden,coverage,true);
    let scope=omitted.iter().find(|&&(lo,hi)|bounds_visible(new,lo,hi,eye,hidden,coverage));
    eprintln!("[horizon-reject-detail] cfg_match={} world_match={} changes_visible={changed} failed_scope={scope:?}",old.cfg==new.cfg,Arc::ptr_eq(&old.regions.root,&new.regions.root));
}

/// Structural refinement must be requested even while residents mask it, so
/// departure has an edited parent ready rather than waiting for exposure.
pub(super) fn structural_changed(old: &WorldSnapshot, new: &WorldSnapshot, center:(i32,i32)) -> bool {
    let relevant=|q:IVec3| {
        let scale=new.cfg.voxel_meters(); let lo=q.as_vec3()*32.0*scale;let hi=lo+Vec3::splat(32.0*scale);
        let c=glam::Vec2::new(center.0 as f32,center.1 as f32);
        c.distance(c.clamp(lo.xz(),hi.xz()))<=super::SOURCE_REACH
    };
    for (q,data) in new.chunks.iter() {
        if !relevant(*q) { continue; }
        if let Some(previous)=old.chunks.get(q) {
            if Arc::ptr_eq(previous,data) { continue; }
            if previous.torches!=data.torches { return true; }
            let bounds=match(previous.structural_bounds,data.structural_bounds) {
                (None,None)=>continue,
                (Some(a),None)|(None,Some(a))=>a,
                (Some((a,b)),Some((c,d)))=>(a.min(c),b.max(d)),
            };
            for y in bounds.0.y..bounds.1.y { for z in bounds.0.z..bounds.1.z { for x in bounds.0.x..bounds.1.x {
                let i=ChunkData::local_index(x,y,z); let a=previous.blocks[i]; let b=data.blocks[i];
                if a!=b && (!matches!(a,B::Air|B::Water)||!matches!(b,B::Air|B::Water)) { return true; }
            } } }
        } else if data.structural_bounds.is_some() { return true; }
    }
    for (q,data) in old.chunks.iter() {
        if !relevant(*q) { continue; }
        if !new.chunks.contains_key(q) && data.structural_bounds.is_some() { return true; }
    }
    old.regions.records.keys().chain(new.regions.records.keys()).any(|q| relevant(*q) && !new.chunks.contains_key(q)
        && old.regions.records.get(q).map(|r|(r.version,r.checksum))!=new.regions.records.get(q).map(|r|(r.version,r.checksum)))
}

#[derive(Default)]
struct OcclusionCache {
    cfg: Option<crate::world::WorldSampleConfig>,
    generated: HashMap<IVec3, B>,
    planes: HashMap<([u32; 3], [u32; 3], IVec3, i32), PlaneProof>,
}
struct PlaneProof {
    signature: u64,
    blocked: bool,
    a: IVec3,
    b: IVec3,
    free_water: Vec<IVec3>,
    // Keep allocation identities unique even after their strong snapshots die.
    // Weak pins retain no voxel arrays and prevent pointer-reuse cache hits.
    _pins: Vec<std::sync::Weak<ChunkData>>,
}
thread_local! {
    static OCCLUSION: std::cell::RefCell<OcclusionCache> = std::cell::RefCell::new(OcclusionCache::default());
}

/// Conservative visibility proof, not a terrain-height guess: every ray from
/// the eye's whole voxel to the padded edit bounds crosses this horizontal
/// plane. Ignore the edit only if EVERY touched plane cell is authoritative,
/// opaque geometry. An opening, transparent cell or unavailable disk record
/// fails the proof. Simulation and collision continue independently.
fn occluded_below(snapshot: &WorldSnapshot, q: IVec3, eye: Vec3) -> bool {
    let low = q.as_vec3() * 32.0 - Vec3::splat(2.0);
    let high = low + Vec3::splat(36.0);
    occluded_bounds(snapshot, low, high, eye)
}

fn occluded_bounds(snapshot: &WorldSnapshot, low: Vec3, high: Vec3, eye: Vec3) -> bool {
    use std::hash::{Hash, Hasher};
    let eye_cell = eye.floor().as_ivec3();
    if eye_cell.y as f32 <= high.y + 4.0 {
        return false;
    }
    let eye_material = if let Some(data) = snapshot.chunks.get(&chunk(eye_cell)) {
        data.blocks[ChunkData::local_index(eye_cell.x, eye_cell.y, eye_cell.z)]
    } else if snapshot.regions.records.contains_key(&chunk(eye_cell)) {
        B::Water // Unknown camera medium cannot establish a water boundary.
    } else { crate::world::sample_block(&snapshot.cfg, eye_cell) };
    // From inside a body, a ray can cross full water cells without crossing a
    // rendered surface. Only a camera outside fluid can use its depth owner.
    let water_occludes = !matches!(eye_material, B::Water | B::Lava);
    OCCLUSION.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.cfg != Some(snapshot.cfg) {
            *cache = OcclusionCache {
                cfg: Some(snapshot.cfg),
                ..Default::default()
            };
        }
        let planes=[snapshot.cfg.water_level()-1,snapshot.cfg.water_level()-2].into_iter().chain([
            0.03125, 0.0625, 0.125, 0.25, 0.5, 0.75, 0.875, 0.9375, 0.96875,
        ].into_iter().map(|fraction|(eye_cell.y as f32 + (high.y - eye_cell.y as f32) * fraction).floor() as i32));
        for y in planes {
            let plane = y as f32 + 0.5;
            if plane <= high.y || plane >= eye_cell.y as f32 {
                continue;
            }
            let key = (low.to_array().map(f32::to_bits), high.to_array().map(f32::to_bits), eye_cell, y);
            // The ray footprint depends only on these exact bounds and the
            // whole eye voxel, not on the current content revision.
            let (a, b) = cache.planes.get(&key).map(|p| (p.a, p.b)).unwrap_or_else(|| {
            let mut minimum = Vec3::splat(f32::INFINITY);
            let mut maximum = Vec3::splat(f32::NEG_INFINITY);
            for origin_y in [eye_cell.y as f32, eye_cell.y as f32 + 1.0] {
                for target_y in [low.y, high.y] {
                    let t = (plane - origin_y) / (target_y - origin_y);
                    for origin_x in [eye_cell.x as f32, eye_cell.x as f32 + 1.0] {
                        for origin_z in [eye_cell.z as f32, eye_cell.z as f32 + 1.0] {
                            let origin = Vec3::new(origin_x, origin_y, origin_z);
                            for x in [low.x, high.x] {
                                for z in [low.z, high.z] {
                                    let p = origin.lerp(Vec3::new(x, target_y, z), t);
                                    minimum = minimum.min(p);
                                    maximum = maximum.max(p);
                                }
                            }
                        }
                    }
                }
            }
            let a = (minimum - Vec3::splat(0.001)).floor().as_ivec3();
            let b = (maximum + Vec3::splat(0.001)).floor().as_ivec3();
            (a, b)
            });
            if (b.x - a.x + 1) as i64 * (b.z - a.z + 1) as i64 > 4096 {
                continue;
            }
            let mut signature = std::collections::hash_map::DefaultHasher::new();
            water_occludes.hash(&mut signature);
            let mut known = true;
            for cy in y.div_euclid(32)..=(y+1).div_euclid(32) {
            for z in a.z.div_euclid(32)..=b.z.div_euclid(32) {
                for x in a.x.div_euclid(32)..=b.x.div_euclid(32) {
                    let c = IVec3::new(x, cy, z);
                    c.hash(&mut signature);
                    if let Some(data) = snapshot.chunks.get(&c) {
                        (Arc::as_ptr(data) as usize).hash(&mut signature);
                    } else if snapshot.regions.records.contains_key(&c) {
                        known = false; // Never perform region IO on the render thread.
                    }
                }
            }
            }
            if !known {
                continue;
            }
            let source_signature = signature.finish();
            let with_water = |positions: &[IVec3]| {
                let mut signature = std::collections::hash_map::DefaultHasher::new();
                source_signature.hash(&mut signature);
                for p in positions {
                    p.hash(&mut signature);
                    snapshot.water_levels.get(p).copied().unwrap_or(8).hash(&mut signature);
                }
                signature.finish()
            };
            if let Some(proof) = cache.planes.get(&key)
                && proof.signature == with_water(&proof.free_water) {
                    if proof.blocked {
                        return true;
                    } else {
                        continue;
                    }
                }
            let mut blocked = true;
            let mut free_water = Vec::new();
            'cells: for z in a.z..=b.z {
                for x in a.x..=b.x {
                    let p = IVec3::new(x, y, z);
                    let block = if let Some(data) = snapshot.chunks.get(&chunk(p)) {
                        data.blocks[ChunkData::local_index(x, y, z)]
                    } else if let Some(&block) = cache.generated.get(&p) {
                        block
                    } else {
                        let block = crate::world::sample_block(&snapshot.cfg, p);
                        if cache.generated.len() >= 65_536 {
                            cache.generated.clear();
                        }
                        cache.generated.insert(p, block);
                        block
                    };
                    let mut solid=occludes(block,water_occludes);
                    if block==B::Water {
                        let above=p+IVec3::Y;
                        let above=if let Some(data)=snapshot.chunks.get(&chunk(above)) {
                            data.blocks[ChunkData::local_index(above.x,above.y,above.z)]
                        } else if let Some(&block)=cache.generated.get(&above) { block }
                        else { let block=crate::world::sample_block(&snapshot.cfg,above); if cache.generated.len()>=65_536 {cache.generated.clear();} cache.generated.insert(above,block); block };
                        // The actual water mesher fills submerged cells to their
                        // top regardless of their stored fractional volume.
                        // Free fractional surfaces still cannot prove this plane.
                        if above == B::Water {
                            solid = water_occludes;
                        } else {
                            // Track full cells with no override too: a later
                            // fractional surface must invalidate this proof.
                            free_water.push(p);
                            solid = water_occludes && snapshot.water_levels.get(&p).copied().unwrap_or(8) >= 8;
                        }
                    }
                    if !solid {
                        blocked = false;
                        break 'cells;
                    }
                }
            }
            let signature = with_water(&free_water);
            // Weak handles prevent pointer reuse, but cache hits allocate none.
            let mut pins = Vec::new();
            for cy in y.div_euclid(32)..=(y+1).div_euclid(32) {
                for z in a.z.div_euclid(32)..=b.z.div_euclid(32) {
                    for x in a.x.div_euclid(32)..=b.x.div_euclid(32) {
                        if let Some(data) = snapshot.chunks.get(&IVec3::new(x, cy, z)) {
                            pins.push(Arc::downgrade(data));
                        }
                    }
                }
            }
            if cache.planes.len() >= 2048 {
                cache.planes.clear();
            }
            cache.planes.insert(
                key,
                PlaneProof {
                    signature,
                    blocked,
                    a,
                    b,
                    free_water,
                    _pins: pins,
                },
            );
            if blocked {
                return true;
            }
        }
        false
    })
}

fn occludes(block: B, water_occludes: bool) -> bool {
    (block == B::Water && water_occludes) || (block.is_opaque() && !block.is_model() && !matches!(block, B::Leaves | B::PineLeaves | B::Bush))
}

#[cfg(test)]
mod water_occlusion_contract {
    use super::*;
    #[test]
    fn only_verified_interior_submerged_volume_changes_are_equivalent() {
        let mut world=crate::world::World::with_seed(crate::world::WorldType::Superflat,crate::fluid::VoxelScalePreset::OneMeter,7);
        let mut cells=ChunkData { blocks:Box::new([B::Air;32768]),light:Box::new([0;32768]),torches:Default::default(),modified_bounds:None,structural_bounds:None };
        for y in [0,1,20,21] { cells.blocks[ChunkData::local_index(16,y,16)]=B::Water; }
        world.chunks.insert(IVec3::ZERO,Arc::new(cells));
        let p=IVec3::new(16,20,16);
        world.water_levels.insert(p,1);
        let old=world.snapshot();
        world.water_levels.insert(p,7);
        let mut new=old.clone();new.water_levels=Arc::new(world.water_levels.snapshot_all());
        let eye=Vec3::new(16.0,80.0,-100.0);
        assert!(submerged_water(&old,p));
        assert!(!changes_visible_resident(&old,&new,eye,0.0,&ResidentCoverage::default()));
        let boundary=IVec3::new(16,0,16);
        assert!(!submerged_water(&new,boundary),"the lower source's top halo remains a dependency");
        world.water_levels.insert(boundary,2);
        new.water_levels=Arc::new(world.water_levels.snapshot_all());
        assert!(changes_visible_resident(&old,&new,eye,0.0,&ResidentCoverage::default()));
        world.water_levels.remove(&boundary);
        world.water_levels.insert(p+IVec3::Y,2);
        new.water_levels=Arc::new(world.water_levels.snapshot_all());
        assert!(changes_visible_resident(&old,&new,eye,0.0,&ResidentCoverage::default()),"a free surface is not equivalent");
        let cells=Arc::make_mut(Arc::make_mut(&mut new.chunks).get_mut(&IVec3::ZERO).unwrap());
        cells.blocks[ChunkData::local_index(16,21,16)]=B::Air;
        cells.modified_bounds=Some((IVec3::new(16,21,16),IVec3::new(17,22,17)));
        assert!(!submerged_water(&new,p),"drainage above the cell must remain visible topology");
        assert!(changes_visible_resident(&old,&new,eye,0.0,&ResidentCoverage::default()));
    }
    #[test]
    fn cold_saved_water_matches_hot_source_without_resurrecting_old_overrides() {
        use crate::world::{World, WorldType, regions::RegionStream};
        use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
        let mut world = World::with_seed(WorldType::Superflat, crate::fluid::VoxelScalePreset::OneMeter, 7);
        let root = std::env::temp_dir().join(format!("horizon-cold-water-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        world.regions = RegionStream::open(root, HashMap::new());
        let p = IVec3::new(5, 40, 5);
        let q = chunk(p);
        world.set_water_volume(p, 3);
        world.request_simulation_writeback();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !world.regions.view.records.contains_key(&q) {
            assert!(Instant::now() < deadline, "saved-water fixture write timed out");
            world.stream_regions(IVec3::ZERO, 4);
            std::thread::sleep(Duration::from_millis(2));
        }
        let cancel = AtomicBool::new(false);
        let hot = world.snapshot();
        assert_eq!(edited_source(&hot, q, &cancel).unwrap().water_levels.get(&p), Some(&3));
        let mut cold = hot.clone();
        cold.chunks = Arc::default();
        cold.water_levels = Arc::default();
        let loaded = edited_source(&cold, q, &cancel).unwrap();
        assert_eq!(loaded.water_levels.get(&p), Some(&3), "cold region must retain its saved fractional level");
        world.set_water_volume(p, 8);
        let current = world.snapshot();
        let source = edited_source(&current, q, &cancel).unwrap();
        assert_eq!(source.water_levels.get(&p).copied().unwrap_or(8), 8, "live full water must supersede the older saved partial override");
    }
    #[test]
    fn submerged_fractional_cells_use_the_rendered_body_and_reject_a_drained_surface() {
        let mut world=crate::world::World::with_seed(crate::world::WorldType::Superflat,crate::fluid::VoxelScalePreset::OneMeter,7);
        let row=world.sample_config().water_level();
        assert!((2..30).contains(&row));
        let mut cells=ChunkData { blocks:Box::new([B::Air;32768]),light:Box::new([0;32768]),torches:Default::default(),modified_bounds:None,structural_bounds:None };
        for z in 0..32 { for x in 0..32 { for y in [row-1,row] {
            cells.blocks[ChunkData::local_index(x,y,z)]=B::Water;
            world.water_levels.insert(IVec3::new(x,y,z),1);
        }}}
        world.chunks.insert(IVec3::ZERO,Arc::new(cells));
        let mut snapshot=world.snapshot();
        let low=Vec3::new(14.0,0.0,14.0);let high=Vec3::new(18.0,(row-4) as f32,18.0);
        let eye=Vec3::new(16.0,(row+18) as f32,16.0);
        assert!(occluded_bounds(&snapshot,low,high,eye),"submerged cells are filled by the shared water mesher even with stored volume1");
        let key=(low.to_array().map(f32::to_bits), high.to_array().map(f32::to_bits), eye.floor().as_ivec3(), row-1);
        let original=OCCLUSION.with(|cache|cache.borrow().planes[&key].signature);
        let levels=&mut world.water_levels;
        for z in 0..32 {for x in 0..32 {levels.insert(IVec3::new(x,row-1,z),2);levels.insert(IVec3::new(x,row,z),1);}}
        snapshot.water_levels=Arc::new(levels.snapshot_all());
        assert!(occluded_bounds(&snapshot,low,high,eye));
        OCCLUSION.with(|cache| {
            let cache=cache.borrow();let proof=&cache.planes[&key];
            assert_eq!(proof.signature,original,"submerged volume churn must reuse the same geometric proof");
            assert!(proof.free_water.is_empty());
        });
        let map=Arc::make_mut(&mut snapshot.chunks);let cells=Arc::make_mut(map.get_mut(&IVec3::ZERO).unwrap());
        for z in 0..32 {for x in 0..32 {cells.blocks[ChunkData::local_index(x,row,z)]=B::Air;}}
        assert!(!occluded_bounds(&snapshot,low,high,eye),"a free fractional surface cannot stand in for a full opaque plane after drainage");
        snapshot.water_levels=Arc::default();
        assert!(occluded_bounds(&snapshot,low,high,eye),"a now-full free surface must invalidate the earlier negative proof");
        snapshot.water_levels=Arc::new(levels.snapshot_all());
        assert!(!occluded_bounds(&snapshot,low,high,eye),"a new fractional override must invalidate a full free-surface proof without any chunk replacement");
    }
}

pub(super) fn trace_changes(old: &WorldSnapshot, new: &WorldSnapshot, eye: Vec3, hidden: f32) {
    let mut shown = 0;
    for (q, data) in new.chunks.iter() {
        if !relevant(new, *q, eye, hidden) {
            continue;
        }
        let previous = old.chunks.get(q);
        if previous.is_some_and(|p| Arc::ptr_eq(p, data)) {
            continue;
        }
        let mut changed = 0;
        let mut low = i32::MAX;
        let mut high = i32::MIN;
        if let Some(previous) = previous {
            for (index, (a, b)) in previous.blocks.iter().zip(data.blocks.iter()).enumerate() {
                if a != b {
                    changed += 1;
                    let y = q.y * 32 + index as i32 / (32 * 32);
                    low = low.min(y);
                    high = high.max(y);
                }
            }
        }
        eprintln!(
            "[horizon-change] chunk={q:?} previous={} changed_cells={changed} y={low}..{high} eye={eye:?} hidden={hidden}",
            previous.is_some()
        );
        shown += 1;
        if shown == 3 {
            break;
        }
    }
}

/// Order-independent identity of edited chunks whose bounds can be visible.
pub(super) fn visible_fingerprint(snapshot: &WorldSnapshot, eye: Vec3, hidden: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut result = 0u64;
    let mut relevance = HashMap::new();
    for q in snapshot.chunks.keys().chain(
        snapshot
            .regions
            .records
            .keys()
            .filter(|q| !snapshot.chunks.contains_key(q)),
    ) {
        let relevant = *relevance
            .entry(*q)
            .or_insert_with(|| relevant(snapshot, *q, eye, hidden));
        if !relevant {
            continue;
        }
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
    // Partial water-volume changes need not replace the chunk Arc (Water stays
    // Water). Hash only relevant cells, never the global simulation revision.
    for (p, volume) in snapshot.water_levels.iter() {
        if !*relevance
            .entry(chunk(*p))
            .or_insert_with(|| relevant(snapshot, chunk(*p), eye, hidden))
        {
            continue;
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        p.hash(&mut h);
        volume.hash(&mut h);
        0x7761746572u64.hash(&mut h);
        result ^= h.finish();
    }
    result
}
