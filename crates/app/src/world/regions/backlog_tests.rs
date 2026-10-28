use super::*;
use crate::fluid::VoxelScalePreset;
use crate::world::{World, WorldType};

fn temp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("voxel-{tag}-{}-{}", std::process::id(), checkpoint_id()))
}

fn toggle(w: &mut World, p: IVec3) -> BlockId {
    let id = if w.get(p) == BlockId::Air { BlockId::Stone } else { BlockId::Air };
    w.set(p, id);
    id
}

#[test]
fn write_backoff_doubles_to_a_cap() {
    let secs: Vec<_> = [1, 2, 3, 6, 40].into_iter().map(|n| write_backoff(n).as_secs()).collect();
    assert_eq!(secs, [2, 4, 8, 64, 64]);
}

#[test]
fn stored_chunk_reads_hit_the_cache_without_disk() {
    let root = temp_root("region-cache");
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    w.regions = RegionStream::open(root.clone(), HashMap::new());
    let p = IVec3::new(70, 40, -50);
    let q = crate::world::chunk_coord_from_world_block(p);
    let id = toggle(&mut w, p);
    w.request_simulation_writeback();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !w.regions.view.records.contains_key(&q) {
        assert!(Instant::now() < deadline, "region write timeout");
        w.request_simulation_writeback();
        w.stream_regions(q, 1);
        std::thread::sleep(Duration::from_millis(2));
    }
    let view = w.regions.view.clone();
    let first = view.load(q).ok().flatten().map(|d| d.chunk.blocks[ChunkData::local_index(p.x, p.y, p.z)]);
    assert_eq!(first, Some(id));
    for entry in fs::read_dir(&root).into_iter().flatten().flatten() {
        if entry.path().extension().is_some_and(|e| e == "vxr") {
            let _ = fs::remove_file(entry.path());
        }
    }
    let second = view.load(q).ok().flatten().map(|d| d.chunk.blocks[ChunkData::local_index(p.x, p.y, p.z)]);
    assert_eq!(second, Some(id), "a repeated stored read must not touch the disk");
}

#[test]
fn generated_edit_chunks_are_captured_without_a_resident_scan() {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    w.regions = RegionStream::open(temp_root("region-generated"), HashMap::new());
    let c = IVec3::new(4, 1, -3);
    assert!(w.resident_edit(c).is_ok());
    assert!(!w.regions.dirty.contains_key(&c));
    assert!(w.capture_edits().ok().flatten().is_some());
    assert!(w.regions.dirty.contains_key(&c), "an untracked resident chunk must enter the checkpoint");
    assert!(w.regions.generated.is_empty());
}

#[test]
fn failing_writes_back_off_and_cap_new_edit_chunks() {
    // A file where the session directory should be makes every write fail.
    let root = temp_root("region-unwritable");
    fs::write(&root, b"not a directory").ok();
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    w.regions = RegionStream::open(root.clone(), HashMap::new());
    w.regions.dirty_ceiling_override = Some(3);
    let points: Vec<_> = (0..4).map(|i| IVec3::new(i * 32 + 5, 40, 5)).collect();
    let mut ids = Vec::new();
    for &p in &points[..3] {
        ids.push(toggle(&mut w, p));
    }
    assert_eq!(w.regions.dirty.len(), 3);
    assert!(w.regions.backlog_full());

    let refused = crate::world::chunk_coord_from_world_block(points[3]);
    assert!(w.resident_edit(refused).is_err(), "a fourth chunk must be refused at the ceiling");
    assert!(!w.chunks.contains_key(&refused));
    let inside = points[0] + IVec3::X;
    let inside_id = toggle(&mut w, inside);
    assert_eq!(w.get(inside), inside_id, "edits to resident dirty chunks still commit");

    let deadline = Instant::now() + Duration::from_secs(5);
    while w.regions.write_failures == 0 {
        assert!(Instant::now() < deadline, "write failure was never reported");
        w.request_simulation_writeback();
        w.stream_regions(IVec3::ZERO, 1);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(w.regions.view.error().is_some_and(|e| e.contains("Region write failed")));
    assert!(w.regions.write_retry_after > Instant::now() + Duration::from_secs(1));
    assert!(w.region_status().contains("new edit chunks refused"));
    assert_eq!(w.regions.dirty.len(), 3, "failed writes must keep every edit dirty");
    for (&p, &id) in points.iter().zip(&ids) {
        assert_eq!(w.get(p), id, "a failed write must not drop the edit at {p}");
    }
    let _ = fs::remove_file(root);
}
