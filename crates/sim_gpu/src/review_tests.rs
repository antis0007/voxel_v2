//! Cases an independent review of an earlier, separate GPU kernel found missing from the equivalence tests:
//! the 64-bit helpers on values the material table never produces, and a wake that crosses a brick face
//! with every other brick asleep. (Pressure and charge are now random in `tests::random_cell`.)
use super::*;
use engine_content::{BlockId as B, SIM_RULES};
use engine_sim::{Cell, VOLUME_FULL, brick_and_cell, split_voxel, step};
use wgpu::util::DeviceExt;

fn device() -> (wgpu::Device, wgpu::Queue) {
    let adapter = engine_gpu::test_adapter();
    pollster::block_on(adapter.request_device(&Default::default())).expect("device")
}

/// `mul64`, `mulhi` and `mul_div_rem` against 128-bit arithmetic on edge values around 0, 2^16, 2^31 and
/// 2^32, which the material table's value ranges never reach.
#[test]
#[ignore = "needs a GPU: just test-gpu"]
fn gpu_64_bit_helpers_match_wide_arithmetic() {
    let (device, queue) = device();
    let source = format!(
        "{}\n@group(0) @binding(20) var<storage, read> t_in: array<vec4<u32>>;\n\
         @group(0) @binding(21) var<storage, read_write> t_out: array<vec4<u32>>;\n\
         @compute @workgroup_size(1)\nfn test_math(@builtin(global_invocation_id) gid: vec3<u32>) {{\n\
         let c = t_in[gid.x];\n let d = mul_div_rem(c.x, c.y, c.z);\n let p = mul64(c.x, c.y);\n\
         t_out[gid.x] = vec4<u32>(d.x, d.y, p.x, p.y);\n}}\n",
        program(OFFLOAD_WGSL)
    );
    let edges = [0u32, 1, 2, 3, 0xFFFF, 0x1_0000, 0x1_0001, 0x7FFF_FFFF, 0x8000_0000, 0x8000_0001, 0xFFFF_FFFE, 0xFFFF_FFFF];
    let mut cases = Vec::new();
    for &a in &edges {
        for &b in &edges {
            for &c in edges.iter().filter(|&&c| c > 0) {
                // The rule's contract: the quotient fits u32.
                if u128::from(a) * u128::from(b) / u128::from(c) <= u128::from(u32::MAX) {
                    cases.push([a, b, c, 0]);
                }
            }
        }
    }
    let bytes: Vec<u8> = cases.iter().flatten().flat_map(|w| w.to_le_bytes()).collect();
    let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
    let input = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: &bytes, usage });
    let output = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: bytes.len() as u64, usage, mapped_at_creation: false });
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes.len() as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(source.into()) });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("test_math"),
        compilation_options: Default::default(),
        cache: None,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 20, resource: input.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 21, resource: output.as_entire_binding() },
        ],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(cases.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &staging, 0, bytes.len() as u64);
    let submission = queue.submit([encoder.finish()]);
    staging.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: None }).expect("poll");
    let mapped = staging.slice(..).get_mapped_range().to_vec();
    for (i, case) in cases.iter().enumerate() {
        let word = |n: usize| u32::from_le_bytes(mapped[(4 * i + n) * 4..][..4].try_into().unwrap());
        let (a, b, c) = (u128::from(case[0]), u128::from(case[1]), u128::from(case[2]));
        let product = a * b;
        assert_eq!(
            [word(0), word(1), word(2), word(3)],
            [(product / c) as u32, (product % c) as u32, product as u32, (product >> 32) as u32],
            "a={a} b={b} c={c}"
        );
    }
}

/// A stone floor in a sleeping brick and one sand grain in the awake brick above it.
fn grain_world(x: i32, z: i32) -> BrickStore {
    let mut store = BrickStore::new(600);
    let stone = Cell { pressure: 7, charge: -9, ..Cell::filled(B::Stone as u8) };
    for dx in -3..3 {
        for dz in -3..3 {
            let (chunk, local) = split_voxel(IVec3::new(x + dx, 1, z + dz));
            store.set(chunk, local, stone).expect("pool room");
        }
    }
    let (chunk, local) = split_voxel(IVec3::new(x, 9, z));
    store.set(chunk, local, Cell { volume: VOLUME_FULL, pressure: 3, charge: 5, ..Cell::filled(B::Sand as u8) }).expect("pool room");
    store.take_all_dirty();
    store.wake(chunk, brick_and_cell(local).0);
    store
}

/// The grain falls across the face between bricks (y = 8) onto a floor in a brick that is asleep. Only the
/// block that spans the face can wake the lower brick, and it must do so at the tick the CPU does.
#[test]
#[ignore = "needs a GPU: just test-gpu"]
fn gpu_wake_crosses_a_brick_face() {
    let (device, queue) = device();
    let window = Window { origin: IVec3::new(-1, -1, -1), size: glam::UVec3::new(2, 2, 2), capacity: 600 };
    for (x, z) in [(0, 0), (3, -5), (-1, 7)] {
        let mut cpu = grain_world(x, z);
        assert_eq!(cpu.active_bricks(), 1);
        let gpu = GpuWorld::new(device.clone(), queue.clone(), &SIM_RULES, window, &cpu).expect("upload");
        for tick in 0..14 {
            step(&mut cpu, &SIM_RULES, 5, tick);
            gpu.step(5, tick);
            let snapshot = gpu.read_back().expect("read back");
            assert_eq!(snapshot.overflow, 0, "grain at ({x}, {z}) tick {tick}: overflow");
            assert_eq!(cpu.dirty_bricks(), snapshot.dirty, "grain at ({x}, {z}) tick {tick}: awake bricks");
        }
        let snapshot = gpu.read_back().expect("read back");
        for p in (-6..6).flat_map(|dx| (0..12).flat_map(move |y| (-6..6).map(move |dz| IVec3::new(x + dx, y, z + dz)))) {
            let (chunk, local) = split_voxel(p);
            assert_eq!(cpu.get(chunk, local), snapshot.get(p), "grain at ({x}, {z}): cell {p}");
        }
    }
}
