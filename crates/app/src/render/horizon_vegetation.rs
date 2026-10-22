//! Distant ecological clusters, sampled on the existing terrain triangles.
//! Visual proxies only: collision and edits remain authoritative voxel data.
use super::proxy_box;
use crate::{
    mesh::{ChunkMesh, Vertex3},
    world::{BlockId, TreeVisual, WorldSampleConfig, tree_visual},
};
use glam::{IVec3, Vec3};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;

/// Canonical placement-cell summaries, independent of derived crown geometry.
/// Exact and fixed-eight-metre source policies never share cached answers.
#[derive(Clone,Default)]
pub(super) struct TreeSamples {
    cfg:Option<WorldSampleConfig>, epoch:u64,
    values:HashMap<(i32,i32,bool),(Option<TreeVisual>,u64)>,
    hits:usize, misses:usize,
}
impl TreeSamples {
    fn begin(&mut self,cfg:WorldSampleConfig) {
        if self.cfg!=Some(cfg) { *self=Self::default();self.cfg=Some(cfg); }
        self.epoch=self.epoch.wrapping_add(1);self.hits=0;self.misses=0;
    }
    fn insert(&mut self,key:(i32,i32,bool),value:Option<TreeVisual>) {
        if self.values.len()>=786_432 {let epoch=self.epoch;self.values.retain(|_,v|v.1==epoch);}
        if self.values.len()>=786_432 {return;} // Cache admission never changes this query's answer.
        self.values.insert(key,(value,self.epoch));
    }
    fn finish(&mut self) {
        if self.values.len()>524_288 {
            let epoch=self.epoch;self.values.retain(|_,v|epoch.wrapping_sub(v.1)<=1);
            if self.values.len()>524_288 {self.values.retain(|_,v|v.1==epoch);}
        }
    }
}

#[derive(Clone)]
pub(super) struct TreeSource {
    pub indices: std::ops::Range<usize>,
    pub low: IVec3,
    pub high: IVec3,
}

fn record_tree(out: &mut Vec<TreeSource>, mesh: &ChunkMesh, start: (usize, usize), tree: &TreeVisual, scale: f32, stratum: Option<(i32,i32,i32,f32)>) {
    if start.1 == mesh.indices.len() { return; }
    let radius = tree.radius + 2.0 * scale;
    let mut low=tree.base-Vec3::new(radius,2.0*scale,radius);
    let mut high=tree.base+Vec3::new(radius,tree.height+tree.crown_height+2.0*scale,radius);
    if let Some((x,z,stride,cell))=stratum {
        // An aggregate represents this source stratum, not just its randomly
        // selected tree. Editing any of its source trees replaces the entire
        // aggregate with authoritative voxels, including the newly empty space.
        low.x=(x.div_euclid(stride)*stride) as f32*cell-16.0;
        low.z=(z.div_euclid(stride)*stride) as f32*cell-16.0;
        high.x=low.x+stride as f32*cell+32.0;
        high.z=low.z+stride as f32*cell+32.0;
        low.y-=32.0; high.y+=32.0;
    }
    out.push(TreeSource { indices:start.1..mesh.indices.len(),low:(low/scale).floor().as_ivec3(),high:(high/scale).ceil().as_ivec3() });
}

fn align_crown(mesh:&mut ChunkMesh,start:usize,tree:&TreeVisual,scale:f32) {
    let base=tree.base.y/scale;
    let leaf=|v:&Vertex3| (v.surface[2].round() as u32)%64==6;
    let top=mesh.vertices[start..].iter().filter(|v|leaf(v)).map(|v|v.pos[1]).fold(base,f32::max);
    if top<=base { return; }
    let ratio=((tree.height+tree.crown_height)/scale)/(top-base);
    for v in &mut mesh.vertices[start..] { if leaf(v) { v.pos[1]=base+(v.pos[1]-base)*ratio; } }
}

fn random(state: &mut u32) -> f32 {
    *state = state.wrapping_add(0x9e3779b9);
    let mut h = *state;
    h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
    h = (h ^ (h >> 15)).wrapping_mul(0x846ca68b);
    ((h ^ (h >> 16)) >> 8) as f32 / 16777216.0
}

