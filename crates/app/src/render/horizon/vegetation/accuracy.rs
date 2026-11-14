//! Proxy crowns against the voxel trees they stand in for.
use super::*;
use crate::world::TreeLobe;
use std::collections::{HashMap, HashSet};

const CELL: f32 = 0.5;

type Views = [HashSet<(i32, i32)>; 3];

fn key(a: f32, b: f32) -> (i32, i32) {
    ((a / CELL).floor() as i32, (b / CELL).floor() as i32)
}

/// Marks top (x,z), front (x,y) and side (z,y) cells whose centre a point set covers.
fn mark(views: &mut Views, p: Vec3) {
    views[0].insert(key(p.x, p.z));
    views[1].insert(key(p.x, p.y));
    views[2].insert(key(p.z, p.y));
}

fn voxel_views(cfg: &WorldSampleConfig, cell: (i32, i32), v: &TreeVisual) -> Option<Views> {
    let profile = engine_worldgen::tree_profile_for_cell(cfg, cell.0, cell.1)?;
    let m = cfg.voxel_meters();
    let anchor = (v.base / m).floor().as_ivec3();
    let reach = (v.radius * 1.6 / m).ceil() as i32 + 2;
    let top = ((v.base.y + v.height + v.crown_height) / m).ceil() as i32 + 3;
    let sub = (m / CELL).round().max(1.0) as i32;
    let mut views: Views = Default::default();
    for y in anchor.y..=top {
        for z in -reach..=reach {
            for x in -reach..=reach {
                let p = IVec3::new(anchor.x + x, y, anchor.z + z);
                let b = engine_worldgen::tree_block(cfg, profile, p);
                if !matches!(b, BlockId::Leaves | BlockId::PineLeaves | BlockId::Log) {
                    continue;
                }
                for k in 0..sub * sub * sub {
                    let o = Vec3::new((k % sub) as f32, ((k / sub) % sub) as f32, (k / (sub * sub)) as f32);
                    mark(&mut views, p.as_vec3() * m + (o + 0.5) * CELL - v.base);
                }
            }
        }
    }
    (!views[0].is_empty()).then_some(views)
}

/// Cells whose centre falls inside any projected proxy triangle.
fn mesh_views(mesh: &ChunkMesh, base: Vec3, scale: f32) -> Views {
    let mut views: Views = Default::default();
    for tri in mesh.indices.chunks_exact(3) {
        let p = [tri[0], tri[1], tri[2]].map(|i| Vec3::from_array(mesh.vertices[i as usize].pos) * scale - base);
        for (view, (a, b)) in [(0, (0, 2)), (1, (0, 1)), (2, (2, 1))] {
            let q = p.map(|v| glam::Vec2::new(v[a], v[b]));
            let lo = q[0].min(q[1]).min(q[2]);
            let hi = q[0].max(q[1]).max(q[2]);
            let area = (q[1] - q[0]).perp_dot(q[2] - q[0]);
            if area.abs() < 1e-6 {
                continue;
            }
            let (x0, y0) = key(lo.x, lo.y);
            let (x1, y1) = key(hi.x, hi.y);
            for j in y0..=y1 {
                for i in x0..=x1 {
                    let c = glam::Vec2::new((i as f32 + 0.5) * CELL, (j as f32 + 0.5) * CELL);
                    let w = [
                        (q[1] - c).perp_dot(q[2] - c),
                        (q[2] - c).perp_dot(q[0] - c),
                        (q[0] - c).perp_dot(q[1] - c),
                    ];
                    if w.iter().all(|&w| w * area >= 0.0) {
                        views[view].insert((i, j));
                    }
                }
            }
        }
    }
    views
}

fn iou(a: &HashSet<(i32, i32)>, b: &HashSet<(i32, i32)>) -> f32 {
    let inter = a.intersection(b).count() as f32;
    inter / (a.len() as f32 + b.len() as f32 - inter)
}

#[test]
fn far_proxies_follow_offset_lobes() {
    let mut lobes = [TreeLobe::default(); 8];
    lobes[0] = TreeLobe { centre: Vec3::new(4.0, 9.0, 0.0), radii: Vec3::new(2.5, 2.0, 2.5) };
    lobes[1] = TreeLobe { centre: Vec3::new(2.0, 11.0, 1.0), radii: Vec3::new(2.0, 1.5, 2.0) };
    let tree = TreeVisual { base: Vec3::new(-40.0, 30.0, 96.0), height: 8.0, radius: 6.5, crown_height: 5.0, conifer: false,
        canopy: TreeCanopy::Broadleaf, foliage: BlockId::Leaves, seed: 3, lobes, lobe_count: 2 };
    for scale in [1.0, 0.25] {
        let mut compact = ChunkMesh::empty();
        compact_tree(&mut compact, &tree, scale, [0.2; 3]);
        assert!(compact.vertices.len() <= 12 && compact.indices.len() <= 60);
        let mean_x = compact.vertices.iter().map(|v| v.pos[0] * scale - tree.base.x).sum::<f32>() / compact.vertices.len() as f32;
        assert!((mean_x - 3.25).abs() < 0.5, "compact crown centre {mean_x}");
        let mut mid = ChunkMesh::empty();
        let mut rng = tree.seed;
        woodland_tree(&mut mid, tree.base, tree.height, proxy_radius(tree.radius, false, 2), tree.crown_height, false, scale,
            [0.2; 3], &mut rng, 2, &tree.lobes[..2]);
        let leaves = mid.vertices.iter().filter(|v| (v.surface[2].round() as u32) % 64 == 6);
        let min_x = leaves.map(|v| v.pos[0] * scale - tree.base.x).fold(f32::MAX, f32::min);
        assert!(min_x > -1.0, "mid crown reaches {min_x} m behind the trunk");
    }
}

