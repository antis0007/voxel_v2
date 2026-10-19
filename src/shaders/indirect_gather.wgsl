@group(1) @binding(3) var scene_tex:texture_2d<f32>;
// Bounded half-resolution reflection trace. No history, extra scene render,
// or read/write feedback: misses leave the world pass's sky reflection intact.
fn water_reflection(uv:vec2<f32>,receiver:vec4<f32>)->vec4<f32>{
 let p=view_position(uv);let depth=dot(p,cam.camera_forward.xyz);
 if depth>700.0 || receiver.z<0.01{return vec4<f32>(0.0);}
 let n=normalize(vec3<f32>(receiver.x,sqrt(max(1.0-dot(receiver.xy,receiver.xy),0.01)),receiver.y));
 let direction=reflect(normalize(p),n);
 if direction.y<0.005{return vec4<f32>(0.0);}
 let start=p+n*max(0.08,depth*0.0005);
 var previous_t=0.0;var previous_delta= -1.0;
 for(var i=1;i<=32;i++){
  let t=0.35*f32(i*i)+0.5*f32(i);
  let q=start+direction*t;
  let clip=cam.view_proj*vec4<f32>(q,1.0);
  if clip.w<=0.0 {break;}
  let at=clip.xy/clip.w*vec2<f32>(0.5,-0.5)+0.5;
  if any(at<vec2<f32>(0.002)) || any(at>vec2<f32>(0.998)){break;}
  let scene_depth=dot(view_position(at),cam.camera_forward.xyz);
  let delta=dot(q,cam.camera_forward.xyz)-scene_depth;
  if depth_at(at)<0.99999 && delta>0.0 && previous_delta<=0.0 {
   var lo=previous_t;var hi=t;var hit=at;
   for(var j=0;j<5;j++){
    let mid=(lo+hi)*0.5;let point=start+direction*mid;
    let projected=cam.view_proj*vec4<f32>(point,1.0);
    hit=projected.xy/projected.w*vec2<f32>(0.5,-0.5)+0.5;
    let difference=dot(point-view_position(hit),cam.camera_forward.xyz);
    if difference>0.0 {hi=mid;}else{lo=mid;}
   }
   let point=start+direction*hi;
   let error=abs(dot(point-view_position(hit),cam.camera_forward.xyz));
   let hit_receiver=textureLoad(receiver_tex,clamp(vec2<i32>(hit*vec2<f32>(textureDimensions(receiver_tex))),vec2<i32>(0),vec2<i32>(textureDimensions(receiver_tex))-1),0);
   if error<max(0.45,scene_depth*0.006) && hit_receiver.a>=0.0 {
    let border=min(min(hit.x,hit.y),min(1.0-hit.x,1.0-hit.y));
    let confidence=smoothstep(0.0,0.08,border)*(1.0-smoothstep(260.0,375.0,hi));
    let hit_color=textureSampleLevel(scene_tex,linear_sampler,hit,0.0).rgb;
    let source=textureSampleLevel(scene_tex,linear_sampler,uv,0.0).rgb;
    let weight=clamp(-receiver.a,0.025,0.92)*receiver.z*confidence;
    return vec4<f32>((hit_color-source)*weight,1.0);
   }
  }
  previous_delta=delta;previous_t=t;
 }
 return vec4<f32>(0.0);
}
@fragment fn fs_main(@builtin(position) pixel:vec4<f32>)->@location(0) vec4<f32>{
 let dims=vec2<i32>(textureDimensions(depth_tex));
 let at=min(vec2<i32>(pixel.xy)*2+1,dims-1);
 let uv=(vec2<f32>(at)+0.5)/vec2<f32>(dims);
 let receiver=textureLoad(receiver_tex,at,0);
 if depth_at(uv)>=0.99999 {return vec4<f32>(0.0);}
 if receiver.a<0.0 {return water_reflection(uv,receiver);}
 if receiver.a==0.0 {return vec4<f32>(0.0);}
 let p=view_position(uv);
 let radius=5.0;let depth=max(dot(p,cam.camera_forward.xyz),0.01);
 if depth>80.0{return vec4<f32>(0.0);}
 let n=surface_normal(uv,p);
 let projected=vec2<f32>(1.0/length(cam.ray_right.xyz),1.0/length(cam.ray_up.xyz))*radius/(2.0*depth);
 let taps=array<vec2<f32>,8>(vec2<f32>(0.19,0.08),vec2<f32>(-0.23,0.29),vec2<f32>(-0.10,-0.47),vec2<f32>(0.45,0.35),vec2<f32>(-0.65,-0.10),vec2<f32>(0.63,-0.44),vec2<f32>(-0.28,0.83),vec2<f32>(-0.39,-0.90));
 var bounce=vec3<f32>(0.0);var obscurance=0.0;
 for(var i=0;i<8;i++){
  let q_uv=uv+taps[i]*projected;
  if any(q_uv<vec2<f32>(0.0))||any(q_uv>vec2<f32>(1.0))||depth_at(q_uv)>=0.99999{continue;}
  let q=view_position(q_uv);let delta=q-p;let length2=dot(delta,delta);
  if length2<0.0025||length2>radius*radius{continue;}
  let direction=delta*inverseSqrt(length2);let cosine=max(dot(n,direction)-0.06,0.0);
  let range=1.0-length2/(radius*radius);let weight=cosine*range*range;
  obscurance+=weight;
  // Screen-space diffuse exchange: incident radiance comes from actual lit
  // surfaces, including emissive machinery, rather than a fixed ambient tint.
  let nq=surface_normal(q_uv,q);let facing=max(dot(nq,-direction),0.0);
  let radiance=min(textureSampleLevel(scene_tex,linear_sampler,q_uv,0.0).rgb,vec3<f32>(4.0));
  bounce+=radiance*weight*facing;
 }
 return vec4<f32>(bounce*0.35,clamp(obscurance*0.23,0.0,0.65))*(1.0-smoothstep(64.0,80.0,depth));
}