// Sampling the authoritative forest can build expensive erosion tiles. Do it
// on bounded shared workers, then consume results in the original cell order:
// random streams, geometry order and whole-tree budget decisions stay identical.
// Each worker builds its own erosion tiles, so the cap trades wall time against
// memory. `VOXEL_HORIZON_TREE_WORKERS` overrides it for timing sweeps.
fn sample_trees(
    cfg: &WorldSampleConfig,
    cells: &[(i32, i32, u32)],
    exact: bool,
    cancel: &AtomicBool,
    cache: &mut TreeSamples,
) -> Option<Vec<Option<TreeVisual>>> {
    let mut result = vec![None; cells.len()];
    let mut misses = Vec::new();
    for (i, &(x, z, _)) in cells.iter().enumerate() {
        if let Some((value, last)) = cache.values.get_mut(&(x, z, exact)) {
            result[i] = *value;
            *last = cache.epoch;
            cache.hits += 1;
        } else {
            misses.push(i);
            cache.misses += 1;
        }
    }
    if misses.is_empty() {
        return Some(result);
    }
    let workers = super::parallel::workers(super::parallel::cap_from_env("VOXEL_HORIZON_TREE_WORKERS", 4));
    let sampled = super::parallel::map_blocks(&misses, 64, workers, cancel, |&i| {
        let (x, z, _) = cells[i];
        Some(if exact { tree_visual(cfg, x, z) } else { crate::world::tree_visual_lod(cfg, x, z, 8.0) })
    });
    let Some(sampled) = sampled else {
        // Never publish a terrain-only or partly forested result after a
        // worker failed. The owner will retry the build.
        cancel.store(true, Ordering::Relaxed);
        return None;
    };
    for (&i, value) in misses.iter().zip(sampled) {
        result[i] = value;
        let (x, z, _) = cells[i];
        cache.insert((x, z, exact), value);
    }
    Some(result)
}

