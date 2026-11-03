//! Chooses which globe tiles to draw and which to bake next.

use super::tile::{MAX_LEVEL, TileId, projected_error_px};
use glam::DVec3;
use std::collections::HashSet;

const SPLIT_PX: f64 = 1.5;
/// Lower merge threshold so a tile hovering near 1.5 px does not flicker between levels.
const MERGE_PX: f64 = 0.75;
const MAX_WANTED: usize = 256;

#[derive(Default)]
pub struct Selection {
    pub draw: Vec<TileId>,
    /// Missing children of tiles that want to split, nearest first.
    pub wanted: Vec<TileId>,
}

/// Buffers `select_into` reuses, so a steady frame allocates nothing.
#[derive(Default)]
pub struct SelectScratch {
    split_last_frame: HashSet<TileId>,
    frontier: Vec<TileId>,
    next: Vec<TileId>,
    children: Vec<TileId>,
}

impl SelectScratch {
    #[cfg(test)]
    pub fn capacities(&self) -> [usize; 4] {
        [self.split_last_frame.capacity(), self.frontier.capacity(), self.next.capacity(), self.children.capacity()]
    }
}

/// `select_into` with fresh buffers; `previous_draw` is last frame's draw list.
#[cfg(test)]
pub fn select(
    ready: &dyn Fn(TileId) -> bool,
    previous_draw: &[TileId],
    eye_body: DVec3,
    focal_px: f64,
    max_draw: usize,
) -> Selection {
    let mut out = Selection { draw: previous_draw.to_vec(), wanted: Vec::new() };
    select_into(ready, eye_body, focal_px, max_draw, &mut SelectScratch::default(), &mut out);
    out
}

/// Breadth-first error-driven selection from the six cube roots.
///
/// `out.draw` holds last frame's draw list on entry (it sets the merge hysteresis) and this
/// frame's on return.
pub fn select_into(
    ready: &dyn Fn(TileId) -> bool,
    eye_body: DVec3,
    focal_px: f64,
    max_draw: usize,
    scratch: &mut SelectScratch,
    out: &mut Selection,
) {
    let SelectScratch { split_last_frame, frontier, next, children } = scratch;
    split_last_frame.clear();
    for t in &out.draw {
        let mut p = t.parent();
        while let Some(a) = p {
            if !split_last_frame.insert(a) {
                break;
            }
            p = a.parent();
        }
    }
    let (draw, wanted) = (&mut out.draw, &mut out.wanted);
    draw.clear();
    wanted.clear();
    let visible = |t: &TileId| projected_error_px(*t, eye_body, focal_px) > 0.0;
    frontier.clear();
    frontier.extend((0..6).map(TileId::root).filter(visible));
    while !frontier.is_empty() {
        next.clear();
        let mut pending = frontier.len();
        for &tile in frontier.iter() {
            pending -= 1;
            let err = projected_error_px(tile, eye_body, focal_px);
            let threshold = if split_last_frame.contains(&tile) { MERGE_PX } else { SPLIT_PX };
            if err <= threshold || tile.level >= MAX_LEVEL {
                draw.push(tile);
                continue;
            }
            children.clear();
            children.extend(tile.children().into_iter().filter(visible));
            let all_ready = children.iter().all(|c| ready(*c));
            // Splitting past the cap would drop coverage, so the parent stays instead.
            let fits = draw.len() + pending + next.len() + children.len() <= max_draw;
            if all_ready && fits {
                next.extend_from_slice(children);
            } else {
                wanted.extend(children.iter().copied().filter(|c| !ready(*c)));
                draw.push(tile);
            }
        }
        std::mem::swap(frontier, next);
    }
    let dist = |t: &TileId| t.centre_body().distance(eye_body);
    // Unstable: a stable sort allocates a merge buffer; the id tie-break keeps the order total.
    wanted.sort_unstable_by(|a, b| dist(a).total_cmp(&dist(b)).then(a.cmp(b)));
    wanted.dedup();
    wanted.truncate(MAX_WANTED);
}

/// Fills `out` with `draw` and each draw tile's parent, sorted for binary search.
///
/// Parents stay resident so a merge back one level draws at once instead of waiting a bake.
pub fn keep_set(draw: &[TileId], out: &mut Vec<TileId>) {
    out.clear();
    out.extend_from_slice(draw);
    out.extend(draw.iter().filter_map(|t| t.parent()));
    out.sort_unstable();
    out.dedup();
}

/// Fills `out` with resident tiles outside sorted `keep`, farthest from the eye first.
pub fn eviction_order(
    resident: impl Iterator<Item = TileId>,
    keep: &[TileId],
    eye_body: DVec3,
    out: &mut Vec<(f64, TileId)>,
) {
    out.clear();
    out.extend(
        resident
            .filter(|t| keep.binary_search(t).is_err())
            .map(|t| (t.centre_body().distance_squared(eye_body), t)),
    );
    out.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
}

