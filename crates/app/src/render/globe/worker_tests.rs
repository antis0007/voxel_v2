use super::*;
use std::sync::atomic::AtomicUsize;
use std::time::Instant;

const A: TileId = TileId { face: 0, level: 2, x: 0, y: 0 };
const B: TileId = TileId { face: 0, level: 2, x: 1, y: 0 };
const C: TileId = TileId { face: 0, level: 2, x: 2, y: 0 };

fn test_cache(name: &str) -> TileCache {
    let dir = std::env::temp_dir().join(format!("globe-worker-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    TileCache::open(&dir, &test_cfg())
}

fn test_cfg() -> WorldSampleConfig {
    WorldSampleConfig {
        planet: None,
        generator_version: 53,
        seed: 74,
        world_type: engine_voxel::WorldType::Hills,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter,
    }
}

fn wait_until(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

/// Moves finished tiles into `pending` until `n` are buffered, without draining them.
fn buffer(w: &Worker, n: usize) -> bool {
    wait_until(|| {
        while let Ok(t) = w.results.try_recv() {
            w.pending.borrow_mut().push_back(t);
        }
        w.pending.borrow().len() >= n
    })
}

fn first_id(w: &Worker) -> Option<TileId> {
    buffer(w, 1).then(|| w.drain(0).first().map(|t| t.id)).flatten()
}

#[test]
fn a_dead_worker_reports_not_alive_and_drains_cleanly() {
    let bake = |_: &WorldSampleConfig, _: TileId| -> Option<TileData> { panic!("bake failed") };
    let w = Worker::spawn_with(test_cfg(), bake, test_cache("dead"));
    w.request(&[TileId::root(0)]);
    assert!(wait_until(|| !w.alive()));
    assert!(w.drain(2 << 20).is_empty());
}

#[test]
fn drain_splits_at_the_byte_cap_but_always_delivers_one() {
    let bake = |_: &WorldSampleConfig, id: TileId| Some(TileData::empty(id));
    let w = Worker::spawn_with(test_cfg(), bake, test_cache("bytes"));
    w.request(&[A, B, C]);
    assert!(buffer(&w, 3));
    assert!(w.alive());
    assert_eq!(w.drain(25_350 * 2).len(), 2);
    assert_eq!(w.drain(1).len(), 1);
    assert!(w.drain(2 << 20).is_empty());
}

/// Bake that needs `n + 1` calls on A and one on any other tile, counting A's calls.
fn slow_a(
    n: usize,
    calls: Arc<AtomicUsize>,
) -> impl FnMut(&WorldSampleConfig, TileId) -> Option<TileData> + Send + 'static {
    move |_, id| {
        if id != A {
            return Some(TileData::empty(id));
        }
        std::thread::sleep(Duration::from_millis(1));
        let c = calls.fetch_add(1, Ordering::SeqCst);
        (c >= n).then(|| TileData::empty(id))
    }
}

#[test]
fn a_new_request_preempts_a_half_baked_tile() {
    let n = 200;
    let calls = Arc::new(AtomicUsize::new(0));
    let w = Worker::spawn_with(test_cfg(), slow_a(n, calls.clone()), test_cache("preempt"));
    w.request(&[A]);
    assert!(wait_until(|| calls.load(Ordering::SeqCst) >= 5));
    w.request(&[B]);
    assert_eq!(first_id(&w), Some(B));
    assert!(calls.load(Ordering::SeqCst) <= n, "A finished before B");
}

#[test]
fn a_preempted_tile_resumes_without_restarting() {
    let n = 50;
    let calls = Arc::new(AtomicUsize::new(0));
    let w = Worker::spawn_with(test_cfg(), slow_a(n, calls.clone()), test_cache("resume"));
    w.request(&[A]);
    assert!(wait_until(|| calls.load(Ordering::SeqCst) >= 5));
    w.request(&[B]);
    assert_eq!(first_id(&w), Some(B));
    w.request(&[A]);
    assert_eq!(first_id(&w), Some(A));
    assert_eq!(calls.load(Ordering::SeqCst), n + 1);
}

#[test]
fn a_bake_that_never_finishes_is_dropped() {
    let bake = |_: &WorldSampleConfig, id: TileId| (id != A).then(|| TileData::empty(id));
    let w = Worker::spawn_with(test_cfg(), bake, test_cache("stuck"));
    w.request(&[A, B]);
    assert_eq!(first_id(&w), Some(B));
}

#[test]
fn dropping_the_worker_stops_its_thread() {
    let bake = |_: &WorldSampleConfig, id: TileId| Some(TileData::empty(id));
    let w = Worker::spawn_with(test_cfg(), bake, test_cache("drop"));
    let flag = w.alive.clone();
    drop(w);
    assert!(wait_until(|| !flag.load(Ordering::Acquire)));
}