pub fn append(
    mesh: &mut ChunkMesh,
    cfg: &WorldSampleConfig,
    center: (i32, i32),
    cancel: &AtomicBool,
    cache:&mut TreeSamples,
) -> Vec<TreeSource> {
    cache.begin(*cfg);
    let mut sources=Vec::new();
    // Resident chunks place no trees on Superflat (`structure_block`), so any proxy
    // here would be a phantom that vanishes on approach.
    if cfg.world_type == crate::world::WorldType::Superflat {
        return sources;
    }
    let scale = cfg.voxel_meters();
    // Include indices and existing terrain in the publication budget. Leave
    // 8 MiB for authoritative edits and keep whole trees, never partial crowns.
    let base_bytes = super::mesh_bytes(mesh);
    let byte_limit = super::MAX_MESH_BYTES
        .saturating_sub(8 * 1024 * 1024)
        .max(base_bytes);
    let mut omitted = 0;
    // Real spawn anchors bridge the old 128--512 m empty belt at fine scales.
    // Profiles use the same generator as editable chunks; sampling and geometry
    // assembly stay off the render thread.
    let cell_m = cfg.blocks_for_span(12.0) as f32 * scale;
    // Placement anchors can cross a cell-centre distance boundary. Candidate
    // culling is conservative; only the canonical anchor owns final membership.
    let anchor_margin = cell_m * 2.0_f32.sqrt();
    let cells = (1100.0 / cell_m).ceil() as i32 + 1;
    let cx = (center.0 as f32 / cell_m).floor() as i32;
    let cz = (center.1 as f32 / cell_m).floor() as i32;
    let mut candidates = Vec::new();
    for iz in cz - cells..=cz + cells {
        for ix in cx - cells..=cx + cells {
            if cancel.load(Ordering::Relaxed) {
                return sources;
            }
            let dx = (ix as f32 + 0.5) * cell_m - center.0 as f32;
            let dz = (iz as f32 + 0.5) * cell_m - center.1 as f32;
            if dx * dx + dz * dz > (1100.0 + anchor_margin).powi(2) {
                continue;
            }
            candidates.push((ix, iz, 0));
        }
    }
    let Some(trees) = sample_trees(cfg, &candidates, true, cancel,cache) else {
        return sources;
    };
    for tree in trees {
        if cancel.load(Ordering::Relaxed) {
            return sources;
        }
        let Some(tree) = tree else {
            continue;
        };
        let distance = (tree.base.x - center.0 as f32).hypot(tree.base.z - center.1 as f32);
        if distance >= 1100.0 {
            continue;
        }
        let base = tree.base;
        let mut rng = tree.seed;
        let tone = random(&mut rng);
        let foliage=if tree.conifer { BlockId::PineLeaves } else { BlockId::Leaves };
        let color=foliage.color((tree.base/scale).floor().as_ivec3()).map(|v|v*(0.97+tone*0.06));
        if super::mesh_bytes(mesh) + 24 * std::mem::size_of::<Vertex3>() + 36 * 4 <= byte_limit {
            let start = (mesh.vertices.len(), mesh.indices.len());
            let tier = if distance < 240.0 { 0 } else { 2 };
            woodland_tree(
                mesh,
                base,
                tree.height,
                proxy_radius(tree.radius, tree.conifer, tier),
                tree.crown_height,
                tree.conifer,
                scale,
                color,
                &mut rng,
                tier,
            );
            align_crown(mesh,start.0,&tree,scale);
            if !retain_tree(mesh, start, byte_limit) {
                omitted += 1;
            } else { record_tree(&mut sources,mesh,start,&tree,scale,None); }
        } else {
            omitted += 1;
        }
    }
    // Preserve every placement through 4.2 km. Compact crowns filter subpixel
    // leaves, rather than replacing sixteen source cells with one enlarged tree.
    // The outermost cheap summaries remain world-aligned source strata.
    for (tier, stride, inner, outer) in [
        (3, 1, 1100.0, 4200.0),
        (4, 8, 4200.0, 8000.0),
    ] {
        let spacing = cell_m * stride as f32;
        let radius = (outer / spacing).ceil() as i32 + 1;
        let cx = (center.0 as f32 / spacing).floor() as i32;
        let cz = (center.1 as f32 / spacing).floor() as i32;
        candidates.clear();
        for iz in cz - radius..=cz + radius {
            for ix in cx - radius..=cx + radius {
                if cancel.load(Ordering::Relaxed) {
                    return sources;
                }
                let mut rng = (ix as u32).wrapping_mul(73856093)
                    ^ (iz as u32).wrapping_mul(19349663)
                    ^ cfg.seed;
                let cell_x = ix * stride + (random(&mut rng) * stride as f32) as i32;
                let cell_z = iz * stride + (random(&mut rng) * stride as f32) as i32;
                let px = (cell_x as f32 + 0.5) * cell_m;
                let pz = (cell_z as f32 + 0.5) * cell_m;
                let distance = (px - center.0 as f32).hypot(pz - center.1 as f32);
                if distance + anchor_margin < inner || distance >= outer + anchor_margin {
                    continue;
                }
                candidates.push((cell_x, cell_z, rng));
            }
        }
        let exact = super::canonical_source() && outer <= 4200.0;
        let Some(trees) = sample_trees(cfg, &candidates, exact, cancel,cache) else {
            return sources;
        };
        for ((source_x, source_z, mut rng), tree) in candidates.iter().copied().zip(trees) {
            if cancel.load(Ordering::Relaxed) {
                return sources;
            }
            let Some(tree) = tree else {
                continue;
            };
            let distance = (tree.base.x - center.0 as f32).hypot(tree.base.z - center.1 as f32);
            if distance < inner || distance >= outer { continue; }
            let y=tree.base.y;
            if y < (cfg.water_level() as f32 + 1.0) * scale {
                continue;
            }
            let tone = random(&mut rng);
            let foliage=if tree.conifer { BlockId::PineLeaves } else { BlockId::Leaves };
            let color=foliage.color((tree.base/scale).floor().as_ivec3()).map(|v|v*(0.97+tone*0.06));
            if super::mesh_bytes(mesh) + 24 * std::mem::size_of::<Vertex3>() + 36 * 4 > byte_limit {
                omitted += 1;
                continue;
            }
            let start = (mesh.vertices.len(), mesh.indices.len());
            // Canopy-area filtering. Do not multiply tree height with LOD.
            // A broad low crown is a forest summary, not a 40 m invented tree.
            if stride==1 { compact_tree(mesh,&tree,scale,color); } else { woodland_tree(
                mesh,
                tree.base,
                tree.height,
                proxy_radius(tree.radius * stride as f32, tree.conifer, tier),
                tree.crown_height,
                tree.conifer,
                scale,
                color,
                &mut rng,
                tier,
            ); }
            align_crown(mesh,start.0,&tree,scale);
            if !retain_tree(mesh, start, byte_limit) {
                omitted += 1;
            } else { record_tree(&mut sources,mesh,start,&tree,scale,if stride==1 {None}else{Some((source_x,source_z,stride,cell_m))}); }
        }
    }
    cache.finish();
    println!("[horizon] vegetation base_bytes={base_bytes} total_bytes={} limit={byte_limit} omitted_trees={omitted} source_cache_hits={} source_cache_misses={} source_cache_entries={} source_cache_capacity={}",super::mesh_bytes(mesh),cache.hits,cache.misses,cache.values.len(),cache.values.capacity());
    sources
}

