use super::*;

#[test]
fn sky_probe_timing_stays_separate_from_legacy_subtotal() {
    let mut queries = [None; GPU_TIMESTAMP_PASS_COUNT as usize];
    queries[GPU_TIMESTAMP_PASS_SKY_PROBE as usize] = Some((0, 1));
    queries[GPU_TIMESTAMP_PASS_TERRAIN as usize] = Some((2, 3));
    let timings = GpuProfiler::decode_timings(&[10, 1_000_010, 0, 2_000_000], &queries, 1.0);
    assert_eq!(timings.sky_probe_ms, 1.0);
    assert_eq!(timings.terrain_ms, 2.0);
    assert_eq!(timings.total_ms, 2.0);
}

#[test]
fn unavailable_probe_query_has_no_cost_or_query_allocation() {
    let mut queries = [None; GPU_TIMESTAMP_PASS_COUNT as usize];
    let mut count = 0;
    assert!(allocate_pass_timestamp_writes(None, &mut count, &mut queries,
        GPU_TIMESTAMP_PASS_SKY_PROBE).is_none());
    assert_eq!(count, 0);
    let timings = GpuProfiler::decode_timings(&[], &queries, 1.0);
    assert_eq!(timings.sky_probe_ms, 0.0);
}

#[test]
fn sky_probe_query_slot_fits_reserved_capacity() {
    assert_eq!(GPU_TIMESTAMP_PASS_SKY_PROBE + 1, GPU_TIMESTAMP_PASS_COUNT);
    assert_eq!(GPU_TIMESTAMP_QUERY_COUNT, GPU_TIMESTAMP_PASS_COUNT * 2);
    assert!(GPU_TIMESTAMP_RESOLVE_BYTES >= GPU_TIMESTAMP_RESULT_BYTES);
}

#[test]
#[ignore = "native GPU timestamp validation; run with just test-gpu"]
fn gpu_sky_probe_timestamp_allocation_resolves_without_validation_errors() {
    let adapter = engine_gpu::test_adapter();
    assert!(adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY));
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TIMESTAMP_QUERY,
        ..Default::default()
    })).unwrap();
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("probe timing contract"), ty: wgpu::QueryType::Timestamp,
        count: GPU_TIMESTAMP_QUERY_COUNT,
    });
    let mut queries = [None; GPU_TIMESTAMP_PASS_COUNT as usize];
    let mut count = 0;
    let timestamps = allocate_pass_timestamp_writes(Some(&query_set), &mut count,
        &mut queries, GPU_TIMESTAMP_PASS_SKY_PROBE).unwrap();
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let _pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("probe timestamp contract"),
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set: timestamps.query_set,
                beginning_of_pass_write_index: timestamps.beginning_of_pass_write_index,
                end_of_pass_write_index: timestamps.end_of_pass_write_index,
            }),
        });
    }
    assert_eq!(count, 2);
    assert_eq!(queries[GPU_TIMESTAMP_PASS_SKY_PROBE as usize], Some((0, 1)));
    let resolve = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probe timestamp resolve"), size: GPU_TIMESTAMP_RESOLVE_BYTES,
        usage: wgpu::BufferUsages::QUERY_RESOLVE, mapped_at_creation: false,
    });
    encoder.resolve_query_set(&query_set, 0..count, &resolve, 0);
    let submission = queue.submit([encoder.finish()]);
    device.poll(wgpu::PollType::Wait { submission_index: Some(submission),
        timeout: Some(std::time::Duration::from_secs(10)) }).unwrap();
    assert!(pollster::block_on(validation.pop()).is_none());
}
