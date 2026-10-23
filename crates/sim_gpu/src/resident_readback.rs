//! Reading a resident world back: only on request, never inside a tick.
use super::{ALLOC_HEADER, GpuWorld};
use crate::{GpuStepError, unpack_cell};
use engine_core::fast_hash::FastMap;
use engine_sim::{BRICK_CELLS, BrickCoord, Cell, NO_SLOT, Scale, split_voxel};
use glam::IVec3;

/// What a readback found.
#[derive(Debug, Default)]
pub struct Snapshot {
    /// Awake bricks, ordered as `BrickStore::dirty_bricks`.
    pub dirty: Vec<BrickCoord>,
    bricks: FastMap<(IVec3, u8), Vec<Cell>>,
    /// 0 when exact. Bit 0: matter tried to leave the window; bit 1: the pool ran out. Slots that lost an
    /// allocation race return only between phases, so a pool needs a little headroom over the CPU's need.
    pub overflow: u32,
    /// Blocks that changed since the world was created.
    pub changed_blocks: u32,
}

impl Snapshot {
    pub fn get(&self, voxel: IVec3) -> Cell {
        let (chunk, local) = split_voxel(voxel);
        let (b, c) = (local / 8, local % 8);
        let brick = (b.x + 4 * b.z + 16 * b.y) as u8;
        self.bricks.get(&(chunk, brick)).map_or(Cell::EMPTY, |cells| cells[(c.x + 8 * c.z + 64 * c.y) as usize])
    }

    pub fn bricks_in_use(&self) -> usize {
        self.bricks.len()
    }
}

impl GpuWorld {
    /// Read the world back: cells of every present brick, the awake set and the counters.
    pub fn read_back(&self) -> Result<Snapshot, GpuStepError> {
        let sources = [(&self.directory, self.bricks as u64 * 4), (&self.bits, self.words as u64 * 4), (&self.alloc, ALLOC_HEADER as u64 * 4), (&self.pool, self.pool.size())];
        let total: u64 = sources.iter().map(|s| s.1).sum();
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ca readback"),
            size: total,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut at = 0;
        for (buffer, bytes) in sources {
            encoder.copy_buffer_to_buffer(buffer, 0, &readback, at, bytes);
            at += bytes;
        }
        let submission = self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: None }).map_err(GpuStepError::Poll)?;
        rx.recv().map_err(|_| GpuStepError::Lost)?.map_err(GpuStepError::Map)?;
        let mapped = readback.slice(..).get_mapped_range();
        let all: &[u32] = bytemuck::cast_slice(&mapped);
        let (directory, rest) = all.split_at(self.bricks as usize);
        let (dirty, rest) = rest.split_at(self.words as usize);
        let (header, pool) = rest.split_at(ALLOC_HEADER as usize);
        let (o, s) = (self.window.origin, self.window.size);
        let coord = |wb: u32| {
            let (ci, brick) = (wb / 64, (wb % 64) as u8);
            let c = IVec3::new((ci % s.x) as i32, (ci / (s.x * s.z)) as i32, ((ci / s.x) % s.z) as i32);
            BrickCoord { chunk: o + c, brick, scale: Scale::FINEST }
        };
        let mut snapshot = Snapshot { overflow: header[2], changed_blocks: header[3], ..Default::default() };
        for (wb, &slot) in directory.iter().enumerate().filter(|(_, s)| **s != NO_SLOT) {
            let b = coord(wb as u32);
            let words = &pool[slot as usize * BRICK_CELLS * 2..][..BRICK_CELLS * 2];
            snapshot.bricks.insert((b.chunk, b.brick), words.chunks_exact(2).map(|w| unpack_cell([w[0], w[1]])).collect());
            if dirty[wb / 32] >> (wb % 32) & 1 != 0 {
                snapshot.dirty.push(b);
            }
        }
        snapshot.dirty.sort_by_key(|b| (b.chunk.z, b.chunk.y, b.chunk.x, b.brick));
        drop(mapped);
        readback.unmap();
        Ok(snapshot)
    }
}
