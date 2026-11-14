//! Production cloud density on a fixed 8 km volume, numeric readback only.
//! --reference=path/to/old-morphology.wgsl [--footprint=2] [--time=125] [--planet]
//! Four weather cases share noise and positions; no image or window is produced.
use anyhow::{Context, Result};

#[path = "../src/render/cloud_noise.rs"]
mod cloud_noise;
#[path = "support/kernel_profile.rs"]
mod kernel_profile;

fn function(source: &str, name: &str) -> String {
    let start = source
        .find(&format!("fn {name}("))
        .expect("production function");
    let body = start + source[start..].find('{').expect("function body");
    let mut depth = 0;
    for (offset, c) in source[body..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return source[start..body + offset + 1].to_owned();
                }
            }
            _ => {}
        }
    }
    panic!("unterminated function {name}")
}

fn main() -> Result<()> {
    pollster::block_on(run())
}

async fn run() -> Result<()> {
    let reference_path = std::env::args()
        .find_map(|a| a.strip_prefix("--reference=").map(str::to_owned))
        .context("supply --reference=old-morphology.wgsl")?;
    let reference = std::fs::read_to_string(reference_path)?;
    let footprint = std::env::args()
        .find_map(|a| {
            a.strip_prefix("--footprint=")
                .and_then(|v| v.parse::<f32>().ok())
        })
        .unwrap_or(2.0);
    let time = std::env::args()
        .find_map(|a| {
            a.strip_prefix("--time=")
                .and_then(|v| v.parse::<f32>().ok())
        })
        .unwrap_or(125.0);
    anyhow::ensure!(
        footprint.is_finite() && footprint > 0.0 && time.is_finite(),
        "invalid sampling inputs"
    );
    let planet = if std::env::args().any(|a| a == "--planet") {
        1.0
    } else {
        0.0
    };
    let instance = wgpu::Instance::default();
    let adapter = instance.request_adapter(&Default::default()).await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::TIMESTAMP_QUERY,
            ..Default::default()
        })
        .await?;
    let shape = cloud_noise::upload(&device, &queue, cloud_noise::gen_shape_noise_data(), 64);
    let detail = cloud_noise::upload(&device, &queue, cloud_noise::gen_detail_noise_data(), 32);
    let shape_view = shape.create_view(&Default::default());
    let detail_views = [
        detail.create_view(&wgpu::TextureViewDescriptor {
            mip_level_count: Some(1),
            ..Default::default()
        }),
        detail.create_view(&Default::default()),
    ];
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        min_filter: wgpu::FilterMode::Linear,
        mag_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        ..Default::default()
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("production cloud noise"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let groups: Vec<_> = detail_views
        .iter()
        .map(|view| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&shape_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
        })
        .collect();
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let cloud = include_str!("../src/shaders/cloud_shader.wgsl");
    let atmosphere = include_str!("../src/shaders/atmosphere_shared.wgsl");
    let mut helpers = String::new();
    for name in [
        "saturate1",
        "rotate2",
        "safe_normalize2",
        "hash_cell1",
        "hash_cell2",
    ] {
        helpers.push_str(&function(atmosphere, name));
        helpers.push('\n');
    }
    for name in [
        "cloud_layer_speed_mult",
        "stable_world_anchor_xz",
        "stable_world_anchor_y",
        "stable_noise_uvw",
        "stable_noise_uvw_scaled",
        "sample_shape_global",
        "sample_detail_global",
        "footprint_texel_lod",
        "layer_wind_advection_offset",
        "authored_cloud_density",
        "authored_anvil_spread",
    ] {
        helpers.push_str(&function(cloud, name));
        helpers.push('\n');
    }
    helpers.push_str(&function(
        include_str!("../src/shaders/cloud_shared.wgsl"),
        "cloud_horizontal_scale",
    ));
    helpers.push_str(include_str!("../src/shaders/cloud_flow_shared.wgsl"));
    let header = r#"
        struct ProbeLight { weather_globals: array<vec4<f32>,4>, cloud_authoring: array<vec4<f32>,2> };
        struct ProbeCam { planet: array<vec4<f32>,1> };
        var<private> light: ProbeLight;
        var<private> cam: ProbeCam;
        struct CellWeather { coverage:f32, cloud_type:f32, humidity:f32, convection:f32, storm:f32,
            low_amount:f32, mid_amount:f32, high_amount:f32, anvil_bias:f32, seed:f32 };
        const SHAPE_TEX_SIZE=64.0; const DETAIL_TEX_SIZE=32.0;
        const CLOUD_NOISE_STABLE_REGION_M=8192.0; const CLOUD_ADVECTION_SPEED_SCALE=1.65;
        const LAYER_LOW:i32=0;
        @group(0) @binding(0) var shape_noise_tex:texture_3d<f32>;
        @group(0) @binding(1) var detail_noise_tex:texture_3d<f32>;
        @group(0) @binding(2) var cloud_field_sampler:sampler;
    "#;
    // 128 x 128 x 32 samples. XZ cross several old lattice cells; vertical
    // samples include the layer limits. Four independent weather cases in RGBA.
    let (width, height) = (4096, 128);
    let entry = format!(
        r#"
        @vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {{
            let p=array<vec2<f32>,3>(vec2<f32>(-1,-1),vec2<f32>(3,-1),vec2<f32>(-1,3));
            return vec4<f32>(p[i],0,1);
        }}
        @fragment fn fs(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32> {{
            light.weather_globals[0]=vec4<f32>({time:.9},0.75,0.9,-0.4);
            light.weather_globals[1]=vec4<f32>(0.5,0.6,0.4,0.75);
            light.weather_globals[2]=vec4<f32>(0.5,0.7,0.5,0.5);
            light.weather_globals[3]=vec4<f32>(0.4,0.5,0.0,0.0);
            light.cloud_authoring[0]=vec4<f32>(0.65,0.5,0.5,0.6);
            light.cloud_authoring[1]=vec4<f32>(0.6,0.0,0.0,0.0);
            cam.planet[0]=vec4<f32>(0.0,0.0,0.0,{planet:.1});
            let index=u32(p.y)*4096u+u32(p.x);
            let x=f32(index%128u); let z=f32((index/128u)%128u); let y=f32(index/16384u);
            let world=vec3<f32>((x+0.5)*64.0-4096.0,700.0+(y+0.5)/32.0*800.0,(z+0.5)*64.0-4096.0);
            let fair=CellWeather(0.36,0.72,0.62,0.45,0.0,0.35,0.0,0.0,0.0,0.0);
            let broken=CellWeather(0.62,0.50,0.76,0.50,0.0,0.65,0.0,0.0,0.0,0.0);
            let overcast=CellWeather(0.92,0.10,0.90,0.10,0.0,0.85,0.0,0.0,0.0,0.0);
            let storm=CellWeather(0.90,0.97,0.98,0.95,0.90,0.97,0.0,0.0,0.90,0.0);
            return vec4<f32>(photographic_low_density(world,700.0,1500.0,{footprint:.9},fair,false),
                photographic_low_density(world,700.0,1500.0,{footprint:.9},broken,false),
                photographic_low_density(world,700.0,1500.0,{footprint:.9},overcast,false),
                photographic_low_density(world,700.0,1500.0,{footprint:.9},storm,false));
        }}"#
    );
    let pipelines: Vec<_> = [
        reference.as_str(),
        include_str!("../src/shaders/cloud_morphology.wgsl"),
    ]
    .iter()
    .map(|morph| {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cloud density numeric profile"),
            source: wgpu::ShaderSource::Wgsl(
                format!("{header}\n{helpers}\n{morph}\n{entry}").into(),
            ),
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    })
    .collect();
    println!(
        "adapter={:?} grid=128x128x32 footprint={footprint} time={time} planet={planet}",
        adapter.get_info()
    );
    kernel_profile::compare_with_readback(&device, &queue, width, height, &pipelines, &[&groups[0], &groups[1]],
        ["ellipsoid", "continuous"], |variant, values| {
            for (channel, name) in ["fair", "broken", "overcast", "storm"].into_iter().enumerate() {
                let mut sum=0.0f64; let mut occupied=0usize; let mut columns=vec![false;128*128];
                for (i, value) in values.chunks_exact(4).enumerate() {
                    sum+=f64::from(value[channel]);
                    if value[channel]>0.05 { occupied+=1; columns[i%(128*128)]=true; }
                }
                println!("variant={variant} weather={name} mean_density={:.6} occupied={:.4} column_coverage={:.4}",
                    sum/(values.len()/4)as f64,occupied as f64/(values.len()/4)as f64,
                    columns.iter().filter(|v|**v).count()as f64/columns.len()as f64);
            }
        }).await
}
