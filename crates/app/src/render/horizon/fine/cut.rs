//! Removes the coarse lattice inside the fine rings' square.
use super::super::edits::{append, clip};
use crate::mesh::ChunkMesh;
use std::sync::atomic::{AtomicBool, Ordering};

/// Cut every lattice triangle that lies inside the outer square. Pieces outside
/// the square are kept. Uses the same four-slab decomposition as the reference
/// patch, so no triangle is emitted twice.
pub(super) fn cut_square(mesh: &mut ChunkMesh, lo: [f32; 2], hi: [f32; 2], cancel: &AtomicBool) -> Option<()> {
    let indices = std::mem::take(&mut mesh.indices);
    for (n, tri) in indices.chunks_exact(3).enumerate() {
        if n % 4096 == 0 && cancel.load(Ordering::Relaxed) {
            return None;
        }
        let v = [tri[0], tri[1], tri[2]].map(|i| mesh.vertices[i as usize]);
        if v.iter().all(|v| v.pos[0] < lo[0])
            || v.iter().all(|v| v.pos[0] > hi[0])
            || v.iter().all(|v| v.pos[2] < lo[1])
            || v.iter().all(|v| v.pos[2] > hi[1])
        {
            mesh.indices.extend_from_slice(tri);
            continue;
        }
        let mut inside = v.to_vec();
        for (axis, bound, less) in [
            (0, lo[0], true),
            (0, hi[0], false),
            (2, lo[1], true),
            (2, hi[1], false),
        ] {
            append(mesh, &clip(&inside, axis, bound, less));
            inside = clip(&inside, axis, bound, !less);
        }
    }
    Some(())
}
