//! Fixed-size cache for procedural collision and picking queries. Edited chunks
//! are read first by World and never enter this cache.
use super::{BlockId, WorldSampleConfig, sample_block};
use glam::IVec3;

const CAPACITY: usize = 16_384;

pub(super) struct SampleCache {
    config: Option<WorldSampleConfig>,
    entries: Box<[Option<(IVec3, BlockId)>]>,
}

impl Default for SampleCache {
    fn default() -> Self {
        Self {
            config: None,
            entries: vec![None; CAPACITY].into_boxed_slice(),
        }
    }
}

impl SampleCache {
    pub fn get(&mut self, config: WorldSampleConfig, position: IVec3) -> BlockId {
        if self.config != Some(config) {
            self.entries.fill(None);
            self.config = Some(config);
        }
        let hash = (position.x as u32).wrapping_mul(0x8da6b343)
            ^ (position.y as u32).wrapping_mul(0xd8163841)
            ^ (position.z as u32).wrapping_mul(0xcb1ab31f);
        let slot = (hash ^ (hash >> 16)) as usize & (CAPACITY - 1);
        if let Some((cached, block)) = self.entries[slot] {
            if cached == position {
                return block;
            }
        }
        let block = sample_block(&config, position);
        self.entries[slot] = Some((position, block));
        block
    }
}