fn retain_tree(mesh: &mut ChunkMesh, start: (usize, usize), byte_limit: usize) -> bool {
    if super::mesh_bytes(mesh) <= byte_limit {
        return true;
    }
    mesh.vertices.truncate(start.0);
    mesh.indices.truncate(start.1);
    false
}
/// `woodland_tree` draws crowns out to `PROXY_REACH[tier] * r` metres. Solve for
/// `r` so the proxy's outer leaves land on the voxel crown's measured reach.
/// Values come from `proxy_spread_per_radius`.
fn proxy_radius(reach: f32, conifer: bool, tier: usize) -> f32 {
    let reach_per_r = match (conifer, tier) {
        (true, _) => 1.12,
        (false, 0) => 1.165,
        (false, _) => 0.966,
    };
    reach / reach_per_r
}

/// Once leaf voxels are subpixel, keep each real crown's position, species,
/// height and footprint with a bounded closed solid instead of a larger tree.
fn compact_tree(mesh:&mut ChunkMesh,tree:&TreeVisual,scale:f32,color:[f32;3]) {
    let block=if tree.conifer {BlockId::PineLeaves}else{BlockId::Leaves};let material=block.surface();
    let r=tree.radius*0.8;let top=tree.height+tree.crown_height;
    let bottom=tree.height*if tree.conifer {0.18}else{0.45};
    let (points,indices):(Vec<Vec3>,&[u32])=if tree.conifer {
        (vec![Vec3::new(-r,bottom,-r),Vec3::new(r,bottom,-r),Vec3::new(r,bottom,r),Vec3::new(-r,bottom,r),Vec3::new(0.0,top,0.0)],
         &[3,2,4,2,1,4,1,0,4,0,3,4,0,1,2,0,2,3])
    }else{
        (vec![Vec3::new(-r,bottom,-r),Vec3::new(r,bottom,-r),Vec3::new(r,top,-r),Vec3::new(-r,top,-r),Vec3::new(-r,bottom,r),Vec3::new(r,bottom,r),Vec3::new(r,top,r),Vec3::new(-r,top,r)],
         &[4,5,6,4,6,7,1,0,3,1,3,2,7,6,2,7,2,3,0,1,5,0,5,4,5,1,2,5,2,6,0,4,7,0,7,3])
    };
    let base=mesh.vertices.len() as u32;
    for p in points {
        let normal=if p.x==0.0&&p.z==0.0 {Vec3::Y}else{Vec3::new(p.x.signum(),if p.y==top {0.7}else{-0.3},p.z.signum()).normalize()};
        mesh.vertices.push(Vertex3 {pos:((tree.base+p)/scale).to_array(),normal:normal.to_array(),color,surface:[material.roughness,material.specular,material.shader_key(),0.0],ao_strength:-1.0});
    }
    mesh.indices.extend(indices.iter().map(|i|base+i));
}

