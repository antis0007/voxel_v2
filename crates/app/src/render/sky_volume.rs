//! Local sky visibility: probes in windows around the camera, traced on the
//! GPU through solid and leaf occupancy. Ambient light only. The fine level
//! has a probe per voxel; the coarse level a probe per `COARSE_SCALE`³ voxels
//! over a window that many times wider. Coordinates below are level cells.
//!
//! Probes live in a toroidal 3D texture, so a window move clears and rebakes
//! only the entering bricks. Chunks are restamped when their published mesh
//! revision changes, and probe bricks within `REACH` of them are rebaked.
//! A chunk that is not resident reads as open air until it arrives.
use crate::world::BlockId;
use glam::{IVec3, Vec3};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

#[path = "sky_volume_cells.rs"]
mod cells;
pub use cells::{Cells, Coarse};

/// Probe window edge in cells.
pub const PROBE_DIM: i32 = 64;
/// Bake unit edge in cells; the window moves in whole bricks.
pub const BRICK: i32 = 8;
/// Voxels per coarse cell.
pub const COARSE_SCALE: i32 = 4;
/// Ray length in cells. The occupancy window extends this far past the probes.
pub const REACH: i32 = 16;
pub const BITS_DIM: i32 = PROBE_DIM + 2 * REACH;
const WORDS: usize = (BITS_DIM as usize).pow(3) / 32;
const HEADER_WORDS: usize = 4;
/// Leaf path in metres that halves sky light, as in `mesh/canopy.rs`.
const LEAF_HALF_PATH_M: f32 = 3.0;

static ENABLED: AtomicBool = AtomicBool::new(true);

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(value: bool) {
    ENABLED.store(value, Ordering::Relaxed);
}

/// Review switch: `VOXEL_SKY_VOLUME=0` keeps the cached column sky access.
pub fn env_allows() -> bool {
    static ALLOWS: OnceLock<bool> = OnceLock::new();
    *ALLOWS.get_or_init(|| std::env::var("VOXEL_SKY_VOLUME").map_or(true, |v| v != "0"))
}

/// Review switch: `VOXEL_SKY_VOLUME_COARSE=0` keeps only the fine level.
pub fn coarse_allowed() -> bool {
    static ALLOWS: OnceLock<bool> = OnceLock::new();
    *ALLOWS.get_or_init(|| std::env::var("VOXEL_SKY_VOLUME_COARSE").map_or(true, |v| v != "0"))
}

/// Review switch: `VOXEL_REVIEW_NO_SSAO=1` removes screen-space AO.
pub fn ssao_scale() -> f32 {
    static SCALE: OnceLock<f32> = OnceLock::new();
    *SCALE.get_or_init(|| {
        if std::env::var("VOXEL_REVIEW_NO_SSAO").is_ok_and(|v| v == "1") { 0.0 } else { 1.0 }
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Occupancy {
    Air,
    Leaf,
    Solid,
}

fn class_lut() -> &'static [Occupancy; BlockId::COUNT] {
    static LUT: OnceLock<[Occupancy; BlockId::COUNT]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut table = [Occupancy::Air; BlockId::COUNT];
        for &id in engine_content::ALL_BLOCK_IDS {
            table[id as usize] = if matches!(id, BlockId::Leaves | BlockId::PineLeaves | BlockId::Bush) {
                Occupancy::Leaf
            } else if id.is_opaque() {
                Occupancy::Solid
            } else {
                Occupancy::Air
            };
        }
        table
    })
}

/// Probe-brick base corner for a window around `eye` (cells).
pub fn probe_origin(eye: Vec3) -> IVec3 {
    let cell = eye.floor().as_ivec3();
    let brick = IVec3::new(
        engine_core::floor_div(cell.x, BRICK),
        engine_core::floor_div(cell.y, BRICK),
        engine_core::floor_div(cell.z, BRICK),
    );
    brick * BRICK - IVec3::splat(PROBE_DIM / 2)
}

