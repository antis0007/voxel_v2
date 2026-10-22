//! Executes extracted production directional facet WGSL and its original BRDF.
#[test]
#[ignore = "native Vulkan pilot contract; requires the explicitly assigned GPU slot"]
fn directional_facet_pilot_actual_shader_contract() {
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
    let helpers = ["terrain_safe_unit3", "terrain_pow5", "terrain_is_backdrop",
        "terrain_fresnel", "terrain_specular", "terrain_axis_facet_specular",
        "terrain_rock_facet_coverage", "terrain_directional_specular"].map(|name| function(source, name)).join("\n");
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
    let n=terrain_safe_unit3(inputs[i+4u].xyz,vec3<f32>(0.0,1.0,0.0));
    let actual=terrain_directional_specular(n,macro_n,v,l,r,f0,
        inputs[i].w,i32(inputs[i+2u].w),inputs[i+3u].w);
    let original=terrain_specular(n,v,l,r,f0);
    let aggregate=terrain_axis_facet_specular(macro_n,v,l,r,f0);
    // GPU bounds from actual BRDF on each contributing facet. A light-backface
    // contributes zero but retains its view-projected coverage in the average.
    let m=terrain_safe_unit3(macro_n,vec3<f32>(0.0,1.0,0.0));
    let x=vec3<f32>(select(-1.0,1.0,m.x>=0.0),0.0,0.0);
    let y=vec3<f32>(0.0,select(-1.0,1.0,m.y>=0.0),0.0);
    let z=vec3<f32>(0.0,0.0,select(-1.0,1.0,m.z>=0.0));
    let faces=array<vec3<f32>,3>(x,y,z);
    var lo=vec3<f32>(1e30); var hi=vec3<f32>(0.0); var total=0.0;
    for(var k=0u;k<3u;k+=1u) {
        let w=abs(m[k])*max(dot(faces[k],v),0.0);
        if w>0.0 {
            let s=terrain_specular(faces[k],v,l,r,f0);
            lo=min(lo,s); hi=max(hi,s); total+=w;
        }
    }
    if total<=1e-12 {lo=vec3<f32>(0.0);hi=vec3<f32>(0.0);}
    result[o]=vec4<f32>(actual,0.0);
    result[o+1u]=vec4<f32>(original,0.0);
    result[o+2u]=vec4<f32>(aggregate,0.0);
    result[o+3u]=vec4<f32>(lo,0.0);
    result[o+4u]=vec4<f32>(hi,0.0);
    // Pure-axis oracle uses the original actual helper, with identical inputs.
    result[o+5u]=vec4<f32>(terrain_specular(m,v,l,r,f0),0.0);
    result[o+6u]=vec4<f32>(terrain_specular(vec3<f32>(0.0,1.0,0.0),v,l,r,f0),0.0);
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
    let instance=wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends:wgpu::Backends::VULKAN,..Default::default()});
    let adapter=pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device,queue)=pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label:Some("actual production directional facet helpers"),source:wgpu::ShaderSource::Wgsl(wgsl.into())});
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
    { let mapped=readback.slice(..).get_mapped_range();
      let data:&[[f32;4]]=bytemuck::cast_slice(&mapped);
      for (i,&(label,axis,excluded,invalid)) in checks.iter().enumerate() {
        let row=&data[i*7..i*7+7];
        for c in 0..3 {
            let near=|a:f32,b:f32| (a-b).abs()<=2e-5+2e-5*a.abs().max(b.abs());
            for r in [0,1,2,6] {assert!(row[r][c].is_finite()&&row[r][c]>=0.,"{i} {label} result{r}/{c}");}
            if axis {assert!(near(row[2][c],row[5][c]),"{i} axis identity lane{c}");}
            if excluded {assert!(near(row[0][c],row[1][c]),"{i} excluded lane{c}");}
            if invalid {assert!(near(row[2][c],row[6][c]),"{i} fallback lane{c}");continue;}
            assert!(row[2][c]>=row[3][c]-2e-5 && row[2][c]<=row[4][c]+2e-5,
                "{i} {label} convex facet bound lane{c}");
            if !excluded {
                let w=cases[i][3][3].clamp(0.,1.);
                let expected=row[1][c]*(1.-w)+row[2][c]*w;
                assert!(near(row[0][c],expected),"{i} continuous rock coverage lane{c}");
            }
        }
      }
    }
    readback.unmap();
    eprintln!("actual production facet contract: {} cases passed",cases.len());
}
