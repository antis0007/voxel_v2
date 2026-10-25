use super::{DrawOffsets, DrawTransform};
use crate::mesh::Vertex3;
use crate::render::shadow::ShadowRenderer;
use glam::{IVec3, Mat4, Vec3};

const WIDTH: u32 = 32;
const HEIGHT: u32 = 8;
const ROW_BYTES: u32 = 256;

fn transform(x: f32) -> DrawTransform {
    DrawTransform { offset: [x, 0.0, 0.0, 0.0], ..DrawTransform::IDENTITY }
}

/// Draws the quad at slots 1 and 2 and returns the depth row through its centre.
fn depth_row(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &ShadowRenderer,
    offsets: &wgpu::Buffer,
    indirect: Option<&wgpu::Buffer>,
) -> Vec<f32> {
    // Ortho over x in [-16, 16] looking down -Y with screen-right = +X: one texel per unit.
    let matrix = Mat4::orthographic_rh(-16.0, 16.0, -4.0, 4.0, 0.1, 40.0)
        * Mat4::look_to_rh(Vec3::new(0.0, 20.0, 0.0), -Vec3::Y, -Vec3::Z);
    let (pipeline, bind_group) = renderer.test_cascade(queue, matrix);
    let vertices = [[-2.0, 0.0, -2.0], [-2.0, 0.0, 2.0], [2.0, 0.0, 2.0], [2.0, 0.0, -2.0]].map(|pos| Vertex3 {
        pos,
        color: [0.5; 3],
        normal: [0.0, 1.0, 0.0],
        surface: [0.9, 0.05, 3.0, 4.0],
        ao_strength: 1.0,
    });
    let buffer = |label, size, usage| {
        device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
    };
    let vertex_buffer = buffer("offset quad", std::mem::size_of_val(&vertices) as u64,
        wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
    queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices));
    let index_buffer = buffer("offset quad indices", 24, wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST);
    queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&[0u32, 1, 2, 0, 2, 3]));
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offset depth"),
        size: wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let readback = buffer("offset depth readback", u64::from(ROW_BYTES * HEIGHT),
        wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("draw offset first instance"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &view,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, offsets.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        match indirect {
            Some(args) => pass.multi_draw_indexed_indirect(args, 0, 2),
            None => {
                pass.draw_indexed(0..6, 0, 1..2);
                pass.draw_indexed(0..6, 0, 2..3);
            }
        }
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
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ROW_BYTES), rows_per_image: Some(HEIGHT) },
        },
        wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
    );
    let submission = queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = sender.send(r);
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(10)),
        })
        .unwrap();
    receiver.recv_timeout(std::time::Duration::from_secs(1)).unwrap().unwrap();
    let mapped = readback.slice(..).get_mapped_range();
    let row = (HEIGHT / 2 * ROW_BYTES) as usize;
    let depths = bytemuck::cast_slice::<u8, f32>(&mapped[row..row + WIDTH as usize * 4]).to_vec();
    drop(mapped);
    readback.unmap();
    depths
}

fn assert_offset_columns(depths: &[f32], path: &str) {
    // Texel column c covers x in [c - 16, c - 15].
    let column = |x: i32| depths[(x + 16) as usize];
    assert!(column(8) < 0.9, "{path}: no depth at x = +8: {depths:?}");
    assert!(column(-8) < 0.9, "{path}: no depth at x = -8: {depths:?}");
    assert_eq!(column(0), 1.0, "{path}: depth at x = 0 means slot 0 was drawn: {depths:?}");
}

#[test]
#[ignore = "native GPU depth readback; run with just test-gpu"]
fn gpu_draw_offset_first_instance() {
    let adapter = engine_gpu::test_adapter();
    let features = adapter.features() & wgpu::Features::INDIRECT_FIRST_INSTANCE;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: features,
        ..Default::default()
    }))
    .unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("draw offset contract"),
        source: wgpu::ShaderSource::Wgsl(crate::shader_sources::SHADOW_SHADER.into()),
    });
    let renderer = ShadowRenderer::new(&device, &shader);
    let mut offsets = DrawOffsets::default();
    offsets.begin_frame(DrawTransform::IDENTITY);
    assert_eq!(offsets.slot(IVec3::X, || transform(8.0)), 1);
    assert_eq!(offsets.slot(IVec3::NEG_X, || transform(-8.0)), 2);
    offsets.upload(&device, &queue);
    let table = offsets.buffer().unwrap();

    assert_offset_columns(&depth_row(&device, &queue, &renderer, table, None), "direct");

    if features.is_empty() {
        println!("[draw-offset] adapter lacks INDIRECT_FIRST_INSTANCE; direct path only");
        return;
    }
    let args: Vec<u8> = [1u32, 2]
        .iter()
        .flat_map(|&first_instance| {
            wgpu::util::DrawIndexedIndirectArgs {
                index_count: 6,
                instance_count: 1,
                first_index: 0,
                base_vertex: 0,
                first_instance,
            }
            .as_bytes()
            .to_vec()
        })
        .collect();
    let indirect = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("draw offset indirect"),
        size: args.len() as u64,
        usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&indirect, 0, &args);
    assert_offset_columns(&depth_row(&device, &queue, &renderer, table, Some(&indirect)), "multi-draw indirect");
    println!("[draw-offset] direct and multi-draw indirect read slots 1 and 2 on {:?}", adapter.get_info());
}
