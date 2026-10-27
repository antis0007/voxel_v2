use super::*;

#[test]
#[ignore = "native Vulkan depth readback; run explicitly with the shared GPU slot"]
fn gpu_far_shadow_gpu_ownership_and_invalidation() {
    let adapter = engine_gpu::test_adapter();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shadow ownership contract"),
        source: wgpu::ShaderSource::Wgsl(crate::shader_sources::SHADOW_SHADER.into()),
    });
    let mut renderer = ShadowRenderer::new(&device, &shader);
    let matrix = Mat4::orthographic_rh(-16.0, 16.0, -16.0, 16.0, 0.1, 40.0)
        * Mat4::look_to_rh(Vec3::new(16.0, 20.0, 16.0), -Vec3::Y, Vec3::Z);
    let mut vertices = [
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 32.0],
        [32.0, 0.0, 32.0],
        [32.0, 0.0, 0.0],
    ]
    .map(|pos| Vertex3 {
        pos,
        color: [0.5; 3],
        normal: [0.0, 1.0, 0.0],
        surface: [0.9, 0.05, 3.0, 4.0],
        ao_strength: -1.0,
    });
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("contract quad"),
        size: std::mem::size_of_val(&vertices) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("contract indices"),
        size: 24,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(
        &index_buffer,
        0,
        bytemuck::cast_slice(&[0u32, 1, 2, 0, 2, 3]),
    );
    let draw_offsets = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("contract draw offset"),
        size: std::mem::size_of::<DrawTransform>() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("contract depth"),
        size: wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("contract depth readback"),
        size: 8 * 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let empty = ResidentCoverage {
        header: [-1, -1, 3, 1],
        ..Default::default()
    };
    let mut owned = empty;
    assert!(owned.set_column(0, 0));
    let mut adjacent = empty;
    assert!(adjacent.set_column(-1, 0));
    let mut sparse_owned = ResidentCoverage {
        header: [1000, 1000, 129, 1],
        ..Default::default()
    };
    assert!(sparse_owned.set_column(0, 0));
    // Far from spawn: the parent quad stays absolute, the matrix is origin-relative.
    let far = RenderOrigin { voxel: glam::IVec3::new(897_024, 0, -4096) };
    let spawn = RenderOrigin { voxel: glam::IVec3::ZERO };
    for (name, ao, kind, coverage, suppressed, origin) in [
        ("parent-unowned", -1.0, 3.0, empty, false, spawn),
        ("parent-owned", -1.0, 3.0, owned, true, spawn),
        ("scene-owned", 1.0, 3.0, owned, false, spawn),
        ("parent-adjacent-owner", -1.0, 3.0, adjacent, false, spawn),
        ("parent-outside-dense-window", -1.0, 3.0, sparse_owned, true, spawn),
        ("resident-water-not-opaque-caster", 1.0, 8.0, empty, true, spawn),
        ("far-water-not-opaque-caster", -1.0, 8.0, empty, true, spawn),
        ("packed-water-not-opaque-caster", 1.0, 72.0, empty, true, spawn),
        ("foliage-remains-caster", 1.0, 6.0, empty, false, spawn),
        ("parent-unowned-far-origin", -1.0, 3.0, empty, false, far),
        ("parent-owned-far-origin", -1.0, 3.0, owned, true, far),
    ] {
        for vertex in &mut vertices {
            vertex.ao_strength = ao;
            vertex.surface[2] = kind;
        }
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        let offset = DrawTransform { offset: origin.absolute_offset(), ..DrawTransform::IDENTITY };
        queue.write_buffer(&draw_offsets, 0, bytemuck::bytes_of(&offset));
        let origin_relative = matrix * Mat4::from_translation(origin.voxel.as_vec3());
        queue.write_buffer(
            &renderer.gen_buffers[0],
            0,
            bytemuck::bytes_of(&ShadowGenUniform {
                light_view_proj: origin_relative.to_cols_array_2d(),
                world_scale: 1.0,
                time: 0.0,
                wind: [0.0; 2],
                resident_coverage: coverage,
                render_origin: origin.voxel.extend(0).to_array(),
            }),
        );
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(name),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            // As in `render`: parent pages use the discard pipeline, chunks the depth-only one.
            pass.set_pipeline(if ao < 0.0 { &renderer.pipeline } else { &renderer.child_pipeline });
            pass.set_bind_group(0, &renderer.gen_bind_groups[0], &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, draw_offsets.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..6, 0, 0..1);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::DepthOnly,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(8),
                },
            },
            wgpu::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
        );
        let submission = queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            sender.send(r).unwrap();
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
            .unwrap();
        let mapped = readback.slice(..).get_mapped_range();
        let center = 4 * 256 + 4 * 4;
        let depth = f32::from_ne_bytes(mapped[center..center + 4].try_into().unwrap());
        drop(mapped);
        readback.unmap();
        if suppressed {
            assert_eq!(depth, 1.0, "{name}");
        } else {
            assert!(depth > 0.0 && depth < 0.9, "{name}: {depth}");
        }
        println!("[far-shadow-contract] case={name} depth={depth}");
    }
    renderer.force_refresh = false;
    renderer.shadow_map_dirty = false;
    renderer.set_horizon_source(7, &owned);
    assert!(renderer.force_refresh && renderer.shadow_map_dirty);
    renderer.force_refresh = false;
    renderer.shadow_map_dirty = false;
    renderer.set_horizon_source(7, &owned);
    assert!(!renderer.force_refresh && !renderer.shadow_map_dirty);
    // An ownership-only change redraws the layers that can see the changed
    // columns, at once; a layer with no fitted volume yet counts as seeing all.
    const ALL: u8 = (1 << CASCADE_COUNT) - 1;
    renderer.set_horizon_source(7, &empty);
    assert!(!renderer.force_refresh && renderer.pending_cascades == ALL && renderer.urgent_cascades == ALL);
    renderer.pending_cascades = 0;
    renderer.urgent_cascades = 0;
    renderer.set_horizon_source(8, &empty);
    assert!(renderer.force_refresh && renderer.shadow_map_dirty);
    renderer.force_refresh = false;
    renderer.shadow_map_dirty = false;
    let mut sparse_change = empty;
    assert!(sparse_change.set_column(-1000, 1000));
    assert_eq!(sparse_change.header, empty.header);
    assert_eq!(sparse_change.words, empty.words);
    renderer.set_horizon_source(8, &sparse_change);
    assert!(
        renderer.urgent_cascades == ALL,
        "sparse-only ownership changes reach unfitted shadow layers"
    );
    println!(
        "[far-shadow-contract] adapter={:?} layers={} gen_bytes={} receiver_bytes={} publication/map/clear invalidation passed",
        adapter.get_info(),
        CASCADE_COUNT,
        std::mem::size_of::<ShadowGenUniform>(),
        std::mem::size_of::<ShadowUniform>()
    );
}

