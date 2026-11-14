//! Completed player sources survive a full writeback queue.
use super::*;
use anyhow::{Result, ensure};

pub(crate) fn run() -> Result<bool> {
    let mut w = World::with_seed(crate::world::WorldType::Superflat,
        crate::fluid::VoxelScalePreset::OneMeter, 7);
    let p = IVec3::new(-770_000, 200, -64);
    let coord = chunk_coord_from_world_block(p);
    let mut data = engine_worldgen::generate(&w.sample_config(), coord);
    data.blocks[crate::world::ChunkData::local_index(p.x, p.y, p.z)] = crate::world::BlockId::Dirt;
    let data = Arc::new(data);
    let (tx, rx) = mpsc::sync_channel(1);
    tx.send(Done { coord, version: None, generation: 0,
        data: Ok(DiskChunk { chunk: data.clone(), water: Arc::default() }) })?;
    let mut source = Source { rx, pending: Some(coord), ..Source::default() };
    let empty = Arc::new(engine_worldgen::generate(&w.sample_config(), IVec3::new(0, 100, 0)));
    let mut count = 0;
    while !w.regions.backlog_full() {
        ensure!(count < 100_000, "fixture writeback ceiling unexpectedly large");
        let c = IVec3::new(count, 100, 0);
        w.chunks.insert(c, empty.clone());
        w.regions.mark_dirty(c);
        count += 1;
    }
    ensure!(matches!(source.prepare(&mut w, p), Admission::Waiting), "full queue admitted a cold source");
    ensure!(!w.chunks.contains_key(&coord), "cold source installed under pressure");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while w.regions.backlog_full() {
        ensure!(std::time::Instant::now() < deadline, "fixture writeback timed out");
        w.request_simulation_writeback();
        w.stream_regions(IVec3::new(0, 100, 0), 0);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let ready = matches!(source.prepare(&mut w, p), Admission::Ready);
    let retained = ready && w.chunks.get(&coord).is_some_and(|chunk| Arc::ptr_eq(chunk, &data));
    println!("[edit-admission] completed-source retained={retained} pressure_chunks={count}");
    Ok(retained)
}

#[test]
fn completed_source_waits_for_writeback_capacity() { assert!(run().unwrap()); }