/// Solid and leaf bits for the occupancy window, header first, as the bake reads
/// them. Storage is toroidal (cell mod `BITS_DIM`), so a move keeps the bits of
/// every cell still inside and only the entering slabs are stamped.
pub struct Window {
    pub origin: IVec3,
    pub words: Vec<u32>,
    /// Chunk edge in cells.
    chunk_cells: i32,
}

impl Window {
    pub fn new(origin: IVec3, chunk_cells: i32) -> Self {
        let mut w = Self { origin, words: vec![0u32; HEADER_WORDS + 2 * WORDS], chunk_cells };
        w.move_to(origin);
        w
    }

    pub fn move_to(&mut self, origin: IVec3) {
        self.origin = origin;
        self.words[..4].copy_from_slice(&[origin.x as u32, origin.y as u32, origin.z as u32, BITS_DIM as u32]);
    }

    #[cfg(test)]
    fn index(&self, cell: IVec3) -> Option<usize> {
        let d = cell - self.origin;
        let w = cell.rem_euclid(IVec3::splat(BITS_DIM));
        (d.min_element() >= 0 && d.max_element() < BITS_DIM)
            .then(|| (w.x + BITS_DIM * (w.z + BITS_DIM * w.y)) as usize)
    }

    #[cfg(test)]
    pub fn get(&self, cell: IVec3) -> Occupancy {
        let Some(i) = self.index(cell) else { return Occupancy::Air };
        let bit = 1u32 << (i & 31);
        if self.words[HEADER_WORDS + (i >> 5)] & bit != 0 {
            Occupancy::Solid
        } else if self.words[HEADER_WORDS + WORDS + (i >> 5)] & bit != 0 {
            Occupancy::Leaf
        } else {
            Occupancy::Air
        }
    }

    /// Overwrites every window cell of chunk `coord`; `None` writes air.
    pub fn stamp<C: Cells>(&mut self, coord: IVec3, chunk: Option<&C>) {
        let (lo, hi) = (self.origin, self.origin + IVec3::splat(BITS_DIM));
        self.stamp_box(coord, chunk, lo, hi);
    }

    /// `stamp` limited to the cells of `[lo, hi)`.
    pub fn stamp_box<C: Cells>(&mut self, coord: IVec3, chunk: Option<&C>, lo: IVec3, hi: IVec3) {
        let base = coord * self.chunk_cells;
        let lo = lo.max(base).max(self.origin);
        let hi = hi.min(base + IVec3::splat(self.chunk_cells)).min(self.origin + IVec3::splat(BITS_DIM));
        if lo.cmpge(hi).any() {
            return;
        }
        for y in lo.y..hi.y {
            for z in lo.z..hi.z {
                let wrapped = (y.rem_euclid(BITS_DIM) * BITS_DIM + z.rem_euclid(BITS_DIM)) * BITS_DIM;
                for x in lo.x..hi.x {
                    let class = chunk.map_or(Occupancy::Air, |c| c.class_at(IVec3::new(x, y, z) - base));
                    let i = (wrapped + x.rem_euclid(BITS_DIM)) as usize;
                    let (word, bit) = (HEADER_WORDS + (i >> 5), 1u32 << (i & 31));
                    self.words[word] &= !bit;
                    self.words[word + WORDS] &= !bit;
                    match class {
                        Occupancy::Solid => self.words[word] |= bit,
                        Occupancy::Leaf => self.words[word + WORDS] |= bit,
                        Occupancy::Air => {}
                    }
                }
            }
        }
    }
}

