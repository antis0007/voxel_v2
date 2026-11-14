//! Gate 136: floodplains, valley walls and a riparian band around solved channels.
//!
//! Every effect ends within `REACH_M` of the centre line. `rivers::nearest`
//! searches two solve cells (about 188 m on the planet face) and only promises
//! the true nearest channel within about one cell, so nothing may depend on a
//! channel farther away than that or the ground would step where the search
//! changes its answer.

use super::SEA_M;
use super::planet::Planet;
use super::rivers::Channel;
use crate::WorldSampleConfig;
use std::sync::{Arc, Mutex, Weak};

pub const VERSION: u32 = 136;
/// Preserve regional drainage in distant geometry; older saves keep their LOD.
pub const LOD_VERSION: u32 = 138;

/// Distance from the centre line past which no corridor effect remains.
pub const REACH_M: f64 = 84.0;

pub fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= VERSION && crate::basin::solved_face(cfg)
}

pub fn lod_active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= LOD_VERSION && active(cfg)
}

/// Filter the valley at its own scale, independently of the 32 m detail cutoff.
/// No widening of river beds or wet bands: these remain the walked world's fields.
pub fn lod_weight(footprint: f32) -> f64 {
    1.0 - smoothstep(REACH_M, REACH_M * 4.0, f64::from(footprint))
}

