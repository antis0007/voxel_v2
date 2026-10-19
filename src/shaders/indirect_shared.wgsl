@group(1) @binding(0) var depth_tex: texture_depth_2d;
@group(1) @binding(1) var receiver_tex: texture_2d<f32>;
@group(1) @binding(2) var linear_sampler: sampler;
@vertex fn vs_main(@builtin(vertex_index) index:u32)->@builtin(position) vec4<f32>{
 let p=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0));return vec4<f32>(p[index],0.0,1.0);
}
fn depth_pixel(uv:vec2<f32>)->vec2<i32>{
 let dims=vec2<i32>(textureDimensions(depth_tex));
 return clamp(vec2<i32>(uv*vec2<f32>(dims)),vec2<i32>(0),dims-1);
}
fn depth_at(uv:vec2<f32>)->f32{
 return textureLoad(depth_tex,depth_pixel(uv),0);
}
fn view_position(uv:vec2<f32>)->vec3<f32>{
 // The ray and depth must describe the SAME full-resolution texel. Using
 // an arbitrary gather UV with a rounded depth bent flat planes into bands.
 let at=depth_pixel(uv);
 let center=(vec2<f32>(at)+0.5)/vec2<f32>(textureDimensions(depth_tex));
 let d=textureLoad(depth_tex,at,0);let linear=cam.ray_up.w/min(d+cam.ray_right.w,-1e-7);
 let ndc=vec2<f32>(center.x*2.0-1.0,1.0-center.y*2.0);
 return (cam.camera_forward.xyz+cam.ray_right.xyz*ndc.x+cam.ray_up.xyz*ndc.y)*linear;
}
fn surface_normal(uv:vec2<f32>,p:vec3<f32>)->vec3<f32>{
 let pixel=1.0/vec2<f32>(textureDimensions(depth_tex));
 let l=p-view_position(uv-vec2<f32>(pixel.x,0.0));let r=view_position(uv+vec2<f32>(pixel.x,0.0))-p;
 let u=p-view_position(uv-vec2<f32>(0.0,pixel.y));let d=view_position(uv+vec2<f32>(0.0,pixel.y))-p;
 let dx=select(r,l,abs(dot(l,cam.camera_forward.xyz))<abs(dot(r,cam.camera_forward.xyz)));
 let dy=select(d,u,abs(dot(u,cam.camera_forward.xyz))<abs(dot(d,cam.camera_forward.xyz)));
 let n=normalize(cross(dx,dy)+vec3<f32>(1e-9));return select(n,-n,dot(n,-p)<0.0);
}
