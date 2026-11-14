@group(1) @binding(3) var scene_tex:texture_2d<f32>;
// Thin crystal approximation: twelve bounded scene-depth queries, no history
// or feedback. Hidden geometry is unavailable; a miss retains shaded facets.
fn crystal_refraction(uv:vec2<f32>,receiver:vec4<f32>,p:vec3<f32>,depth:f32)->vec4<f32>{
 if depth>60.0 || receiver.z<0.01 {return vec4<f32>(0.0);}
 let n=normalize(normalize(cam.ray_right.xyz)*receiver.x+normalize(cam.ray_up.xyz)*receiver.y
     -cam.camera_forward.xyz*sqrt(max(1.0-dot(receiver.xy,receiver.xy),0.001)));
 let incident=normalize(p);let ray=refract(incident,n,1.0/1.46);
 let fresnel=0.035+0.965*pow(1.0-clamp(dot(-incident,n),0.0,1.0),5.0);
 let clip_start=cam.view_proj*vec4<f32>(p,1.0);
 let clip_direction=cam.view_proj*vec4<f32>(ray,0.0);
 let depth_direction=dot(ray,cam.camera_forward.xyz);
 for(var i=1;i<=12;i++){
  let distance=0.10+f32(i*i)*0.045;
  let clip=clip_start+clip_direction*distance;
  if clip.w<=0.0 {break;}
  let at=clip.xy/clip.w*vec2<f32>(0.5,-0.5)+0.5;
  if any(at<vec2<f32>(0.01)) || any(at>vec2<f32>(0.99)) {break;}
  let target_pixel=depth_pixel(at);
  let raw_depth=textureLoad(depth_tex,target_pixel,0);
  if raw_depth<=0.00001 {continue;}
  let target_depth=linear_depth(raw_depth);
  // Never borrow foreground colour, another crystal, or a distant surface
  // that the refracted ray has not actually approached.
  let gap=depth+depth_direction*distance-target_depth;
  if target_depth<=depth+0.08 || gap<0.0 || gap>0.30 {continue;}
  if textureLoad(receiver_tex,target_pixel,0).a< -1.5 {continue;}
  let edge=smoothstep(0.01,0.06,min(min(at.x,at.y),min(1.0-at.x,1.0-at.y)));
  let transmission=0.40*(1.0-fresnel)*receiver.z*edge;
  let source=textureSampleLevel(scene_tex,linear_sampler,uv,0.0).rgb;
  let behind=textureLoad(scene_tex,target_pixel,0).rgb;
  return vec4<f32>((behind-source)*transmission,1.0);
 }
 return vec4<f32>(0.0);
}
// Bounded half-resolution reflection trace. No history, extra scene render,
// or read/write feedback: misses leave the world pass's sky reflection intact.
fn water_reflection(uv:vec2<f32>,receiver:vec4<f32>,p:vec3<f32>,depth:f32)->vec4<f32>{
 if depth>700.0 || receiver.z<0.01{return vec4<f32>(0.0);}
 let n=normalize(vec3<f32>(receiver.x,sqrt(max(1.0-dot(receiver.xy,receiver.xy),0.01)),receiver.y));
 let direction=reflect(normalize(p),n);
 if direction.y<0.005{return vec4<f32>(0.0);}
 let start=p+n*max(0.08,depth*0.0005);
 // A projected ray is linear before the perspective divide. Transform its
 // origin and direction once, then only advance the ray parameter per tap.
 let clip_start=cam.view_proj*vec4<f32>(start,1.0);
 let clip_direction=cam.view_proj*vec4<f32>(direction,0.0);
 let start_depth=dot(start,cam.camera_forward.xyz);
 let depth_direction=dot(direction,cam.camera_forward.xyz);
 var previous_t=0.0;var previous_delta= -1.0;
 for(var i=1;i<=32;i++){
  let t=0.35*f32(i*i)+0.5*f32(i);
  let clip=clip_start+clip_direction*t;
  if clip.w<=0.0 {break;}
  let at=clip.xy/clip.w*vec2<f32>(0.5,-0.5)+0.5;
  if any(at<vec2<f32>(0.002)) || any(at>vec2<f32>(0.998)){break;}
  let raw_depth=depth_at(at);
  let delta=select(-1.0,start_depth+depth_direction*t-linear_depth(raw_depth),raw_depth>0.00001);
  if delta>0.0 && previous_delta<=0.0 {
   var lo=previous_t;var hi=t;
   for(var j=0;j<5;j++){
    let mid=(lo+hi)*0.5;
    let projected=clip_start+clip_direction*mid;
    let hit=projected.xy/projected.w*vec2<f32>(0.5,-0.5)+0.5;
    let raw_hit=depth_at(hit);
    if raw_hit>0.00001 && start_depth+depth_direction*mid>linear_depth(raw_hit) {hi=mid;}else{lo=mid;}
   }
   let projected=clip_start+clip_direction*hi;
   let hit=projected.xy/projected.w*vec2<f32>(0.5,-0.5)+0.5;
   let hit_pixel=depth_pixel(hit);
   let hit_raw=textureLoad(depth_tex,hit_pixel,0);
   let hit_depth=linear_depth(hit_raw);
   let error=abs(start_depth+depth_direction*hi-hit_depth);
   let thickness=max(0.45,hit_depth*0.006);
   // The refined sample can land on clear sky or the opposite side of a
   // silhouette. Fade marginal hits back to the original sky reflection.
   if hit_raw>0.00001 && error<thickness && textureLoad(receiver_tex,hit_pixel,0).a>=0.0 {
    let border=min(min(hit.x,hit.y),min(1.0-hit.x,1.0-hit.y));
    let confidence=smoothstep(0.0,0.08,border)*(1.0-smoothstep(260.0,375.0,hi))
        *(1.0-smoothstep(thickness*0.5,thickness,error))*(1.0-smoothstep(550.0,700.0,depth));
    // Same 0.8 as WATER_REFLECTION_GAIN: a reflection is darker than its source.
    let hit_color=textureSampleLevel(scene_tex,linear_sampler,hit,0.0).rgb*0.8;
    let source=textureSampleLevel(scene_tex,linear_sampler,uv,0.0).rgb;
    let weight=clamp(-receiver.a,0.025,0.92)*receiver.z*confidence;
    return vec4<f32>((hit_color-source)*weight,1.0);
   }
  }
  previous_delta=delta;previous_t=t;
 }
 return vec4<f32>(0.0);
}
// Reuse the half-resolution indirect pass and the lit opaque pixels visible
// around the water silhouette. Hidden bed pixels retain the material fallback.
override WATER_SCENE_REFRACTION: bool = true;
fn water_scene_refraction(uv: vec2<f32>, receiver: vec4<f32>, p: vec3<f32>, depth: f32) -> vec4<f32> {
    if depth > 48.0 || receiver.z < 0.05 { return vec4<f32>(0.0); }
    let normal = vec3<f32>(receiver.x, sqrt(max(1.0 - dot(receiver.xy, receiver.xy), 0.01)), receiver.y);
    let ray = refract(normalize(p), normal, 0.75);
    if ray.y >= -0.10 { return vec4<f32>(0.0); }
    let clip_start = cam.view_proj * vec4<f32>(p, 1.0);
    let clip_step = cam.view_proj * vec4<f32>(ray, 0.0);
    for (var i = 0; i < 2; i++) {
        let distance = 0.35 + 0.55 * f32(i);
        let clip = clip_start + clip_step * distance;
        if clip.w <= 0.0 { break; }
        let at = clip.xy / clip.w * vec2<f32>(0.5, -0.5) + 0.5;
        if any(at <= vec2<f32>(0.0)) || any(at >= vec2<f32>(1.0)) { break; }
        let pixel = depth_pixel(at);
        let raw = textureLoad(depth_tex, pixel, 0);
        if raw <= 0.00001 { continue; }
        let target_depth = linear_depth(raw);
        if target_depth <= depth + 0.04 || abs(target_depth - clip.w) > 0.30 { continue; }
        let target_position = view_position_at(pixel, target_depth);
        let submerged = p.y - target_position.y;
        if submerged < 0.04 || submerged > 1.1 || textureLoad(receiver_tex, pixel, 0).a < 0.0 { continue; }
        let edge = smoothstep(0.0, 0.035, min(min(at.x, at.y), min(1.0 - at.x, 1.0 - at.y)));
        let weight = 0.33 * (1.0 + receiver.a) * receiver.z * edge
            * (1.0 - smoothstep(24.0, 48.0, depth)) * smoothstep(0.04, 0.17, submerged);
        let source = textureSampleLevel(scene_tex, linear_sampler, uv, 0.0).rgb;
        let bed = textureLoad(scene_tex, pixel, 0).rgb * exp(-vec3<f32>(0.78, 0.26, 0.31) * submerged);
        return vec4<f32>((bed - source) * weight, 1.0);
    }
    return vec4<f32>(0.0);
}