/// A narrow bed disappears before its much broader valley does.
pub fn bed_weight(c: &Channel, footprint: f32) -> f64 {
    let start = (2.0 * c.half_width).max(32.0);
    let end = (8.0 * c.half_width).max(64.0);
    1.0 - smoothstep(start, end, f64::from(footprint))
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Half-width of the flat floodplain: headwaters get a strip, main stems a plain.
pub fn floor_half(c: &Channel) -> f64 {
    (c.half_width + 4.0 + 4.0 * c.size).min(50.0)
}

/// Highest the ground may stand at `c.distance`: floodplain, then a wall whose
/// slope eases from incised headwaters to broad main stems, then a steep
/// return so the ceiling clears any ground before `REACH_M`.
pub fn ceiling_m(c: &Channel) -> f64 {
    let floor = floor_half(c);
    let d = c.distance.max(c.half_width);
    let wall = (0.85 - 0.04 * c.size).clamp(0.32, 0.85);
    let rise = if d < floor {
        0.5 + 0.9 * smoothstep(c.half_width, floor, d)
    } else {
        1.4 + (d - floor) * wall
    };
    let close = (d - (REACH_M - 16.0)).max(0.0);
    c.water_m + rise + close * close * 0.6
}

/// Detail noise fades toward the floodplain so it stays a readable flat.
pub fn calm(c: &Channel) -> f64 {
    let floor = floor_half(c);
    smoothstep(floor * 0.6, floor + 16.0, c.distance)
}

/// Deepest cut below the solved surface, reached by rivers of 7.5 km2 and up.
pub const MAX_INCISION_M: f32 = 9.0;
/// Links over which a cut eases out toward the sea, a lake or a lake outlet.
const RAMP_LINKS: u16 = 4;

/// Water level per solve node with each reach cut into its valley.
/// Sea, lakes and the face edge keep their level, and every node stays at or
/// above its receiver, so incised water never runs uphill into still water.
pub fn incised_levels(p: &Arc<Planet>) -> Arc<[f32]> {
    static CACHE: Mutex<Vec<(Weak<Planet>, Arc<[f32]>)>> = Mutex::new(Vec::new());
    let Ok(mut cache) = CACHE.lock() else { return solve_levels(p).into() };
    cache.retain(|(w, _)| w.strong_count() > 0);
    if let Some((_, l)) = cache.iter().find(|(w, _)| w.upgrade().is_some_and(|q| Arc::ptr_eq(&q, p))) {
        return Arc::clone(l);
    }
    let levels: Arc<[f32]> = solve_levels(p).into();
    cache.push((Arc::downgrade(p), Arc::clone(&levels)));
    levels
}

fn solve_levels(p: &Planet) -> Vec<f32> {
    let n = p.height.len();
    let receiver = |i: usize| p.receiver[i] as usize;
    let still = |i: usize| !p.land[i] || p.lake[i] > f32::MIN || receiver(i) == i;
    let surface = |i: usize| if p.lake[i] > f32::MIN { p.lake[i] } else { p.height[i] };
    // Links from the nearest still water, upstream and downstream.
    let mut near = vec![RAMP_LINKS; n];
    let mut chain = Vec::new();
    let mut down = vec![u16::MAX; n];
    for start in 0..n {
        let mut i = start;
        while down[i] == u16::MAX && !still(i) && chain.len() <= n {
            chain.push(i);
            i = receiver(i);
        }
        let mut hops = if still(i) || down[i] == u16::MAX { 0 } else { down[i] };
        down[i] = hops;
        while let Some(j) = chain.pop() {
            hops = hops.saturating_add(1);
            down[j] = hops;
        }
    }
    for s in (0..n).filter(|&i| still(i)) {
        let mut i = s;
        for k in 1..RAMP_LINKS {
            let r = receiver(i);
            if r == i || still(r) {
                break;
            }
            near[r] = near[r].min(k);
            i = r;
        }
    }
    let cut = |i: usize| {
        let size = (p.discharge[i].max(0.0) / 1.0e6).sqrt();
        let links = f32::from(near[i].min(down[i]).min(RAMP_LINKS)) / f32::from(RAMP_LINKS);
        (1.2 * size).min(MAX_INCISION_M) * links * links * (3.0 - 2.0 * links)
    };
    let mut level = vec![f32::NAN; n];
    for start in 0..n {
        let mut i = start;
        while level[i].is_nan() && !still(i) && chain.len() <= n {
            chain.push(i);
            i = receiver(i);
        }
        if level[i].is_nan() {
            level[i] = surface(i);
        }
        let mut below = level[i];
        while let Some(j) = chain.pop() {
            below = (surface(j) - cut(j)).max(below).max(SEA_M as f32);
            level[j] = below;
        }
    }
    level
}

/// Riparian strength 0..1: full across the floodplain, gone 30 m past it.
/// Trickles count for little; rivers with over 25 km2 upstream count fully.
pub fn riparian(c: &Channel) -> f32 {
    let floor = floor_half(c);
    let band = 1.0 - smoothstep(floor, floor + 30.0, c.distance);
    (band * smoothstep(1.6, 5.0, c.size)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(size: f64, distance: f64) -> Channel {
        Channel { distance, half_width: (1.2 + 0.9 * size).min(26.0), depth: 1.0, water_m: 40.0, size, downstream: glam::Vec2::ZERO }
    }

    #[test]
    fn every_effect_ends_inside_the_search_reach() {
        for size in [1.6, 4.0, 12.0, 40.0] {
            let far = channel(size, REACH_M);
            assert!(ceiling_m(&far) > far.water_m + 60.0, "size {size}: {}", ceiling_m(&far));
            assert_eq!(riparian(&far), 0.0);
            assert_eq!(calm(&far), 1.0);
        }
    }

    #[test]
    fn main_stems_get_wider_floors_and_gentler_walls_than_headwaters() {
        let (head, main) = (channel(1.6, 0.0), channel(14.0, 0.0));
        assert!(floor_half(&main) > floor_half(&head) * 2.5);
        let rise = |size: f64| {
            let a = channel(size, floor_half(&channel(size, 0.0)) + 5.0);
            let b = channel(size, floor_half(&channel(size, 0.0)) + 15.0);
            ceiling_m(&b) - ceiling_m(&a)
        };
        assert!(rise(1.6) > rise(14.0) * 1.5, "head {} main {}", rise(1.6), rise(14.0));
        assert!(riparian(&main) > 0.99 && riparian(&head) == 0.0);
    }

    #[test]
    fn the_ceiling_never_drops_toward_the_banks() {
        for size in [1.6, 6.0, 20.0] {
            let mut last = f64::MIN;
            for k in 0..=100 {
                let c = channel(size, k as f64 * REACH_M / 100.0);
                let h = ceiling_m(&c);
                assert!(h >= last, "size {size} at {} m", c.distance);
                last = h;
            }
        }
    }
}
