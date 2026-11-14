use super::{DrawOffsets, DrawTransform, RenderOrigin};
use crate::mesh::Vertex3;
use crate::render::{CameraUniform, LightingUniform, REVERSE_Z};
use bytemuck::Zeroable;
use engine_core::CHUNK_SIZE;
use engine_core::cube_sphere::atlas;
use glam::{DVec3, IVec3, Mat4, Vec3};

const SIZE: u32 = 64;
const ROW_BYTES: u32 = SIZE * 16;
const SCALE: f32 = 0.5;

/// Writes the material pattern position's texel phase, as `fs_main` reads it.
const PROBE: &str = "
@fragment
fn fs_pattern_probe(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(fract(world_pattern_pos(in) * PIXEL_RES), 1.0);
}
";

/// Renders a planet chunk's floor quad and returns the per-pixel texel phase.
///
/// `chunk` is in chart chunks and `eye` in chart voxels; the draw goes through
/// the bent (rigid-frame) planet path with an untilted frame.
fn phase_image(device: &wgpu::Device, queue: &wgpu::Queue, chunk: IVec3, eye: DVec3) -> Vec<[f32; 4]> {
    let source = format!("{}\n{PROBE}", crate::shader_sources::world_shader_source());
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pattern probe"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pattern probe"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex3>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x4, 4 => Float32],
                }),
                Some(DrawTransform::LAYOUT),
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_pattern_probe"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba32Float.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let origin = RenderOrigin::follow(None, eye, 0);
    let eye_m = eye * f64::from(SCALE);
    let projection = glam::camera::rh::proj::directx::perspective(90f32.to_radians(), 1.0, 0.1, 100.0);
    let forward = Vec3::new(0.5, -1.0, 0.3).normalize();
    let mut camera = CameraUniform::zeroed();
    camera.view_proj = (REVERSE_Z * projection * glam::camera::rh::view::look_to_mat4(Vec3::ZERO, forward, Vec3::Y)).to_cols_array_2d();
    camera.camera_pos = [eye_m.x as f32, eye_m.y as f32, eye_m.z as f32, 0.0];
    camera.world_params = [SCALE, SCALE.recip(), 0.0, 1000.0];
    camera.render_origin = [origin.voxel.x, origin.voxel.y, origin.voxel.z, 0];
    let eye_rel = origin.eye_uniform(eye_m, f64::from(SCALE), Vec3::ZERO);
    camera.eye_from_origin = [eye_rel.x, eye_rel.y, eye_rel.z, 0.0];
    camera.planet[0] = [atlas::RADIUS_M as f32, atlas::HALF_M as f32, atlas::PITCH_M as f32, 1.0];
    let lighting = LightingUniform::zeroed();

    let chart = origin.chunk_offset(chunk);
    let axis = |unit: Vec3, lane: f32| (unit * SCALE).extend(lane).to_array();
    let transform = DrawTransform {
        offset: [chart[0] * SCALE, chart[1] * SCALE, chart[2] * SCALE, 1.0],
        axis_x: axis(Vec3::X, chart[0]),
        axis_y: axis(Vec3::Y, chart[1]),
        axis_z: axis(Vec3::Z, chart[2]),
        ..DrawTransform::IDENTITY
    };
    let edge = CHUNK_SIZE as f32;
    let vertices = [[0.0, 0.0, 0.0], [0.0, 0.0, edge], [edge, 0.0, edge], [edge, 0.0, 0.0]].map(|pos| Vertex3 {
        pos,
        color: [0.5; 3],
        normal: [0.0, 1.0, 0.0],
        surface: [0.9, 0.05, 0.0, 4.0],
        ao_strength: 1.0,
    });
    let mut offsets = DrawOffsets::default();
    offsets.begin_frame(DrawTransform::IDENTITY);
    let slot = offsets.slot(chunk, || transform);
    offsets.upload(device, queue);
    let table = offsets.buffer().expect("offset table");

    let init = |label, contents: &[u8], usage| {
        wgpu::util::DeviceExt::create_buffer_init(device, &wgpu::util::BufferInitDescriptor { label: Some(label), contents, usage })
    };
    let vertex_buffer = init("probe quad", bytemuck::cast_slice(&vertices), wgpu::BufferUsages::VERTEX);
    let index_buffer = init("probe indices", bytemuck::cast_slice(&[0u32, 1, 2, 0, 2, 3]), wgpu::BufferUsages::INDEX);
    let camera_buffer = init("probe camera", bytemuck::bytes_of(&camera), wgpu::BufferUsages::UNIFORM);
    let lighting_buffer = init("probe lighting", bytemuck::bytes_of(&lighting), wgpu::BufferUsages::UNIFORM);
    let group = |index: u32, buffer: &wgpu::Buffer| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(index),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
        })
    };
    let camera_group = group(0, &camera_buffer);
    let lighting_group = group(1, &lighting_buffer);

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("probe phase"),
        size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probe readback"),
        size: u64::from(ROW_BYTES * SIZE),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some("pattern probe"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &camera_group, &[]);
        pass.set_bind_group(1, &lighting_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, table.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..6, 0, slot..slot + 1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ROW_BYTES), rows_per_image: Some(SIZE) },
        },
        wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
    );
    let submission = queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = sender.send(r);
    });
    device
        .poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: Some(std::time::Duration::from_secs(10)) })
        .unwrap();
    receiver.recv_timeout(std::time::Duration::from_secs(1)).unwrap().unwrap();
    let pixels = bytemuck::cast_slice::<u8, [f32; 4]>(&readback.slice(..).get_mapped_range().expect("GPU readback range")).to_vec();
    readback.unmap();
    pixels
}

