//! Deterministic visual/performance capture of the production resolve, without a world.
//! cargo run --offline --profile release-fast -p minecraft_style_voxel_prototype
//!   --example postprocess_capture -- --output=target/resolve.png [--benchmark-reference-resolve]
//! --no-image reports numeric sharpness and timings without producing an image.
//! --raw=path / --compare-raw=path support numeric A/B; GPU times cover only resolve + presentation.
use anyhow::Result;

#[path = "../src/render/postprocess.rs"]
mod postprocess;
mod shader_sources {
    pub const POSTPROCESS_SHADER: &str = concat!(
        include_str!("../src/shaders/color_shared.wgsl"),
        "\n",
        include_str!("../src/shaders/postprocess.wgsl")
    );
    pub const PRESENT_SHADER: &str = concat!(
        include_str!("../src/shaders/color_shared.wgsl"),
        "\n",
        include_str!("../src/shaders/present.wgsl")
    );
}
fn scene_dims(w: u32, h: u32) -> (u32, u32) {
    let percent = std::env::args()
        .find_map(|a| {
            a.strip_prefix("--scene-percent=")
                .and_then(|v| v.parse::<u32>().ok())
        })
        .unwrap_or(100)
        .clamp(50, 100);
    ((w * percent + 50) / 100, (h * percent + 50) / 100)
}

fn main() -> Result<()> {
    pollster::block_on(run())
}

