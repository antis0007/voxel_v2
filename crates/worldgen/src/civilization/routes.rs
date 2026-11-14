//! Terrain-following connections between surveyed holds. Routes never reshape
//! cliffs or place bridges: unsafe edges remain disconnected.
use super::*;
use std::{cmp::Reverse, collections::BinaryHeap};

pub(super) fn find(cfg: &WorldSampleConfig, start: Vec2, end: Vec2) -> Vec<Vec2> {
    const STEP: f32 = 16.0;
    let low = start.min(end) - Vec2::splat(64.0);
    let extent = start.max(end) + Vec2::splat(64.0) - low;
    let nx = (extent.x / STEP).ceil() as usize + 1;
    let nz = (extent.y / STEP).ceil() as usize + 1;
    if nx * nz > 2048 { return Vec::new(); }
    let index = |p: Vec2| {
        let q = ((p - low) / STEP).round().as_uvec2();
        q.y as usize * nx + q.x as usize
    };
    let first = index(start);
    let last = index(end);
    let position = |i: usize| {
        if i == first { start } else if i == last { end }
        else { low + Vec2::new((i % nx) as f32, (i / nx) as f32) * STEP }
    };
    let mut sampled: Vec<Option<crate::geo::Ground>> = vec![None; nx * nz];
    let mut blocked = std::collections::HashSet::new();
    for _ in 0..4 {
        let mut costs = vec![u32::MAX; nx * nz];
        let mut previous = vec![usize::MAX; nx * nz];
        let mut queue = BinaryHeap::new();
        costs[first] = 0;
        queue.push(Reverse((0u32, first)));
        while let Some(Reverse((cost, i))) = queue.pop() {
            if cost != costs[i] { continue; }
            if i == last { break; }
            let p = position(i);
            let a = *sampled[i].get_or_insert_with(|| planetary::ground(cfg, p.x, p.y, STEP));
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)] {
                let x = (i % nx) as i32 + dx;
                let z = (i / nx) as i32 + dz;
                if x < 0 || z < 0 || x >= nx as i32 || z >= nz as i32 { continue; }
                let j = z as usize * nx + x as usize;
                if blocked.contains(&(i.min(j), i.max(j))) { continue; }
                let q = position(j);
                let b = *sampled[j].get_or_insert_with(|| planetary::ground(cfg, q.x, q.y, STEP));
                let distance = p.distance(q).max(0.01);
                let grade = (a.top_m - b.top_m).abs() / distance;
                if !dry(&a) || !dry(&b) || grade > 0.48 || b.slope > 0.75 { continue; }
                let weight = distance * (1.0 + grade * grade * 18.0 + b.slope * 1.5 + b.riparian * 0.25);
                let next = cost.saturating_add((weight * 100.0).ceil() as u32);
                if next < costs[j] { costs[j] = next; previous[j] = i; queue.push(Reverse((next, j))); }
            }
        }
        if costs[last] == u32::MAX { return Vec::new(); }
        let mut indices = vec![last];
        while *indices.last().unwrap() != first { indices.push(previous[*indices.last().unwrap()]); }
        indices.reverse();
        let mut valid = true;
        for edge in indices.windows(2) {
            if !walkable(cfg, position(edge[0]), position(edge[1])) {
                blocked.insert((edge[0].min(edge[1]), edge[0].max(edge[1])));
                valid = false;
            }
        }
        if valid { return indices.into_iter().map(position).collect(); }
    }
    Vec::new()
}

fn dry(g: &crate::geo::Ground) -> bool {
    g.top_m > crate::WATER_LEVEL_METERS + 0.5 && g.water_m.is_none()
}

fn walkable(cfg: &WorldSampleConfig, start: Vec2, end: Vec2) -> bool {
    let count = (start.distance(end) / 2.0).ceil().max(1.0) as usize;
    let mut previous = None;
    for i in 0..=count {
        let p = start.lerp(end, i as f32 / count as f32);
        let g = planetary::ground(cfg, p.x, p.y, 1.0);
        if !dry(&g) || previous.is_some_and(|h: f32| (g.top_m - h).abs() > 1.1) { return false; }
        previous = Some(g.top_m);
    }
    true
}

pub(super) fn contains(cfg: &WorldSampleConfig, p: Vec2, radius: f32) -> bool {
    if !enabled(cfg) || !planetary::landscape_cycles(cfg.generator_version) { return false; }
    let key = key_at(p);
    let local = (p - cell_center(key)).abs();
    if local.x > 248.0 || local.y > 120.0 { return false; }
    plan(cfg, key).route.windows(2).any(|edge| {
        let ab = edge[1] - edge[0];
        let t = ((p - edge[0]).dot(ab) / ab.length_squared().max(0.01)).clamp(0.0, 1.0);
        p.distance_squared(edge[0] + ab * t) < radius * radius
    })
}