/// `just test proxy_silhouette` with `-- --ignored --nocapture` in release.
/// Mean silhouette IoU per proxy tier, broadleaf trees with lobes, plus geometry cost.
#[test]
#[ignore]
fn proxy_silhouette_accuracy() {
    let cfg = crate::world::World::with_seed(crate::world::WorldType::Hills, crate::fluid::VoxelScalePreset::OneMeter, 7)
        .sample_config();
    let scale = cfg.voxel_meters();
    let tiers = ["tier0 <240m", "tier2 <1100m", "compact <4200m"];
    let mut sum = [[0.0f32; 3]; 3];
    let mut tris = [0usize; 3];
    let mut n = 0;
    for cz in -14..14 {
        for cx in -14..14 {
            let Some(v) = engine_worldgen::tree_visual(&cfg, cx, cz) else { continue };
            if v.conifer || v.lobe_count == 0 || v.canopy != TreeCanopy::Broadleaf {
                continue;
            }
            let Some(truth) = voxel_views(&cfg, (cx, cz), &v) else { continue };
            n += 1;
            for (t, tier) in [0usize, 2, 3].into_iter().enumerate() {
                let mut mesh = ChunkMesh::empty();
                let mut rng = v.seed;
                random(&mut rng);
                if tier == 3 {
                    compact_tree(&mut mesh, &v, scale, [0.2; 3]);
                } else {
                    woodland_tree(&mut mesh, v.base, v.height, proxy_radius(v.radius, false, tier), v.crown_height,
                        false, scale, [0.2; 3], &mut rng, tier, &v.lobes[..v.lobe_count as usize]);
                }
                align_crown(&mut mesh, 0, &v, scale);
                tris[t] += mesh.indices.len() / 3;
                let proxy = mesh_views(&mesh, v.base, scale);
                for view in 0..3 {
                    sum[t][view] += iou(&truth[view], &proxy[view]);
                }
            }
        }
    }
    assert!(n > 20, "only {n} lobed trees sampled");
    for (t, name) in tiers.iter().enumerate() {
        let s = sum[t].map(|v| v / n as f32);
        println!("{name}: IoU top {:.3} front {:.3} side {:.3}, {:.1} tris/tree ({n} trees)", s[0], s[1], s[2], tris[t] as f32 / n as f32);
    }
}

/// `proxy_append_cost -- --ignored --nocapture` in release.
/// Cold build pays tree sampling; warm rebuilds pay crown geometry only.
#[test]
#[ignore]
fn proxy_append_cost() {
    let cfg = crate::world::World::with_seed(crate::world::WorldType::Hills, crate::fluid::VoxelScalePreset::OneMeter, 7)
        .sample_config();
    let cancel = AtomicBool::new(false);
    let mut cache = TreeSamples::default();
    for pass in 0..4 {
        let mut mesh = ChunkMesh::empty();
        let start = std::time::Instant::now();
        let sources = append(&mut mesh, &cfg, (0, 0), &cancel, &mut cache);
        println!("pass {pass}: {:.1} ms, {} trees, {} tris, {:.1} MiB",
            start.elapsed().as_secs_f64() * 1e3, sources.len(), mesh.indices.len() / 3, super::super::mesh_bytes(&mesh) as f64 / 1048576.0);
    }
    // Every base rebuild clones the cache. Compare with crowns stored inline.
    let inline: HashMap<(i32, i32, bool), (Option<TreeVisual>, u64)> =
        cache.values.iter().map(|(k, v)| (*k, (v.0.as_deref().copied(), v.1))).collect();
    let (mut shared_ms, mut inline_ms) = (f64::MAX, f64::MAX);
    for _ in 0..3 {
        let t = std::time::Instant::now();
        std::hint::black_box(cache.clone());
        shared_ms = shared_ms.min(t.elapsed().as_secs_f64() * 1e3);
        let t = std::time::Instant::now();
        std::hint::black_box(inline.clone());
        inline_ms = inline_ms.min(t.elapsed().as_secs_f64() * 1e3);
    }
    let entry = |value: usize| (std::mem::size_of::<(i32, i32, bool)>() + value) as f64 * cache.values.capacity() as f64 / 1048576.0;
    println!("cache clone: shared {shared_ms:.1} ms (~{:.0} MiB), inline {inline_ms:.1} ms (~{:.0} MiB), {} entries",
        entry(std::mem::size_of::<(Option<Arc<TreeVisual>>, u64)>()), entry(std::mem::size_of::<(Option<TreeVisual>, u64)>()), cache.values.len());
}
