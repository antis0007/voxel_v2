//! Chooses which globe tiles to draw and which to bake next.

use super::tile::{MAX_LEVEL, TileId, projected_error_in_view, projected_error_px};
use glam::{DVec3, Vec4};
use std::collections::HashSet;

const SPLIT_PX: f64 = 1.0;
/// A separate merge threshold prevents oscillation around one pixel.
const MERGE_PX: f64 = 0.5;
const MAX_WANTED: usize = 256;

#[derive(Default)]
pub struct Selection {
    pub draw: Vec<TileId>,
    /// Missing children of tiles that want to split, largest visible error first.
    pub wanted: Vec<TileId>,
}

/// Buffers `select_into` reuses, so a steady frame allocates nothing.
#[derive(Default)]
pub struct SelectScratch {
    split_last_frame: HashSet<TileId>,
    frontier: Vec<(TileId, f64)>,
    next: Vec<(TileId, f64)>,
    children: Vec<(TileId, f64, bool)>,
    wanted_scores: Vec<(TileId, f64)>,
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
    select_into_with_maturity(ready, &|_| true, eye_body, focal_px, &[], max_draw, scratch, out);
}

/// A new tile first converges to its parent before it can refine again.
pub fn select_into_with_maturity(
    ready: &dyn Fn(TileId) -> bool,
    mature: &dyn Fn(TileId) -> bool,
    eye_body: DVec3,
    focal_px: f64,
    planes: &[Vec4],
    max_draw: usize,
    scratch: &mut SelectScratch,
    out: &mut Selection,
) {
    let SelectScratch { split_last_frame, frontier, next, children, wanted_scores } = scratch;
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
    let visible = |t: TileId| {
        let error = projected_error_in_view(t, eye_body, focal_px, planes);
        (error > 0.0).then_some((t, error))
    };
    frontier.clear();
    frontier.extend((0..6).map(TileId::root).filter_map(visible));
    while !frontier.is_empty() {
        next.clear();
        frontier.sort_unstable_by(|(a, ea), (b, eb)| eb.total_cmp(ea).then(a.cmp(b)));
        let mut pending = frontier.len();
        for &(tile, err) in frontier.iter() {
            pending -= 1;
            let threshold = if split_last_frame.contains(&tile) { MERGE_PX } else { SPLIT_PX };
            if err <= threshold || tile.level >= MAX_LEVEL {
                draw.push(tile);
                continue;
            }
            children.clear();
            // Keep the split resident across camera turns. Otherwise revealing
            // one unready sibling makes every already-drawn child merge back.
            children.extend(tile.children().into_iter().filter_map(|child| {
                let error = projected_error_in_view(child, eye_body, focal_px, planes);
                // Keep siblings a turn could reveal. A tile beyond the limb
                // needs camera movement before it can enter the view.
                if error <= 0.0 && (planes.is_empty() || projected_error_px(child, eye_body, focal_px) <= 0.0) {
                    return None;
                }
                Some((child, error, ready(child)))
            }));
            let all_ready = children.iter().all(|&(_, _, ready)| ready);
            let visible_count = children.iter().filter(|&&(_, error, _)| error > 0.0).count();
            // Splitting past the cap would drop coverage, so the parent stays instead.
            let fits = draw.len() + pending + next.len() + visible_count <= max_draw;
            if all_ready && fits && visible_count != 0 && mature(tile) {
                next.extend(children.iter().filter_map(|&(child, error, _)| {
                    (error > 0.0).then_some((child, error))
                }));
            } else {
                if fits && visible_count != 0 {
                    wanted.extend(children.iter().filter_map(|&(child, _, ready)| (!ready).then_some(child)));
                }
                draw.push(tile);
            }
        }
        std::mem::swap(frontier, next);
    }
    // Resolve the coarsest visible pixels first. Distance ordering repeatedly
    // refined the centre while multi-pixel patches elsewhere stayed on roots.
    // Score once per tile instead of repeating projection math inside the sort.
    wanted_scores.clear();
    wanted_scores.extend(wanted.iter().copied().map(|tile| {
        let visible_error = projected_error_in_view(tile, eye_body, focal_px, planes);
        let score = if visible_error > 0.0 { visible_error }
            else { projected_error_px(tile, eye_body, focal_px) * 0.05 };
        (tile, score)
    }));
    wanted_scores.sort_unstable_by(|(a, ea), (b, eb)| eb.total_cmp(ea).then(a.cmp(b)));
    wanted.clear();
    wanted.extend(wanted_scores.iter().map(|(tile, _)| *tile));
    wanted.dedup();
    wanted.truncate(MAX_WANTED);
}

/// Fills `out` with `draw` and all ancestors, sorted for binary search.
///
/// Parents stay resident so a merge back one level draws at once instead of waiting a bake.
pub fn keep_set(draw: &[TileId], out: &mut Vec<TileId>) {
    out.clear();
    out.extend_from_slice(draw);
    // Every ancestor is a valid fallback after a fast retreat or view change.
    // Keeping only the immediate parent could evict the root under a fine tree.
    out.extend((0..6).map(TileId::root));
    for tile in draw {
        let mut ancestor = tile.parent();
        while let Some(parent) = ancestor {
            out.push(parent);
            // Retain the entire split so a sibling entering the view can
            // render immediately instead of triggering another bake.
            out.extend(parent.children());
            ancestor = parent.parent();
        }
    }
    out.sort_unstable();
    out.dedup();
}

pub fn detail_factor(id: TileId, eye: DVec3, focal_px: f64) -> f32 {
    let Some(parent) = id.parent() else { return 1.0; };
    // At a merge the children equal the raw parent, while that parent's
    // own parent is at least twice the projected size and fully detailed.
    let t = ((projected_error_px(parent,eye,focal_px)-MERGE_PX)/(SPLIT_PX-MERGE_PX)).clamp(0.0,1.0);
    (t*t*(3.0-2.0*t)) as f32
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
        // Tile error is cell size over distance, so a radius multiple keeps this tree at any planet size.
        let eye = DVec3::new(0.0, 0.0, chart::RADIUS_M * 20.6);
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
