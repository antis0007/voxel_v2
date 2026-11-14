//! Headless timing of the production weather sampler; no window or image output.
//! cargo run --offline --profile release-fast -p minecraft_style_voxel_prototype
//!   --example weather_filter_profile -- --width=1920
//! Reports component timing and numeric differences, not whole-game frame rate.
use anyhow::Result;

#[allow(dead_code)]
#[path = "../src/shader_sources.rs"]
mod shader_sources;

#[path = "support/kernel_profile.rs"]
mod kernel_profile;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

fn main() -> Result<()> {
    pollster::block_on(run())
}

async fn run() -> Result<()> {
    let width = std::env::args()
        .find_map(|a| a.strip_prefix("--width=").and_then(|s| s.parse().ok()))
        .unwrap_or(1920u32)
        .clamp(256, 7680);
    let height = width * 9 / 16;
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await?;
    let features = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::FLOAT32_FILTERABLE;
    anyhow::ensure!(
        adapter.features().contains(features),
        "adapter lacks float32 filtering or GPU timestamps"
    );
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features: features,
            ..Default::default()
        })
        .await?;
    println!(
        "adapter={:?} size={width}x{height} samples=64",
        adapter.get_info()
    );

    let field = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("128px weather clip levels"),
        size: wgpu::Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 3,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        view_formats: &[],
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    });
    // Smooth, nonconstant optical-depth fields, with different values at opposite
    // edges. Coordinates below also exercise the production clamp-to-centre rule.
    let data: Vec<[f32; 4]> = (0..3 * 128 * 128)
        .map(|i| {
            let x = (i % 128) as f32;
            let y = (i / 128 % 128) as f32;
            let layer = (i / (128 * 128)) as f32;
            std::array::from_fn(|c| {
                let phase = c as f32 * 1.3 + layer * 0.7;
                (0.6 + (x * 0.071 + phase).sin() * 0.3 + (y * 0.11 - phase).cos() * 0.2)
                    * (1.0 + c as f32 * 0.25)
            })
        })
        .collect();
    queue.write_texture(
        field.as_image_copy(),
        bytemuck::cast_slice(&data),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(128 * 16),
            rows_per_image: Some(128),
        },
        field.size(),
    );
    let field_view = field.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        ..Default::default()
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let inputs = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&field_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipelines: Vec<_> = [false, true].into_iter().map(|hardware| {
        let source = format!(r#"
            @group(0) @binding(0) var field: texture_2d_array<f32>;
            @group(0) @binding(1) var cloud_field_sampler: sampler;
            {}
            @vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {{
                let p=array<vec2<f32>,3>(vec2<f32>(-1,-1),vec2<f32>(3,-1),vec2<f32>(-1,3));
                return vec4<f32>(p[i],0,1);
            }}
            @fragment fn fs(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32> {{
                let uv=p.xy/vec2<f32>({width}.0,{height}.0)*1.2-0.1;
                let low=sample_weather_bilinear(field,uv+vec2<f32>(0.013,0.031),0).x;
                let mid=sample_weather_bilinear(field,uv+vec2<f32>(-0.021,0.057),1).y;
                let high=sample_weather_bilinear(field,uv+vec2<f32>(0.041,-0.017),2).z;
                let local=sample_weather_bilinear(field,uv,0);
                return vec4<f32>(exp(-(low+mid+high)/0.7), 1.0-exp(-dot(local.xyz,vec3<f32>(1.0))),low,high);
            }}
        "#, shader_sources::weather_sampling_source(hardware));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(source.into()) });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(if hardware { "hardware weather" } else { "four-tap weather" }), layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            depth_stencil: None, primitive: Default::default(), multisample: Default::default(), multiview_mask: None, cache: None,
        })
    }).collect();
    kernel_profile::compare(
        &device,
        &queue,
        width,
        height,
        &pipelines,
        Some(&inputs),
        ["four-tap", "hardware"],
    )
    .await
}
