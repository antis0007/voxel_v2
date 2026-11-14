//! A district's historical alignment and surveyed physical deck failures.
//! Alignments are orientation hints, not built regional rail connections.
//! Local lines terminate at stations until a surveyed regional route exists.
use super::{CELL, rotate};
use crate::{CachedProfiles, Profiles, WorldSampleConfig, sample_column};
use glam::Vec2;

pub const VERSION: u32 = engine_voxel::ANCHOR_LINE_GENERATOR;

/// Farthest a district line runs straight to the exchange.
const EXCHANGE_REACH: f32 = 3.0 * CELL;
const STEP_M: f32 = 6.0;

#[derive(Clone, Copy, Debug)]
pub struct Line {
    /// Historical bearing only; this does not establish a regional connection.
    pub toward: Vec2,
    /// Local x of the lost span, from a river under the deck or ground below the footings.
    pub gap: Option<f32>,
}

pub(super) fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= VERSION
}

fn exchange(cfg: &WorldSampleConfig) -> Option<Vec2> {
    crate::meridian_terminal::enabled(cfg)
        .then(|| Vec2::new(crate::meridian_terminal::X, crate::meridian_terminal::Z))
}

/// Historical bearing for district cell `(ix, iz)`; candidates are not stations.
pub(super) fn destination(cfg: &WorldSampleConfig, ix: i32, iz: i32, centre: Vec2) -> Option<Vec2> {
    let exchange = exchange(cfg);
    if let Some(at) = exchange
        && at.distance(centre) < EXCHANGE_REACH
    {
        return Some(at);
    }
    let toward = |p: Vec2| exchange.map_or(0.0, |at| at.distance(p));
    let own = toward(centre);
    let mut best: Option<(bool, f32, Vec2)> = None;
    for dz in -1..=1 {
        for dx in -1..=1 {
            if (dx, dz) == (0, 0) {
                continue;
            }
            let Some(c) = super::candidate(cfg, ix + dx, iz + dz) else { continue };
            // Downstream neighbours (closer to the exchange) win over nearer ones.
            let key = (toward(c) < own, -c.distance(centre), c);
            if best.is_none_or(|b| (key.0, key.1) > (b.0, b.1)) {
                best = Some(key);
            }
        }
    }
    best.map(|(_, _, c)| c)
}

/// Turn whose local +x points most nearly at `target`.
pub(super) fn turn_toward(centre: Vec2, target: Vec2) -> u32 {
    let bearing = (target - centre).normalize_or_zero();
    (0..4)
        .map(|turn| (turn, rotate(turn, 1.0, 0.0).dot(bearing)))
        .fold((0, f32::NEG_INFINITY), |best, next| if next.1 > best.1 { next } else { best })
        .0
}

/// Lost span inside `window`: running water under the deck first, then the deepest drop past the footings.
pub(super) fn failure(
    cfg: &WorldSampleConfig,
    toward: Vec2,
    centre: Vec2,
    turn: u32,
    datum: f32,
    corridor: f32,
    window: (f32, f32),
) -> Line {
    let mut worst: Option<(f32, f32)> = None;
    let mut x = window.0;
    while x <= window.1 {
        let w = centre + rotate(turn, x, corridor);
        let (bx, bz) = (cfg.blocks_for_height(w.x), cfg.blocks_for_height(w.y));
        // Whole metres so every voxel scale finds the same span.
        let drop = datum - cfg.world_y(sample_column(cfg, bx, bz).height + 1).floor();
        let score = if CachedProfiles.river_top(cfg, bx, bz).is_some() {
            Some(1000.0 + drop)
        } else {
            (drop > super::mega_detail::FOOTING).then_some(drop)
        };
        if let Some(score) = score
            && worst.is_none_or(|(_, s)| score > s)
        {
            worst = Some((x, score));
        }
        x += STEP_M;
    }
    Line { toward, gap: worst.map(|(x, _)| x) }
}

/// Review probe for district cell `(ix, iz)`: centre, turn and line.
pub fn review(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Option<(Vec2, u32, Option<Line>)> {
    let d = super::district_at(cfg, ix as f32 * CELL, iz as f32 * CELL)?;
    (d.cell == (ix, iz)).then_some((d.centre, d.turn, d.line))
}

/// Proximity to an actual surveyed street approach. Never award access for
/// the unbuilt strip beyond a rail abutment or an unsurveyed neighbour.
pub fn route_proximity(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    if !active(cfg) {
        return 0.0;
    }
    let m = glam::Vec3::new(x, 0.0, z);
    // Every approach is axis-aligned in district metres: edge <=220m,
    // outward run <=60m and width5m. Rotation only swaps/signs coordinates.
    //512m is a conservative support bound; keep the original search/eligibility.
    super::near_supported(cfg, m, 2.0 * CELL, 512.0)
        .iter()
        .map(|d| {
            let q = d.local(glam::Vec3::new(x, d.datum, z));
            d.approaches.iter()
                .filter_map(|a| a.offset(Vec2::new(q.x, q.z), 0.0))
                .map(|(_, across)| 1.0 - across / super::urban::AVENUE)
                .fold(0.0, f32::max)
        })
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_points_local_x_at_the_target() {
        let centre = Vec2::new(-300.0, 500.0);
        for target in [Vec2::new(900.0, 520.0), Vec2::new(-310.0, -800.0), Vec2::new(-2000.0, 400.0), Vec2::new(-250.0, 4000.0)] {
            let turn = turn_toward(centre, target);
            let axis = rotate(turn, 1.0, 0.0);
            assert!(axis.dot((target - centre).normalize()) > 0.7, "turn {turn} for {target}");
        }
    }
}
