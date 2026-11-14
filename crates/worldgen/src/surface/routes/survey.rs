//! Graded path search around ground a direct trail curve cannot climb.
//!
//! A bounded A* over a coarse grid refuses moves steeper than the trail grade,
//! so a slope is crossed by traversing or switching back across it. The cell
//! path is pulled taut where the ground allows and its corners are rounded.
use super::{MAX_GRADE, Trail, Vec3, WorldSampleConfig, ground};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

const CELL: f32 = 6.0;
const MARGIN: f32 = 60.0;
const MAX_SIDE: usize = 110;
const MAX_EXPANDED: usize = 9_000;
/// Grade one move may take; the margin under `MAX_GRADE` leaves room for the bed fit.
const MOVE_GRADE: f32 = MAX_GRADE * 0.8;
/// Distance from a cell centre to a reserve the verge must keep, metres.
const CLEARANCE_M: f32 = 3.0;
/// Fillet radius a corner gets when its legs are long enough, metres.
const CORNER_RADIUS_M: f32 = 5.0;
const FILLET_STEPS: usize = 8;

struct Grid<'a> {
    cfg: &'a WorldSampleConfig,
    x0: f32,
    z0: f32,
    w: usize,
    h: usize,
    cell_size: f32,
    move_grade: f32,
    clearance: f32,
    height: Vec<f32>,
    blocked: &'a dyn Fn(f32, f32) -> bool,
}

impl Grid<'_> {
    fn centre(&self, i: usize) -> (f32, f32) {
        (self.x0 + (i % self.w) as f32 * self.cell_size, self.z0 + (i / self.w) as f32 * self.cell_size)
    }

    fn cell(&self, x: f32, z: f32) -> Option<usize> {
        let (cx, cz) = (((x - self.x0) / self.cell_size).round(), ((z - self.z0) / self.cell_size).round());
        (cx >= 0.0 && cz >= 0.0 && (cx as usize) < self.w && (cz as usize) < self.h).then(|| cz as usize * self.w + cx as usize)
    }

    /// Ground height of a cell, `NAN` when the trail may not use it.
    fn height(&mut self, i: usize) -> f32 {
        if self.height[i].is_infinite() {
            let (x, z) = self.centre(i);
            let h = ground(self.cfg, x, z);
            // The bed fit tests its verges too, so a cell beside a reserve is closed.
            let c = self.clearance;
            let near_reserve = [(0.0, 0.0), (c, 0.0), (-c, 0.0), (0.0, c), (0.0, -c)]
                .iter()
                .any(|(dx, dz)| (self.blocked)(x + dx, z + dz));
            self.height[i] = if h <= 14.0 || near_reserve { f32::NAN } else { h };
        }
        self.height[i]
    }

    /// Straight leg whose ground stays open and within the move grade.
    fn clear(&mut self, a: (f32, f32), b: (f32, f32)) -> bool {
        let length = (b.0 - a.0).hypot(b.1 - a.1);
        let steps = (length / (self.cell_size * 0.5)).ceil().max(1.0) as usize;
        let mut last: Option<(f32, f32)> = None;
        for k in 0..=steps {
            let f = k as f32 / steps as f32;
            let (x, z) = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
            let Some(i) = self.cell(x, z) else { return false };
            let h = self.height(i);
            if h.is_nan() {
                return false;
            }
            let along = f * length;
            if let Some((h0, s0)) = last
                && along > s0
                && (h - h0).abs() > (along - s0).max(self.cell_size) * self.move_grade
            {
                return false;
            }
            if last.is_none_or(|(h0, _)| h != h0) {
                last = Some((h, along));
            }
        }
        true
    }
}

/// A surveyed trail from `a` to `b`, or `None` when no graded path fits the window.
pub(super) fn route(cfg: &WorldSampleConfig, a: Vec3, b: Vec3, blocked: &dyn Fn(f32, f32) -> bool, seed: u32, half_width: f32) -> Option<Trail> {
    let (x0, z0) = (a.x.min(b.x) - MARGIN, a.z.min(b.z) - MARGIN);
    let (width, height) = (a.x.max(b.x) + MARGIN - x0, a.z.max(b.z) + MARGIN - z0);
    let anchor = super::super::planet_roads::active(cfg);
    let cell_size = if anchor { (width.max(height) / (MAX_SIDE - 1) as f32).ceil().max(CELL) } else { CELL };
    let w = (width / cell_size).ceil() as usize + 1;
    let h = (height / cell_size).ceil() as usize + 1;
    if w > MAX_SIDE || h > MAX_SIDE {
        return None;
    }
    let mut grid = Grid {
        cfg, x0, z0, w, h, cell_size, height: vec![f32::INFINITY; w * h], blocked,
        move_grade: if anchor && half_width > super::FOOTPATH_HALF_WIDTH { super::ANCHOR_ROAD_GRADE * 0.9 } else { MOVE_GRADE },
        clearance: if anchor { half_width + 0.6 } else { CLEARANCE_M },
    };
    let start = grid.cell(a.x, a.z)?;
    let goal = grid.cell(b.x, b.z)?;
    let cells = search(&mut grid, start, goal)?;
    let mut points: Vec<(f32, f32)> = cells.iter().map(|&i| grid.centre(i)).collect();
    points[0] = (a.x, a.z);
    let last = points.len() - 1;
    points[last] = (b.x, b.z);
    let taut = pull(&mut grid, &points);
    let path = round(&taut);
    (path.len() >= 2).then(|| Trail::polyline(path, seed, half_width))
}

