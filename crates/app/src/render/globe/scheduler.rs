//! Chooses which globe tiles to draw and which to bake next.

use super::tile::{MAX_LEVEL, TileId, projected_error_px};
use glam::DVec3;
use std::collections::HashSet;

const SPLIT_PX: f64 = 1.5;
/// Lower merge threshold so a tile hovering near 1.5 px does not flicker between levels.
const MERGE_PX: f64 = 0.75;
const MAX_WANTED: usize = 256;

pub struct Selection {
    pub draw: Vec<TileId>,
    /// Missing children of tiles that want to split, nearest first.
    pub wanted: Vec<TileId>,
}

/// Breadth-first error-driven selection from the six cube roots.
pub fn select(
    ready: &dyn Fn(TileId) -> bool,
    previous_draw: &[TileId],
    eye_body: DVec3,
    focal_px: f64,
    max_draw: usize,
) -> Selection {
    let mut split_last_frame = HashSet::new();
    for t in previous_draw {
        let mut p = t.parent();
        while let Some(a) = p {
            if !split_last_frame.insert(a) {
                break;
            }
            p = a.parent();
        }
    }
    let visible = |t: &TileId| projected_error_px(*t, eye_body, focal_px) > 0.0;
    let mut frontier: Vec<TileId> = (0..6).map(TileId::root).filter(visible).collect();
    let mut draw = Vec::new();
    let mut wanted = Vec::new();
    while !frontier.is_empty() {
        let mut next = Vec::new();
        let mut pending = frontier.len();
        for tile in frontier {
            pending -= 1;
            let err = projected_error_px(tile, eye_body, focal_px);
            let threshold = if split_last_frame.contains(&tile) { MERGE_PX } else { SPLIT_PX };
            if err <= threshold || tile.level >= MAX_LEVEL {
                draw.push(tile);
                continue;
            }
            let children: Vec<TileId> = tile.children().into_iter().filter(visible).collect();
            let missing: Vec<TileId> = children.iter().copied().filter(|c| !ready(*c)).collect();
            // Splitting past the cap would drop coverage, so the parent stays instead.
            let fits = draw.len() + pending + next.len() + children.len() <= max_draw;
            if missing.is_empty() && fits {
                next.extend(children);
            } else {
                wanted.extend(missing);
                draw.push(tile);
            }
        }
        frontier = next;
    }
    let dist = |t: &TileId| t.centre_body().distance(eye_body);
    wanted.sort_by(|a, b| dist(a).total_cmp(&dist(b)).then(a.cmp(b)));
    wanted.dedup();
    wanted.truncate(MAX_WANTED);
    Selection { draw, wanted }
}

#[cfg(test)]
mod tests {
    use super::super::bake::TileData;
    use super::super::cache::TileCache;
    use super::super::chart;
    use super::super::worker::Worker;
    use super::*;
    use std::time::Duration;

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

    fn test_cache(name: &str) -> TileCache {
        let dir = std::env::temp_dir().join(format!("globe-worker-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        TileCache::open(&dir, 1, 1)
    }

    fn test_cfg() -> engine_voxel::WorldSampleConfig {
        engine_voxel::WorldSampleConfig {
            generator_version: 53,
            seed: 74,
            world_type: engine_voxel::WorldType::Hills,
            voxel_scale: engine_core::VoxelScalePreset::OneMeter,
        }
    }

    #[test]
    fn a_dead_worker_reports_not_alive_and_drains_cleanly() {
        let bake = |_: &engine_voxel::WorldSampleConfig, _: TileId| -> Option<TileData> { panic!("bake failed") };
        let w = Worker::spawn_with(test_cfg(), bake, test_cache("dead"), Duration::from_millis(2));
        w.request(&[TileId::root(0)]);
        std::thread::sleep(Duration::from_millis(50));
        assert!(!w.alive());
        assert!(w.drain(2 << 20).is_empty());
    }

    #[test]
    fn a_worker_delivers_tiles_within_the_byte_cap() {
        let bake = |_: &engine_voxel::WorldSampleConfig, id: TileId| Some(TileData::empty(id));
        let w = Worker::spawn_with(test_cfg(), bake, test_cache("ok"), Duration::from_millis(2));
        let ids: Vec<TileId> = (0..3u32).map(|x| TileId { face: 0, level: 2, x, y: 0 }).collect();
        w.request(&ids);
        std::thread::sleep(Duration::from_millis(200));
        assert!(w.alive());
        assert_eq!(w.drain(25_350 * 2).len(), 2);
        assert_eq!(w.drain(2 << 20).len(), 1);
    }
}
