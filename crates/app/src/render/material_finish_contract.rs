//! Execute the production vertex finish-resolution block against real palette data.
#[test]
#[ignore = "native Vulkan actual VS finish block; run explicitly in the shared GPU slot"]
fn gpu_unset_rock_finish_preserves_authored_palette_on_actual_vs_block() {
    use crate::{material::BlockId, material_studio::Palette};
    let source = include_str!("../shaders/world_shader.wgsl");
    let start = source
        .find("// Resolve IDs at vertices; interpolate colors/finish, never material IDs.")
        .unwrap();
    let end = source[start..]
        .find("out.material_props=properties;")
        .unwrap()
        + start
        + "out.material_props=properties;".len();
    let finish = &source[start..end];
    let kind = source
        .lines()
        .find(|s| s.trim_start().starts_with("let geometry_kind="))
        .unwrap();
    let weights = &source[source.find("fn terrain_coverage_for_family").unwrap()
        ..source.find("fn gerstner_vertex_point").unwrap()];
    let reflectance = source
        .lines()
        .find(|s| {
            s.trim_start()
                .starts_with("var reflectance = clamp(material_props.x")
        })
        .unwrap();
    let f0 = source
        .lines()
        .find(|s| s.trim_start().starts_with("let dielectric_f0 ="))
        .unwrap();
    let shader_source = format!(
        r#"
struct ContractVertex {{surface:vec4<f32>,ao_strength:f32,}};
struct ContractOut {{terrain_weights:vec4<f32>,gravel_coverage:f32,model_uv:vec2<f32>,material_tint:vec4<f32>,material_props:vec4<f32>,}};
struct ContractLight {{material_tints:array<vec4<f32>,256>,material_properties:array<vec4<f32>,256>,}};
@group(0) @binding(0) var<uniform> light:ContractLight;
@group(0) @binding(1) var<storage,read> inputs:array<vec4<f32>>;
@group(0) @binding(2) var<storage,read_write> result:array<vec4<f32>>;
{weights}
fn vertex_finish(in:ContractVertex)->ContractOut {{
 var out:ContractOut;
 {kind}
 {finish}
 return out;
}}
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id:vec3<u32>) {{
 let out=vertex_finish(ContractVertex(inputs[id.x*2u],inputs[id.x*2u+1u].x));
 let material_props=out.material_props;
 {reflectance}
 {f0}
 result[id.x*3u]=out.material_props;
 result[id.x*3u+1u]=out.material_tint;
 result[id.x*3u+2u]=vec4<f32>(dielectric_f0,0.0);
}}
"#
    );
    let adapter = engine_gpu::test_adapter();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("production rock finish resolution"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("actual VS finish contract"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let cases = [
        (BlockId::Stone, false, 1.0),
        (BlockId::Stone, true, -1.0),
        (BlockId::Basalt, false, 1.0),
        (BlockId::Basalt, true, -1.0),
        (BlockId::Basalt, false, -2.6),
        (BlockId::Dirt, false, 1.0),
        (BlockId::Grass, false, 1.0),
        (BlockId::Water, false, 1.0),
    ];
    let mut inputs = Vec::<[f32; 4]>::new();
    for &(block, parent, ao) in &cases {
        let s = block.surface();
        inputs.push([
            s.roughness,
            s.specular,
            if parent { 0.0 } else { s.shader_key() },
            if parent { s.shader_key() } else { 4.8 },
        ]);
        inputs.push([ao, 0.0, 0.0, 0.0]);
    }
    let make = |label, size, usage| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let uniform = make(
        "palette arrays",
        8192,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    );
    let input = make(
        "real block surface keys",
        std::mem::size_of_val(inputs.as_slice()) as u64,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    );
    let bytes = (cases.len() * 3 * 16) as u64;
    let output = make(
        "resolved properties tint F0",
        bytes,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    );
    let readback = make(
        "finish readback",
        bytes,
        wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    );
    queue.write_buffer(&input, 0, bytemuck::cast_slice(&inputs));
    let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("finish contract"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output.as_entire_binding(),
            },
        ],
    });
    for scenario in 0..5 {
        let mut palette = Palette::default();
        if scenario > 0 {
            palette.tints[3] = [1.1, 0.8, 0.7, 0.91];
            palette.properties[3] = [0.8, 2.0, 0.75, 0.6];
        }
        let basalt = 64 + BlockId::Basalt as usize;
        if scenario == 2 || scenario == 3 {
            palette.tints[basalt] = [0.9, 1.2, 1.1, 0.33];
            palette.properties[basalt] = [if scenario == 3 { 0.0 } else { 0.2 }, 0.5, 1.2, 0.7];
        }
        if scenario == 4 {
            palette.properties[3][0] = 0.0;
        }
        queue.write_buffer(&uniform, 0, bytemuck::cast_slice(&palette.tints));
        queue.write_buffer(&uniform, 4096, bytemuck::cast_slice(&palette.properties));
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups(cases.len() as u32, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, bytes);
        let submission = queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
            .unwrap();
        {
            let mapped = readback.slice(..).get_mapped_range();
            let actual: &[[f32; 4]] = bytemuck::cast_slice(&mapped);
            for (i, &(block, parent, ao)) in cases.iter().enumerate() {
                let s = block.surface();
                let rock = s.texture_kind == 3;
                let individual = block == BlockId::Basalt && (scenario == 2 || scenario == 3);
                let r = if individual {
                    if scenario == 3 {
                        0.0
                    } else {
                        0.2
                    }
                } else if rock {
                    match scenario {
                        0 => 0.5,
                        4 => 0.0,
                        _ => 0.8,
                    }
                } else {
                    (0.5 * (s.specular / 0.04).sqrt()).clamp(0.0, 1.0)
                };
                let rough = if individual {
                    0.33
                } else if rock && scenario > 0 {
                    0.91
                } else if block == BlockId::Water {
                    0.17
                } else {
                    s.roughness
                };
                let expected_props = if individual {
                    [r, 1.0, 0.9, 0.42]
                } else if rock && scenario > 0 {
                    [r, 2.0, 0.75, 0.6]
                } else {
                    [r, 1.0, 1.0, 1.0]
                };
                let expected_tint = if individual {
                    [0.99, 0.96, 0.77, rough]
                } else if rock && scenario > 0 {
                    [1.1, 0.8, 0.7, rough]
                } else {
                    [1.0, 1.0, 1.0, rough]
                };
                for (label, got, want) in [
                    ("props", actual[i * 3], expected_props),
                    ("tint/roughness", actual[i * 3 + 1], expected_tint),
                    (
                        "dry dielectric F0",
                        actual[i * 3 + 2],
                        [0.16 * r * r, 0.16 * r * r, 0.16 * r * r, 0.0],
                    ),
                ] {
                    for lane in 0..4 {
                        assert!((got[lane]-want[lane]).abs()<0.00003,"scenario={scenario} block={block:?} parent={parent} ao={ao} {label}[{lane}]={} expected{}",got[lane],want[lane]);
                    }
                }
            }
        }
        readback.unmap();
    }
    println!("[rock-finish-contract] actual VS resolve+dry F0,real Stone/Basalt/Dirt/Grass/Water keys,40case/scenarios,default/family/individual/explicit-zero andresident/parent/exact-fallback passed on {:?}",adapter.get_info());
}