fn woodland_tree(
    mesh: &mut ChunkMesh,
    p: Vec3,
    h: f32,
    r: f32,
    crown_h: f32,
    conifer: bool,
    scale: f32,
    color: [f32; 3],
    rng: &mut u32,
    tier: usize,
) {
    if tier < 3 {
        let trunk_h = if conifer { h * 0.24 } else { h * 0.56 };
        let trunk_w = (r * 0.16).clamp(0.32, 0.58);
        proxy_box(
            mesh,
            p + Vec3::Y * (trunk_h * 0.5),
            Vec3::new(trunk_w, trunk_h, trunk_w),
            scale,
            BlockId::Log.color((p/scale).floor().as_ivec3()),
            BlockId::Log.surface().shader_key(),
        );
    }
    if conifer {
        voxel_cone(mesh, p, h, r, scale, color, rng);
        return;
    }
    let turn = random(rng) * std::f32::consts::TAU;
    let lobes = if tier < 2 {
        4
    } else if tier < 3 {
        3
    } else {
        2
    };
    for i in 0..lobes {
        let variation = random(rng);
        let angle = turn + i as f32 * 2.399963;
        let (offset, extent) = {
            let reach = r * if tier < 2 { 0.48 } else { 0.28 };
            (
                Vec3::new(
                    angle.cos() * reach,
                    h + (variation - 0.5) * crown_h * 0.70,
                    angle.sin() * reach,
                ),
                Vec3::new(
                    r * (0.50 + variation * 0.16),
                    crown_h * (0.37 + variation * 0.18),
                    r * (0.46 + variation * 0.14),
                ),
            )
        };
        let shade = 0.90 + variation * 0.17;
        let centre = p + offset;
        voxel_crown(
            mesh,
            centre,
            extent,
            scale,
            color.map(|c| c * shade),
            BlockId::Leaves,
            rng,
            |q| ((q - centre) / extent).length_squared() <= 1.0,
        );
    }
}

/// Crowns are voxels, like the trees they stand in for. A cell is filled when
/// its centre lies inside the crown's ellipsoid, and only faces against empty
/// cells are emitted, so interior cells cost nothing. Cells scale with the
/// crown, so a distant tree keeps the same block language as a near one.
fn voxel_crown(
    mesh: &mut ChunkMesh,
    center: Vec3,
    extent: Vec3,
    scale: f32,
    color: [f32; 3],
    block: BlockId,
    rng: &mut u32,
    inside: impl Fn(Vec3) -> bool,
) {
    // Blocks across the crown's smaller horizontal axis stays near 3–4, so the
    // silhouette reads as blocks without exploding the vertex count.
    let cell = (extent.x.min(extent.z) * 2.0 / 3.5).max(scale);
    let half = (extent / cell).ceil().as_ivec3() + IVec3::ONE;
    let dims = (half * 2 + IVec3::ONE).max(IVec3::ONE);
    let origin = center - Vec3::splat(cell) * half.as_vec3();
    let index = |c: IVec3| ((c.y * dims.z + c.z) * dims.x + c.x) as usize;
    let mut filled = vec![false; (dims.x * dims.y * dims.z) as usize];
    let mut any = false;
    for y in 0..dims.y {
        for z in 0..dims.z {
            for x in 0..dims.x {
                let c = origin + (IVec3::new(x, y, z).as_vec3() + Vec3::splat(0.5)) * cell;
                if inside(c) {
                    filled[index(IVec3::new(x, y, z))] = true;
                    any = true;
                }
            }
        }
    }
    if !any {
        return;
    }
    let filled_at = |c: IVec3| {
        c.min_element() >= 0 && c.x < dims.x && c.y < dims.y && c.z < dims.z && filled[index(c)]
    };
    // Preserve the random stream used by subsequent lobes/trees. Per-cell tint
    // prevented coplanar faces merging; the shared leaf shader supplies texels.
    for _ in filled.iter().filter(|&&value| value) {
        random(rng);
    }
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let width = dims[u] as usize;
        let height = dims[v] as usize;
        let mut mask = vec![false; width * height];
        for sign in [-1, 1] {
            for plane in 0..dims[axis] {
                for j in 0..height {
                    for i in 0..width {
                        let mut c = IVec3::ZERO;
                        c[axis] = plane;
                        c[u] = i as i32;
                        c[v] = j as i32;
                        let mut next = c;
                        next[axis] += sign;
                        mask[j * width + i] = filled_at(c) && !filled_at(next);
                    }
                }
                for j in 0..height {
                    for i in 0..width {
                        if !mask[j * width + i] {
                            continue;
                        }
                        let mut w = 1;
                        while i + w < width && mask[j * width + i + w] {
                            w += 1;
                        }
                        let mut h = 1;
                        while j + h < height && (0..w).all(|k| mask[(j + h) * width + i + k]) {
                            h += 1;
                        }
                        for row in j..j + h {
                            mask[row * width + i..row * width + i + w].fill(false);
                        }
                        let mut face = origin;
                        face[axis] += (plane as f32 + if sign > 0 { 1.0 } else { 0.0 }) * cell;
                        face[u] += (i as f32 + w as f32 * 0.5) * cell;
                        face[v] += (j as f32 + h as f32 * 0.5) * cell;
                        emit_block_face(
                            mesh,
                            face,
                            [w as f32 * cell * 0.5, h as f32 * cell * 0.5],
                            axis,
                            sign,
                            scale,
                            color,
                            block,
                        );
                    }
                }
            }
        }
    }
}