/// Largest texel-phase difference over covered pixels, wrapping at 1.
fn max_phase_difference(a: &[[f32; 4]], b: &[[f32; 4]]) -> f32 {
    let wrap = |d: f32| d.min(1.0 - d);
    a.iter()
        .zip(b)
        .filter(|(p, q)| p[3] > 0.5 && q[3] > 0.5)
        .map(|(p, q)| wrap((p[0] - q[0]).abs()).max(wrap((p[2] - q[2]).abs())))
        .fold(0.0, f32::max)
}

#[test]
#[ignore = "native GPU colour readback; run with just test-gpu"]
fn gpu_far_face_patterns_match_spawn_face() {
    let adapter = engine_gpu::test_adapter();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();

    // The screenshot position: atlas face 5, 1760 m east of the face centre.
    let face_voxels = (5.0 * atlas::PITCH_M / f64::from(SCALE)) as i32;
    assert_eq!(face_voxels % CHUNK_SIZE, 0, "face offset must be whole chunks");
    let near_chunk = IVec3::new(3_520 / CHUNK_SIZE, 0, 7);
    let near_eye = DVec3::new(3_536.3, 10.0, 240.7);
    let far_chunk = near_chunk + IVec3::new(face_voxels / CHUNK_SIZE, 0, 0);
    let far_eye = near_eye + DVec3::new(f64::from(face_voxels), 0.0, 0.0);

    let near = phase_image(&device, &queue, near_chunk, near_eye);
    let covered = near.iter().filter(|p| p[3] > 0.5).count();
    assert!(covered > near.len() / 4, "quad must fill the view: {covered} pixels");
    let far = phase_image(&device, &queue, far_chunk, far_eye);
    let diff = max_phase_difference(&near, &far);
    println!("[pattern-probe] face 5 vs face 0 max texel phase difference {diff} on {:?}", adapter.get_info().backend);
    // One PIXEL_RES texel is 5 cm; an f32 step at 329 km is 3.1 cm, 0.62 texel.
    assert!(diff < 0.01, "far-face pattern phase differs from the spawn face by {diff} texel");
}
