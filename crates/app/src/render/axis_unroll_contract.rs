//! PREPARED ONLY: actual loop reference vs production unrolled helpers on native GPU.
#[test]
#[ignore = "native Vulkan pilot contract; requires the explicitly assigned GPU slot"]
fn gpu_axis_unroll_actual_shader_equivalence() {
    fn function<'a>(source: &'a str, name: &str) -> &'a str {
        let start = source.find(&format!("fn {name}(")).expect(name);
        let open = source[start..].find('{').unwrap() + start;
        let mut depth = 0;
        for (i, c) in source[open..].char_indices() {
            if c == '{' { depth += 1; }
            if c == '}' {
                depth -= 1;
                if depth == 0 { return &source[start..open+i+1]; }
            }
        }
        panic!("unterminated {name}");
    }
    let source = include_str!("../shaders/world_shader.wgsl");
    let helpers = ["terrain_safe_unit3", "terrain_pow5", "terrain_fresnel",
        "terrain_specular", "oren_nayar", "terrain_diffuse_lobe", "terrain_axis_facet_specular",
        "terrain_axis_facet_diffuse"].map(|name| function(source, name)).join("\n")
        + include_str!("axis_loop_reference.wgsl.txt");
    let wgsl = helpers + r#"
@group(0) @binding(0) var<storage,read> inputs:array<vec4<f32>>;
@group(0) @binding(1) var<storage,read_write> result:array<vec4<f32>>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id:vec3<u32>) {
    let i=id.x*5u; let o=id.x*7u;
    let macro_n=inputs[i].xyz;
    let v=terrain_safe_unit3(inputs[i+1u].xyz,vec3<f32>(0.0,1.0,0.0));
    let l=terrain_safe_unit3(inputs[i+2u].xyz,vec3<f32>(0.0,1.0,0.0));
    let r=inputs[i+1u].w; let f0=inputs[i+3u].xyz;
    result[o]=vec4<f32>(terrain_axis_facet_specular_loop_reference(macro_n,v,l,r,f0),0.0);
    result[o+1u]=vec4<f32>(terrain_axis_facet_specular(macro_n,v,l,r,f0),0.0);
    result[o+2u]=vec4<f32>(terrain_axis_facet_diffuse_loop_reference(macro_n,v,l,r,0.22),0.0,0.0,0.0);
    result[o+3u]=vec4<f32>(terrain_axis_facet_diffuse(macro_n,v,l,r,0.22),0.0,0.0,0.0);
    result[o+4u]=vec4<f32>(terrain_axis_facet_diffuse_loop_reference(macro_n,v,l,r,0.16),0.0,0.0,0.0);
    result[o+5u]=vec4<f32>(terrain_axis_facet_diffuse(macro_n,v,l,r,0.16),0.0,0.0,0.0);
    result[o+6u]=vec4<f32>(0.0);
}
"#;
    // Five vec4 per case: macro/AO, view/roughness, light/kind,
    // F0/coverage, original perturbed shading normal/padding.
    let mut cases = Vec::<[[f32;4];5]>::new();
    let mut checks = Vec::<(&str, bool, bool, bool)>::new(); // axis, excluded, invalid
    let views = [[0.3,0.8,0.5],[-0.9,0.15,0.3],[0.0,1.0,0.0]];
    let lights = [[0.7,0.6,-0.3],[-0.4,0.8,0.2],[0.0,-1.0,0.0]];
    for axis in [[1.,0.,0.],[-1.,0.,0.],[0.,1.,0.],[0.,0.,1.],[0.,0.,-1.]] {
        for v in views { for l in lights { for r in [0.06,0.35,0.8] {
            cases.push([[axis[0],axis[1],axis[2],-1.], [v[0],v[1],v[2],r],
                [l[0],l[1],l[2],0.], [0.04,0.06,0.08,1.], [axis[0],axis[1],axis[2],0.]]);
            checks.push(("axis identity",true,false,false));
        }}}
    }
    for m in [[0.25,1.,0.6],[-0.75,0.2,0.4],[0.01,1.,-0.01],[-1.,0.01,-1.]] {
        for v in views { for l in lights { for coverage in [0.,0.25,0.5,1.] {
            cases.push([[m[0],m[1],m[2],-1.], [v[0],v[1],v[2],0.65],
                [l[0],l[1],l[2],0.], [0.04,0.04,0.04,coverage], [0.,1.,0.,0.]]);
            checks.push(("rock slope blend",false,false,false));
        }}}
    }
    for (ao,kind) in [(1.,0.),(-2.6,0.),(-1.5,0.),(0.,0.),(-1.,8.),(-1.,6.),(-1.,3.)] {
        cases.push([[0.6,0.8,0.3,ao],[0.3,0.9,0.2,0.65],[0.4,0.8,-0.3,kind],
            [0.04,0.04,0.04,1.],[0.,1.,0.,0.]]);
        checks.push(("excluded geometry",false,true,false));
    }
    for bad in [[0.,-1.,0.],[0.,0.,0.],[f32::NAN,1.,0.],[f32::INFINITY,1.,0.]] {
        cases.push([[bad[0],bad[1],bad[2],-1.],[0.3,0.9,0.2,0.65],
            [0.4,0.8,-0.3,0.],[0.04,0.04,0.04,1.],[0.,1.,0.,0.]]);
        checks.push(("invalid macro fallback",false,false,true));
    }
    // Deterministic input generation only; all old/new shading runs on GPU.
    let mut seed=0x39a8_15c7u32;
    let mut unit=|| {seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;
        (seed>>8) as f32 / 16777216.0};
    for j in 0..4096 {
        let mut m=[unit()*2.-1.,unit(),unit()*2.-1.,-1.];
        if j%16==0 {m[1]=1e-6;}
        if j%31==0 {m[0]=0.;}
        let v=[unit()*2.-1.,unit()*2.-1.,unit()*2.-1.,0.06+unit()*0.94];
        let mut l=[unit()*2.-1.,unit()*2.-1.,unit()*2.-1.,0.];
        if j%47==0 {l=[-v[0],-v[1],-v[2],0.];}
        let f0=[unit()*0.16,unit()*0.16,unit()*0.16,1.];
        cases.push([m,v,l,f0,[0.,1.,0.,0.]]);
        checks.push(("random finite",false,false,false));
    }
    let adapter = engine_gpu::test_adapter();
    let (device,queue)=pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label:Some("actual loop versus unrolled shader helpers"),source:wgpu::ShaderSource::Wgsl(wgsl.into())});
    let pipeline=device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label:Some("directional facet contract"),layout:None,module:&shader,entry_point:Some("main"),
        compilation_options:Default::default(),cache:None});
    let make=|size,usage| device.create_buffer(&wgpu::BufferDescriptor {
        label:None,size,usage,mapped_at_creation:false});
    let input=make(std::mem::size_of_val(cases.as_slice()) as u64,
        wgpu::BufferUsages::STORAGE|wgpu::BufferUsages::COPY_DST);
    let bytes=cases.len() as u64*7*16;
    let output=make(bytes,wgpu::BufferUsages::STORAGE|wgpu::BufferUsages::COPY_SRC);
    let readback=make(bytes,wgpu::BufferUsages::COPY_DST|wgpu::BufferUsages::MAP_READ);
    queue.write_buffer(&input,0,bytemuck::cast_slice(&cases));
    let bindings=device.create_bind_group(&wgpu::BindGroupDescriptor {
        label:None,layout:&pipeline.get_bind_group_layout(0),entries:&[
            wgpu::BindGroupEntry{binding:0,resource:input.as_entire_binding()},
            wgpu::BindGroupEntry{binding:1,resource:output.as_entire_binding()}]});
    let mut encoder=device.create_command_encoder(&Default::default());
    { let mut pass=encoder.begin_compute_pass(&Default::default());
      pass.set_pipeline(&pipeline);pass.set_bind_group(0,&bindings,&[]);
      pass.dispatch_workgroups(cases.len() as u32,1,1); }
    encoder.copy_buffer_to_buffer(&output,0,&readback,0,bytes);
    let submission=queue.submit([encoder.finish()]);
    let (tx,rx)=std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read,move|r|{let _=tx.send(r);});
    device.poll(wgpu::PollType::Wait {submission_index:Some(submission),
        timeout:Some(std::time::Duration::from_secs(10))}).unwrap();
    rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap().unwrap();
    { let mapped=readback.slice(..).get_mapped_range().expect("GPU readback range");
      let data:&[[f32;4]]=bytemuck::cast_slice(&mapped);
      let mut bit_differences=0usize;
      let mut max_absolute=0.0f32;
      let mut max_relative=0.0f32;
      for i in 0..cases.len() {
        let row=&data[i*7..i*7+7];
        for (old,new,lanes) in [(0,1,3),(2,3,1),(4,5,1)] {
            for c in 0..lanes {
                let a=row[old][c];let b=row[new][c];
                assert!(a.is_finite()&&b.is_finite()&&a>=0.&&b>=0.,
                    "case{i} {} pair{old}/{new} lane{c} nonfinite/negative: {a}/{b}",checks[i].0);
                if a.to_bits()!=b.to_bits() {bit_differences+=1;}
                let absolute=(a-b).abs();
                let relative=absolute/a.abs().max(b.abs()).max(1e-8);
                max_absolute=max_absolute.max(absolute);max_relative=max_relative.max(relative);
                assert!(absolute<=2e-6+2e-5*a.abs().max(b.abs()),
                    "case{i} {} pair{old}/{new} lane{c}: loop={a} unrolled={b} abs={absolute} rel={relative}",checks[i].0);
            }
        }
      }
      eprintln!("GPU old/new equivalence: {} cases; bit differences={bit_differences}, max abs={max_absolute}, max rel={max_relative}",cases.len());
    }
    readback.unmap();
    eprintln!("actual unroll equivalence contract: {} cases passed",cases.len());
}