/// Cells of the window at `new` that the window at `old` did not hold, as up to
/// three overlapping boxes.
pub fn entering_boxes(old: IVec3, new: IVec3) -> Vec<(IVec3, IVec3)> {
    let edge = IVec3::splat(BITS_DIM);
    let mut boxes = Vec::new();
    for axis in 0..3 {
        let d = new[axis] - old[axis];
        if d == 0 {
            continue;
        }
        let (mut lo, mut hi) = (new, new + edge);
        if d > 0 {
            lo[axis] = (old[axis] + BITS_DIM).max(new[axis]);
        } else {
            hi[axis] = old[axis].min(new[axis] + BITS_DIM);
        }
        boxes.push((lo, hi));
    }
    boxes
}
/// Chunk coordinates overlapping the occupancy window at `origin`.
pub fn window_chunks(origin: IVec3, chunk_cells: i32) -> impl Iterator<Item = IVec3> {
    let lo = origin.map(|v| engine_core::floor_div(v, chunk_cells));
    let hi = (origin + IVec3::splat(BITS_DIM - 1)).map(|v| engine_core::floor_div(v, chunk_cells));
    (lo.y..=hi.y).flat_map(move |y| {
        (lo.z..=hi.z).flat_map(move |z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z)))
    })
}

/// Probe bricks (brick coordinates) of the window at `probe_origin`.
pub fn window_bricks(probe_origin: IVec3) -> impl Iterator<Item = IVec3> {
    let lo = probe_origin / BRICK;
    let n = PROBE_DIM / BRICK;
    (0..n).flat_map(move |y| (0..n).flat_map(move |z| (0..n).map(move |x| lo + IVec3::new(x, y, z))))
}

/// Probe bricks whose rays can reach any cell of `[lo, hi)`.
pub fn bricks_touching(lo: IVec3, hi: IVec3) -> impl Iterator<Item = IVec3> {
    let a = (lo - IVec3::splat(REACH)).map(|v| engine_core::floor_div(v, BRICK));
    let b = (hi + IVec3::splat(REACH - 1)).map(|v| engine_core::floor_div(v, BRICK));
    (a.y..=b.y).flat_map(move |y| (a.z..=b.z).flat_map(move |z| (a.x..=b.x).map(move |x| IVec3::new(x, y, z))))
}

/// Per-cell leaf transmittance at this cell size.
pub fn leaf_transmittance(cell_m: f32) -> f32 {
    (-std::f32::consts::LN_2 * cell_m.max(0.01) / LEAF_HALF_PATH_M).exp()
}

/// CPU bookkeeping: which chunks are stamped and which bricks need a bake.
pub struct Schedule {
    pub window: Window,
    pub probe_origin: IVec3,
    stamped: HashMap<IVec3, Option<u64>>,
    pub pending: HashSet<IVec3>,
    pub cleared: Vec<IVec3>,
}

impl Schedule {
    /// `chunk_cells` is the chunk edge in this level's cells.
    pub fn new(probe_origin: IVec3, chunk_cells: i32) -> Self {
        Self {
            window: Window::new(probe_origin - IVec3::splat(REACH), chunk_cells),
            probe_origin,
            stamped: HashMap::new(),
            pending: window_bricks(probe_origin).collect(),
            cleared: window_bricks(probe_origin).collect(),
        }
    }

