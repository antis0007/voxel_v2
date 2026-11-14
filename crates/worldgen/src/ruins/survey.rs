use super::*;

/// Fit the long face to the terrain before the full reservation and water survey.
pub(super) fn aligned_turn(cfg: &WorldSampleConfig, centre: Vec2, turn: u32, half: Vec2) -> u32 {
    let relief = |turn| {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for z in [-half.y, 0.0, half.y] {
            for x in [-half.x, 0.0, half.x] {
                let p = centre + rotate(turn, x, z);
                let h = cfg.world_y(sample_column(cfg, cfg.blocks_for_height(p.x), cfg.blocks_for_height(p.y)).height + 1);
                lo = lo.min(h);
                hi = hi.max(h);
            }
        }
        hi - lo
    };
    let alternate = (turn + 1) % 4;
    if relief(alternate) + 3.0 < relief(turn) { alternate } else { turn }
}

/// Median ground under the deck, or None where the plot is unbuildable.
pub(super) fn survey(cfg: &WorldSampleConfig, centre: Vec2, turn: u32, half: Vec2) -> Option<f32> {
    let margin = (half + Vec2::new(4.0, 64.0)).length();
    if crate::planetary::reserves_volcanic_summit(cfg, centre.x, centre.y, margin) { return None; }
    let mut heights = Vec::new();
    let mut river = 0;
    let stride = if half == urban::CITY_HALF { 18.0 } else { 12.0 };
    let steps = (half / stride).ceil().as_ivec2();
    for sz in -steps.y..=steps.y {
        for sx in -steps.x..=steps.x {
            let d = rotate(turn, sx as f32 * stride, sz as f32 * stride);
            let w = centre + d;
            if crate::valley::reserves(cfg, w.x, w.y) || crate::amberroot::reserves(cfg, w.x, w.y) {
                return None;
            }
            let (x, z) = (cfg.blocks_for_height(w.x), cfg.blocks_for_height(w.y));
            // Whole metres: every voxel scale must plan the same district.
            let h = cfg.world_y(sample_column(cfg, x, z).height + 1).floor();
            let on_deck = d.x.abs() <= half.x + 6.0 && d.y.abs() <= half.y + 6.0;
            if crate::undercrown::reserved(cfg, Vec3::new(w.x, h, w.y)) {
                return None;
            }
            if on_deck && CachedProfiles.river_top(cfg, x, z).is_some() {
                river += 1;
            }
            heights.push(h);
        }
    }
    heights.sort_by(f32::total_cmp);
    let n = heights.len();
    let (low, median, high) = (heights[n / 10], heights[n / 2], heights[n * 9 / 10]);
    // A stream or two passes under the deck in culverts; a river valley does not.
    if river * 12 > n || low < WATER_LEVEL_METERS - 2.0 || median < WATER_LEVEL_METERS + 4.0 || high - low > 45.0 || median > 200.0 {
        return None;
    }
    Some(median)
}
