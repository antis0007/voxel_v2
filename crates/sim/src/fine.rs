//! Fine sub-bricks: sixty-four 0.25 m cells inside one 1 m block.
//!
//! A block stays a single [`Cell`] until something makes it uneven: an acid
//! bite, a pick chip, water flowing in, a partial placement. It is then
//! promoted into this separate pool and indexed by its block coordinate. When
//! the sixty-four cells become uniform again it demotes and the pool slot
//! returns.
//!
//! Volume is conserved both ways. One block volume unit is 1/64 m³; one fine
//! unit is 1/4096 m³, so a block unit is exactly 64 fine units and promoting a
//! uniform block copies the same `volume` into every fine cell.
//!
//! The pool is intentionally not reachable from [`Cell`]: the eight-byte record
//! has no spare slot index. The block coordinate map here is the Phase 0 index;
//! a future block-record revision can move it into the header without changing
//! these rules.
use crate::cell::{Cell, VOLUME_FULL};
use crate::store::PoolFull;
use engine_core::fast_hash::FastMap;
use glam::IVec3;

/// Fine cells per axis inside one block.
pub const FINE_SIZE: i32 = 4;
/// Fine cells in one block.
pub const FINE_CELLS: usize = (FINE_SIZE * FINE_SIZE * FINE_SIZE) as usize;
/// A fine sub-brick is 64 × 8 B.
pub const SUB_BRICK_BYTES: usize = FINE_CELLS * size_of::<Cell>();
/// Full fine units in one full block: 64 cells × 64 units.
pub const BLOCK_UNITS: u32 = FINE_CELLS as u32 * VOLUME_FULL as u32;

/// One demoted block and the fine volume that did not fit.
///
/// `remainder` is in fine units and belongs in a neighbouring block; the
/// caller carries it there rather than destroying it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Collapse {
    pub cell: Cell,
    pub remainder: u32,
}

/// The fine-cell pool and the block → slot index.
pub struct FineStore {
    capacity: u32,
    bricks: Vec<[Cell; FINE_CELLS]>,
    free: Vec<u32>,
    slots: FastMap<IVec3, u32>,
    changed: FastMap<IVec3, u32>,
}

