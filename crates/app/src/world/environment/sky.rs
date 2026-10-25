//! Bounded derived chunk summaries for repeated sky queries. Two bit planes
//! retain every vertical opening and leaf cell; this does not alter geometry.
use super::*;
use crate::world::{ChunkData, chunk_coord_from_world_block};
pub const MAX_CHUNKS: usize = 256;
pub const RESERVATION: usize = 8_512;

pub struct Summary {
    opaque: Box<[u32; 1024]>,
    leaves: Box<[u32; 1024]>,
    pub touched: u64,
}
impl Summary {
    pub fn new(chunk: &ChunkData, touched: u64) -> Self {
        let mut s = Self { opaque: Box::new([0; 1024]), leaves: Box::new([0; 1024]), touched };
        for (i, &b) in chunk.blocks.iter().enumerate() {
            let column = i % 1024;
            let bit = 1 << (i / 1024);
            if matches!(b, BlockId::Leaves | BlockId::PineLeaves | BlockId::Bush) {
                s.leaves[column] |= bit;
            } else if b.is_solid() {
                s.opaque[column] |= bit;
            }
        }
        s
    }
    /// Exact part of one column at/above this voxel in the cached chunk.
    pub fn above(&self, p: IVec3) -> (bool, u32) {
        let i = (p.z.rem_euclid(32) * 32 + p.x.rem_euclid(32)) as usize;
        let mask = u32::MAX << p.y.rem_euclid(32);
        (self.opaque[i] & mask != 0, (self.leaves[i] & mask).count_ones())
    }
}
impl State {
    pub fn invalidate_sky(&mut self, p: IVec3) {
        self.sky.remove(&key(chunk_coord_from_world_block(p)));
    }
}
