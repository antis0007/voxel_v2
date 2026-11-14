//! Numeric and paired GPU profiling of production shader helpers, without images.
//! --width=3840 [--rough-diffuse]; default profiles the final sRGB dither transfer.
//! Component timings include fixture arithmetic and float32 writes, not a game frame.
use anyhow::Result;

#[path = "support/kernel_profile.rs"]
mod kernel_profile;

fn function<'a>(source: &'a str, name: &str) -> &'a str {
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
                    return &source[start..body + offset + 1];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated production function")
}

fn main() -> Result<()> {
    pollster::block_on(run())
}

async fn run() -> Result<()> {
    let width = std::env::args()
        .find_map(|a| a.strip_prefix("--width=").and_then(|v| v.parse().ok()))
        .unwrap_or(3840u32)
        .clamp(256, 7680);
    let height = width * 9 / 16;
    let rough = std::env::args().any(|a| a == "--rough-diffuse");
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::TIMESTAMP_QUERY,
            ..Default::default()
        })
        .await?;
    println!(
        "adapter={:?} size={width}x{height} samples=64 kernel={}",
        adapter.get_info(),
        if rough {
            "rough-diffuse"
        } else {
            "dither-transfer"
        }
    );

    let helpers = if rough {
        format!(
            "override REFERENCE: bool = false;\n{}\n{}",
            function(
                include_str!("../src/shaders/world_shader.wgsl"),
                "oren_nayar"
            ),
            include_str!("support/rough_diffuse_reference.wgsl")
        )
    } else {
        format!(
            "override REFERENCE: bool = false;\n{}\n{}",
            include_str!("../src/shaders/color_shared.wgsl"),
            include_str!("support/dither_candidate.wgsl")
        )
    };
    let fragment = if rough {
        r#"
        let n=normalize(vec3<f32>(fract(p.x*0.618)-0.5,1.0,fract(p.y*0.414)-0.5));
        let t=normalize(cross(n,vec3<f32>(0.0,0.0,1.0)));
        let b=cross(n,t);
        var nl=uv.x*uv.x;
        var nv=uv.y*uv.y;
        // Include exact/near normal incidence, grazing, and back-facing inputs.
        if p.y<4.0 { nv=1.0; }
        else if p.y<8.0 { nv=1.0-1e-7; }
        else if p.y<12.0 { nv=1e-6; }
        else if p.y<16.0 { nl=-nl; }
        let phi=fract(p.x*0.7548777+p.y*0.5698403)*6.2831853;
        let l=normalize(n*nl+t*sqrt(max(1.0-nl*nl,0.0)));
        let v=normalize(n*nv+(t*cos(phi)+b*sin(phi))*sqrt(max(1.0-nv*nv,0.0)));
        if REFERENCE { return vec4<f32>(oren_nayar_reference(n,v,l,0.05),
            oren_nayar_reference(n,v,l,0.3),oren_nayar_reference(n,v,l,0.65),oren_nayar_reference(n,v,l,1.0)); }
        return vec4<f32>(oren_nayar(n,v,l,0.05),oren_nayar(n,v,l,0.3),
            oren_nayar(n,v,l,0.65),oren_nayar(n,v,l,1.0));
    "#
    } else {
        r#"
        var c=vec3<f32>(uv.x,uv.x*uv.x,uv.x*uv.x*uv.x);
        // Dense sweeps across both sides of the piecewise transfer knee, plus
        // black/white clipping. Noise spans its full production +/-0.5 range.
        if p.y<16.0 { c=vec3<f32>(0.0028+uv.x*0.0007); }
        else if p.y<24.0 { c=vec3<f32>(uv.x*0.0002); }
        else if p.y<32.0 { c=vec3<f32>(0.9998+uv.x*0.0002); }
        let noise=fract(p.y*0.6180339)-0.5;
        if REFERENCE { return vec4<f32>(dithered_surface_write(c,noise),0.0); }
        return vec4<f32>(dithered_surface_write_candidate(c,noise),0.0);
    "#
    };
    let source = format!(
        r#"{helpers}
        @vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {{
            let p=array<vec2<f32>,3>(vec2<f32>(-1,-1),vec2<f32>(3,-1),vec2<f32>(-1,3));
            return vec4<f32>(p[i],0,1);
        }}
        @fragment fn fs(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32> {{
            let uv=p.xy/vec2<f32>({width}.0,{height}.0);
            {fragment}
        }}"#
    );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("production helper / numeric reference"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let key = "REFERENCE";
    let pipelines: Vec<_> = [1.0, 0.0]
        .into_iter()
        .map(|reference| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("paired shader profile"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &[(key, reference)],
                        ..Default::default()
                    },
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba32Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                depth_stencil: None,
                primitive: Default::default(),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        })
        .collect();
    kernel_profile::compare(
        &device,
        &queue,
        width,
        height,
        &pipelines,
        None,
        ["reference", "candidate"],
    )
    .await
}