#[test]
#[ignore = "native Vulkan production world/shadow sparse lookup; run serially"]
fn gpu_dense_sparse_world_and_shadow_ownership_match_after_origin_shift() {
    use bytemuck::Zeroable;
    use std::collections::HashSet;
    let camera = include_str!("../shaders/camera_lighting_shared.wgsl");
    let world = include_str!("../shaders/world_shader.wgsl");
    let shadow = include_str!("../shaders/shadow_shader.wgsl");
    let camera_prefix = &camera[..camera.find("// Projection-scaled").unwrap()];
    let world_helpers = &world[world.find("fn terrain_resident_column_owned").unwrap()
        ..world.find("fn terrain_vertex_ao").unwrap()];
    let shadow_prefix =
        shadow[..shadow.find("struct VsIn").unwrap()].replace("@binding(0)", "@binding(3)");
    let shadow_helpers = &shadow
        [shadow.find("fn shadow_resident_owned").unwrap()..shadow.find("@fragment").unwrap()];
    let shader_source = format!(
        "{camera_prefix}\n{world_helpers}\n{shadow_prefix}\n{shadow_helpers}\n{}",
        r#"
@group(0) @binding(1) var<storage, read> points: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> result: array<u32>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= arrayLength(&points) { return; }
    let p = points[id.x].xy;
    result[id.x] = select(0u,1u,terrain_resident_column_owned(p,0.0))
        | select(0u,2u,shadow_resident_owned(p));
}
"#
    );
    let adapter = engine_gpu::test_adapter();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("actual world and shadow sparse ownership"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("sparse ownership ABI contract"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let buffer = |label, size, usage| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let camera_buffer = buffer(
        "actual camera uniform",
        std::mem::size_of::<super::super::CameraUniform>() as u64,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    );
    let shadow_buffer = buffer(
        "actual shadow uniform",
        std::mem::size_of::<ShadowGenUniform>() as u64,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    );
    let mut pins = vec![(i32::MIN, i32::MIN)];
    pins.extend((0..511).map(|i| (1000 + i * 7919, -2000 - i * 37)));
    let mut checked = 0;
    for (cx, cz) in [(0, 0), (1000, -2000), (1_000_000, -1_000_000)] {
        let mut map = ResidentCoverage {
            header: [cx - 64, cz - 64, 129, 1],
            ..Default::default()
        };
        let mut owners: HashSet<_> = pins.iter().copied().collect();
        owners.insert((cx, cz));
        let mut sorted: Vec<_> = owners.iter().copied().collect();
        sorted.sort_unstable();
        for &(x, z) in &sorted {
            assert!(map.set_column(x, z));
        }
        let mut queries = sorted.clone();
        queries.extend(sorted.iter().map(|&(x, z)| (x, z.saturating_add(1))));
        queries.extend([
            (cx + 1, cz),
            (cx - 65, cz),
            (cx + 65, cz),
            (i32::MIN, i32::MIN + 1024),
        ]);
        for scale in [1.0, 0.5, 0.25] {
            let points: Vec<[f32; 4]> = queries
                .iter()
                .map(|&(x, z)| {
                    [
                        (x as f32 + 0.5) * 32.0 * scale,
                        (z as f32 + 0.5) * 32.0 * scale,
                        0.0,
                        0.0,
                    ]
                })
                .collect();
            // Extreme sentinel +1 is below float precision; expected membership
            // follows the actual representable point rather than claiming precision.
            let expected: Vec<_> = points
                .iter()
                .map(|p| {
                    let key = (
                        (p[0] / (32.0 * scale)).floor() as i32,
                        (p[1] / (32.0 * scale)).floor() as i32,
                    );
                    if owners.contains(&key) {
                        3u32
                    } else {
                        0u32
                    }
                })
                .collect();
            let point_buffer = buffer(
                "ownership queries",
                std::mem::size_of_val(points.as_slice()) as u64,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            );
            let bytes = (points.len() * 4) as u64;
            let output = buffer(
                "world-shadow answers",
                bytes,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            );
            let readback = buffer(
                "world-shadow readback",
                bytes,
                wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            );
            let mut uniform = super::super::CameraUniform::zeroed();
            uniform.world_params[0] = scale;
            uniform.resident_coverage = map;
            queue.write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&uniform));
            queue.write_buffer(
                &shadow_buffer,
                0,
                bytemuck::bytes_of(&ShadowGenUniform {
                    light_view_proj: [[0.0; 4]; 4],
                    world_scale: scale,
                    time: 0.0,
                    wind: [0.0; 2],
                    resident_coverage: map,
                    render_origin: [0; 4],
                }),
            );
            queue.write_buffer(&point_buffer, 0, bytemuck::cast_slice(&points));
            let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("actual two-uniform ABI"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: point_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: output.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: shadow_buffer.as_entire_binding(),
                    },
                ],
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bindings, &[]);
                pass.dispatch_workgroups((points.len() as u32).div_ceil(64), 1, 1);
            }
            encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, bytes);
            let submission = queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
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
                let mapped = readback.slice(..).get_mapped_range();
                let actual: &[u32] = bytemuck::cast_slice(&mapped);
                assert_eq!(
                    actual,
                    expected.as_slice(),
                    "centre=({cx},{cz}),scale={scale}; world bit1,shadow bit2"
                );
                checked += actual.len();
            }
            readback.unmap();
        }
    }
    println!("[sparse-owner-contract] checked={checked} actual Camera/Shadow ABI and production helpers,512pins,3scales,3world origins on {:?}",adapter.get_info());
}
