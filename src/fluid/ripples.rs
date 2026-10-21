//! Sparse 2-D shallow-water ripples on the free surface of the voxel water.
//!
//! The volume field in `simulation` stays the only authority for mass and flow.
//! This layer is a visual-physics companion: one height offset and one vertical
//! velocity per open surface column, in voxel units (one column is one voxel),
//! advanced by the discrete wave equation
//!
//! ```text
//! v += c² · (Σ neighbour h − 4 h) · dt,   v *= e^(−γ dt),   h += v · dt
//! ```
//!
//! Every column also remembers the Y of its surface cell. The caller answers
//! "is there exposed water at this column, and at which Y?" with a local lookup,
//! so the solver never scans. Only energetic columns and their neighbours are
//! visited, so cost follows the disturbed area rather than the size of the lake.
//! A closed neighbour (wall, shore, roof, or a surface at a different Y) is
//! reflective: its height is replaced by the centre's, so waves bounce instead
//! of leaking. Columns that stop being open surface lose their energy.
//!
//! Integration is in fixed sub-steps no longer than `MAX_SUBSTEP`, so the result
//! does not depend on frame rate and the CFL bound `c · dt ≤ 1/√2` holds.

use crate::fast_hash::{FastMap, FastSet};
use glam::IVec2;

/// Hard bound on simultaneously energetic columns. Overflow drops new
/// disturbances instead of growing without limit during a storm of splashes.
pub const MAX_COLUMNS: usize = 16_384;
/// Wave speed in columns (voxels) per second.
pub const WAVE_SPEED: f32 = 8.0;
/// Exponential amplitude decay per second. Keeps a splash from ringing forever.
pub const DAMPING: f32 = 1.6;
/// Largest integration step. `WAVE_SPEED * MAX_SUBSTEP` stays well under 1/√2.
pub const MAX_SUBSTEP: f32 = 1.0 / 120.0;
/// Upper bound on sub-steps per `advance` call, so a long hitch cannot stall a
/// frame. Time beyond this is dropped, which only slows the ripples down.
const MAX_SUBSTEPS_PER_CALL: u32 = 8;
const SLEEP_HEIGHT: f32 = 1e-4;
const SLEEP_SPEED: f32 = 1e-4;
/// Height of a full-strength impact at its centre column.
pub const IMPACT_VOXELS: f32 = 0.3;
/// Clamp keeps a repeated impact from pushing the explicit scheme unstable.
pub const MAX_HEIGHT: f32 = 0.5;

const NEIGHBOURS: [IVec2; 4] = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ripple {
    /// Voxels above the rest surface.
    pub height: f32,
    /// Voxels per second.
    pub velocity: f32,
    /// Y of the surface voxel this column's wave lives on.
    pub top: i32,
}

impl Ripple {
    fn is_quiet(self) -> bool {
        self.height.abs() < SLEEP_HEIGHT && self.velocity.abs() < SLEEP_SPEED
    }
}

/// Per-advance cache of "does this column have an open surface, and at which Y".
type OpenMap = FastMap<IVec2, Option<i32>>;

#[derive(Default)]
pub struct RippleField {
    cells: FastMap<IVec2, Ripple>,
    // Scratch buffers reused across sub-steps: no allocation in steady state.
    next: FastMap<IVec2, Ripple>,
    seen: FastSet<IVec2>,
    open: OpenMap,
    leftover: f32,
}

