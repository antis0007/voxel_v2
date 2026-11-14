//! Foreground reads retain a small versioned working set despite worker churn.
use super::{BlockId, regions::DiskChunk};
use glam::IVec3;
use std::collections::VecDeque;

const MAX_ENTRIES: usize = 8;
const MAX_BYTES: usize = 1024 * 1024;
struct Entry {
    coord: IVec3,
    instance: u64,
    version: u64,
    bytes: usize,
    data: DiskChunk,
}

#[derive(Default)]
pub(super) struct Cache {
    entries: VecDeque<Entry>,
    bytes: usize,
}

impl Cache {
    pub(super) fn read(&mut self, coord: IVec3, instance: u64, version: u64,
        load: impl FnOnce() -> Option<DiskChunk>) -> Option<DiskChunk> {
        if let Some(i) = self.entries.iter().position(|e| e.coord == coord && e.instance == instance && e.version == version) {
            let entry = self.entries.remove(i)?;
            let data = entry.data.clone();
            self.entries.push_back(entry);
            return Some(data);
        }
        let data = load()?;
        let bytes = std::mem::size_of_val(&data.chunk.blocks[..])
            + std::mem::size_of_val(&data.chunk.light[..])
            + data.chunk.torches.capacity() * 32 + data.water.capacity() * 32;
        if bytes > MAX_BYTES { return Some(data); }
        self.entries.retain(|e| e.coord != coord);
        self.bytes = self.entries.iter().map(|e| e.bytes).sum();
        while self.entries.len() >= MAX_ENTRIES || self.bytes + bytes > MAX_BYTES {
            let Some(old) = self.entries.pop_front() else { break; };
            self.bytes -= old.bytes;
        }
        self.bytes += bytes;
        self.entries.push_back(Entry { coord, instance, version, bytes, data: data.clone() });
        Some(data)
    }
}

pub(crate) fn review() -> anyhow::Result<()> {
    use anyhow::ensure;
    use crate::{fluid::VoxelScalePreset, world::{World, WorldType}};
    use std::{cell::Cell, sync::Arc};
    let w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let mut raw = engine_worldgen::generate(&w.sample_config(), IVec3::ZERO);
    raw.blocks[0] = BlockId::Dirt;
    let dirt = DiskChunk { chunk: Arc::new(raw), water: Arc::default() };
    let mut raw = (*dirt.chunk).clone();
    raw.blocks[0] = BlockId::Stone;
    let stone = DiskChunk { chunk: Arc::new(raw), water: Arc::default() };
    let mut cache = Cache::default();
    let loads = Cell::new(0);
    let first = cache.read(IVec3::ZERO, 1, 1, || { loads.set(loads.get() + 1); Some(dirt.clone()) });
    ensure!(first.is_some(), "initial stored read failed");
    ensure!(cache.read(IVec3::ZERO, 1, 1, || None).is_some(), "warm read used the shared loader");
    let changed = cache.read(IVec3::ZERO, 1, 2, || Some(stone.clone()));
    ensure!(changed.is_some_and(|d| d.chunk.blocks[0] == BlockId::Stone), "stale record version returned");
    let changed = cache.read(IVec3::ZERO, 2, 2, || Some(dirt.clone()));
    ensure!(changed.is_some_and(|d| d.chunk.blocks[0] == BlockId::Dirt), "cache crossed storage identity");
    for x in 1..=MAX_ENTRIES {
        let _ = cache.read(IVec3::new(x as i32, 0, 0), 2, 1, || Some(dirt.clone()));
    }
    ensure!(cache.entries.len() <= MAX_ENTRIES && cache.bytes <= MAX_BYTES, "foreground cache exceeded its bound");
    ensure!(cache.read(IVec3::ZERO, 2, 2, || None).is_none(), "cold entry was not evicted");
    let mut oversize = stone.clone();
    Arc::make_mut(&mut oversize.chunk).torches.reserve(MAX_BYTES / 16);
    let at = IVec3::new(100, 0, 0);
    ensure!(cache.read(at, 2, 1, || Some(oversize)).is_some(), "oversized source changed read semantics");
    ensure!(cache.read(at, 2, 1, || None).is_none(), "oversized source exceeded the payload bound");
    ensure!(cache.bytes <= MAX_BYTES, "oversized source exceeded the byte bound");
    ensure!(loads.get() == 1, "warm lookup decoded again");
    println!("[edit-admission] foreground-cache versioned=true instance_guard=true bounded=true");
    Ok(())
}

#[test]
fn foreground_samples_are_versioned_and_bounded() { review().unwrap(); }