impl FineStore {
    /// A pool of at most `capacity` sub-bricks (512 B each).
    pub fn new(capacity: u32) -> Self {
        Self {
            capacity,
            bricks: Vec::new(),
            free: Vec::new(),
            slots: FastMap::default(),
            changed: FastMap::default(),
        }
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Sub-bricks currently allocated.
    pub fn in_use(&self) -> usize {
        self.bricks.len() - self.free.len()
    }

    pub fn free_slots(&self) -> usize {
        self.free.len() + self.capacity as usize - self.bricks.len()
    }

    /// Resident pool bytes, the number the fine budget is spent against.
    pub fn bytes(&self) -> usize {
        self.bricks.len() * SUB_BRICK_BYTES
    }

    /// Reserved payload and an entry-storage estimate for pool indexing.
    /// Hash-table control bytes and allocator overhead are excluded.
    pub(crate) fn allocated_bytes(&self) -> (usize, usize) {
        (
            self.bricks.capacity() * SUB_BRICK_BYTES,
            self.free.capacity() * size_of::<u32>()
                + (self.slots.capacity() + self.changed.capacity()) * size_of::<(IVec3, u32)>(),
        )
    }

    pub(crate) fn discard(&mut self, block: IVec3) {
        if let Some(&slot) = self.slots.get(&block) {
            self.release(block, slot);
        }
    }

    pub fn is_fine(&self, block: IVec3) -> bool {
        self.slots.contains_key(&block)
    }

    /// Split a uniform block into sixty-four equal cells.
    ///
    /// Promoting an already-fine block changes nothing: an authored partial
    /// block must not be overwritten by a later uniform write.
    pub fn promote(&mut self, block: IVec3, uniform: Cell, tick: u32) -> Result<(), PoolFull> {
        if self.slots.contains_key(&block) {
            return Ok(());
        }
        let slot = match self.free.pop() {
            Some(slot) => slot,
            None if self.bricks.len() < self.capacity as usize => {
                self.bricks.push([uniform; FINE_CELLS]);
                (self.bricks.len() - 1) as u32
            }
            None => return Err(PoolFull),
        };
        self.bricks[slot as usize] = [uniform; FINE_CELLS];
        self.slots.insert(block, slot);
        self.changed.insert(block, tick);
        Ok(())
    }

    pub fn cells(&self, block: IVec3) -> Option<&[Cell; FINE_CELLS]> {
        self.slots.get(&block).map(|&slot| &self.bricks[slot as usize])
    }

    /// Write one fine cell. Outside the 4³ sub-brick changes nothing.
    pub fn set(&mut self, block: IVec3, cell: IVec3, value: Cell, tick: u32) -> bool {
        let inside = cell.min_element() >= 0 && cell.max_element() < FINE_SIZE;
        let Some(&slot) = self.slots.get(&block) else {
            return false;
        };
        if !inside {
            return false;
        }
        let index = (cell.x + FINE_SIZE * (cell.z + FINE_SIZE * cell.y)) as usize;
        self.bricks[slot as usize][index] = value;
        self.changed.insert(block, tick);
        true
    }

    /// Demote a sub-brick whose cells are all equal. The pool slot returns.
    ///
    /// `hysteresis` keeps a block fine for a while after its last change so a
    /// settling water surface does not promote and demote every tick.
    pub fn demote_uniform(&mut self, block: IVec3, tick: u32, hysteresis: u32) -> Option<Cell> {
        let first = self.uniform_candidate(block, tick, hysteresis)?;
        self.discard(block);
        Some(first)
    }

    pub(crate) fn uniform_candidate(&self, block: IVec3, tick: u32, hysteresis: u32) -> Option<Cell> {
        let &slot = self.slots.get(&block)?;
        if tick.wrapping_sub(*self.changed.get(&block)?) < hysteresis {
            return None;
        }
        let cells = &self.bricks[slot as usize];
        let first = cells[0];
        if cells.iter().any(|c| *c != first) {
            return None;
        }
        Some(first)
    }

    /// Demote a heterogeneous sub-brick by volume-weighted averaging.
    ///
    /// Legacy lossy helper: preserves aggregate volume only, not composition or
    /// thermal energy. The coordinated `MatterStore` intentionally never uses
    /// or exposes it. Do not use for conservative budget-driven demotion.
    pub fn demote_averaged(&mut self, block: IVec3, tick: u32, hysteresis: u32) -> Option<Collapse> {
        let &slot = self.slots.get(&block)?;
        if tick.wrapping_sub(*self.changed.get(&block)?) < hysteresis {
            return None;
        }
        let collapse = collapsed(&self.bricks[slot as usize]);
        self.release(block, slot);
        Some(collapse)
    }

    fn release(&mut self, block: IVec3, slot: u32) {
        self.slots.remove(&block);
        self.changed.remove(&block);
        self.bricks[slot as usize] = [Cell::EMPTY; FINE_CELLS];
        self.free.push(slot);
    }
}

/// The block a sub-brick collapses to, plus volume that does not fit.
///
/// Material is the one with the largest total volume (first wins ties, so the
/// result is deterministic); volume is the mean of all sixty-four cells;
/// temperature is volume-weighted; pressure and charge are plain means.
pub fn collapsed(cells: &[Cell; FINE_CELLS]) -> Collapse {
    let mut totals = [0u32; 256];
    let mut total = 0u32;
    let mut weighted_temperature = 0u64;
    let mut pressure = 0i64;
    let mut charge = 0i64;
    for cell in cells {
        let volume = u32::from(cell.volume);
        totals[cell.material as usize] += volume;
        total += volume;
        weighted_temperature += u64::from(volume) * u64::from(cell.temperature);
        pressure += i64::from(cell.pressure);
        charge += i64::from(cell.charge);
    }
    let mut material = 0u8;
    let mut best = 0u32;
    for (index, &volume) in totals.iter().enumerate() {
        if volume > best {
            best = volume;
            material = index as u8;
        }
    }
    let volume = (total / FINE_CELLS as u32).min(u32::from(VOLUME_FULL)) as u8;
    let remainder = total.saturating_sub(u32::from(volume) * FINE_CELLS as u32);
    let temperature = if total == 0 {
        Cell::EMPTY.temperature
    } else {
        (weighted_temperature / u64::from(total)).min(u64::from(u16::MAX)) as u16
    };
    Collapse {
        cell: Cell {
            material,
            volume,
            temperature,
            pressure: (pressure / FINE_CELLS as i64) as i16,
            charge: (charge / FINE_CELLS as i64) as i16,
        },
        remainder,
    }
}