// Read-side API: tests and the GPU height upload (`fill_tile`) use it; the
// upload lands with the render slice.
#[allow(dead_code)]
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

    pub fn clear(&mut self) {
        self.cells.clear();
        self.leftover = 0.0;
    }

    /// Impact at `column` (whose surface voxel is at `top`): a crest of
    /// `amount` voxels at the centre and a trough of a quarter of that on each
    /// neighbour. The impulse has zero net displacement, so a splash moves
    /// water rather than adding it, and the pool returns to its level.
    /// Returns `false` when the field is full and the centre was dropped.
    pub fn disturb(&mut self, column: IVec2, top: i32, amount: f32) -> bool {
        if !amount.is_finite() || amount == 0.0 {
            return true;
        }
        let accepted = self.add_height(column, top, amount);
        for d in NEIGHBOURS {
            self.add_height(column + d, top, -amount * 0.25);
        }
        accepted
    }

    fn add_height(&mut self, column: IVec2, top: i32, amount: f32) -> bool {
        if !self.cells.contains_key(&column) && self.cells.len() >= MAX_COLUMNS {
            return false;
        }
        let cell = self.cells.entry(column).or_insert(Ripple {
            top,
            ..Ripple::default()
        });
        cell.height = (cell.height + amount).clamp(-MAX_HEIGHT, MAX_HEIGHT);
        true
    }

    /// Advance by `dt` seconds. `surface(column, hint)` returns the Y of the
    /// exposed water surface at `column` (searching near `hint`), or `None` if
    /// the column is not open surface.
    pub fn advance(&mut self, dt: f32, surface: impl Fn(IVec2, i32) -> Option<i32>) {
        if self.cells.is_empty() || !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.leftover += dt;
        self.open.clear();
        let mut substeps = 0;
        while self.leftover >= MAX_SUBSTEP && substeps < MAX_SUBSTEPS_PER_CALL {
            self.leftover -= MAX_SUBSTEP;
            self.substep(MAX_SUBSTEP, DAMPING, &surface);
            substeps += 1;
        }
        if substeps == MAX_SUBSTEPS_PER_CALL {
            self.leftover = 0.0;
        }
        self.open.clear();
    }

    fn substep(&mut self, dt: f32, damping: f32, surface: &impl Fn(IVec2, i32) -> Option<i32>) {
        let c2 = WAVE_SPEED * WAVE_SPEED;
        let decay = (-damping * dt).exp();
        let Self {
            cells,
            next,
            seen,
            open,
            ..
        } = self;
        next.clear();
        seen.clear();
        for (&centre, centre_cell) in cells.iter() {
            for column in std::iter::once(centre).chain(NEIGHBOURS.map(|d| centre + d)) {
                if !seen.insert(column) {
                    continue;
                }
                let Some(top) = classify(open, cells, column, centre_cell.top, surface) else {
                    continue;
                };
                let old = cells.get(&column).copied().unwrap_or(Ripple {
                    top,
                    ..Ripple::default()
                });
                let h = old.height;
                let mut lap = 0.0;
                for d in NEIGHBOURS {
                    let q = column + d;
                    // A closed neighbour, or one whose surface sits at another
                    // height, mirrors the centre: a reflective wall.
                    let hq = match classify(open, cells, q, top, surface) {
                        Some(qtop) if qtop == top => cells.get(&q).map_or(0.0, |r| r.height),
                        _ => h,
                    };
                    lap += hq - h;
                }
                let velocity = (old.velocity + c2 * lap * dt) * decay;
                let height = (h + velocity * dt).clamp(-MAX_HEIGHT, MAX_HEIGHT);
                let updated = Ripple {
                    height,
                    velocity,
                    top,
                };
                if !updated.is_quiet() {
                    next.insert(column, updated);
                }
            }
        }
        std::mem::swap(cells, next);
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
    /// Each edge is counted once, and edges to closed columns carry no term,
    /// matching the reflective walls the integrator uses.
    pub fn energy(&self, surface: impl Fn(IVec2, i32) -> Option<i32>) -> f64 {
        let c2 = (WAVE_SPEED * WAVE_SPEED) as f64;
        let mut e = 0.0;
        for (&p, r) in &self.cells {
            if surface(p, r.top) != Some(r.top) {
                continue;
            }
            e += (r.velocity as f64).powi(2);
            // Edges between two live cells are counted from the +X/+Y side.
            // Edges to an empty column (height zero) are counted from the live
            // side whichever way they face.
            for d in [IVec2::X, IVec2::Y, IVec2::NEG_X, IVec2::NEG_Y] {
                let q = p + d;
                let forward = d.x + d.y > 0;
                if (!forward && self.cells.contains_key(&q)) || surface(q, r.top) != Some(r.top) {
                    continue;
                }
                let hq = self.height(q) as f64;
                e += c2 * (hq - r.height as f64).powi(2);
            }
        }
        e
    }
}

