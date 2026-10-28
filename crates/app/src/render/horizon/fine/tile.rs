//! Error-bounded resolution for one square tile of the fine distant grid.
//!
//! A tile is `CELLS` x `CELLS` cells of one ring's step. Level `l` draws it with
//! quads of `2^l` cells whose corners are tile knots, so every level reuses the
//! sampled heights and two tiles at the same level share their edge vertices.

/// Cells along a tile edge. A power of two, so every level lands on knots.
pub(super) const CELLS: usize = 32;
/// Knots along a tile edge.
pub(super) const SIDE: usize = CELLS + 1;
/// Coarsest level: one quad per tile.
pub(super) const MAX_LEVEL: u32 = CELLS.trailing_zeros();

/// Largest gap between the knot heights and the surface drawn at `level`.
///
/// Each quad is split along the diagonal from its (0, 0) corner to its (1, 1)
/// corner, the same split `super::push_tile` emits, so the bound holds for the
/// triangles actually drawn.
pub(super) fn level_error(heights: &[f32], level: u32) -> f32 {
    debug_assert_eq!(heights.len(), SIDE * SIDE);
    let q = 1usize << level;
    let at = |i: usize, k: usize| heights[k * SIDE + i];
    let mut worst = 0.0_f32;
    for k0 in (0..CELLS).step_by(q) {
        for i0 in (0..CELLS).step_by(q) {
            let a = at(i0, k0);
            let b = at(i0 + q, k0);
            let c = at(i0, k0 + q);
            let d = at(i0 + q, k0 + q);
            for v in 0..=q {
                for u in 0..=q {
                    let (fu, fv) = (u as f32 / q as f32, v as f32 / q as f32);
                    let drawn = if fv >= fu {
                        a + (d - c) * fu + (c - a) * fv
                    } else {
                        a + (b - a) * fu + (d - b) * fv
                    };
                    worst = worst.max((at(i0 + u, k0 + v) - drawn).abs());
                }
            }
        }
    }
    worst
}

/// Number of levels, from 0 (every knot) to `MAX_LEVEL`.
pub(super) const LEVELS: usize = MAX_LEVEL as usize + 1;

/// Error at every level, in the heights' units.
pub(super) fn level_errors(heights: &[f32]) -> [f32; LEVELS] {
    std::array::from_fn(|level| level_error(heights, level as u32))
}

/// Surface triangles of one tile drawn at `level`.
pub(super) fn triangles(level: u32) -> usize {
    2 * (CELLS >> level).pow(2)
}

/// Levels for every tile under a triangle budget.
///
/// `errors[t][l]` is tile t's error at level l in reference pixels. Every tile
/// starts at the coarsest level; the tile with the largest error is refined one
/// level at a time until the largest error is at most `target` or the next
/// refinement would exceed `budget`. Ties go to the lower index.
pub(super) fn allocate(errors: &[[f32; LEVELS]], target: f32, budget: usize) -> Vec<u32> {
    use std::cmp::Reverse;
    let mut levels = vec![MAX_LEVEL; errors.len()];
    let mut used = errors.len() * triangles(MAX_LEVEL);
    // Errors are never negative, so their bit patterns order like the values.
    let key = |t: usize, level: u32| (errors[t][level as usize].max(0.0).to_bits(), Reverse(t));
    let mut heap: std::collections::BinaryHeap<_> = (0..errors.len()).map(|t| key(t, MAX_LEVEL)).collect();
    while let Some((bits, Reverse(t))) = heap.pop() {
        if f32::from_bits(bits) <= target || levels[t] == 0 {
            break;
        }
        let finer = levels[t] - 1;
        let extra = triangles(finer) - triangles(levels[t]);
        if used + extra > budget {
            break;
        }
        used += extra;
        levels[t] = finer;
        heap.push(key(t, finer));
    }
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(f: impl Fn(f32, f32) -> f32) -> Vec<f32> {
        (0..SIDE * SIDE)
            .map(|n| f((n % SIDE) as f32, (n / SIDE) as f32))
            .collect()
    }

    #[test]
    fn planes_are_exact_at_one_quad() {
        let tilted = grid(|x, z| 3.0 + 0.7 * x - 1.3 * z);
        assert!(level_errors(&tilted).iter().all(|&e| e < 1e-3));
    }

    #[test]
    fn a_single_spike_is_lost_by_any_coarser_level() {
        let mut spiky = grid(|_, _| 10.0);
        spiky[17 * SIDE + 17] = 40.0;
        let errors = level_errors(&spiky);
        assert_eq!(errors[0], 0.0);
        assert!(errors[1..].iter().all(|&e| e >= 29.9), "{errors:?}");
    }

    #[test]
    fn allocation_refines_the_worst_tile_first_and_keeps_the_budget() {
        let flat = [0.0; LEVELS];
        let rough: [f32; LEVELS] = std::array::from_fn(|l| if l == 0 { 0.0 } else { 4.0 * l as f32 });
        let errors = [flat, rough, flat];
        let coarse = 3 * triangles(MAX_LEVEL);
        assert_eq!(allocate(&errors, 0.5, coarse), vec![MAX_LEVEL; 3]);
        let one_step = coarse + triangles(MAX_LEVEL - 1) - triangles(MAX_LEVEL);
        assert_eq!(allocate(&errors, 0.5, one_step), vec![MAX_LEVEL, MAX_LEVEL - 1, MAX_LEVEL]);
        assert_eq!(allocate(&errors, 0.5, usize::MAX), vec![MAX_LEVEL, 0, MAX_LEVEL]);
        for budget in [coarse, one_step, 5_000, 50_000] {
            let used: usize = allocate(&errors, 0.5, budget).iter().map(|&l| triangles(l)).sum();
            assert!(used <= budget.max(coarse));
        }
    }

    #[test]
    fn allocation_stops_once_every_tile_is_under_target() {
        let small: [f32; LEVELS] = std::array::from_fn(|l| 0.1 * l as f32);
        assert_eq!(allocate(&[small; 4], 0.5, usize::MAX), vec![MAX_LEVEL; 4]);
    }

    #[test]
    fn error_never_falls_as_quads_grow_on_a_ridge() {
        // The crest runs across the split diagonal, so no level is exact.
        let ridge = grid(|x, z| 50.0 - 3.0 * (x + z - 32.0).abs());
        let errors: Vec<f32> = (0..=MAX_LEVEL).map(|l| level_error(&ridge, l)).collect();
        assert_eq!(errors[0], 0.0);
        assert!(errors[1] > 0.0);
        assert!(errors.windows(2).all(|w| w[1] + 1e-4 >= w[0]), "{errors:?}");
    }
}
