use crate::fast_hash::FastMap;
use crate::world::{CHUNK_SIZE, ChunkData, World, chunk_coord_from_world_block};
use glam::IVec3;
use std::sync::{Arc, Weak};

const COLUMN_LIMIT: usize = 256;
const SIDE: u32 = CHUNK_SIZE as u32;
const _: () = assert!(SIDE <= u32::BITS);

#[derive(Clone, Debug, Default)]
pub(super) struct Cache {
    columns: FastMap<(IVec3, usize), Column>,
    pub(super) stats: Stats,
}

#[derive(Clone, Debug)]
struct Column {
    // The weak reference prevents address reuse and lets Arc::make_mut
    // invalidate the mask even when the world owns the only strong reference.
    chunk: Weak<ChunkData>,
    known: u32,
    opaque: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Stats {
    pub queries: u64,
    pub mask_hits: u64,
    pub mask_builds: u64,
    pub resident_reads: u64,
    pub scalar_reads: u64,
}

impl Cache {
    pub(super) fn covered(&mut self, world: &World, cell: IVec3, mut remaining: u32, mut need: u32) -> bool {
        self.stats.queries += 1;
        let mut column_reader = world.column_reader(cell.x, cell.z);
        let mut start = cell + IVec3::Y;
        while remaining != 0 {
            let coord = chunk_coord_from_world_block(start);
            let y = start.y.rem_euclid(CHUNK_SIZE) as u32;
            let count = remaining.min(SIDE - y);
            // Cellular overrides take precedence over resident chunk blocks.
            if let Some(chunk) = world.chunks.get(&coord).filter(|_| world.ca_view.is_none()) {
                let index = ChunkData::local_index(start.x, 0, start.z);
                let key = (coord, index);
                if self.columns.len() == COLUMN_LIMIT && !self.columns.contains_key(&key) {
                    self.columns.clear();
                }
                let column = self.columns.entry(key).or_insert_with(|| {
                    self.stats.mask_builds += 1;
                    Column { chunk: Arc::downgrade(chunk), known: 0, opaque: 0 }
                });
                if column.chunk.as_ptr() != Arc::as_ptr(chunk) {
                    self.stats.mask_builds += 1;
                    *column = Column { chunk: Arc::downgrade(chunk), known: 0, opaque: 0 };
                }
                let range = (u32::MAX >> (u32::BITS - count)) << y;
                self.stats.mask_hits += u64::from((range & column.known).count_ones());
                let covered = (range & column.opaque).count_ones();
                if covered >= need { return true; }
                need -= covered;

                // A cold entry reads only the requested cells, in scan order,
                // and stops as soon as the cover threshold is met.
                let mut missing = range & !column.known;
                while missing != 0 {
                    let height = missing.trailing_zeros();
                    let bit = 1 << height;
                    missing &= missing - 1;
                    column.known |= bit;
                    self.stats.resident_reads += 1;
                    if chunk.blocks[index + (height * SIDE * SIDE) as usize].is_opaque() {
                        column.opaque |= bit;
                        need -= 1;
                        if need == 0 { return true; }
                    }
                }
            } else {
                for dy in 0..count {
                    self.stats.scalar_reads += 1;
                    if column_reader(start.y + dy as i32).is_opaque() {
                        need -= 1;
                        if need == 0 { return true; }
                    }
                }
            }
            remaining -= count;
            if remaining != 0 { start.y += count as i32; }
        }
        false
    }
}