/// Cached open-surface test. A column's surface search starts from the hint
/// of a neighbouring live cell, so sloped water keeps its walls consistent.
fn classify(
    open: &mut OpenMap,
    cells: &FastMap<IVec2, Ripple>,
    column: IVec2,
    hint: i32,
    surface: &impl Fn(IVec2, i32) -> Option<i32>,
) -> Option<i32> {
    *open.entry(column).or_insert_with(|| {
        let start = cells.get(&column).map_or(hint, |r| r.top);
        surface(column, start)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_plane(_: IVec2, _: i32) -> Option<i32> {
        Some(0)
    }

    fn run(f: &mut RippleField, steps: usize, surface: impl Fn(IVec2, i32) -> Option<i32>) {
        for _ in 0..steps {
            f.advance(MAX_SUBSTEP, &surface);
        }
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
        f.disturb(IVec2::ZERO, 0, 0.1);
        // Two seconds at WAVE_SPEED reaches about 16 columns; column 3 must move
        // and column 40 must still be untouched.
        run(&mut f, (2.0 / MAX_SUBSTEP) as usize, open_plane);
        assert!(f.height(IVec2::new(1, 0)).abs() > 0.0);
        assert!(
            f.height(IVec2::new(3, 0)).abs() > 0.0,
            "wave never left the source"
        );
        assert!(
            f.height(IVec2::new(40, 0)).abs() < 1e-6,
            "wave outran its speed"
        );
    }

    #[test]
    fn closed_basin_conserves_energy_without_damping() {
        // A 12×12 basin: outside is closed, so waves reflect. Run the scheme
        // with zero damping so any drift is discretisation error alone.
        let basin =
            |p: IVec2, _: i32| ((0..12).contains(&p.x) && (0..12).contains(&p.y)).then_some(0);
        let mut f = RippleField::default();
        f.disturb(IVec2::new(4, 5), 0, 0.08);
        // The symplectic scheme's energy error oscillates at about ω·dt around
        // the true value, so a single sample is not the invariant. What must
        // hold is no secular drift: the mean over an early window and a late
        // window of the same run agree.
        let mut trace = Vec::with_capacity(800);
        for _ in 0..800 {
            f.substep(MAX_SUBSTEP, 0.0, &basin);
            trace.push(f.energy(basin));
        }
        let mean = |w: &[f64]| w.iter().sum::<f64>() / w.len() as f64;
        let early = mean(&trace[200..300]);
        let late = mean(&trace[700..800]);
        assert!(early > 0.0);
        let drift = ((late - early) / early).abs();
        assert!(drift < 0.02, "secular energy drift {drift:.4} exceeds 2%");
    }

    #[test]
    fn energy_decays_and_the_field_sleeps() {
        let mut f = RippleField::default();
        f.disturb(IVec2::ZERO, 0, 0.2);
        run(&mut f, (20.0 / MAX_SUBSTEP) as usize, open_plane);
        assert!(f.is_empty(), "{} columns still awake after 20 s", f.len());
    }

    #[test]
    fn columns_that_stop_being_open_drop_their_energy() {
        let mut f = RippleField::default();
        f.disturb(IVec2::new(5, 5), 0, 0.1);
        f.advance(MAX_SUBSTEP, open_plane);
        assert!(f.height(IVec2::new(5, 5)) != 0.0);
        f.advance(MAX_SUBSTEP, |p: IVec2, _| {
            (p != IVec2::new(5, 5)).then_some(0)
        });
        assert_eq!(f.get(IVec2::new(5, 5)), Ripple::default());
    }

    #[test]
    fn a_step_in_the_surface_reflects_instead_of_leaking() {
        // Left half of the lake sits at Y=0, right half at Y=1. A wave that
        // reaches the step must not pass through it as if the level matched.
        let stepped = |p: IVec2, _: i32| Some(if p.x < 8 { 0 } else { 1 });
        let mut f = RippleField::default();
        f.disturb(IVec2::new(4, 0), 0, 0.2);
        run(&mut f, (1.0 / MAX_SUBSTEP) as usize, stepped);
        let beyond = f.height(IVec2::new(9, 0)).abs();
        assert!(beyond < 1e-3, "wave leaked across a surface step: {beyond}");
    }

    #[test]
    fn large_impacts_stay_finite_and_bounded() {
        let mut f = RippleField::default();
        for i in 0..50 {
            f.disturb(IVec2::new(i % 5, i / 5), 0, 10.0);
        }
        run(&mut f, (3.0 / MAX_SUBSTEP) as usize, open_plane);
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
            if !f.add_height(IVec2::new(i, 0), 0, 0.01) {
                dropped += 1;
            }
        }
        assert_eq!(f.len(), MAX_COLUMNS);
        assert_eq!(dropped, 200);
    }

    #[test]
    fn tile_export_matches_sparse_heights() {
        let mut f = RippleField::default();
        f.disturb(IVec2::new(2, 3), 0, 0.05);
        let mut tile = vec![9.0; 8 * 8];
        f.fill_tile(IVec2::new(0, 0), 8, &mut tile);
        assert_eq!(tile[3 * 8 + 2], f.height(IVec2::new(2, 3)));
        assert_eq!(tile[0], 0.0);
        assert_eq!(f.height(IVec2::new(9, 9)), 0.0);
    }

    #[test]
    fn advance_is_deterministic() {
        let run_once = || {
            let mut f = RippleField::default();
            f.disturb(IVec2::new(1, 1), 0, 0.07);
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
        assert_eq!(run_once(), run_once());
    }

    #[test]
    fn impact_displaces_no_net_volume() {
        let mut f = RippleField::default();
        f.disturb(IVec2::new(3, 3), 0, 0.3);
        let total: f32 = f.cells.values().map(|r| r.height).sum();
        assert!(total.abs() < 1e-6, "impact added volume: {total}");
    }

    #[test]
    fn clear_empties_the_field() {
        let mut f = RippleField::default();
        f.disturb(IVec2::ZERO, 0, 0.1);
        f.clear();
        assert!(f.is_empty());
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
        // A 40×40 lake with a scattering of impacts.
        for x in 0..40 {
            for y in 0..40 {
                if (x + y) % 9 == 0 {
                    f.disturb(IVec2::new(x, y), 0, 0.02);
                }
            }
        }
        let surface =
            |p: IVec2, _: i32| ((0..40).contains(&p.x) && (0..40).contains(&p.y)).then_some(0);
        let steps = 2000usize;
        let start = std::time::Instant::now();
        let mut column_steps = 0usize;
        for _ in 0..steps {
            column_steps += f.len();
            f.substep(MAX_SUBSTEP, DAMPING, &surface);
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

#[cfg(test)]
mod world_contract {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::{BlockId as B, World, WorldType};
    use glam::{IVec3, Vec3};

    /// A stone basin with a 1-voxel-deep water pool and open air above it.
    fn pool() -> World {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let mut edits = Vec::new();
        for x in 4..17 {
            for z in 4..13 {
                edits.push((IVec3::new(x, 100, z), B::Stone));
                edits.push((IVec3::new(x, 102, z), B::Air));
                world.set_water_volume(IVec3::new(x, 101, z), 8);
            }
        }
        world.set_many(&edits);
        world
    }

    #[test]
    fn surface_top_finds_the_exposed_water_voxel() {
        let world = pool();
        assert_eq!(world.water_surface_top(IVec2::new(8, 8), 101), Some(101));
        // A hint one voxel off still finds it.
        assert_eq!(world.water_surface_top(IVec2::new(8, 8), 102), Some(101));
        // Dry air has no surface.
        assert_eq!(world.water_surface_top(IVec2::new(0, 0), 101), None);
    }

    #[test]
    fn roofed_water_is_not_a_surface() {
        let mut world = pool();
        world.set_many(&[(IVec3::new(6, 102, 6), B::Stone)]);
        assert_eq!(world.water_surface_top(IVec2::new(6, 6), 101), None);
        assert!(!world.disturb_water_surface(Vec3::new(6.5, 101.9, 6.5), 1.0));
    }

    #[test]
    fn impact_on_the_pool_spreads_and_dies_out() {
        let mut world = pool();
        assert!(world.disturb_water_surface(Vec3::new(8.5, 101.9, 8.5), 1.0));
        advance_frames(&mut world, 0.8);
        assert!(
            world.ripples.height(IVec2::new(12, 8)).abs() > 0.0,
            "wave did not travel"
        );
        assert!(
            world.ripples.height(IVec2::new(20, 8)).abs() < 1e-6,
            "wave outran its speed"
        );
        // Frame-rate stepping, as the game does, must let the pool go still.
        // The pool must come back to its level, not hold a raised plateau.
        advance_frames(&mut world, 30.0);
        assert!(world.ripples.is_empty());
    }

    /// Advance at 60 Hz. A single large `dt` is deliberately under-simulated
    /// by the sub-step cap, so tests must step like a frame loop.
    fn advance_frames(world: &mut World, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            world.advance_ripples(1.0 / 60.0);
        }
    }

    #[test]
    fn a_raised_ledge_in_the_world_stops_a_wave() {
        // Raising column x=10 by one voxel turns it into a wall for the
        // level-101 surface. The wave must reach x=9 and not cross to x=11.
        let mut world = pool();
        let ledge: Vec<_> = (4..13)
            .map(|z| (IVec3::new(10, 102, z), B::Water))
            .collect();
        world.set_many(&ledge);
        for z in 4..13 {
            world.set_water_volume(IVec3::new(10, 102, z), 1);
        }
        assert_eq!(world.water_surface_top(IVec2::new(10, 8), 101), Some(102));
        world.disturb_water_surface(Vec3::new(7.5, 101.9, 8.5), 1.0);
        advance_frames(&mut world, 1.0);
        assert!(
            world.ripples.height(IVec2::new(9, 8)).abs() > 0.0,
            "wave never reached the ledge"
        );
        assert!(
            world.ripples.height(IVec2::new(11, 8)).abs() < 1e-3,
            "wave crossed the ledge"
        );
    }
}

#[cfg(test)]
mod world_bench {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::{BlockId as B, World, WorldType};
    use glam::{IVec3, Vec3};

    /// `cargo test ripples_world_cost -- --ignored --nocapture`
    #[test]
    #[ignore = "benchmark; run explicitly"]
    fn ripples_world_cost() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let mut edits = Vec::new();
        for x in 4..40 {
            for z in 4..40 {
                edits.push((IVec3::new(x, 100, z), B::Stone));
                edits.push((IVec3::new(x, 102, z), B::Air));
                world.set_water_volume(IVec3::new(x, 101, z), 8);
            }
        }
        world.set_many(&edits);
        // Impacts across the whole lake, then time the per-frame advance.
        for x in (6..38).step_by(4) {
            for z in (6..38).step_by(4) {
                world.disturb_water_surface(Vec3::new(x as f32 + 0.5, 101.9, z as f32 + 0.5), 1.0);
            }
        }
        let frames = 240usize;
        let start = std::time::Instant::now();
        let mut columns = 0usize;
        for _ in 0..frames {
            columns += world.ripples.len();
            world.advance_ripples(1.0 / 60.0);
        }
        let ms = start.elapsed().as_secs_f64() * 1e3 / frames as f64;
        println!(
            "ripples world: {} active columns avg, {ms:.3} ms per 60 Hz frame",
            columns / frames
        );
    }
}
