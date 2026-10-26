//! The Margolus pass at 0.25 m: which fine blocks a phase visits and one block's step.
use crate::FINE_SIZE;
use glam::IVec3;

/// Block coordinate and the fine cell inside it (each axis `0..4`) of a fine coordinate.
pub(super) fn split_fine(fine: IVec3) -> (IVec3, IVec3) {
    let size = IVec3::splat(FINE_SIZE);
    (fine.div_euclid(size), fine.rem_euclid(size))
}

/// Anchors, in fine coordinates, of this phase's 2×2×2 blocks that touch any of `blocks`, each once,
/// in `(z, y, x)` order. The even phase tiles a fine block exactly; the odd phase's blocks start one
/// cell before it and the last ones reach one cell into the next block, which is how fine matter meets
/// a uniform neighbour.
pub(super) fn fine_anchors(blocks: &[IVec3], offset: IVec3) -> Vec<IVec3> {
    let steps: &[i32] = if offset == IVec3::ZERO { &[0, 2] } else { &[-1, 1, 3] };
    let mut out = Vec::with_capacity(blocks.len() * steps.len().pow(3));
    for &block in blocks {
        let origin = block * FINE_SIZE;
        for &dy in steps {
            for &dz in steps {
                for &dx in steps {
                    out.push(origin + IVec3::new(dx, dy, dz));
                }
            }
        }
    }
    out.sort_unstable_by_key(|a| (a.z, a.y, a.x));
    out.dedup();
    out
}