async fn run() -> Result<()> {
    let output = std::env::args()
        .find_map(|a| a.strip_prefix("--output=").map(str::to_owned))
        .unwrap_or_else(|| "target/postprocess.png".into());
    let width = std::env::args()
        .find_map(|a| a.strip_prefix("--width=").and_then(|v| v.parse().ok()))
        .unwrap_or(1920u32);
    let height = std::env::args()
        .find_map(|a| a.strip_prefix("--height=").and_then(|v| v.parse().ok()))
        .unwrap_or(width * 9 / 16);
    anyhow::ensure!(
        width >= 8 && height >= 8 && width <= 7680 && height <= 4320,
        "invalid extent"
    );
    let (scene_width, scene_height) = scene_dims(width, height);
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await?;
    let timed = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features: if timed {
                wgpu::Features::TIMESTAMP_QUERY
            } else {
                wgpu::Features::empty()
            },
            ..Default::default()
        })
        .await?;
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        color_space: Default::default(),
    };
    let texture = |name, w, h, format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let noise = texture(
        "fixed write noise",
        64,
        64,
        wgpu::TextureFormat::R8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    // Identical zero-mean input in both runs; this fixture measures the write,
    // not the spectral quality of the game's separately authored blue noise.
    let noise_bytes: Vec<u8> = (0..4096u32)
        .map(|i| {
            let h = i.wrapping_mul(747796405).wrapping_add(2891336453);
            ((h ^ (h >> 16)) & 255) as u8
        })
        .collect();
    queue.write_texture(
        noise.as_image_copy(),
        &noise_bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(64),
            rows_per_image: Some(64),
        },
        noise.size(),
    );
    let mut resolve = postprocess::PostProcess::new(
        &device,
        &config,
        &noise.create_view(&Default::default()),
        None,
    );
    let depth = texture(
        "fixture depth",
        scene_width,
        scene_height,
        wgpu::TextureFormat::Depth32Float,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let depth_view = depth.create_view(&Default::default());
    let target = texture(
        "resolved capture",
        width,
        height,
        config.format,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let target_view = target.create_view(&Default::default());
    let fixture = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("HDR ramps, fine edges, emissive and reverse-Z steps"),
        source: wgpu::ShaderSource::Wgsl(
            format!(
                r#"
        @vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {{
            let p=array<vec2<f32>,3>(vec2<f32>(-1,-1),vec2<f32>(3,-1),vec2<f32>(-1,3));
            return vec4<f32>(p[i],0.5,1);
        }}
        struct Out {{ @location(0) color:vec4<f32>, @builtin(frag_depth) depth:f32 }};
        @fragment fn fs(@builtin(position) p:vec4<f32>)->Out {{
            let uv=p.xy/vec2<f32>({scene_width}.0,{scene_height}.0);
            var c=mix(vec3<f32>(0.0001,0.0003,0.001),vec3<f32>(1.5,0.8,0.4),uv.x*uv.x);
            if uv.y<0.35 {{c=mix(vec3<f32>(0.04,0.14,0.3),vec3<f32>(0.8,1.2,2),uv.x);}}
            if uv.y>0.7 {{c=vec3<f32>(0.15,0.55,0.2)*select(0.2,1.0,(u32(p.x)&1u)==0u);}}
            if distance(uv,vec2<f32>(0.72,0.55))<0.025 {{c=vec3<f32>(12,3,0.4);}}
            let z=select(0.02,0.08,fract(uv.x*12.0)<0.5);
            return Out(vec4<f32>(c,1),select(z,0.0,uv.y<0.35));
        }}"#
            )
            .into(),
        ),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fixture"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &fixture,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &fixture,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: postprocess::SCENE_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        primitive: Default::default(),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("fixture"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &resolve.scene,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    queue.submit([encoder.finish()]);
    let mut settings = [0.0; 32];
    settings[..12].copy_from_slice(&[
        1.1, 1.0, 1.0, 0.0, 1.0, 0.85, 0.0, 0.0, 0.26, 0.72, 12.0, 0.06,
    ]);
    settings[27] = 1.0;
    settings[29] = 1.0;
    if std::env::args().any(|a| a == "--pixel-grid") {
        // Isolate source pixel placement; contour, AA, bloom, vignette and AO
        // are deliberately disabled, while the production grade/write remain.
        settings[2] = 0.0;
        settings[8] = 0.0;
        settings[11] = 0.0;
        settings[29] = 0.0;
    }
    if let Some(scale) = std::env::args().find_map(|a| {
        a.strip_prefix("--art-scale=")
            .and_then(|v| v.parse::<f32>().ok())
    }) {
        anyhow::ensure!(
            scale.is_finite() && (1.0..=8.0).contains(&scale),
            "invalid art scale"
        );
        settings[4] = scale;
    }
    if std::env::args().any(|a| a == "--magic") {
        settings[12..16].copy_from_slice(&[0.3, 0.7, 1.0, 0.65]);
        settings[20..28].copy_from_slice(&[0.4, 0.2, 0.6, 0.3, 0.1, 0.7, 0.4, 1.0]);
        settings[28] = 0.4;
    }
    if std::env::args().any(|a| a == "--debug") {
        settings[3] = 1.0;
    }
    let queries = timed.then(|| {
        device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("resolve measurement"),
            ty: wgpu::QueryType::Timestamp,
            count: 192,
        })
    });
    for warmup in [true, false] {
        let mut encoder = device.create_command_encoder(&Default::default());
        for i in 0..96 {
            let stamps =
                queries
                    .as_ref()
                    .filter(|_| !warmup)
                    .map(|q| wgpu::RenderPassTimestampWrites {
                        query_set: q,
                        beginning_of_pass_write_index: Some(i * 2),
                        end_of_pass_write_index: Some(i * 2 + 1),
                    });
            resolve.render(
                &device,
                &queue,
                &mut encoder,
                &target_view,
                &depth_view,
                settings,
                stamps,
            );
        }
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::wait_indefinitely())?;
    }
    if let Some(queries) = queries {
        let gpu = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 1536,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 1536,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.resolve_query_set(&queries, 0..192, &gpu, 0);
        encoder.copy_buffer_to_buffer(&gpu, 0, &read, 0, 1536);
        queue.submit([encoder.finish()]);
        read.slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.expect("timing readback"));
        device.poll(wgpu::PollType::wait_indefinitely())?;
        let data = read.slice(..).get_mapped_range()?;
        let mut ms: Vec<f64> = bytemuck::cast_slice::<u8, u64>(&data)
            .chunks_exact(2)
            .map(|v| (v[1] - v[0]) as f64 * queue.get_timestamp_period() as f64 / 1e6)
            .collect();
        ms.sort_by(f64::total_cmp);
        println!(
            "adapter={:?} size={}x{} median_ms={:.6} p95_ms={:.6}",
            adapter.get_info(),
            width,
            height,
            ms[48],
            ms[91]
        );
    }
    let row = (width * 4).div_ceil(256) * 256;
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &read,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        target.size(),
    );
    queue.submit([encoder.finish()]);
    read.slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.expect("image readback"));
    device.poll(wgpu::PollType::wait_indefinitely())?;
    let data = read.slice(..).get_mapped_range()?;
    let pixels: Vec<u8> = data
        .chunks_exact(row as usize)
        .flat_map(|r| r[..(width * 4) as usize].iter().copied())
        .collect();
    if !std::env::args().any(|a| a == "--no-image") {
        image::save_buffer(output, &pixels, width, height, image::ColorType::Rgba8)?;
    }
    if let Some(path) = std::env::args().find_map(|a| a.strip_prefix("--raw=").map(str::to_owned)) {
        std::fs::write(path, &pixels)?;
    }
    let mut stripe_delta = 0i64;
    let mut stripe_channels = 0u64;
    for y in height * 4 / 5..height * 9 / 10 {
        for x in (((width / 4 + 1) & !1)..(width * 3 / 4 - 1)).step_by(2) {
            let i = ((y * width + x) * 4) as usize;
            for channel in 0..3 {
                stripe_delta += i64::from(pixels[i + channel]) - i64::from(pixels[i + 4 + channel]);
                stripe_channels += 1;
            }
        }
    }
    println!(
        "fine_stripe_mean_code_contrast={:.6}",
        stripe_delta as f64 / stripe_channels.max(1) as f64
    );
    let reference = if let Some(path) =
        std::env::args().find_map(|a| a.strip_prefix("--compare-raw=").map(str::to_owned))
    {
        Some(std::fs::read(path)?)
    } else if let Some(path) =
        std::env::args().find_map(|a| a.strip_prefix("--compare=").map(str::to_owned))
    {
        let reference = image::open(path)?.to_rgba8();
        anyhow::ensure!(
            reference.dimensions() == (width, height),
            "capture dimensions differ"
        );
        Some(reference.into_raw())
    } else {
        None
    };
    if let Some(reference) = reference {
        anyhow::ensure!(reference.len() == pixels.len(), "raw extent differs");
        let mut changed = 0usize;
        let mut max_code = 0u8;
        let mut total = 0u64;
        for (a, b) in pixels.chunks_exact(4).zip(reference.chunks_exact(4)) {
            let delta: [u8; 3] = std::array::from_fn(|i| a[i].abs_diff(b[i]));
            changed += usize::from(delta.iter().any(|d| *d > 0));
            for d in delta {
                max_code = max_code.max(d);
                total += u64::from(d);
            }
        }
        println!("comparison pixels={} changed_pixels={} max_code_difference={} mean_code_difference={:.9}",
            width*height,changed,max_code,total as f64/f64::from(width*height*3));
    }
    Ok(())
}