@fragment fn fs_main(@builtin(position) pixel:vec4<f32>)->@location(0) vec4<f32>{
 let dims=vec2<i32>(textureDimensions(depth_tex));
 let at=min(vec2<i32>(pixel.xy)*2+1,dims-1);
 let uv=(vec2<f32>(at)+0.5)/vec2<f32>(dims);
 let receiver=textureLoad(receiver_tex,at,0);
 if receiver.a==0.0 {return vec4<f32>(0.0);}
 let raw_depth=textureLoad(depth_tex,at,0);
 if raw_depth<=0.00001 {return vec4<f32>(0.0);}
 let depth=linear_depth(raw_depth);
 let p=view_position_at(at,depth);
 if receiver.a< -1.5 {return crystal_refraction(uv,receiver,p,depth);}
 if receiver.a<0.0 {
  let shallow=receiver.z>=2.0;
  let water=vec4<f32>(receiver.xy,receiver.z-select(0.0,2.0,shallow),receiver.a);
  let reflection=water_reflection(uv,water,p,depth);
  if WATER_SCENE_REFRACTION && shallow {
   let refraction=water_scene_refraction(uv,water,p,depth);
   return vec4<f32>(reflection.rgb+refraction.rgb,max(reflection.a,refraction.a));
  }
  return reflection;
 }
 let radius=5.0;
 if depth>80.0{return vec4<f32>(0.0);}
 let n=surface_normal(uv,p);
 let projected=vec2<f32>(1.0/length(cam.ray_right.xyz),1.0/length(cam.ray_up.xyz))*radius/(2.0*max(depth,0.01));
 let taps=array<vec2<f32>,8>(vec2<f32>(0.19,0.08),vec2<f32>(-0.23,0.29),vec2<f32>(-0.10,-0.47),vec2<f32>(0.45,0.35),vec2<f32>(-0.65,-0.10),vec2<f32>(0.63,-0.44),vec2<f32>(-0.28,0.83),vec2<f32>(-0.39,-0.90));
 var bounce=vec3<f32>(0.0);var obscurance=0.0;
 for(var i=0;i<8;i++){
  let q_uv=uv+taps[i]*projected;
  if any(q_uv<vec2<f32>(0.0))||any(q_uv>vec2<f32>(1.0)){continue;}
  let q_pixel=depth_pixel(q_uv);
  let q_depth=textureLoad(depth_tex,q_pixel,0);
  if q_depth<=0.00001{continue;}
  let q=view_position_at(q_pixel,linear_depth(q_depth));let delta=q-p;let length2=dot(delta,delta);
  if length2<0.0025||length2>radius*radius{continue;}
  let direction=delta*inverseSqrt(length2);let cosine=max(dot(n,direction)-0.06,0.0);
  if cosine<=0.0{continue;}
  let range=1.0-length2/(radius*radius);let weight=cosine*range*range;
  obscurance+=weight;
  // Screen-space diffuse exchange: incident radiance comes from actual lit
  // surfaces, including emissive machinery, rather than a fixed ambient tint.
  let nq=surface_normal(q_uv,q);let facing=max(dot(nq,-direction),0.0);
  if facing<=0.0{continue;}
  let radiance=min(textureSampleLevel(scene_tex,linear_sampler,q_uv,0.0).rgb,vec3<f32>(4.0));
  bounce+=radiance*weight*facing;
 }
 return vec4<f32>(bounce*0.35,clamp(obscurance*0.23,0.0,0.65))*(1.0-smoothstep(64.0,80.0,depth));
}
