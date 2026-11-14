//! Joins every installed dry perimeter edge, retaining the original parent for rollback.
use super::{ResidentCoverage, collar};
use crate::mesh::ChunkMesh;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(crate) struct Set {
    // Sorted by (column, side), independent of publication/hash iteration order.
    pub edges: Vec<Arc<collar::Input>>,
}

impl Set {
    pub fn edge(&self, column: (i32, i32), side: usize) -> Option<&Arc<collar::Input>> {
        self.edges
            .binary_search_by_key(&(column, side), |e| (e.column, e.side))
            .ok()
            .map(|i| &self.edges[i])
    }

    pub fn same_geometry(&self, other: &Self) -> bool {
        self.edges.len() == other.edges.len()
            && self
                .edges
                .iter()
                .zip(&other.edges)
                .all(|(a, b)| Arc::ptr_eq(a, b) || a.same_geometry(b))
    }

    pub fn valid_for(&self, current: &Self, coverage: &ResidentCoverage) -> bool {
        self.edges.iter().all(|edge| {
            coverage.contains_column(edge.column.0, edge.column.1)
                && edge
                    .unowned_columns()
                    .is_some_and(|columns| !coverage.contains_column(columns[0].0, columns[0].1))
                && current
                    .edge(edge.column, edge.side)
                    .is_some_and(|now| Arc::ptr_eq(edge, now) || edge.same_geometry(now))
        })
    }
}

pub(crate) fn same(a: Option<&Arc<Set>>, b: Option<&Arc<Set>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a.same_geometry(b),
        _ => false,
    }
}

type Buckets = HashMap<(i32, i32), Vec<usize>>;

// Only index columns which actually contain a seam. This avoids scanning the
// entire landscape once per edge, including all its distant trees and rocks.
fn index_triangle(
    mesh: &ChunkMesh,
    tri: [u32; 3],
    id: usize,
    wanted: &HashSet<(i32, i32)>,
    bounds: [(i32, i32); 2],
    buckets: &mut Buckets,
) {
    let p = tri.map(|i| mesh.vertices[i as usize].pos);
    let low = |axis| p.iter().map(|v| v[axis]).fold(f32::INFINITY, f32::min);
    let high = |axis| p.iter().map(|v| v[axis]).fold(f32::NEG_INFINITY, f32::max);
    let x0 = ((low(0) / 32.0).floor() as i32).max(bounds[0].0);
    let z0 = ((low(2) / 32.0).floor() as i32).max(bounds[0].1);
    let x1 = ((high(0) / 32.0).floor() as i32).min(bounds[1].0);
    let z1 = ((high(2) / 32.0).floor() as i32).min(bounds[1].1);
    for z in z0..=z1 {
        for x in x0..=x1 {
            if wanted.contains(&(x, z)) {
                buckets.entry((x, z)).or_default().push(id);
            }
        }
    }
}

/// Worker-only transaction. Each cut sees the surviving geometry of preceding
/// cuts, so adjacent edges/corners never publish overlapping parent variants.
/// The complete original index list is kept once, rather than once per edge.
pub(crate) fn apply(
    mesh: &mut ChunkMesh,
    set: &Set,
    cancel: &AtomicBool,
    limit: usize,
) -> Option<collar::Outcome> {
    let wanted: HashSet<_> = set
        .edges
        .iter()
        .filter_map(|e| e.unowned_columns().map(|c| c[0]))
        .collect();
    if wanted.is_empty() {
        return Some(collar::Outcome::Ineligible);
    }
    let mut bounds = [(i32::MAX, i32::MAX), (i32::MIN, i32::MIN)];
    for &(x, z) in &wanted {
        bounds[0] = (bounds[0].0.min(x), bounds[0].1.min(z));
        bounds[1] = (bounds[1].0.max(x), bounds[1].1.max(z));
    }
    let vertex_count = mesh.vertices.len();
    let original = std::mem::take(&mut mesh.indices);
    let mut triangles: Vec<[u32; 3]> = original
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    let mut live = vec![true; triangles.len()];
    let mut buckets = Buckets::new();
    for (id, &tri) in triangles.iter().enumerate() {
        if id % 1024 == 0 && cancel.load(Ordering::Relaxed) {
            return None;
        }
        index_triangle(mesh, tri, id, &wanted, bounds, &mut buckets);
    }
    let mut applied = 0;
    let mut curtains = Vec::new();
    // Bound additional seam vertices independently of the landscape budget.
    let extra_vertices = 32 * 1024 * 1024 / std::mem::size_of::<crate::mesh::Vertex3>();
    for edge in &set.edges {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let column = edge.unowned_columns()?[0];
        let candidates: Vec<_> = buckets
            .get(&column)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&id| live[id])
            .collect();
        mesh.indices.clear();
        mesh.indices
            .extend(candidates.iter().flat_map(|&id| triangles[id]));
        match collar::apply(mesh, edge, cancel, limit.saturating_sub(original.len() * 4))? {
            collar::Outcome::Ineligible => curtains.push(edge),
            collar::Outcome::Applied(ranges) => {
                // Keep the new cut and its survivors. The pristine full parent
                // below replaces all per-edge fallback copies.
                mesh.indices.truncate(ranges.uncollared.start as usize);
                for id in candidates {
                    live[id] = false;
                }
                for tri in mesh.indices.chunks_exact(3) {
                    let tri = [tri[0], tri[1], tri[2]];
                    let id = triangles.len();
                    triangles.push(tri);
                    live.push(true);
                    index_triangle(mesh, tri, id, &wanted, bounds, &mut buckets);
                }
                applied += 1;
            }
        }
        if mesh.vertices.len() - vertex_count > extra_vertices {
            break;
        }
    }
    mesh.indices.clear();
    for (&tri, active) in triangles.iter().zip(live) {
        if active {
            mesh.indices.extend(tri);
        }
    }
    for edge in &curtains {
        if cancel.load(Ordering::Relaxed) { return None; }
        if mesh.vertices.len() - vertex_count >= extra_vertices { break; }
        edge.append_curtain(mesh);
    }
    let count = mesh.indices.len();
    let bytes = mesh
        .vertices
        .len()
        .checked_mul(std::mem::size_of::<crate::mesh::Vertex3>())?
        .checked_add(count.checked_add(original.len())?.checked_mul(4)?)?;
    if (applied == 0 && mesh.vertices.len() == vertex_count)
        || bytes > limit || count + original.len() > u32::MAX as usize {
        mesh.vertices.truncate(vertex_count);
        mesh.indices = original;
        return Some(collar::Outcome::Ineligible);
    }
    mesh.indices.extend(original);
    println!(
        "[terrain-seams] joined_edges={applied} curtain_edges={} eligible_edges={} added_vertices={}",
        curtains.len(), set.edges.len(),
        mesh.vertices.len() - vertex_count
    );
    Some(collar::Outcome::Applied(collar::Ranges {
        collared: 0..count as u32,
        uncollared: count as u32..mesh.indices.len() as u32,
    }))
}
