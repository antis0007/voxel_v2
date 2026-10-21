//! Sparse 2-D shallow-water ripples on the free surface of the voxel water.
//!
//! The volume field in `simulation` stays the only authority for mass and flow.
//! This layer is a visual-physics companion: one height offset (metres above
//! the rest surface) and one vertical velocity per open surface column, advanced
//! by the discrete wave equation
//!
//! ```text
//! v += c² · (Σ neighbour h − 4 h) · dt,   v *= e^(−γ dt),   h += v · dt
//! ```
//!
//! Only columns that hold energy, plus their four neighbours, are visited, so
//! cost follows the disturbed area rather than the size of the lake. Closed
//! neighbours (walls, shore, air above the surface) are reflective: their
//! height is replaced by the centre's, so a wave bounces instead of leaking.
//! Columns that stop being open surface lose their energy, which makes shores
//! absorb ripples without any special casing in the caller.
//!
//! Integration is in fixed sub-steps no longer than `MAX_SUBSTEP`, so the
//! result is independent of frame rate and the CFL bound `c · dt ≤ 1/√2` always
//! holds for the speeds used here.

use crate::fast_hash::{FastMap, FastSet};
use glam::IVec2;

/// Hard bound on simultaneously energetic columns. Overflow drops new
/// disturbances instead of growing without limit during a storm of splashes.
pub const MAX_COLUMNS: usize = 16_384;
/// Wave speed in columns per second.
pub const WAVE_SPEED: f32 = 8.0;
/// Exponential amplitude decay per second. Keeps a splash from ringing forever.
pub const DAMPING: f32 = 1.6;
/// Largest integration step. `WAVE_SPEED * MAX_SUBSTEP` stays well under 1/√2.
pub const MAX_SUBSTEP: f32 = 1.0 / 120.0;
/// Upper bound on sub-steps per `advance` call, so a long hitch cannot stall
/// a frame. Time beyond this is dropped, which only slows the ripples down.
const MAX_SUBSTEPS_PER_CALL: u32 = 8;
const SLEEP_HEIGHT: f32 = 1e-4;
const SLEEP_SPEED: f32 = 1e-4;
/// Clamp keeps a repeated impact from pushing the explicit scheme unstable.
const MAX_HEIGHT: f32 = 0.5;

const NEIGHBOURS: [IVec2; 4] = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ripple {
    /// Metres above the rest surface.
    pub height: f32,
    /// Metres per second.
    pub velocity: f32,
}

impl Ripple {
    fn is_quiet(self) -> bool {
        self.height.abs() < SLEEP_HEIGHT && self.velocity.abs() < SLEEP_SPEED
    }
}

#[derive(Default)]
pub struct RippleField {
    cells: FastMap<IVec2, Ripple>,
    // Scratch buffers reused across sub-steps: no allocation in steady state.
    next: FastMap<IVec2, Ripple>,
    seen: FastSet<IVec2>,
    leftover: f32,
}

impl RippleField {
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn height(&self, column: IVec2) -> f32 {
        self.cells.get(&column).map_or(0.0, |r| r.height)
    }

    pub fn get(&self, column: IVec2) -> Ripple {
        self.cells.get(&column).copied().unwrap_or_default()
    }

    /// Push the surface at `column` by `amount` metres, with a softer share
    /// spread to the four neighbours. This is how an impact becomes a wave.
    /// Returns `false` when the field is full and the impact was dropped.
    pub fn disturb(&mut self, column: IVec2, amount: f32) -> bool {
        if !amount.is_finite() || amount == 0.0 {
            return true;
        }
        let mut accepted = self.add_height(column, amount);
        for d in NEIGHBOURS {
            accepted &= self.add_height(column + d, amount * 0.5);
        }
        accepted
    }

    fn add_height(&mut self, column: IVec2, amount: f32) -> bool {
        if !self.cells.contains_key(&column) && self.cells.len() >= MAX_COLUMNS {
            return false;
        }
        let cell = self.cells.entry(column).or_default();
        cell.height = (cell.height + amount).clamp(-MAX_HEIGHT, MAX_HEIGHT);
        true
    }

    /// Advance by `dt` seconds. `is_open` reports whether a column currently
    /// has exposed water at its surface; anything else is a reflective wall.
    pub fn advance(&mut self, dt: f32, is_open: impl Fn(IVec2) -> bool) {
        if self.cells.is_empty() || !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.leftover += dt;
        let mut substeps = 0;
        while self.leftover >= MAX_SUBSTEP && substeps < MAX_SUBSTEPS_PER_CALL {
            self.leftover -= MAX_SUBSTEP;
            self.substep(MAX_SUBSTEP, DAMPING, &is_open);
            substeps += 1;
        }
        if substeps == MAX_SUBSTEPS_PER_CALL {
            self.leftover = 0.0;
        }
    }

