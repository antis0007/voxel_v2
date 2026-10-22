@group(1) @binding(3) var indirect_tex:texture_2d<f32>;
@fragment fn fs_main(@builtin(position) pixel:vec4<f32>)->@location(0) vec4<f32>{
 let uv=pixel.xy/vec2<f32>(textureDimensions(depth_tex));
 let receiver=textureLoad(receiver_tex,vec2<i32>(pixel.xy),0);
 if receiver.a==0.0{return vec4<f32>(0.0);}
 let water=receiver.a<0.0;
 let p=view_position(uv);let d=dot(p,cam.camera_forward.xyz);
 if d>select(80.0,700.0,water){return vec4<f32>(0.0);}
 let dims=vec2<i32>(textureDimensions(depth_tex));
 let half_dims=vec2<i32>(textureDimensions(indirect_tex));
 // Filter individual gather texels AFTER rejecting their depth/material.
 // Bilinear sampling first blended across silhouettes before rejection.
 let half_position=(pixel.xy-1.5)*0.5;
 let base=vec2<i32>(floor(half_position));let blend=fract(half_position);
 var irradiance=vec4<f32>(0.0);var weights=0.0;
 for(var i=0;i<4;i++){
  let offset=vec2<i32>(i%2,i/2);
  let tap=clamp(base+offset,vec2<i32>(0),half_dims-1);
  let full_pixel=min(tap*2+1,dims-1);
  let tap_uv=(vec2<f32>(full_pixel)+0.5)/vec2<f32>(dims);
  let tap_depth=dot(view_position(tap_uv),cam.camera_forward.xyz);
  let tap_receiver=textureLoad(receiver_tex,full_pixel,0);
  if (tap_receiver.a<0.0)!=water {continue;}
  let spatial=select(vec2<f32>(1.0)-blend,blend,offset!=vec2<i32>(0));
  let w=exp(-abs(tap_depth-d)/max(0.18,d*0.003))*spatial.x*spatial.y;
  irradiance+=textureLoad(indirect_tex,tap,0)*w;weights+=w;
 }
 irradiance/=max(weights,0.0001);
 if water {return vec4<f32>(irradiance.rgb,0.0);}
 // Occlude ambient energy only; direct sun and emissive light are retained.
 let delta=receiver.rgb*(irradiance.rgb-vec3<f32>(receiver.a*irradiance.a));
 return vec4<f32>(delta,0.0);
}