fn search(grid: &mut Grid, start: usize, goal: usize) -> Option<Vec<usize>> {
    let n = grid.w * grid.h;
    let mut cost = vec![f32::INFINITY; n];
    let mut from = vec![usize::MAX; n];
    let mut heap = BinaryHeap::new();
    let goal_at = grid.centre(goal);
    let estimate = |p: (f32, f32)| (p.0 - goal_at.0).hypot(p.1 - goal_at.1);
    // Doors and waypoints may sit on a step the grid rejects; their own cells stay open.
    let start_h = { let h = grid.height(start); if h.is_nan() { ground(grid.cfg, grid.centre(start).0, grid.centre(start).1) } else { h } };
    grid.height[start] = start_h;
    let goal_h = grid.height(goal);
    if goal_h.is_nan() {
        grid.height[goal] = ground(grid.cfg, goal_at.0, goal_at.1);
    }
    cost[start] = 0.0;
    heap.push((Reverse((estimate(grid.centre(start)) * 16.0) as u32), start));
    let mut expanded = 0;
    while let Some((_, i)) = heap.pop() {
        if i == goal {
            let mut out = vec![goal];
            let mut at = goal;
            while at != start {
                at = from[at];
                out.push(at);
            }
            out.reverse();
            return Some(out);
        }
        expanded += 1;
        if expanded > MAX_EXPANDED {
            return None;
        }
        let (cx, cz) = ((i % grid.w) as i32, (i / grid.w) as i32);
        let hi = grid.height[i];
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (nx, nz) = (cx + dx, cz + dz);
            if nx < 0 || nz < 0 || nx as usize >= grid.w || nz as usize >= grid.h {
                continue;
            }
            let j = nz as usize * grid.w + nx as usize;
            let hj = grid.height(j);
            if hj.is_nan() {
                continue;
            }
            let length = if dx != 0 && dz != 0 { grid.cell_size * std::f32::consts::SQRT_2 } else { grid.cell_size };
            let slope = (hj - hi).abs() / length;
            if slope > grid.move_grade {
                continue;
            }
            // Climbing costs extra, so level traverses win over the steepest legal line.
            let next = cost[i] + length * (1.0 + 2.5 * slope / grid.move_grade);
            if next < cost[j] {
                cost[j] = next;
                from[j] = i;
                heap.push((Reverse(((next + estimate(grid.centre(j))) * 16.0) as u32), j));
            }
        }
    }
    None
}

/// Drops points a straight, graded leg can skip.
fn pull(grid: &mut Grid, points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut out = vec![points[0]];
    let mut anchor = 0;
    while anchor < points.len() - 1 {
        let mut next = anchor + 1;
        for j in (anchor + 2..points.len()).rev() {
            if grid.clear(points[anchor], points[j]) {
                next = j;
                break;
            }
        }
        out.push(points[next]);
        anchor = next;
    }
    out
}

/// Replaces each interior corner with a circular fillet tangent to both legs.
pub(super) fn round(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out = vec![points[0]];
    let mut leg_start = points[0];
    for k in 1..points.len() - 1 {
        let (c, n) = (points[k], points[k + 1]);
        let unit = |q: (f32, f32)| {
            let d = (q.0 - c.0).hypot(q.1 - c.1).max(1e-6);
            ((q.0 - c.0) / d, (q.1 - c.1) / d, d)
        };
        let (ix, iz, din) = unit(leg_start);
        let (ox, oz, dout) = unit(n);
        let interior = (ix * ox + iz * oz).clamp(-1.0, 1.0).acos();
        if interior > 3.1 {
            out.push(c);
            leg_start = c;
            continue;
        }
        let half = interior * 0.5;
        // A hairpin cannot keep the full radius within its legs; it takes the widest that fits.
        let tangent = (CORNER_RADIUS_M / half.tan()).min(din * 0.45).min(dout * 0.45);
        let radius = tangent * half.tan();
        let (bx, bz) = ((ix + ox) * 0.5, (iz + oz) * 0.5);
        let bl = bx.hypot(bz).max(1e-6);
        let centre_d = tangent / half.cos();
        let centre = (c.0 + bx / bl * centre_d, c.1 + bz / bl * centre_d);
        let enter = (c.0 + ix * tangent, c.1 + iz * tangent);
        let leave = (c.0 + ox * tangent, c.1 + oz * tangent);
        let a0 = (enter.1 - centre.1).atan2(enter.0 - centre.0);
        let a1 = (leave.1 - centre.1).atan2(leave.0 - centre.0);
        let mut sweep = a1 - a0;
        if sweep > std::f32::consts::PI {
            sweep -= std::f32::consts::TAU;
        } else if sweep < -std::f32::consts::PI {
            sweep += std::f32::consts::TAU;
        }
        for step in 0..=FILLET_STEPS {
            let a = a0 + sweep * step as f32 / FILLET_STEPS as f32;
            out.push((centre.0 + radius * a.cos(), centre.1 + radius * a.sin()));
        }
        leg_start = leave;
    }
    out.push(points[points.len() - 1]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_keeps_the_ends_and_softens_a_hairpin() {
        let path = round(&[(0.0, 0.0), (40.0, 0.0), (0.0, 10.0)]);
        assert_eq!(path[0], (0.0, 0.0));
        assert_eq!(*path.last().unwrap(), (0.0, 10.0));
        // No point sits on the sharp corner any more.
        assert!(path.iter().all(|&(x, z)| (x - 40.0).hypot(z) > 1.0));
        for w in path.windows(3) {
            let a = (w[1].0 - w[0].0, w[1].1 - w[0].1);
            let b = (w[2].0 - w[1].0, w[2].1 - w[1].1);
            let turn = (a.0 * b.0 + a.1 * b.1) / (a.0.hypot(a.1) * b.0.hypot(b.1)).max(1e-6);
            assert!(turn > 0.5, "turn of {turn} between {w:?}");
        }
    }
}