    fn substep(&mut self, dt: f32, damping: f32, is_open: &impl Fn(IVec2) -> bool) {
        let c2 = WAVE_SPEED * WAVE_SPEED;
        let decay = (-damping * dt).exp();
        self.next.clear();
        self.seen.clear();
        for &centre in self.cells.keys() {
            for column in std::iter::once(centre).chain(NEIGHBOURS.map(|d| centre + d)) {
                if !self.seen.insert(column) {
                    continue;
                }
                if !is_open(column) {
                    continue;
                }
                let old = self.get(column);
                let h = old.height;
                let mut lap = 0.0;
                for d in NEIGHBOURS {
                    let q = column + d;
                    // A closed neighbour mirrors the centre: a reflective wall.
                    let hq = if is_open(q) { self.height(q) } else { h };
                    lap += hq - h;
                }
                let velocity = (old.velocity + c2 * lap * dt) * decay;
                let height = (h + velocity * dt).clamp(-MAX_HEIGHT, MAX_HEIGHT);
                let updated = Ripple { height, velocity };
                if !updated.is_quiet() {
                    self.next.insert(column, updated);
                }
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
    }

    /// Dense `size × size` tile of heights with `origin` as its minimum corner,
    /// row-major by Z. This is the upload format for a GPU height texture.
    pub fn fill_tile(&self, origin: IVec2, size: usize, out: &mut [f32]) {
        assert!(out.len() >= size * size, "tile buffer too small");
        out[..size * size].fill(0.0);
        for (&column, r) in &self.cells {
            let local = column - origin;
            if local.x >= 0
                && local.y >= 0
                && (local.x as usize) < size
                && (local.y as usize) < size
            {
                out[local.y as usize * size + local.x as usize] = r.height;
            }
        }
    }

    /// Total of `v² + c²·|∇h|²` over open columns: a proxy for wave energy.
    /// Edges to closed columns carry no term, matching the reflective walls
    /// the integrator uses, so a closed basin's energy is well defined.
    pub fn energy(&self, is_open: impl Fn(IVec2) -> bool) -> f64 {
        let c2 = (WAVE_SPEED * WAVE_SPEED) as f64;
        let mut e = 0.0;
        for (&p, r) in &self.cells {
            if !is_open(p) {
                continue;
            }
            e += (r.velocity as f64).powi(2);
            // Each edge is counted once. Edges between two live cells are
            // counted from the +X/+Y side; edges to an empty column (height
            // zero) are counted from the live side whichever way they face.
            for d in [IVec2::X, IVec2::Y, IVec2::NEG_X, IVec2::NEG_Y] {
                let q = p + d;
                let forward = d.x + d.y > 0;
                if !is_open(q) || (!forward && self.cells.contains_key(&q)) {
                    continue;
                }
                let hq = self.height(q) as f64;
                e += c2 * (hq - r.height as f64).powi(2);
            }
        }
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_plane(_: IVec2) -> bool {
        true
    }

    #[test]
    fn quiet_field_stays_empty() {
        let mut f = RippleField::default();
        f.advance(1.0, open_plane);
        assert!(f.is_empty());
    }

    #[test]
    fn impulse_propagates_outward_at_wave_speed() {
        let mut f = RippleField::default();
        f.disturb(IVec2::ZERO, 0.1);
        // Two seconds of propagation at WAVE_SPEED columns/s reaches roughly
        // 16 columns out; a column 3 away must already be moving.
        for _ in 0..(2.0 / MAX_SUBSTEP) as usize {
            f.advance(MAX_SUBSTEP, open_plane);
        }
        let near = f.height(IVec2::new(1, 0)).abs();
        let far = f.height(IVec2::new(3, 0)).abs();
        assert!(near > 0.0 && far > 0.0, "wave never left the source");
        assert!(
            f.height(IVec2::new(40, 0)).abs() < 1e-6,
            "wave outran its speed"
        );
    }

    #[test]
    fn closed_basin_conserves_energy_without_damping() {
        // A 12×12 basin: outside is closed, so waves reflect. Run the scheme
        // with zero damping so any drift is discretisation error alone.
        let basin = |p: IVec2| (0..12).contains(&p.x) && (0..12).contains(&p.y);
        let mut f = RippleField::default();
        f.disturb(IVec2::new(4, 5), 0.08);
        let e0 = f.energy(basin);
        assert!(e0 > 0.0);
        let mut max_drift: f64 = 0.0;
        for _ in 0..400 {
            f.substep(MAX_SUBSTEP, 0.0, &basin);
            max_drift = max_drift.max(((f.energy(basin) - e0) / e0).abs());
        }
        assert!(max_drift < 0.05, "energy drift {max_drift:.4} exceeds 5%");
    }

    #[test]
    fn energy_decays_and_the_field_sleeps() {
        let mut f = RippleField::default();
        f.disturb(IVec2::ZERO, 0.2);
        for _ in 0..(20.0 / MAX_SUBSTEP) as usize {
            f.advance(MAX_SUBSTEP, open_plane);
        }
        assert!(f.is_empty(), "{} columns still awake after 20 s", f.len());
    }

    #[test]
    fn columns_that_stop_being_open_drop_their_energy() {
        // A shore that moves: the column is water on the first step and dry
        // afterwards, so its wave must vanish instead of reflecting or
        // lingering as a phantom surface.
        let mut f = RippleField::default();
        f.disturb(IVec2::new(5, 5), 0.1);
        f.advance(MAX_SUBSTEP, open_plane);
        assert!(f.height(IVec2::new(5, 5)) != 0.0);
        f.advance(MAX_SUBSTEP, |p: IVec2| p != IVec2::new(5, 5));
        assert_eq!(f.get(IVec2::new(5, 5)), Ripple::default());
    }

    #[test]
    fn large_impacts_stay_finite_and_bounded() {
        let mut f = RippleField::default();
        for i in 0..50 {
            f.disturb(IVec2::new(i % 5, i / 5), 10.0);
        }
        for _ in 0..(3.0 / MAX_SUBSTEP) as usize {
            f.advance(MAX_SUBSTEP, open_plane);
        }
        for r in f.cells.values() {
            assert!(r.height.is_finite() && r.velocity.is_finite());
            assert!(r.height.abs() <= MAX_HEIGHT);
        }
    }

    #[test]
    fn capacity_is_bounded() {
        let mut f = RippleField::default();
        let mut dropped = 0;
        for i in 0..(MAX_COLUMNS as i32 + 200) {
            if !f.add_height(IVec2::new(i, 0), 0.01) {
                dropped += 1;
            }
        }
        assert_eq!(f.len(), MAX_COLUMNS);
        assert_eq!(dropped, 200);
    }

    #[test]
    fn tile_export_matches_sparse_heights() {
        let mut f = RippleField::default();
        f.disturb(IVec2::new(2, 3), 0.05);
        let mut tile = vec![9.0; 8 * 8];
        f.fill_tile(IVec2::new(0, 0), 8, &mut tile);
        assert_eq!(tile[3 * 8 + 2], f.height(IVec2::new(2, 3)));
        assert_eq!(tile[0], 0.0);
        let outside = f.height(IVec2::new(9, 9));
        assert_eq!(outside, 0.0);
    }

    #[test]
    fn advance_is_deterministic() {
        let run = || {
            let mut f = RippleField::default();
            f.disturb(IVec2::new(1, 1), 0.07);
            for _ in 0..500 {
                f.advance(1.0 / 60.0, open_plane);
            }
            let mut v: Vec<_> = f
                .cells
                .iter()
                .map(|(p, r)| (*p, r.height.to_bits()))
                .collect();
            v.sort_by_key(|(p, _)| (p.x, p.y));
            v
        };
        assert_eq!(run(), run());
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    /// `cargo test ripples_step_cost -- --ignored --nocapture`
    #[test]
    #[ignore = "benchmark; run explicitly"]
    fn ripples_step_cost() {
        let mut f = RippleField::default();
        // A 40×40 lake: one splash in the middle, then a settled-ish spread.
        for x in 0..40 {
            for y in 0..40 {
                if (x + y) % 9 == 0 {
                    f.disturb(IVec2::new(x, y), 0.02);
                }
            }
        }
        let open = |p: IVec2| (0..40).contains(&p.x) && (0..40).contains(&p.y);
        let steps = 2000usize;
        let start = std::time::Instant::now();
        let mut column_steps = 0usize;
        for _ in 0..steps {
            column_steps += f.len();
            f.substep(MAX_SUBSTEP, DAMPING, &open);
        }
        let ns = start.elapsed().as_nanos() as f64;
        println!(
            "ripples: {} active columns avg, {:.1} ns per column-substep, {:.3} ms per 60 Hz frame at this load",
            column_steps / steps,
            ns / column_steps.max(1) as f64,
            ns / steps as f64 * (1.0 / 60.0 / MAX_SUBSTEP as f64) / 1e6
        );
    }
}
