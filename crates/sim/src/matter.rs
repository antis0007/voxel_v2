//! Coordinated block/fine storage. The two backing pools never escape mutably.
//! A promoted block's old coarse value is only backing storage: reads must use
//! `block`/`fine_cell`. This owner deliberately has no block-only CA adapter.
use crate::{
    BRICK_SIZE, BrickStore, Cell, FINE_CELLS, FINE_SIZE, FineStore, PoolFull, split_voxel,
};
use engine_core::fast_hash::FastMap;
use glam::IVec3;

/// Borrowed authoritative contents of one block; uniform reads allocate nothing.
#[derive(Clone, Copy, Debug)]
pub enum BlockView<'a> {
    Uniform(Cell),
    Fine(&'a [Cell; FINE_CELLS]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatterError {
    FineCellOutsideBlock,
    PoolFull,
}

impl From<PoolFull> for MatterError {
    fn from(_: PoolFull) -> Self {
        Self::PoolFull
    }
}

/// Payload is separate from indexing. Index estimates exclude hash-table control
/// bytes, allocator overhead and the size of the owner itself; not a heap total.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatterMemory {
    pub block_payload_live: usize,
    pub block_payload_reserved: usize,
    pub block_index_estimate: usize,
    pub fine_payload_live: usize,
    pub fine_payload_reserved: usize,
    pub fine_index_estimate: usize,
    pub fine_mask_estimate: usize,
}

/// One 1 m build grid with optional 4³ fine cells per block. World coordinates
/// here are block coordinates, independent of render LOD or legacy world scale.
/// Masks are sparse brick headers: eight u64 words, one bit per block. Missing
/// headers mean entirely uniform bricks, with no fine-pool lookup on reads.
pub struct MatterStore {
    blocks: BrickStore,
    fine: FineStore,
    masks: FastMap<IVec3, [u64; 8]>,
}

impl MatterStore {
    pub fn new(block_bricks: u32, fine_blocks: u32) -> Self {
        Self {
            blocks: BrickStore::new(block_bricks),
            fine: FineStore::new(fine_blocks),
            masks: FastMap::default(),
        }
    }

    /// World brick coordinates, including Euclidean division for negative blocks.
    fn address(block: IVec3) -> (IVec3, usize, u64) {
        let brick = IVec3::new(
            block.x.div_euclid(BRICK_SIZE),
            block.y.div_euclid(BRICK_SIZE),
            block.z.div_euclid(BRICK_SIZE),
        );
        let p = block.rem_euclid(IVec3::splat(BRICK_SIZE));
        let i = (p.x + BRICK_SIZE * (p.z + BRICK_SIZE * p.y)) as usize;
        (brick, i / 64, 1u64 << (i % 64))
    }

    pub fn fine_mask(&self, brick: IVec3) -> [u64; 8] {
        self.masks.get(&brick).copied().unwrap_or([0; 8])
    }

    pub fn is_fine(&self, block: IVec3) -> bool {
        let (brick, word, bit) = Self::address(block);
        self.masks.get(&brick).is_some_and(|m| m[word] & bit != 0)
    }

    pub fn block(&self, block: IVec3) -> BlockView<'_> {
        if self.is_fine(block) {
            BlockView::Fine(self.fine.cells(block).expect("fine mask owns a pool slot"))
        } else {
            let (chunk, local) = split_voxel(block);
            BlockView::Uniform(self.blocks.get(chunk, local))
        }
    }

    fn fine_index(local: IVec3) -> Result<usize, MatterError> {
        if local.min_element() < 0 || local.max_element() >= FINE_SIZE {
            return Err(MatterError::FineCellOutsideBlock);
        }
        Ok((local.x + FINE_SIZE * (local.z + FINE_SIZE * local.y)) as usize)
    }

    /// Uniform neighbours behave as 64 identical cells without promotion.
    pub fn fine_cell(&self, block: IVec3, local: IVec3) -> Result<Cell, MatterError> {
        let i = Self::fine_index(local)?;
        Ok(match self.block(block) {
            BlockView::Uniform(cell) => cell,
            BlockView::Fine(cells) => cells[i],
        })
    }

    /// Pool exhaustion leaves both representations and the mask unchanged.
    pub fn promote(&mut self, block: IVec3, tick: u32) -> Result<(), MatterError> {
        let uniform = match self.block(block) {
            BlockView::Fine(_) => return Ok(()),
            BlockView::Uniform(cell) => cell,
        };
        self.fine.promote(block, uniform, tick)?;
        let (brick, word, bit) = Self::address(block);
        self.masks.entry(brick).or_insert([0; 8])[word] |= bit;
        Ok(())
    }

    /// Equal writes do not allocate or restart hysteresis. A changed uniform
    /// block is promoted before the edit, preserving all its other fine cells.
    pub fn set_fine(
        &mut self,
        block: IVec3,
        local: IVec3,
        value: Cell,
        tick: u32,
    ) -> Result<(), MatterError> {
        if self.fine_cell(block, local)? == value {
            return Ok(());
        }
        self.promote(block, tick)?;
        assert!(self.fine.set(block, local, value, tick));
        Ok(())
    }

    /// Explicit build-grid replacement discards partial geometry only after the
    /// coarse write succeeds. This is an edit, not a conservative collapse.
    pub fn replace_block(&mut self, block: IVec3, value: Cell) -> Result<(), MatterError> {
        let (chunk, local) = split_voxel(block);
        self.blocks.set(chunk, local, value)?;
        self.clear_fine(block);
        Ok(())
    }

    /// Only identical cells can collapse. Mixed material, temperature, pressure
    /// or charge stays fine; there is no lossy averaged-demotion escape hatch.
    /// If the coarse pool is full, the fine slot and mask remain authoritative.
    pub fn demote_uniform(
        &mut self,
        block: IVec3,
        tick: u32,
        hysteresis: u32,
    ) -> Result<bool, MatterError> {
        let Some(value) = self.fine.uniform_candidate(block, tick, hysteresis) else {
            return Ok(false);
        };
        self.replace_block(block, value)?;
        Ok(true)
    }

    fn clear_fine(&mut self, block: IVec3) {
        self.fine.discard(block);
        let (brick, word, bit) = Self::address(block);
        if let Some(mask) = self.masks.get_mut(&brick) {
            mask[word] &= !bit;
            if *mask == [0; 8] {
                self.masks.remove(&brick);
            }
        }
    }

    pub fn fine_blocks(&self) -> usize {
        self.fine.in_use()
    }

    pub fn memory(&self) -> MatterMemory {
        let (reserved, index) = self.fine.allocated_bytes();
        let (block_reserved, block_index) = self.blocks.allocated_bytes();
        MatterMemory {
            block_payload_live: self.blocks.bricks_in_use()
                * crate::BRICK_CELLS
                * size_of::<Cell>(),
            block_payload_reserved: block_reserved,
            block_index_estimate: block_index,
            fine_payload_live: self.fine.in_use() * crate::SUB_BRICK_BYTES,
            fine_payload_reserved: reserved,
            fine_index_estimate: index,
            fine_mask_estimate: self.masks.capacity() * size_of::<(IVec3, [u64; 8])>(),
        }
    }
}
