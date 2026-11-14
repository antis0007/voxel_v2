//! The labyrinth of false walls.
use super::{Complex, Job, Room};
use crate::{BlockId as B, hash2};

/// Generator 72: the engine-works maze. Cable and pipe run along the wall
/// tops, cell banks stand at the posts, every dead end holds a cache and the
/// farthest one a relic.
pub const TECH_VERSION: u32 = crate::gates::CONNECTED_ROOFS_GENERATOR;
const G: f32 = 3.0;

/// Whether this room is a machined maze rather than a plain one.
pub fn machined(c: &Complex, r: &Room) -> bool {
    c.version >= TECH_VERSION && (c.job == Job::Workshop || hash2(0, 0, r.salt.wrapping_add(41)) < 0.3)
}

fn grid(r: &Room) -> (i32, i32) {
    ((r.w() / G).floor() as i32, (r.d() / G).floor() as i32)
}

/// Binary-tree rule: a cell carves east when true, north when false.
fn opens_x(r: &Room, i: i32, j: i32) -> bool {
    let (nx, nz) = grid(r);
    if j >= nz - 1 { true } else if i >= nx - 1 { false } else { hash2(i, j, r.salt.wrapping_add(31)) < 0.5 }
}

/// Passages leaving cell `(i, j)`.
fn exits(r: &Room, i: i32, j: i32) -> u32 {
    let (nx, nz) = grid(r);
    let east = opens_x(r, i, j) && i < nx - 1;
    let north = !opens_x(r, i, j) && j < nz - 1;
    let west = i > 0 && opens_x(r, i - 1, j);
    let south = j > 0 && !opens_x(r, i, j - 1);
    u32::from(east) + u32::from(north) + u32::from(west) + u32::from(south)
}

fn dead_end(r: &Room, i: i32, j: i32) -> bool {
    (i > 0 || j > 0) && exits(r, i, j) == 1
}

/// The dead end farthest from the entry corner holds the relic.
fn relic_cell(r: &Room) -> Option<(i32, i32)> {
    let (nx, nz) = grid(r);
    let mut best: Option<((i32, i32), i32)> = None;
    for j in 0..nz {
        for i in 0..nx {
            if dead_end(r, i, j) && best.is_none_or(|(_, d)| i + j > d) {
                best = Some(((i, j), i + j));
            }
        }
    }
    best.map(|(cell, _)| cell)
}

/// Binary-tree maze on a 3 m grid; half its walls are illusions.
pub fn labyrinth(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let (nx, nz) = grid(r);
    let (ci, cj) = ((rx / G).floor() as i32, (rz / G).floor() as i32);
    let (lx, lz) = (rx - ci as f32 * G, rz - cj as f32 * G);
    if fy >= r.h || ci <= 0 && cj <= 0 {
        return None;
    }
    let tech = machined(c, r);
    let wall = |seg: (i32, i32, bool)| -> B {
        let mirage = hash2(seg.0 * 2 + seg.2 as i32, seg.1, r.salt.wrapping_add(37)) < 0.5;
        if mirage {
            B::MirageBrick
        } else if tech && fy >= r.h - v {
            if hash2(seg.0, seg.1 + 97, r.salt.wrapping_add(43)) < 0.5 { B::PowerCable } else { B::ItemPipe }
        } else {
            c.pal.wall
        }
    };
    if lx < v && lz < v && ci > 0 && cj > 0 {
        let bank = tech && fy < v && hash2(ci, cj, r.salt.wrapping_add(45)) > 0.8;
        return Some(if bank { B::BatteryBank } else { c.pal.trim });
    }
    if tech && dead_end(r, ci, cj) && (lx - 1.5).abs() < 0.5 && (lz - 1.5).abs() < 0.5 {
        let relic = relic_cell(r) == Some((ci, cj));
        if fy < v {
            return Some(if relic { c.pal.accent } else { c.pal.trim });
        }
        if fy < 2.0 * v {
            return Some(if relic { B::SurveyCache } else { B::SalvageCrate });
        }
    }
    // A lamp in one cell in four, tucked into a corner beside its post.
    if hash2(ci, cj, r.salt.wrapping_add(39)) > 0.75 && (v..2.0 * v).contains(&lx) && (v..2.0 * v).contains(&lz) && fy < v {
        return Some(if tech { B::Crystal } else { B::Torch });
    }
    if lx < v && ci > 0 && ci < nx && !opens_x(r, ci - 1, cj) {
        return Some(wall((ci, cj, true)));
    }
    if lz < v && cj > 0 && cj < nz && opens_x(r, ci, cj - 1) {
        return Some(wall((ci, cj, false)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every dead end is reachable from the entry corner through real walls
    /// alone, and the relic sits in the farthest one.
    #[test]
    fn machined_maze_dead_ends_are_caches_and_reachable() {
        let r = Room {
            x0: 0.0, x1: 27.0, z0: 0.0, z1: 24.0, floor: 0.0, h: 4.6, level: 0,
            kind: super::super::Kind::Labyrinth, damage: super::super::Damage::Intact, ceiling: super::super::Ceiling::Beams,
            outline: super::super::Outline::Rect, rift: 0.0, cache: false, stair: 0, sealed: true, salt: 9,
        };
        let (nx, nz) = grid(&r);
        let mut seen = vec![false; (nx * nz) as usize];
        let mut stack = vec![(0, 0)];
        while let Some((i, j)) = stack.pop() {
            if seen[(j * nx + i) as usize] { continue; }
            seen[(j * nx + i) as usize] = true;
            let mut push = |a: i32, b: i32| if a >= 0 && b >= 0 && a < nx && b < nz { stack.push((a, b)) };
            if opens_x(&r, i, j) && i < nx - 1 { push(i + 1, j); }
            if !opens_x(&r, i, j) && j < nz - 1 { push(i, j + 1); }
            if i > 0 && opens_x(&r, i - 1, j) { push(i - 1, j); }
            if j > 0 && !opens_x(&r, i, j - 1) { push(i, j - 1); }
        }
        assert!(seen.iter().all(|s| *s), "every cell reachable");
        let ends: Vec<_> = (0..nz).flat_map(|j| (0..nx).map(move |i| (i, j))).filter(|&(i, j)| dead_end(&r, i, j)).collect();
        assert!(ends.len() >= 3, "a 9x8 maze has several dead ends: {ends:?}");
        let relic = relic_cell(&r).expect("a relic cell");
        assert!(ends.contains(&relic));
        assert!(ends.iter().all(|&(i, j)| i + j <= relic.0 + relic.1));
    }
}
