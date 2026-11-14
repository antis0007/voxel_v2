//! Which 2×2×2 blocks a phase must visit: those that touch an active brick.
use crate::cell::{BRICK_SIZE, BRICKS_PER_AXIS, BrickCoord};
use engine_core::fast_hash::FastMap;
use engine_core::floor_div;
use glam::IVec3;

fn brick_origin(b: BrickCoord) -> IVec3 {
    let (i, per) = (b.brick as i32, BRICKS_PER_AXIS);
    b.chunk * BRICK_SIZE * per + IVec3::new(i % per, i / (per * per), (i / per) % per) * BRICK_SIZE
}

// Odd-phase anchors can touch the previous brick, just this brick, or the
// next brick on each axis. Precompute the earlier claimants for each case.
const fn claim_masks() -> [u32; 27] {
    let mut masks = [0; 27];
    let mut reach = 0;
    while reach < 27 {
        let (x, y, z) = (reach % 3, reach / 3 % 3, reach / 9);
        let mut neighbor = 0;
        while neighbor < 13 {
            let (nx, ny, nz) = (neighbor % 3, neighbor / 3 % 3, neighbor / 9);
            if (nx == 1 || nx == x) && (ny == 1 || ny == y) && (nz == 1 || nz == z) {
                masks[reach] |= 1 << neighbor;
            }
            neighbor += 1;
        }
        reach += 1;
    }
    masks
}

const CLAIM_MASKS: [u32; 27] = claim_masks();

/// Anchors of this phase's blocks that overlap any brick in `bricks`, each exactly once, in no
/// particular order (blocks of one phase are disjoint, so order cannot change the result).
///
/// A block may touch up to eight bricks. Of those that are active, the first in (z, y, x) order
/// lists it, so no sort or dedup is needed and the cost is a few lookups only for blocks that
/// cross a brick face.
pub(crate) fn anchors(bricks: &[BrickCoord], offset: IVec3) -> Vec<IVec3> {
    if offset == IVec3::ZERO {
        let mut out = Vec::with_capacity(bricks.len() * 64);
        for &brick in bricks {
            let origin = brick_origin(brick);
            for y in (0..BRICK_SIZE).step_by(2) {
                for z in (0..BRICK_SIZE).step_by(2) {
                    for x in (0..BRICK_SIZE).step_by(2) {
                        out.push(origin + IVec3::new(x, y, z));
                    }
                }
            }
        }
        return out;
    }

    let mut active: FastMap<IVec3, u64> = FastMap::default();
    for b in bricks {
        *active.entry(b.chunk).or_default() |= 1 << b.brick;
    }
    let is_active = |global_brick: IVec3| {
        let chunk = IVec3::new(
            floor_div(global_brick.x, BRICKS_PER_AXIS),
            floor_div(global_brick.y, BRICKS_PER_AXIS),
            floor_div(global_brick.z, BRICKS_PER_AXIS),
        );
        let l = global_brick - chunk * BRICKS_PER_AXIS;
        let index = l.x + BRICKS_PER_AXIS * (l.z + BRICKS_PER_AXIS * l.y);
        active.get(&chunk).is_some_and(|mask| mask >> index & 1 != 0)
    };
    let mut out = Vec::with_capacity(bricks.len() * 90);
    for &b in bricks {
        let o = brick_origin(b);
        let global = o / BRICK_SIZE;
        let mut active_neighbors = 0;
        // Encode neighbors in (z, y, x) order. Index 13 is the current brick.
        for neighbor in 0..13 {
            let delta = IVec3::new(neighbor % 3 - 1, neighbor / 3 % 3 - 1, neighbor / 9 - 1);
            if is_active(global + delta) {
                active_neighbors |= 1 << neighbor;
            }
        }
        let axis = [(-1, 0), (1, 1), (3, 1), (5, 1), (7, 2)];
        for (y, y_reach) in axis {
            for (z, z_reach) in axis {
                for (x, x_reach) in axis {
                    let reach = x_reach + 3 * y_reach + 9 * z_reach;
                    if active_neighbors & CLAIM_MASKS[reach] == 0 {
                        out.push(o + IVec3::new(x, y, z));
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PHASE_OFFSETS;

    /// The plain definition: every anchor of the phase's lattice inside any active brick's reach, sorted and deduped.
    fn oracle(bricks: &[BrickCoord], offset: IVec3) -> Vec<IVec3> {
        let mut out = Vec::new();
        for &b in bricks {
            let o = brick_origin(b);
            let first = |axis: usize| {
                let start = o[axis] - 1;
                if (start - offset[axis]).rem_euclid(2) == 0 { start } else { o[axis] }
            };
            let mut x = first(0);
            while x < o.x + BRICK_SIZE {
                let mut y = first(1);
                while y < o.y + BRICK_SIZE {
                    let mut z = first(2);
                    while z < o.z + BRICK_SIZE {
                        out.push(IVec3::new(x, y, z));
                        z += 2;
                    }
                    y += 2;
                }
                x += 2;
            }
        }
        out.sort_unstable_by_key(|a| (a.z, a.y, a.x));
        out.dedup();
        out
    }

    #[test]
    fn each_block_is_listed_once_and_exactly_the_blocks_that_touch_an_active_brick() {
        // Neighbours in every direction, across chunk faces and at negative coordinates, plus a lone brick.
        let mut bricks = Vec::new();
        for chunk in [IVec3::new(-1, -1, -1), IVec3::new(0, 0, 0)] {
            for brick in [0u8, 1, 3, 4, 16, 21, 42, 63] {
                bricks.push(BrickCoord { chunk, brick, scale: crate::Scale::FINEST });
            }
        }
        bricks.push(BrickCoord { chunk: IVec3::new(7, -3, 2), brick: 37, scale: crate::Scale::FINEST });
        for offset in PHASE_OFFSETS {
            let mut got = anchors(&bricks, offset);
            let listed = got.len();
            got.sort_unstable_by_key(|a| (a.z, a.y, a.x));
            got.dedup();
            assert_eq!(got.len(), listed, "a block was listed twice at offset {offset}");
            assert_eq!(got, oracle(&bricks, offset), "offset {offset}");
        }
    }
}
