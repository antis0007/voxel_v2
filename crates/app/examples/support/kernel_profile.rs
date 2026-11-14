//! Shared paired, interleaved GPU kernel profiler. No window or image output.
use anyhow::Result;
const SAMPLES: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

pub async fn compare(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    pipelines: &[wgpu::RenderPipeline],
    inputs: Option<&wgpu::BindGroup>,
    labels: [&str; 2],
) -> Result<()> {
    let groups: Vec<_> = inputs.into_iter().collect();
    compare_with_readback(
        device,
        queue,
        width,
        height,
        pipelines,
        &groups,
        labels,
        |_, _| {},
    )
    .await
}

pub async fn compare_with_readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    pipelines: &[wgpu::RenderPipeline],
    inputs: &[&wgpu::BindGroup],
    labels: [&str; 2],
    mut inspect: impl FnMut(usize, &[f32]),
) -> Result<()> {
    let targets: Vec<_> = (0..2)
        .map(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        })
        .collect();
    let views: Vec<_> = targets
        .iter()
        .map(|t| t.create_view(&Default::default()))
        .collect();
    let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: None,
        ty: wgpu::QueryType::Timestamp,
        count: SAMPLES * 4,
    });
    for warmup in [true, false] {
        let mut encoder = device.create_command_encoder(&Default::default());
        for i in 0..SAMPLES {
            // Interleave both variants and reverse their order on alternate pairs.
            for order in 0..2u32 {
                let variant = (i + order) % 2;
                let query = (i * 2 + variant) * 2;
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    multiview_mask: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &views[variant as usize],
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: (!warmup).then_some(wgpu::RenderPassTimestampWrites {
                        query_set: &queries,
                        beginning_of_pass_write_index: Some(query),
                        end_of_pass_write_index: Some(query + 1),
                    }),
                });
                pass.set_pipeline(&pipelines[variant as usize]);
                if let Some(inputs) = inputs.get(variant as usize).or_else(|| inputs.first()) {
                    pass.set_bind_group(0, *inputs, &[]);
                }
                pass.draw(0..3, 0..1);
            }
        }
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::wait_indefinitely())?;
    }
    let query_bytes = u64::from(SAMPLES) * 4 * 8;
    let gpu = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: query_bytes,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: query_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.resolve_query_set(&queries, 0..SAMPLES * 4, &gpu, 0);
    encoder.copy_buffer_to_buffer(&gpu, 0, &read, 0, query_bytes);
    queue.submit([encoder.finish()]);
    read.slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.expect("timing readback"));
    device.poll(wgpu::PollType::wait_indefinitely())?;
    let times = read.slice(..).get_mapped_range()?;
    let pairs = bytemuck::cast_slice::<u8, u64>(&times);
    for variant in 0..2usize {
        let mut ms: Vec<_> = pairs
            .chunks_exact(4)
            .map(|p| {
                (p[variant * 2 + 1] - p[variant * 2]) as f64
                    * f64::from(queue.get_timestamp_period())
                    / 1e6
            })
            .collect();
        ms.sort_by(f64::total_cmp);
        println!(
            "variant={} median_ms={:.6} p95_ms={:.6}",
            labels[variant], ms[32], ms[60]
        );
    }
    let mut reference = Vec::<f32>::new();
    let row = (width * 16).div_ceil(256) * 256;
    for (variant, target) in targets.iter().enumerate() {
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
            .map_async(wgpu::MapMode::Read, |r| r.expect("numeric readback"));
        device.poll(wgpu::PollType::wait_indefinitely())?;
        let bytes = read.slice(..).get_mapped_range()?;
        let values: Vec<f32> = bytes
            .chunks_exact(row as usize)
            .flat_map(|r| {
                bytemuck::cast_slice::<u8, f32>(&r[..width as usize * 16])
                    .iter()
                    .copied()
            })
            .collect();
        anyhow::ensure!(
            values.iter().all(|v| v.is_finite()),
            "non-finite shader output"
        );
        inspect(variant, &values);
        if variant == 0 {
            reference = values;
        } else {
            anyhow::ensure!(
                reference.iter().chain(&values).all(|v| v.is_finite()),
                "non-finite shader output"
            );
            let mut max_error = 0.0f32;
            let mut total = 0.0f64;
            for (&a, &b) in reference.iter().zip(&values) {
                let e = (a - b).abs();
                max_error = max_error.max(e);
                total += f64::from(e);
            }
            println!(
                "numeric_comparison samples={} max_abs_error={max_error:.9} mean_abs_error={:.9}",
                values.len(),
                total / values.len() as f64
            );
        }
    }
    Ok(())
}
