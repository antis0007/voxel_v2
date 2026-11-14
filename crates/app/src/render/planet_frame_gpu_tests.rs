use super::Eye;
use crate::render::CameraUniform;
use bytemuck::Zeroable;
use engine_core::cube_sphere::{Face, atlas};
use glam::DVec3;
use wgpu::util::DeviceExt;

/// Runs `planet_project`, the cover vertex path, for each probe.
const PROBE: &str = "
@group(1) @binding(1) var<storage, read> probes: array<vec4<f32>>;
@group(1) @binding(2) var<storage, read_write> results: array<vec4<f32>>;

@compute @workgroup_size(1)
fn probe_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = probes[id.x];
    results[id.x] = vec4<f32>(planet_project(u32(p.w), p.xyz), 0.0);
}
";

fn gpu_project(camera: &CameraUniform, probes: &[[f32; 4]]) -> Vec<[f32; 4]> {
    let adapter = engine_gpu::test_adapter();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let source = format!("{}\n{PROBE}", include_str!("../shaders/camera_lighting_shared.wgsl"));
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("planet project probe"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("planet project probe"),
        layout: None,
        module: &module,
        entry_point: Some("probe_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let init = |contents: &[u8], usage| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents, usage })
    };
    let camera_buffer = init(bytemuck::bytes_of(camera), wgpu::BufferUsages::UNIFORM);
    let probe_buffer = init(bytemuck::cast_slice(probes), wgpu::BufferUsages::STORAGE);
    let bytes = std::mem::size_of_val(probes) as u64;
    let results = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = |index: u32, entries: &[wgpu::BindGroupEntry]| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(index),
            entries,
        })
    };
    let camera_group = group(0, &[wgpu::BindGroupEntry { binding: 0, resource: camera_buffer.as_entire_binding() }]);
    let probe_group = group(1, &[
        wgpu::BindGroupEntry { binding: 1, resource: probe_buffer.as_entire_binding() },
        wgpu::BindGroupEntry { binding: 2, resource: results.as_entire_binding() },
    ]);
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &camera_group, &[]);
        pass.set_bind_group(1, &probe_group, &[]);
        pass.dispatch_workgroups(probes.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&results, 0, &readback, 0, bytes);
    let submission = queue.submit([encoder.finish()]);
    readback.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device
        .poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: Some(std::time::Duration::from_secs(10)) })
        .unwrap();
    bytemuck::cast_slice::<u8, [f32; 4]>(&readback.slice(..).get_mapped_range().expect("GPU readback range")).to_vec()
}

#[test]
#[ignore = "native GPU compute readback; run with just test-gpu"]
fn gpu_cover_projection_matches_chunk_placement() {
    // The owner's corner: chart axes meet at 60 degrees on the sphere here.
    let eye = DVec3::new(atlas::HALF_M - 492.0, 86.0, -(atlas::HALF_M - 150.0));
    // far_m for a 28-chunk view at 0.25 m voxels.
    let far_m = 28.0 * 8.0 * 2.0;
    let frame = Eye::new(eye, far_m);
    let mut camera = CameraUniform::zeroed();
    camera.planet = crate::planet::camera(eye, true, 0);
    camera.planet_flat = frame.shader_rows();
    let mut locals = Vec::new();
    for (dx, dz) in [(0.0, 0.0), (20.0, -10.0), (-35.0, 40.0), (90.0, 60.0), (-120.0, -100.0), (300.0, 140.0)] {
        for height in [60.0, 86.0] {
            locals.push(DVec3::new(eye.x + dx, height, (eye.z + dz).max(-atlas::HALF_M)));
        }
    }
    let probes: Vec<[f32; 4]> = locals.iter().map(|p| [p.x as f32, p.y as f32, p.z as f32, Face::PosX as u8 as f32]).collect();
    let gpu = gpu_project(&camera, &probes);
    for (local, out) in locals.iter().zip(gpu) {
        let cpu = frame.relative(atlas::join(Face::PosX, *local));
        let off = (DVec3::new(f64::from(out[0]), f64::from(out[1]), f64::from(out[2])) - cpu).length();
        assert!(off < 0.05, "cover point {local}: GPU is {off} m from the chunk placement");
    }
}
