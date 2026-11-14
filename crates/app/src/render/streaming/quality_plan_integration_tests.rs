use super::*;
use crate::{
    fluid::VoxelScalePreset,
    render::streaming::ChunkStreamer,
    world::{BlockId, World, WorldType},
};

fn fixture() -> (
    World,
    ChunkCache,
    ChunkStreamer,
    wgpu::Device,
    Vec<(i32, i32)>,
) {
    let adapter = engine_gpu::test_adapter();
    let (device, _) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let snapshot = world.snapshot();
    let mut cache = ChunkCache::new(IVec3::ZERO);
    let mut streamer = ChunkStreamer::new(&device, 1).unwrap();
    let wanted: Vec<_> = (8..11).flat_map(|x| (-3..0).map(move |z| (x, z))).collect();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        cache.set_quality_columns(&snapshot, &wanted, false, &mut streamer);
        if cache.quality_proxies.columns.len() == wanted.len()
            && cache
                .quality_proxies
                .columns
                .values()
                .all(|c| c.planned.is_some())
        {
            break;
        }
        assert!(Instant::now() < deadline, "column planner did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
    (world, cache, streamer, device, wanted)
}

fn members(cache: &ChunkCache) -> Vec<((i32, i32), Vec<IVec3>, usize)> {
    let mut result: Vec<_> = cache
        .quality_proxies
        .columns
        .iter()
        .map(|(&key, c)| (key, c.members.clone(), c.dependencies.len()))
        .collect();
    result.sort_by_key(|c| c.0);
    result
}

#[test]
#[ignore = "native Vulkan device; run serially"]
fn gpu_quality_plan_reuse_matches_rebuild_and_promotes_new_high_edit() {
    let (mut world, mut cache, mut streamer, _device, wanted) = fixture();
    let snapshot = world.snapshot();
    let cached = cache.set_quality_columns(&snapshot, &wanted, false, &mut streamer);
    let before = members(&cache);
    for column in cache.quality_proxies.columns.values_mut() {
        column.planned = None;
    }
    let rebuilt = cache.set_quality_columns(&snapshot, &wanted, false, &mut streamer);
    assert_eq!(cached, rebuilt);
    assert_eq!(before, members(&cache));
    let key = wanted[0];
    world.set(
        IVec3::new(key.0 * CHUNK_SIZE, 255, key.1 * CHUNK_SIZE),
        BlockId::Stone,
    );
    cache.set_quality_columns(&world.snapshot(), &wanted, false, &mut streamer);
    assert!(cache.edit_proxies.persistent_wanted(&key));
    assert!(!cache.quality_proxies.columns.contains_key(&key));
    assert!(
        cache
            .edit_proxies
            .coords
            .iter()
            .any(|q| q.x == key.0 && q.z == key.1 && q.y >= 255 / CHUNK_SIZE)
    );
}

#[test]
#[ignore = "native Vulkan device and timing experiment; run serially"]
fn gpu_quality_plan_reuse_benchmark() {
    let (mut world, mut cache, mut streamer, _device, wanted) = fixture();
    let snapshot = world.snapshot();
    let expected = members(&cache);
    let mut cached = Vec::new();
    let mut rebuilt = Vec::new();
    for batch in 0..40 {
        // Interleave the controls to reduce scheduling and thermal bias.
        for force_replan in [batch % 2 == 0, batch % 2 != 0] {
            let start = Instant::now();
            for _ in 0..50 {
                if force_replan {
                    for column in cache.quality_proxies.columns.values_mut() {
                        column.planned = None;
                    }
                }
                std::hint::black_box(cache.set_quality_columns(
                    &snapshot,
                    &wanted,
                    false,
                    &mut streamer,
                ));
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / 50.0;
            if force_replan {
                rebuilt.push(ms);
            } else {
                cached.push(ms);
            }
        }
    }
    assert_eq!(expected, members(&cache));
    cached.sort_by(f64::total_cmp);
    rebuilt.sort_by(f64::total_cmp);
    println!(
        "[quality-plan-bench] columns={} members={} dependencies={} samples=40 calls_per_sample=50 cached_p50_ms={:.6} cached_p95_ms={:.6} rebuild_p50_ms={:.6} rebuild_p95_ms={:.6}",
        wanted.len(),
        expected.iter().map(|c| c.1.len()).sum::<usize>(),
        expected.iter().map(|c| c.2).sum::<usize>(),
        cached[20],
        cached[38],
        rebuilt[20],
        rebuilt[38]
    );
}
