use super::GpuWorld;
use crate::pack_cell;
use engine_sim::{BRICK_CELLS, BrickCoord, Cell, brick_and_cell, split_voxel};
use glam::IVec3;

#[derive(Debug, PartialEq, Eq)]
pub enum ReservedEditError {
    MissingBrick,
    InvalidCell,
    InvalidActivity,
}

impl GpuWorld {
    /// Upload reserved-cell edits and an exact acknowledged awake set.
    /// The caller owns sequencing: no later solver tick may be in flight.
    pub fn edit_reserved(
        &self,
        edits: &[(IVec3, Cell)],
        awake: &[BrickCoord],
    ) -> Result<(), ReservedEditError> {
        let mut writes = Vec::with_capacity(edits.len());
        for &(at, cell) in edits {
            if cell.volume > 64 {
                return Err(ReservedEditError::InvalidCell);
            }
            let (chunk, local) = split_voxel(at);
            let (brick, index) = brick_and_cell(local);
            let slot = self
                .reserved_slots
                .get(&(chunk, brick))
                .ok_or(ReservedEditError::MissingBrick)?;
            writes.push((
                (u64::from(*slot) * BRICK_CELLS as u64 + index as u64) * 8,
                pack_cell(cell),
            ));
        }
        let mut bits = vec![0_u32; self.words as usize];
        for b in awake {
            if b.scale != engine_sim::Scale::FINEST
                || !self.reserved_slots.contains_key(&(b.chunk, b.brick))
            {
                return Err(ReservedEditError::InvalidActivity);
            }
            let local = b.chunk - self.window.origin;
            let size = self.window.size;
            if !local.cmpge(IVec3::ZERO).all() || !local.as_uvec3().cmplt(size).all() {
                return Err(ReservedEditError::InvalidActivity);
            }
            let index = (local.x as u32 + size.x * (local.z as u32 + size.z * local.y as u32)) * 64
                + u32::from(b.brick);
            bits[(index / 32) as usize] |= 1 << (index % 32);
        }
        for (offset, words) in writes {
            self.queue
                .write_buffer(&self.pool, offset, bytemuck::cast_slice(&words));
        }
        self.queue
            .write_buffer(&self.bits, 0, bytemuck::cast_slice(&bits));
        Ok(())
    }
}
