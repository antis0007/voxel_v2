use super::*;
use crate::mesh::{GeneratedChunk, PADDED_CHUNK_SIZE};
use crate::world::BlockId;

/// Ground below y = 8, a stone roof at y = 12 over x,z 8..20 with open sides,
/// and a leaf canopy over x 24..32, y 10..14.
fn scene() -> GeneratedChunk {
    let mut c = GeneratedChunk::new(IVec3::ZERO, vec![BlockId::Air; (PADDED_CHUNK_SIZE as usize).pow(3)]);
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let roof = y == 12 && (8..20).contains(&x) && (8..20).contains(&z);
                let canopy = (10..14).contains(&y) && (24..32).contains(&x) && (8..20).contains(&z);
                let block = if y < 8 || roof {
                    BlockId::Stone
                } else if canopy {
                    BlockId::Leaves
                } else {
                    BlockId::Air
                };
                c.set_world(IVec3::new(x, y, z), block);
            }
        }
    }
    c
}

/// Whole probe texture of one level; `probe(cell)` returns state and the
/// +y, +x, -x, +z, -z visibilities.
fn read_probes(device: &wgpu::Device, queue: &wgpu::Queue, level: &Level) -> Vec<u8> {
    let edge = PROBE_DIM as u32;
    let row = edge * 8;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sky volume readback"),
        size: u64::from(row * edge * edge),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        level.texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(edge) },
        },
        wgpu::Extent3d { width: edge, height: edge, depth_or_array_layers: edge },
    );
    let submission = queue.submit([encoder.finish()]);
    readback.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device
        .poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: Some(std::time::Duration::from_secs(10)) })
        .unwrap();
    let bytes = readback.slice(..).get_mapped_range().expect("GPU readback range").to_vec();
    readback.unmap();
    bytes
}

fn probe(bytes: &[u8], cell: IVec3) -> (u16, [f32; 5]) {
    let edge = PROBE_DIM as u32;
    let s = cell.rem_euclid(IVec3::splat(PROBE_DIM)).as_uvec3();
    let i = ((s.z * edge + s.y) * edge + s.x) as usize * 8;
    let w = |k: usize| u16::from_le_bytes([bytes[i + 2 * k], bytes[i + 2 * k + 1]]);
    let v = |x: u16| f32::from(x & 255) / 255.0;
    (w(0) >> 8, [v(w(0)), v(w(1)), v(w(1) >> 8), v(w(2)), v(w(2) >> 8)])
}

fn wait(device: &wgpu::Device) {
    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(10)) }).unwrap();
}

#[test]
#[ignore = "native GPU compute; run serially"]
fn gpu_sky_volume_bakes_roof_canopy_and_open_ground() {
    let adapter = engine_gpu::test_adapter();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let chunk = scene();
    let mut volume = SkyVolumeGpu::new(&device);
    let eye = Vec3::new(16.0, 10.0, 16.0);
    let mut batch_ms = Vec::new();
    for _ in 0..32 {
        let start = std::time::Instant::now();
        volume.fine.update_from(&device, &queue, &volume.pipeline, eye, 1.0, |c| (c == IVec3::ZERO).then_some((&chunk, 1)));
        wait(&device);
        batch_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    // The first update also clears all 512 bricks; batches 2..16 trace 32 bricks each.
    let full: Vec<f64> = batch_ms[1..16].to_vec();
    println!("[sky-volume-contract] 32-brick batch wall ms (update + GPU wait): min {:.3} median {:.3} max {:.3}; idle update {:.3}",
        full.iter().cloned().fold(f64::MAX, f64::min), {
            let mut s = full.clone();
            s.sort_by(f64::total_cmp);
            s[s.len() / 2]
        }, full.iter().cloned().fold(0.0, f64::max), batch_ms[31]);
    let bytes = read_probes(&device, &queue, &volume.fine);
    let open = probe(&bytes, IVec3::new(2, 8, 28));
    let roofed = probe(&bytes, IVec3::new(14, 8, 14));
    let leafy = probe(&bytes, IVec3::new(28, 8, 14));
    let solid = probe(&bytes, IVec3::new(14, 3, 14)).0;
    println!("[sky-volume-contract] open {:?} roof {:?} canopy {:?} solid state {solid} on {:?}",
        open.1, roofed.1, leafy.1, adapter.get_info().name);
    assert_eq!(open.0, 2);
    assert!(open.1.iter().all(|&v| v > 0.97), "open ground and its walls keep the whole sky: {:?}", open.1);
    assert!(roofed.1[0] < 0.25, "a roof shades an upward face: {:?}", roofed.1);
    assert!(roofed.1[1] > roofed.1[0], "side sky still reaches a face turned to the open side: {:?}", roofed.1);
    assert!(leafy.1[0] > roofed.1[0] && leafy.1[0] < 0.9, "a canopy transmits part of the sky: {:?}", leafy.1);
    assert_eq!(solid, 1, "solid cells carry no probe");

    // The coarse level keeps the one-voxel roof: cells are 4 voxels, so the roof
    // spans cells 2..5 at y = 3 and the air under it is y = 2.
    let coarse = Coarse::build(&chunk, COARSE_SCALE);
    for _ in 0..40 {
        volume.coarse.update_from(&device, &queue, &volume.pipeline, eye, 1.0, |c| (c == IVec3::ZERO).then_some((&coarse, 1)));
    }
    wait(&device);
    let bytes = read_probes(&device, &queue, &volume.coarse);
    let open = probe(&bytes, IVec3::new(0, 2, 7));
    let roofed = probe(&bytes, IVec3::new(3, 2, 3));
    println!("[sky-volume-contract] coarse open {:?} roof {:?}", open.1, roofed.1);
    assert_eq!(open.0, 2);
    assert!(open.1[0] > 0.97, "coarse open ground keeps the sky: {:?}", open.1);
    assert!(roofed.1[0] < 0.4, "a one-voxel roof still shades the coarse level: {:?}", roofed.1);
}
