//! Opt-in art export through the production egui/wgpu pipelines; no saves or window.
use anyhow::Result;
use std::path::Path;

pub(crate) async fn run(root: &Path) -> Result<()> {
    std::fs::create_dir_all(root)?;
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = egui_wgpu::Renderer::new(&device, format, Default::default());
    super::menu_scene::install(&mut renderer, &device, format);

    for (label, width, height, seconds, loading) in [
        ("title", 1600, 900, 8.0, false),
        ("title-later", 1600, 900, 24.0, false),
        ("title-compact", 640, 480, 8.0, false),
        ("loading", 1600, 900, 8.0, true),
    ] {
        let ctx = egui::Context::default();
        super::presentation::apply_theme(&ctx);
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Menu art export"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: 1.0,
        };
        // Resolve egui's initial layout/font pass before exporting the image.
        for frame in 0..2 {
            let mut output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width as f32, height as f32),
                    )),
                    time: Some(seconds + frame as f64 * 0.016),
                    ..Default::default()
                },
                |ctx| {
                    if loading {
                        super::loading_screen::draw(
                            ctx,
                            &super::LoadingUiState {
                                title: "Loading terrain".into(),
                                progress_01: None,
                            },
                        );
                    } else {
                        super::main_menu::draw(ctx, Some("Workshop"), false, None);
                    }
                },
            );
            for (id, delta) in &mut output.textures_delta.set {
                delta.options = egui::TextureOptions::NEAREST;
                renderer.update_texture(&device, &queue, *id, delta);
            }
            let shapes = ctx.tessellate(output.shapes, 1.0);
            let mut encoder = device.create_command_encoder(&Default::default());
            let commands = renderer.update_buffers(&device, &queue, &mut encoder, &shapes, &screen);
            {
                let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Menu art export"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                renderer.render(&mut pass.forget_lifetime(), &shapes, &screen);
            }
            queue.submit(
                commands
                    .into_iter()
                    .chain(std::iter::once(encoder.finish())),
            );
            for id in output.textures_delta.free {
                renderer.free_texture(&id);
            }
        }
        let row = (width * 4).div_ceil(256) * 256;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Menu export readback"),
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            size,
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        })?;
        rx.recv_timeout(std::time::Duration::from_secs(30))??;
        let data = buffer.slice(..).get_mapped_range();
        let pixels: Vec<u8> = data
            .chunks(row as usize)
            .flat_map(|r| r[..width as usize * 4].iter().copied())
            .collect();
        image::save_buffer(
            root.join(format!("{label}.png")),
            &pixels,
            width,
            height,
            image::ColorType::Rgba8,
        )?;
        drop(data);
        buffer.unmap();
        println!("[menu-capture] {label} {width}x{height}");
    }
    Ok(())
}
