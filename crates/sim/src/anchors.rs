//! Which 2×2×2 blocks a phase must visit: those that touch an active brick.
use crate::cell::{BRICK_SIZE, BRICKS_PER_AXIS, BrickCoord};
use engine_core::fast_hash::FastMap;
use engine_core::floor_div;
use glam::IVec3;

fn brick_origin(b: BrickCoord) -> IVec3 {
    let (i, per) = (b.brick as i32, BRICKS_PER_AXIS);
    b.chunk * BRICK_SIZE * per + IVec3::new(i % per, i / (per * per), (i / per) % per) * BRICK_SIZE
}

/// Anchors of this phase's blocks that overlap any brick in `bricks`, each exactly once, in no
/// particular order (blocks of one phase are disjoint, so order cannot change the result).
///
/// A block may touch up to eight bricks. Of those that are active, the first in (z, y, x) order
/// lists it, so no sort or dedup is needed and the cost is a few lookups only for blocks that
/// cross a brick face.
pub(crate) fn anchors(bricks: &[BrickCoord], offset: IVec3) -> Vec<IVec3> {
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
    let odd = offset != IVec3::ZERO;
    let mut out = Vec::with_capacity(bricks.len() * if odd { 90 } else { 64 });
    for &b in bricks {
        let o = brick_origin(b);
        let global = o / BRICK_SIZE;
        // Per axis: (anchor, first and last brick offset the block reaches). In an odd phase the block
        // starting one cell before the brick reaches back into the previous brick, and the block at the
        // brick's last cell reaches into the next.
        let axis = |a: usize| -> Vec<(i32, i32, i32)> {
            let mut v = Vec::with_capacity(5);
            if odd {
                v.push((o[a] - 1, -1, 0));
            }
            v.extend((0..BRICK_SIZE / 2).map(|k| {
                let anchor = o[a] + 2 * k + offset[a];
                (anchor, 0, i32::from(anchor == o[a] + BRICK_SIZE - 1))
            }));
            v
        };
        let (xs, ys, zs) = (axis(0), axis(1), axis(2));
        for &(y, y_lo, y_hi) in &ys {
            for &(z, z_lo, z_hi) in &zs {
                for &(x, x_lo, x_hi) in &xs {
                    let mut claimed = true;
                    'reach: for sz in z_lo..=z_hi {
                        for sy in y_lo..=y_hi {
                            for sx in x_lo..=x_hi {
                                // An active brick that comes first in (z, y, x) order lists this block.
                                if (sz, sy, sx) < (0, 0, 0) && is_active(global + IVec3::new(sx, sy, sz)) {
                                    claimed = false;
                                    break 'reach;
                                }
                            }
                        }
                    }
                    if claimed {
                        out.push(IVec3::new(x, y, z));
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