/// One coplanar rectangle, wound counter-clockwise as seen from `sign * axis`.
fn emit_block_face(
    mesh: &mut ChunkMesh,
    centre: Vec3,
    half: [f32; 2],
    axis: usize,
    sign: i32,
    scale: f32,
    color: [f32; 3],
    block: BlockId,
) {
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let mut normal = Vec3::ZERO;
    normal[axis] = sign as f32;
    let corner = |su: f32, sv: f32| {
        let mut p = centre;
        p[u] += su * half[0];
        p[v] += sv * half[1];
        p
    };
    let quad = if sign > 0 {
        [
            corner(-1.0, -1.0),
            corner(1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, 1.0),
        ]
    } else {
        [
            corner(-1.0, -1.0),
            corner(-1.0, 1.0),
            corner(1.0, 1.0),
            corner(1.0, -1.0),
        ]
    };
    let base = mesh.vertices.len() as u32;
    let material=block.surface();
    for p in quad {
        mesh.vertices.push(Vertex3 {
            pos: (p / scale).to_array(),
            normal: normal.to_array(),
            color,
            surface: [material.roughness, material.specular, material.shader_key(), 0.0],
            ao_strength: -1.0,
        });
    }
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Conifer crowns as stacked voxel tiers: each tier's radius follows the
/// spreading envelope of the old smooth cone, quantised to blocks.
fn voxel_cone(
    mesh: &mut ChunkMesh,
    p: Vec3,
    h: f32,
    r: f32,
    scale: f32,
    color: [f32; 3],
    rng: &mut u32,
) {
    let lean = Vec3::new(random(rng) - 0.5, 0.0, random(rng) - 0.5).normalize_or_zero() * r * 0.22;
    let top = p.y + h * 1.015;
    let base = p.y + h * 0.18;
    voxel_crown(
        mesh,
        Vec3::new(p.x, (base + top) * 0.5, p.z),
        Vec3::new(r, (top - base) * 0.5, r),
        scale,
        color,
        BlockId::PineLeaves,
        rng,
        |q| {
            let t = ((q.y - base) / (top - base)).clamp(0.0, 1.0);
            let radius = r * (1.0 - t * 0.96).powf(0.78);
            let local = q - p - lean * t * t;
            Vec3::new(local.x, 0.0, local.z).length() <= radius
        },
    );
}

#[cfg(test)]
mod proxy_reach {
    use super::*;

    #[test]
    fn forest_cache_separates_source_policy_and_world_in_both_query_orders() {
        let mut cfg=crate::world::World::with_seed(crate::world::WorldType::Hills,crate::fluid::VoxelScalePreset::OneMeter,2738958700).sample_config();
        let cells=vec![(0,-286,0),(-1,-287,0),(1,-288,0)];let cancel=AtomicBool::new(false);
        let mut cache=TreeSamples::default();cache.begin(cfg);
        let exact=sample_trees(&cfg,&cells,true,&cancel,&mut cache).unwrap();
        let filtered=sample_trees(&cfg,&cells,false,&cancel,&mut cache).unwrap();
        assert_eq!(cache.misses,6);assert_eq!(cache.values.len(),6);
        let mut reversed=cells.clone();reversed.reverse();cache.begin(cfg);
        let mut a=sample_trees(&cfg,&reversed,false,&cancel,&mut cache).unwrap();a.reverse();assert_eq!(a,filtered);
        let mut b=sample_trees(&cfg,&reversed,true,&cancel,&mut cache).unwrap();b.reverse();assert_eq!(b,exact);
        assert_eq!((cache.hits,cache.misses),(6,0));
        let mut fresh=TreeSamples::default();fresh.begin(cfg);
        assert_eq!(sample_trees(&cfg,&cells,false,&cancel,&mut fresh).unwrap(),filtered);
        assert_eq!(sample_trees(&cfg,&cells,true,&cancel,&mut fresh).unwrap(),exact);
        cfg.seed^=1;cache.begin(cfg);assert!(cache.values.is_empty());
    }

    #[test]
    fn canonical_crown_top_and_edit_source_survive_proxy_detail() {
        for scale in [1.0,0.5,0.25] { for conifer in [false,true] {
            let tree=TreeVisual { base:Vec3::new(-75.0,27.0,-63.0),height:12.0,radius:5.0,crown_height:4.0,conifer,seed:12345 };
            let mut mesh=ChunkMesh::empty(); let mut rng=tree.seed;
            woodland_tree(&mut mesh,tree.base,tree.height,proxy_radius(tree.radius,conifer,2),tree.crown_height,conifer,scale,[0.2;3],&mut rng,2);
            align_crown(&mut mesh,0,&tree,scale);
            let top=mesh.vertices.iter().filter(|v|(v.surface[2].round() as u32)%64==6).map(|v|v.pos[1]*scale).fold(f32::NEG_INFINITY,f32::max);
            assert!((top-(tree.base.y+tree.height+tree.crown_height)).abs()<1e-4);
            let mut records=Vec::new(); record_tree(&mut records,&mesh,(0,0),&tree,scale,Some((-7,-6,4,12.0)));
            assert_eq!(records[0].indices,0..mesh.indices.len());
            assert!((records[0].low.as_vec3()*scale).cmple(tree.base).all());
            assert!((records[0].high.as_vec3()*scale).cmpgt(tree.base+Vec3::Y*16.0).all());
            // Negative cells share the same world-aligned source stratum.
            assert!((records[0].low.x as f32*scale+112.0).abs()<1e-4);
        }}
    }

    #[test]
    fn compact_source_crowns_keep_height_and_bound_geometry_at_every_scale() {
        for scale in [1.0,0.5,0.25] {for conifer in [false,true] {
            let tree=TreeVisual {base:Vec3::new(-96.0,27.0,320.0),height:12.0,radius:5.0,crown_height:4.0,conifer,seed:7};
            let mut mesh=ChunkMesh::empty();compact_tree(&mut mesh,&tree,scale,[0.2;3]);
            assert!(mesh.vertices.len()<=8 && mesh.indices.len()<=36);
            let top=mesh.vertices.iter().map(|v|v.pos[1]*scale).fold(f32::NEG_INFINITY,f32::max);assert!((top-43.0).abs()<1e-4);
            for v in &mesh.vertices {let delta=Vec3::from_array(v.pos)*scale-tree.base;assert!(delta.x.hypot(delta.z)<=tree.radius*1.14);}
            for tri in mesh.indices.chunks_exact(3) {let p=[tri[0],tri[1],tri[2]].map(|i|Vec3::from_array(mesh.vertices[i as usize].pos));assert!((p[1]-p[0]).cross(p[2]-p[0]).length_squared()>0.0);}
        }}
    }

    /// `cargo test --release proxy_reach -- --ignored --nocapture`
    /// Outermost horizontal vertex of a proxy tree, per unit of `r`.
    #[test]
    #[ignore]
    fn proxy_spread_per_radius() {
        for conifer in [false, true] {
            for tier in [0usize, 2] {
                let mut worst = 0.0f32;
                let mut seed = 12345u32;
                for _ in 0..200 {
                    let mut mesh = ChunkMesh::empty();
                    let mut rng = seed;
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    // Scale 1 so vertex coordinates are metres.
                    woodland_tree(
                        &mut mesh,
                        Vec3::ZERO,
                        10.0,
                        1.0,
                        5.0,
                        conifer,
                        1.0,
                        [0.2; 3],
                        &mut rng,
                        tier,
                    );
                    for v in &mesh.vertices {
                        worst = worst.max(Vec3::new(v.pos[0], 0.0, v.pos[2]).length());
                    }
                }
                println!("conifer={conifer} tier={tier}: outer reach {worst:.3} r");
            }
        }
    }
}
