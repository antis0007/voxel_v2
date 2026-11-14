use super::*;
use crate::mesh::models::posed::pose;
use glam::{Quat, Vec3};

fn vertex(i: u32) -> Vertex3 {
    let f = |k: u32| ((i * 7 + k * 13) % 97) as f32 * 0.37 - 17.0;
    Vertex3 {
        pos: [f(0) * 40.0, f(1) * 40.0, f(2) * 40.0],
        color: [f(3), f(4), f(5)],
        normal: Vec3::new(f(6), f(7), f(8)).normalize_or_zero().to_array(),
        surface: [f(9), f(10), 62.0, f(12)],
        ao_strength: f(13),
    }
}

#[test]
#[ignore = "native GPU compute; run serially"]
fn gpu_posed_vertices_match_cpu_pose() {
    let adapter = engine_gpu::test_adapter();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let pool: Vec<Vertex3> = (0..1500).map(vertex).collect();
    // Uneven counts, a repeated range and kilometre offsets like real crowns.
    let placements = [
        (0, 700, Quat::from_rotation_y(0.7) * Quat::from_rotation_x(-0.2), Vec3::new(12_000.0, 18_300.0, -4_200.0)),
        (700, 800, Quat::from_axis_angle(Vec3::new(1.0, 2.0, 3.0).normalize(), 2.1), Vec3::new(-650.0, 18_000.0, 260.0)),
        (0, 700, Quat::IDENTITY, Vec3::ZERO),
        (1499, 1, Quat::from_rotation_z(3.0), Vec3::splat(1.0)),
    ]
    .map(|(first, count, rotation, translation)| Placement { first, count, rotation, translation, scale: 0.25 });
    let mut posed = PosedAssets::new(&device);
    posed.upload(&device, &queue, &placements, &pool, 1);
    let total = placements.iter().map(|p| p.count).sum::<u32>();
    let (_, count) = posed.vertices().expect("posed vertices");
    assert_eq!(count, total);
    let bytes = u64::from(total) * VERTEX_BYTES;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("posed readback"),
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    posed.record(&mut encoder);
    encoder.copy_buffer_to_buffer(posed.output.as_ref().unwrap(), 0, &readback, 0, bytes);
    let submission = queue.submit([encoder.finish()]);
    readback.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device
        .poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: Some(std::time::Duration::from_secs(10)) })
        .unwrap();
    let gpu: Vec<Vertex3> = bytemuck::cast_slice(&readback.slice(..).get_mapped_range().expect("GPU readback range")).to_vec();
    let mut i = 0;
    for p in &placements {
        for source in &pool[p.first as usize..(p.first + p.count) as usize] {
            let want = pose(source, p.rotation, p.translation, p.scale);
            let got = gpu[i];
            for (a, b) in want.pos.iter().zip(got.pos).chain(want.normal.iter().zip(got.normal)) {
                // A few f32 ulps of a 70 km coordinate, well below a voxel.
                assert!((a - b).abs() <= 1e-5 * a.abs().max(1.0) * 8.0, "vertex {i}: {a} vs {b}");
            }
            assert_eq!(want.color, got.color, "vertex {i} colour");
            assert_eq!(want.surface, got.surface, "vertex {i} surface");
            assert_eq!(want.ao_strength, got.ao_strength, "vertex {i} ao");
            i += 1;
        }
    }
    println!("[posed-assets-contract] {total} vertices in {} placements match the CPU pose on {:?}", placements.len(), adapter.get_info());
}
