//! Opt-in art export through the production egui/wgpu pipelines; no saves or window.
use anyhow::Result;
use std::path::Path;

pub(crate) async fn run(root: &Path) -> Result<()> {
    std::fs::create_dir_all(root)?;
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = egui_wgpu::Renderer::new(&device, format, Default::default());
    super::prism::install(&mut renderer, &device, format);
    super::magic_fx::install(&mut renderer, &device, format);
    super::menu_scene::install(&mut renderer, &device, format);
    super::world_loader::install(&mut renderer, &device, format);

    let filter = std::env::var("VOXEL_MENU_CAPTURE_FILTER").ok();
    if filter.as_deref().is_none_or(|f| "world-load".starts_with(f) || f.starts_with("world-load")) {
        super::world_loader::preview::prewarm();
    }
    let run_started = std::time::Instant::now();
    for (label, width, height, seconds, loading, theme) in [
        ("tools-build", 1100, 800, 8.0, false, 1),
        ("tools-service", 1100, 800, 8.0, false, 1),
        ("tools-compact", 640, 720, 8.0, false, 1),
        ("title", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-later", 1600, 900, 24.0, false, super::themes::DEFAULT),
        ("title-focused", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-focused-later", 1600, 900, 8.5, false, super::themes::DEFAULT),
        ("title-still", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-still-later", 1600, 900, 24.0, false, super::themes::DEFAULT),
        ("title-night", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-dawn", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-day", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-river", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-river-night", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-river-day", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-high", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-high-dawn", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-high-night", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-page", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("title-compact", 640, 480, 8.0, false, super::themes::DEFAULT),
        ("title-narrow", 360, 640, 8.0, false, super::themes::DEFAULT),
        ("title-fresh", 640, 480, 8.0, false, super::themes::DEFAULT),
        ("title-long-world", 640, 480, 8.0, false, super::themes::DEFAULT),
        ("title-busy", 640, 480, 8.0, false, super::themes::DEFAULT),
        ("front-new-world", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("front-settings-audio", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("front-settings-controls", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("front-settings-graphics", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("front-multiplayer", 1600, 900, 8.0, false, super::themes::DEFAULT),
        ("multiplayer", 1280, 720, 8.0, false, super::themes::DEFAULT),
        ("multiplayer-compact", 640, 480, 8.0, false, super::themes::DEFAULT),
        ("loading", 1600, 900, 8.0, true, 0),
        ("world-load", 1600, 900, 8.0, false, 0),
        ("world-load-stream", 1600, 900, 8.0, false, 0),
        ("world-load-planet-stream", 1600, 900, 8.0, false, 0),
        ("world-load-planet-pole", 1600, 900, 8.0, false, 0),
        ("world-load-planet-seam", 1600, 900, 8.0, false, 0),
        ("world-load-solved", 1600, 900, 8.0, false, 0),
        ("world-load-survey-stream", 1600, 900, 8.0, false, 0),
        ("world-load-compact", 640, 720, 8.0, false, 0),
        ("world-load-planet-map", 1600, 900, 8.0, false, 0),
        ("world-load-history-0", 1600, 900, 8.0, false, 0),
        ("world-load-history-1", 1600, 900, 8.0, false, 0),
        ("world-load-history-2", 1600, 900, 8.0, false, 0),
        ("world-load-history-3", 1600, 900, 8.0, false, 0),
        ("world-load-history-4", 1600, 900, 8.0, false, 0),
        ("theme-minecraft", 1600, 900, 8.0, false, 1),
        ("theme-noita", 1600, 900, 8.0, false, 5),
        ("theme-ftl", 1600, 900, 8.0, false, 9),
        ("theme-mgsv", 1600, 900, 8.0, false, 13),
        ("theme-prism", 1600, 900, 8.0, false, 17),
        ("theme-aurora", 1600, 900, 8.0, false, 18),
        ("theme-amethyst", 1600, 900, 8.0, false, 19),
        ("theme-brass", 1600, 900, 8.0, false, 20),
        ("theme-aurora-compact", 640, 480, 8.0, false, 18),
        ("theme-palettes", 900, 1100, 8.0, false, 17),
        ("prism-inventory", 1200, 760, 8.0, false, 17),
        ("prism-inventory-later", 1200, 760, 11.0, false, 17),
        ("prism-inventory-still", 1200, 760, 8.0, false, 17),
        ("prism-inventory-still-later", 1200, 760, 11.0, false, 17),
    ] {
        if filter.as_deref().is_some_and(|filter| !label.starts_with(filter)) {continue;}
        let label_started = std::time::Instant::now();
        let ctx = egui::Context::default();
        super::menu_scene::force(
            if label.contains("night") { 1 } else if label.contains("dawn") { 2 }
                else if label.contains("day") { 3 } else { 0 },
            if label.contains("river") { 1 } else if label.contains("high") { 2 } else { 0 });
        super::presentation::apply_theme(&ctx);
        super::themes::set(&ctx, theme);
        if label.contains("-still") {
            let mut style = super::magic_style::get(&ctx);
            style.reduced = true;
            super::magic_style::set(&ctx, style);
        }
        super::themes::apply(&ctx);
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
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width as f32, height as f32),
                    )),
                    time: Some(seconds + frame as f64 * 0.016),
                    events: if label.starts_with("title-focused") || label.starts_with("title-still") {
                        let target = ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("title-primary-rect")));
                        target.map(|r| vec![egui::Event::PointerMoved(r.center())]).unwrap_or_default()
                    } else { Vec::new() },
                    ..Default::default()
                },
                |root| {
                    let ctx = &root.ctx().clone();
                    super::prism::begin_frame(ctx);
                    super::magic_fx::begin_frame(ctx);
                    if label.starts_with("tools-") {
                        egui::CentralPanel::default().show(root,|ui| {
                            egui::ScrollArea::vertical().show(ui,|ui|super::tool_station::preview(ui,label=="tools-service"));
                        });
                    } else if label.starts_with("title-page") {
                        // A home page over the dimmed vista, as `front_end::home` frames it.
                        super::front_end::backdrop(ctx);
                        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                            ui.add_space(60.0);
                            ui.vertical_centered(|ui| {
                                ui.set_max_width(720.0);
                                super::front_end::surface(ui.ctx()).show(ui, |ui| {
                                    super::front_end::heading(ui, "Settings", "");
                                    super::themes::controls(ui);
                                });
                            });
                        });
                    } else if let Some(page) = label.strip_prefix("front-") {
                        let tab = match page { "settings-controls" => 1, "settings-graphics" => 2, _ => 0 };
                        super::front_end::preview(root, page, tab);
                    } else if label.starts_with("multiplayer") {
                        super::online_browser::preview(root);
                    } else if label.starts_with("prism-inventory") {
                        egui::CentralPanel::default().show(root, |ui| {
                            super::prism::surface(ui.painter(), ui.max_rect());
                            ui.add_space(60.0);
                            ui.horizontal(|ui| {
                                ui.add_space(100.0);
                                ui.vertical(|ui| super::prism::showcase(ui));
                            });
                        });
                    } else if label == "theme-palettes" {
                        egui::CentralPanel::default().show(root, |ui| {
                            ui.set_max_width(540.0);
                            super::themes::quick_switch(ui);
                            super::themes::gallery(ui);
                        });
                    } else if let Some((report, myr)) = super::world_loader::preview::history_report(label) {
                        let now = seconds + frame as f64 * 0.016;
                        super::world_loader::preview::seed_clocks(ctx, &report, now);
                        super::world_loader::preview::pin_history(ctx, &report, myr, now);
                        super::world_loader::draw(ctx, &report);
                    } else if label == "world-load-planet-map" {
                        super::world_loader::preview::planet_map(ctx);
                    } else if label.starts_with("world-load") {
                        let report = super::world_loader::preview::report(label);
                        super::world_loader::preview::seed_clocks(ctx, &report, seconds + frame as f64 * 0.016);
                        super::world_loader::draw(ctx, &report);
                    } else if loading {
                        super::loading_screen::draw(
                            ctx,
                            &super::LoadingUiState {
                                title: "Loading terrain".into(),
                                progress_01: None,
                            },
                        );
                    } else {
                        let recent = if label == "title-fresh" { None }
                            else if label == "title-long-world" { Some("The Copper Valley — Northern Observatory and Riverside Workshop") }
                            else { Some("Workshop") };
                        super::main_menu::draw(root, recent.map(|name| (name, "Wildlands · 2 h ago")), label == "title-busy", None);
                    }
                },
            );
            for (id, deltas) in output.textures_delta.set.drain() {
                for mut delta in deltas {
                    // The loading globe uses smooth filtering.
                    if !label.starts_with("world-load") {
                        delta.options = egui::TextureOptions::NEAREST;
                    }
                    renderer.update_texture(&device, &queue, id, &delta);
                }
            }
            let shapes = ctx.tessellate(output.shapes, 1.0);
            let mut encoder = device.create_command_encoder(&Default::default());
            let commands = renderer.update_buffers(&device, &queue, &mut encoder, &shapes, &screen);
            {
                let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    multiview_mask: None,
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
            for id in output.textures_delta.free.drain() {
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
        let data = buffer.slice(..).get_mapped_range()?;
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
        println!("[menu-capture] {label} {width}x{height} {} ms", label_started.elapsed().as_millis());
    }
    println!("[menu-capture] total {:.1} s", run_started.elapsed().as_secs_f64());
    Ok(())
}
