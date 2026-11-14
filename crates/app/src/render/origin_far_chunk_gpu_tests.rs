use super::{DrawOffsets, DrawTransform, RenderOrigin};
use crate::mesh::Vertex3;
use crate::render::{CameraUniform, LightingUniform, REVERSE_Z};
use bytemuck::Zeroable;
use engine_core::CHUNK_SIZE;
use glam::{DVec3, IVec3, Mat4, Vec3};

const SIZE: u32 = 64;
const ROW_BYTES: u32 = SIZE * 4;
const SCALE: f32 = 0.5;

/// Vertex positions, chunk slot offset and origin for one depth render.
struct Case {
    positions: [[f32; 3]; 4],
    offset: [f32; 4],
    origin: IVec3,
    eye: DVec3,
}

/// Chunk-floor 32 x 32 quad in uploader chunk-local coordinates.
fn chunk_case(chunk: IVec3, eye: DVec3, origin: RenderOrigin) -> Case {
    Case {
        positions: [[0.0, 0.0, 0.0], [0.0, 0.0, 32.0], [32.0, 0.0, 32.0], [32.0, 0.0, 0.0]],
        offset: origin.chunk_offset(chunk),
        origin: origin.voxel,
        eye,
    }
}

/// Renders the world shader's vertex stage to 64x64 depth.
fn depth_image(device: &wgpu::Device, queue: &wgpu::Queue, case: &Case) -> Vec<f32> {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("far chunk world shader"),
        source: wgpu::ShaderSource::Wgsl(crate::shader_sources::world_shader_source().into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("far chunk depth"),
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
        fragment: None,
        primitive: Default::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Greater),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let eye_m = case.eye * f64::from(SCALE);
    let origin = RenderOrigin { voxel: case.origin };
    let projection = glam::camera::rh::proj::directx::perspective(90f32.to_radians(), 1.0, 0.1, 100.0);
    let mut camera = CameraUniform::zeroed();
    // Oblique, so a horizontal position error changes depth, not only coverage.
    let forward = Vec3::new(0.5, -1.0, 0.3).normalize();
    camera.view_proj = (REVERSE_Z * projection * glam::camera::rh::view::look_to_mat4(Vec3::ZERO, forward, Vec3::Y)).to_cols_array_2d();
    camera.camera_pos = [eye_m.x as f32, eye_m.y as f32, eye_m.z as f32, 0.0];
    camera.world_params = [SCALE, SCALE.recip(), 0.0, 1000.0];
    camera.render_origin = [case.origin.x, case.origin.y, case.origin.z, 0];
    let eye_rel = origin.eye_uniform(eye_m, f64::from(SCALE), Vec3::ZERO);
    camera.eye_from_origin = [eye_rel.x, eye_rel.y, eye_rel.z, 0.0];
    let lighting = LightingUniform::zeroed();

    let vertices = case.positions.map(|pos| Vertex3 {
        pos,
        color: [0.5; 3],
        normal: [0.0, 1.0, 0.0],
        surface: [0.9, 0.05, 0.0, 4.0],
        ao_strength: 1.0,
    });
    let mut offsets = DrawOffsets::default();
    offsets.begin_frame(DrawTransform::IDENTITY);
    let slot = offsets.slot(IVec3::ZERO, || DrawTransform { offset: case.offset, ..DrawTransform::IDENTITY });
    offsets.upload(device, queue);
    let table = offsets.buffer().expect("offset table");

    let init = |label, contents: &[u8], usage| {
        wgpu::util::DeviceExt::create_buffer_init(device, &wgpu::util::BufferInitDescriptor { label: Some(label), contents, usage })
    };
    let vertex_buffer = init("far quad", bytemuck::cast_slice(&vertices), wgpu::BufferUsages::VERTEX);
    let index_buffer = init("far quad indices", bytemuck::cast_slice(&[0u32, 1, 2, 0, 2, 3]), wgpu::BufferUsages::INDEX);
    let camera_buffer = init("far camera", bytemuck::bytes_of(&camera), wgpu::BufferUsages::UNIFORM);
    let lighting_buffer = init("far lighting", bytemuck::bytes_of(&lighting), wgpu::BufferUsages::UNIFORM);
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
        label: Some("far depth"),
        size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("far depth readback"),
        size: u64::from(ROW_BYTES * SIZE),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some("far chunk depth"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &view,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
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
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::DepthOnly },
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
    let depths = bytemuck::cast_slice::<u8, f32>(&readback.slice(..).get_mapped_range().expect("GPU readback range")).to_vec();
    readback.unmap();
    depths
}

fn max_difference(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max)
}

#[test]
#[ignore = "native GPU depth readback; run with just test-gpu"]
fn gpu_far_chunk_matches_near_chunk() {
    let adapter = engine_gpu::test_adapter();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();

    let near_eye = DVec3::new(16.0, 10.0, 16.0);
    let near = depth_image(&device, &queue, &chunk_case(IVec3::ZERO, near_eye, RenderOrigin { voxel: IVec3::ZERO }));
    let covered = near.iter().filter(|&&d| d > 0.0).count();
    assert!(covered > near.len() / 4 && covered < near.len(), "quad edge must be in view: {covered} texels");

    let far_chunk = IVec3::new(28_126, 0, 0);
    let far_eye = DVec3::new(900_048.0, 10.0, 16.0);
    let origin = RenderOrigin::follow(None, far_eye, 0);
    let far = depth_image(&device, &queue, &chunk_case(far_chunk, far_eye, origin));
    let diff = max_difference(&near, &far);
    assert!(diff <= 1e-6, "far chunk depth differs from near chunk by {diff}");

    // Diagnostics only: today's absolute path (absolute vertices, zero offset,
    // origin at the world origin), then both frames with a sub-voxel eye.
    let legacy = |eye: DVec3| {
        let base = (far_chunk * CHUNK_SIZE).as_vec3();
        let absolute = |x: f32, z: f32| [base.x + x, base.y, base.z + z];
        Case {
            positions: [absolute(0.0, 0.0), absolute(0.0, 32.0), absolute(32.0, 32.0), absolute(32.0, 0.0)],
            offset: [0.0; 4],
            origin: IVec3::ZERO,
            eye,
        }
    };
    let legacy_diff = max_difference(&near, &depth_image(&device, &queue, &legacy(far_eye)));
    let fraction = DVec3::new(0.3, 0.0, 0.0);
    let near_fraction = depth_image(&device, &queue, &chunk_case(IVec3::ZERO, near_eye + fraction, RenderOrigin { voxel: IVec3::ZERO }));
    let far_fraction = chunk_case(far_chunk, far_eye + fraction, RenderOrigin::follow(None, far_eye + fraction, 0));
    let origin_fraction = max_difference(&near_fraction, &depth_image(&device, &queue, &far_fraction));
    let legacy_fraction = max_difference(&near_fraction, &depth_image(&device, &queue, &legacy(far_eye + fraction)));
    println!(
        "[far-chunk] max depth diff: origin frame {diff:e} (absolute frame {legacy_diff:e}); eye +0.3 voxel: origin frame {origin_fraction:e} (absolute frame {legacy_fraction:e}) on {:?}",
        adapter.get_info().backend
    );
}
