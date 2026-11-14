use super::ResidentCoverage;

#[test]
#[ignore = "native Vulkan compute readback; run explicitly with the shared GPU slot"]
fn gpu_production_water_ownership_edges_and_corners() {
    use bytemuck::Zeroable;
    let camera_source = include_str!("../shaders/camera_lighting_shared.wgsl");
    let world_source = include_str!("../shaders/world_shader.wgsl");
    let prefix = &camera_source[..camera_source.find("// Projection-scaled").unwrap()];
    let helpers = &world_source[world_source
        .find("fn terrain_resident_column_owned")
        .unwrap()
        ..world_source.find("fn terrain_vertex_ao").unwrap()];
    let source = format!(
        "{prefix}\n{helpers}\n{}",
        r#"
@group(0) @binding(1) var<storage, read_write> result: array<f32>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let points = array<vec2<f32>, 10>(
        vec2(0.0,16.0), vec2(32.0,16.0), vec2(16.0,0.0), vec2(16.0,32.0),
        vec2(0.0,0.0), vec2(16.0,16.0), vec2(-0.25,16.0), vec2(1.0,16.0),
        vec2(0.5,0.5), vec2(1.0,1.0));
    result[id.x] = terrain_water_ownership_weight(points[id.x] * cam.world_params.x, cam.world_params.x);
}
"#
    );
    let adapter = engine_gpu::test_adapter();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("production water ownership contract"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("water ownership contract"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("actual camera ABI"),
        size: std::mem::size_of::<super::super::CameraUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("water weights"),
        size: 40,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("water weights readback"),
        size: 40,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("water contract bindings"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let mut center = ResidentCoverage {
        header: [-1, -1, 3, 1],
        ..Default::default()
    };
    assert!(center.set_column(0, 0));
    let mut corner_hole = ResidentCoverage {
        header: [-1, -1, 3, 1],
        ..Default::default()
    };
    for z in -1..=1 {
        for x in -1..=1 {
            if (x, z) != (-1, -1) {
                assert!(corner_hole.set_column(x, z));
            }
        }
    }
    let mut sparse_center = ResidentCoverage {
        header: [1000, 2000, 3, 1],
        ..Default::default()
    };
    assert!(sparse_center.set_column(0, 0));
    let mut sparse_corner_hole = ResidentCoverage {
        header: [1000, 2000, 3, 1],
        ..Default::default()
    };
    for z in -1..=1 {
        for x in -1..=1 {
            if (x, z) != (-1, -1) {
                assert!(sparse_corner_hole.set_column(x, z));
            }
        }
    }
    // Analytic distances to the complement of the published footprint. The
    // diagonal case distinguishes a real corner hole from four side checks.
    for scale in [1.0, 0.5, 0.25] {
        for (name, coverage, expected) in [
            (
                "isolated",
                center,
                [0., 0., 0., 0., 0., 1., 0., 0.5, 0.15625, 0.5],
            ),
            (
                "diagonal hole",
                corner_hole,
                [1., 1., 1., 1., 0., 1., 1., 1., 0.28661165, 0.79289322],
            ),
            ("disabled", ResidentCoverage::default(), [1.; 10]),
            (
                "out-of-window isolated",
                sparse_center,
                [0., 0., 0., 0., 0., 1., 0., 0.5, 0.15625, 0.5],
            ),
            (
                "out-of-window diagonal hole",
                sparse_corner_hole,
                [1., 1., 1., 1., 0., 1., 1., 1., 0.28661165, 0.79289322],
            ),
        ] {
            let mut camera = super::super::CameraUniform::zeroed();
            camera.world_params[0] = scale;
            camera.resident_coverage = coverage;
            queue.write_buffer(&uniform, 0, bytemuck::bytes_of(&camera));
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bindings, &[]);
                pass.dispatch_workgroups(10, 1, 1);
            }
            encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, 40);
            let submission = queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                tx.send(r).unwrap();
            });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: Some(std::time::Duration::from_secs(10)),
                })
                .unwrap();
            rx.recv_timeout(std::time::Duration::from_secs(1))
                .unwrap()
                .unwrap();
            {
                let bytes = readback.slice(..).get_mapped_range().expect("GPU readback range");
                let actual: &[f32] = bytemuck::cast_slice(&bytes);
                for (i, (&got, want)) in actual.iter().zip(expected).enumerate() {
                    assert!(
                        (got - want).abs() < 0.00003,
                        "{name} scale={scale} point={i}: {got} != {want}"
                    );
                }
            }
            readback.unmap();
        }
    }
    eprintln!(
        "water ownership contract passed: {:?}; three scales, dense/sparse side/corner holes, disabled map",
        adapter.get_info()
    );
}