    /// Moves the window and restamps changed chunks. `source` returns a chunk's
    /// data and the revision that identifies it. Returns true when the occupancy
    /// words changed.
    pub fn refresh<'a, C: Cells + 'a>(
        &mut self,
        probe_origin: IVec3,
        source: impl Fn(IVec3) -> Option<(&'a C, u64)>,
    ) -> bool {
        let mut changed = false;
        if probe_origin != self.probe_origin {
            let (old_lo, old_hi) = (self.window.origin, self.window.origin + IVec3::splat(BITS_DIM));
            let old_bricks = (self.probe_origin / BRICK, self.probe_origin / BRICK + IVec3::splat(PROBE_DIM / BRICK));
            let new_lo = probe_origin - IVec3::splat(REACH);
            let jumped = (new_lo - old_lo).abs().max_element() >= BITS_DIM;
            let entering = entering_boxes(old_lo, new_lo);
            self.window.move_to(new_lo);
            self.probe_origin = probe_origin;
            let (lo, hi) = self.brick_bounds();
            self.pending.retain(|b| b.cmpge(lo).all() && b.cmplt(hi).all());
            for brick in window_bricks(probe_origin) {
                let ray_lo = brick * BRICK - IVec3::splat(REACH);
                let ray_hi = brick * BRICK + IVec3::splat(BRICK + REACH);
                if brick.cmplt(old_bricks.0).any() || brick.cmpge(old_bricks.1).any() {
                    self.cleared.push(brick);
                    self.pending.insert(brick);
                } else if ray_lo.cmplt(old_lo).any() || ray_hi.cmpgt(old_hi).any() {
                    // Rays of this brick ran past the old occupancy window.
                    self.pending.insert(brick);
                }
            }
            // Unchanged chunks stamp only their entering cells. A changed chunk is
            // restamped whole and rebaked around; a jump restamps everything.
            let previous = std::mem::take(&mut self.stamped);
            for coord in window_chunks(self.window.origin, self.window.chunk_cells) {
                let entry = source(coord);
                let key = entry.map(|e| e.1);
                match previous.get(&coord) {
                    Some(old) if !jumped && *old == key => {
                        for &(lo, hi) in &entering {
                            self.window.stamp_box(coord, entry.map(|e| e.0), lo, hi);
                        }
                    }
                    old => {
                        self.window.stamp(coord, entry.map(|e| e.0));
                        if !jumped && old.is_some_and(|old| *old != key) {
                            self.mark_chunk(coord);
                        }
                    }
                }
                self.stamped.insert(coord, key);
            }
            return true;
        }
        for coord in window_chunks(self.window.origin, self.window.chunk_cells) {
            let entry = source(coord);
            let key = entry.map(|e| e.1);
            if self.stamped.get(&coord) == Some(&key) {
                continue;
            }
            let first = !self.stamped.contains_key(&coord);
            self.window.stamp(coord, entry.map(|e| e.0));
            self.stamped.insert(coord, key);
            changed = true;
            // A blank window already reads absent chunks as air.
            if !(first && key.is_none()) {
                self.mark_chunk(coord);
            }
        }
        changed
    }

    fn brick_bounds(&self) -> (IVec3, IVec3) {
        let lo = self.probe_origin / BRICK;
        (lo, lo + IVec3::splat(PROBE_DIM / BRICK))
    }

    fn mark_chunk(&mut self, coord: IVec3) {
        let (lo, hi) = self.brick_bounds();
        let edge = self.window.chunk_cells;
        let base = coord * edge;
        for brick in bricks_touching(base, base + IVec3::splat(edge)) {
            if brick.cmpge(lo).all() && brick.cmplt(hi).all() {
                self.pending.insert(brick);
            }
        }
    }

    /// Nearest pending bricks to `eye` (cells), at most `budget`, removed from the queue.
    pub fn take_batch(&mut self, eye: Vec3, budget: usize) -> Vec<IVec3> {
        if budget == 0 || self.pending.is_empty() {
            return Vec::new();
        }
        let centre = |b: &IVec3| ((*b * BRICK).as_vec3() + Vec3::splat(BRICK as f32 * 0.5) - eye).length_squared();
        let mut ranked: Vec<_> = self.pending.iter().map(|&brick| (centre(&brick), brick)).collect();
        let by_distance = |a: &(f32, IVec3), b: &(f32, IVec3)| {
            a.0.total_cmp(&b.0).then(a.1.to_array().cmp(&b.1.to_array()))
        };
        if ranked.len() > budget {
            ranked.select_nth_unstable_by(budget, by_distance);
            ranked.truncate(budget);
        }
        ranked.sort_unstable_by(by_distance);
        ranked.into_iter().map(|(_, brick)| {
            self.pending.remove(&brick);
            brick
        }).collect()
    }
}

#[cfg(test)]
#[path = "sky_volume_tests.rs"]
mod tests;