#[cfg(test)]
mod tests {
    use super::super::chart;
    use super::*;

    #[test]
    fn roots_draw_until_children_are_ready_then_split() {
        let eye = DVec3::new(0.0, 0.0, chart::RADIUS_M + 2_000.0);
        let none = |_: TileId| false;
        let s = select(&none, &[], eye, 900.0, 512);
        assert!(s.draw.iter().all(|t| t.level == 0));
        assert!(!s.wanted.is_empty() && s.wanted.iter().all(|t| t.level == 1));
        let all = |_: TileId| true;
        let s = select(&all, &[], eye, 900.0, 512);
        assert!(s.draw.iter().any(|t| t.level >= 6), "2 km up wants 7.8 m cells under the eye");
        assert!(s.draw.iter().all(|t| t.level <= MAX_LEVEL));
        assert!(s.draw.len() <= 512);
    }

    #[test]
    fn tiles_behind_the_limb_are_never_drawn_or_wanted() {
        let eye = DVec3::new(0.0, 0.0, chart::RADIUS_M + 400_000.0);
        let all = |_: TileId| true;
        let s = select(&all, &[], eye, 900.0, 512);
        // The four side roots straddle the limb with centres exactly on it; they stay for coverage.
        assert!(s.draw.iter().all(|t| t.centre_body().dot(eye) > -1.0));
        assert!(s.draw.iter().all(|t| projected_error_px(*t, eye, 900.0) > 0.0));
        let none = |_: TileId| false;
        let s = select(&none, &[], eye, 900.0, 512);
        assert!(s.wanted.iter().all(|t| projected_error_px(*t, eye, 900.0) > 0.0));
    }

    #[test]
    fn wanted_tiles_are_nearest_first() {
        let eye = DVec3::new(0.0, 0.0, chart::RADIUS_M + 2_000.0);
        let shallow = |t: TileId| t.level < 4;
        let s = select(&shallow, &[], eye, 900.0, 512);
        assert!(s.wanted.len() > 1);
        let d: Vec<f64> = s.wanted.iter().map(|t| t.centre_body().distance(eye)).collect();
        assert!(d.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn a_split_merges_back_only_under_the_lower_threshold() {
        let all = |_: TileId| true;
        let eye = DVec3::new(0.0, 0.0, chart::RADIUS_M + 2_000.0);
        let near = select(&all, &[], eye, 900.0, 1 << 16);
        // Shrinking the focal length puts some parents between 0.75 and 1.5 px; they stay split.
        let kept = select(&all, &near.draw, eye, 900.0 / 1.6, 1 << 16);
        let fresh = select(&all, &[], eye, 900.0 / 1.6, 1 << 16);
        assert!(kept.draw.len() > fresh.draw.len());
    }

    #[test]
    fn the_draw_cap_keeps_full_coverage() {
        let all = |_: TileId| true;
        let eye = DVec3::new(0.0, 0.0, chart::RADIUS_M + 2_000.0);
        let s = select(&all, &[], eye, 900.0, 40);
        assert!(s.draw.len() <= 40 && s.draw.iter().any(|t| t.level > 0));
    }

    #[test]
    fn eviction_keeps_the_drawn_set_and_their_parents_first() {
        let eye = DVec3::new(0.0, 0.0, chart::RADIUS_M + 2_000.0);
        let level6 = |k: u32| TileId { face: 2, level: 6, x: k % 64, y: k / 64 };
        let draw: Vec<TileId> = (0..300).map(level6).collect();
        let mut keep = Vec::new();
        keep_set(&draw, &mut keep);
        let mut parents: Vec<TileId> = draw.iter().filter_map(|t| t.parent()).collect();
        parents.sort_unstable();
        parents.dedup();
        assert!(parents.iter().all(|p| keep.binary_search(p).is_ok()));
        let mut resident: Vec<TileId> = (0..2_100 - parents.len() as u32).map(level6).collect();
        resident.extend(&parents);
        resident.sort();
        resident.dedup();
        assert_eq!(resident.len(), 2_100);
        let excess = resident.len() - 2_048;
        let mut order = Vec::new();
        eviction_order(resident.iter().copied(), &keep, eye, &mut order);
        let evicted: Vec<TileId> = order.iter().take(excess).map(|e| e.1).collect();
        assert_eq!(evicted.len(), excess);
        assert!(evicted.iter().all(|t| keep.binary_search(t).is_err()));
        let dist = |t: &TileId| t.centre_body().distance_squared(eye);
        let nearest_evicted = evicted.iter().map(dist).fold(f64::INFINITY, f64::min);
        let farthest_kept = resident
            .iter()
            .filter(|t| keep.binary_search(t).is_err() && !evicted.contains(t))
            .map(dist)
            .fold(0.0, f64::max);
        assert!(nearest_evicted >= farthest_kept, "farthest unused tiles go first");
    }
}
