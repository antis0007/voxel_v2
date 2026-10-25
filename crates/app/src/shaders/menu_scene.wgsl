// Artificers' instrument room, not a generated destination preview.
// Material grain uses the world's 20 texel/metre lattice. Linear lighting shares
// the game's colour transfer functions; copper, ember and crystal use its palette.
@group(0) @binding(0) var<uniform> clock: vec4<f32>;
struct Vertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return Vertex(vec4<f32>(uv * vec2<f32>(2.0,-2.0) + vec2<f32>(-1.0,1.0),0.0,1.0),uv);
}
struct Hit { distance: f32, normal: vec3<f32>, material: u32 }
struct Block { center: vec3<f32>, extent: vec3<f32>, material: u32 }
// Materials: 1 stone, 2 wood, 3 iron, 4 copper, 5 crystal, 6 fire, 7 paper.
const BLOCKS = array<Block, 51>(
    Block(vec3(0.0,-0.2,0.0),vec3(9.0,0.2,7.0),1u),
    Block(vec3(-4.1,2.5,-3.8),vec3(3.1,2.5,0.3),1u),
    Block(vec3(5.4,2.5,-3.8),vec3(1.7,2.5,0.3),1u),
    Block(vec3(1.35,0.5,-3.8),vec3(2.35,0.5,0.3),1u),
    Block(vec3(1.35,4.8,-3.8),vec3(2.35,0.4,0.3),1u),
    Block(vec3(1.35,1.05,-3.6),vec3(2.6,0.12,0.55),1u),
    Block(vec3(1.35,2.8,-3.7),vec3(0.065,1.7,0.12),3u),
    Block(vec3(1.35,3.45,-3.7),vec3(2.4,0.065,0.12),3u),
    Block(vec3(1.4,1.25,0.0),vec3(2.15,0.15,0.9),2u),
    Block(vec3(-0.45,0.55,0.60),vec3(0.12,0.6,0.14),3u),
    Block(vec3(3.25,0.55,0.60),vec3(0.12,0.6,0.14),3u),
    Block(vec3(1.4,0.3,0.1),vec3(1.9,0.08,0.7),2u),
    Block(vec3(2.1,1.48,-0.12),vec3(0.54,0.08,0.44),3u),
    Block(vec3(2.1,1.95,-0.12),vec3(0.17,0.42,0.17),5u),
    Block(vec3(2.1,2.57,-0.12),vec3(0.11,0.23,0.11),5u),
    Block(vec3(1.55,1.92,-0.12),vec3(0.08,0.40,0.10),3u),
    Block(vec3(2.65,1.92,-0.12),vec3(0.08,0.40,0.10),3u),
    Block(vec3(2.1,2.34,-0.12),vec3(0.63,0.06,0.22),4u),
    Block(vec3(0.4,1.49,0.24),vec3(0.52,0.035,0.36),7u),
    Block(vec3(0.33,1.535,0.24),vec3(0.012,0.01,0.33),4u),
    Block(vec3(0.94,1.56,0.51),vec3(0.05,0.03,0.21),4u),
    Block(vec3(-2.1,0.78,-1.6),vec3(0.80,0.78,0.68),3u),
    Block(vec3(-2.1,0.67,-0.90),vec3(0.57,0.40,0.03),6u),
    Block(vec3(-2.1,1.68,-1.6),vec3(0.95,0.12,0.8),3u),
    Block(vec3(-2.1,3.1,-1.8),vec3(0.23,1.35,0.23),3u),
    Block(vec3(-0.6,3.8,-2.4),vec3(0.07,1.0,0.07),4u),
    Block(vec3(1.2,4.05,-2.4),vec3(1.8,0.07,0.07),4u),
    Block(vec3(3.0,2.8,-2.4),vec3(0.07,1.3,0.07),4u),
    Block(vec3(3.0,1.66,-1.7),vec3(0.07,0.07,0.70),4u),
    Block(vec3(4.4,1.0,0.6),vec3(0.45,1.0,0.5),8u),
    Block(vec3(4.4,2.18,0.6),vec3(0.55,0.18,0.6),3u),
    Block(vec3(4.4,2.50,0.25),vec3(0.40,0.14,0.09),4u),
    Block(vec3(-3.5,7.0,-14.0),vec3(0.10,13.0,0.12),3u),
    Block(vec3(-3.5,1.0,-14.0),vec3(0.65,0.45,0.40),3u),
    Block(vec3(-3.5,1.15,-13.55),vec3(0.4,0.12,0.015),5u),
    Block(vec3(0.4,4.85,0.8),vec3(5.8,0.18,0.22),2u),
    Block(vec3(-3.6,2.4,0.8),vec3(0.18,2.4,0.22),2u),
    Block(vec3(5.0,2.4,0.8),vec3(0.18,2.4,0.22),2u),
    Block(vec3(0.4,4.6,-2.7),vec3(5.8,0.12,0.17),2u),
    Block(vec3(-2.4,2.7,-3.2),vec3(1.35,0.08,0.35),2u),
    Block(vec3(-3.25,2.99,-3.2),vec3(0.18,0.22,0.20),8u),
    Block(vec3(-2.70,2.92,-3.2),vec3(0.17,0.15,0.20),4u),
    Block(vec3(-1.95,2.99,-3.2),vec3(0.38,0.22,0.20),3u),
    Block(vec3(4.4,2.38,0.6),vec3(0.34,0.035,0.34),9u),
    Block(vec3(4.0,2.15,1.22),vec3(0.055,0.055,0.04),4u),
    Block(vec3(4.3,2.15,1.22),vec3(0.055,0.055,0.04),4u),
    Block(vec3(4.65,2.15,1.22),vec3(0.055,0.055,0.04),4u),
    Block(vec3(4.4,1.1,1.11),vec3(0.35,0.48,0.02),3u),
    Block(vec3(0.1,1.46,-0.5),vec3(0.55,0.04,0.04),2u),
    Block(vec3(0.68,1.46,-0.5),vec3(0.06,0.05,0.05),5u),
    Block(vec3(3.4,1.53,-0.55),vec3(0.10,0.13,0.10),7u)
);
fn hash(p: vec3<f32>) -> f32 { return fract(sin(dot(p,vec3(127.1,311.7,74.7))) * 43758.5453); }
fn box_hit(ro: vec3<f32>, rd: vec3<f32>, b: Block) -> Hit {
    let inverse = 1.0 / (rd + vec3<f32>(0.000001));
    let a = (b.center - b.extent - ro) * inverse;
    let z = (b.center + b.extent - ro) * inverse;
    let near = min(a,z); let far = max(a,z);
    let lo = max(near.x,max(near.y,near.z)); let hi = min(far.x,min(far.y,far.z));
    if lo > hi || lo < 0.002 { return Hit(1000.0,vec3(0.0),0u); }
    var n = vec3(0.0,0.0,-sign(rd.z));
    if near.x >= near.y && near.x >= near.z { n = vec3(-sign(rd.x),0.0,0.0); }
    else if near.y >= near.z { n = vec3(0.0,-sign(rd.y),0.0); }
    return Hit(lo,n,b.material);
}
fn trace(ro: vec3<f32>, rd: vec3<f32>) -> Hit {
    var result = Hit(1000.0,vec3(0.0),0u);
    for (var i = 0u; i < 51u; i++) {
        var b = BLOCKS[i];
        if i == 33u || i == 34u { b.center.y += sin(clock.x * 0.08) * 1.5; }
        let hit = box_hit(ro,rd,b);
        if hit.distance < result.distance { result = hit; }
    }
    // Wound copper and separate grate bars, not a generic glowing ornament.
    for (var i = 0u; i < 7u; i++) {
        let y = 1.62 + f32(i) * 0.09;
        let coil = box_hit(ro,rd,Block(vec3(2.1,y,-0.12),vec3(0.36,0.025,0.30),4u));
        if coil.distance < result.distance { result = coil; }
    }
    for (var i = 0u; i < 6u; i++) {
        let bar = box_hit(ro,rd,Block(vec3(-2.60+f32(i)*0.2,0.67,-0.85),vec3(0.035,0.43,0.035),3u));
        if bar.distance < result.distance { result = bar; }
    }
    return result;
}
fn sky(rd: vec3<f32>) -> vec3<f32> {
    var c = mix(vec3(0.075,0.14,0.17),vec3(0.018,0.035,0.08),clamp(rd.y*1.6,0.0,1.0));
    let ridge = sin(rd.x*18.0)*0.025 + sin(rd.x*41.0)*0.014;
    if rd.y < ridge + 0.04 { c = vec3(0.055,0.105,0.12); }
    if rd.y < ridge*0.6 - 0.035 { c = vec3(0.028,0.06,0.065); }
    let clouds = smoothstep(0.71,0.88,sin(rd.x*5.0+clock.x*0.009)*0.22+sin(rd.y*31.0)*0.3+0.5);
    return c + vec3(0.045,0.05,0.05)*clouds*max(rd.y,0.0);
}
@fragment fn fs_main(v: Vertex) -> @location(0) vec4<f32> {
    let uv = v.uv;
    let ro = vec3(7.8,4.4,9.8);
    let forward = normalize(vec3(0.4,1.8,-0.8)-ro);
    let right = normalize(cross(forward,vec3(0.0,1.0,0.0)));
    let up = cross(right,forward);
    let xy = (uv - vec2(0.5)) * vec2(clock.y,-1.0);
    let rd = normalize(forward + (right*xy.x + up*xy.y)*0.83);
    let hit = trace(ro,rd);
    var color = sky(rd);
    if hit.material > 0u {
        let point = ro + rd * hit.distance;
        let grain = hash(floor(point*20.0));
        var albedo = vec3(0.31,0.34,0.33);
        var emission = vec3(0.0);
        if hit.material == 1u {
            let face = select(point.xy,point.xz,abs(hit.normal.y)>0.5);
            let row = floor(face.y*2.0);
            let brick = fract(face*vec2(1.0,2.0)+vec2(row*0.5,0.0));
            albedo *= select(0.90+grain*0.15,0.58,min(brick.x,brick.y)<0.035);
        } else if hit.material == 2u {
            albedo = vec3(0.31,0.19,0.10)*(0.8+hash(floor(point*vec3(3.0,40.0,28.0)))*0.3);
            if fract(point.x*3.0)<0.025 { albedo *= 0.4; }
        } else if hit.material == 3u {
            albedo = vec3(0.12,0.15,0.17)*(0.75+grain*0.5);
        } else if hit.material == 4u {
            albedo = vec3(0.757,0.4,0.192)*(0.82+grain*0.25);
        } else if hit.material == 5u {
            albedo = vec3(0.396,0.718,0.686);
            let row = floor(point.y*20.0);
            emission = albedo*(0.30+0.24*pow(0.5+0.5*sin(row*0.34-clock.x*0.7),4.0));
        } else if hit.material == 6u {
            albedo = vec3(0.757,0.4,0.192);
            emission = albedo*(0.7+0.15*sin(clock.x*1.9+floor(point.x*20.0)));
        } else if hit.material == 7u {
            albedo = vec3(0.922,0.882,0.773);
            let ink = step(0.63,fract(point.z*18.0))*step(0.18,fract(point.x*5.0));
            albedo *= 1.0-ink*0.6;
        } else if hit.material == 8u {
            albedo = mix(vec3(0.13,0.26,0.23),vec3(0.34,0.21,0.11),step(0.94,grain));
            let rivet = fract(point.xy*4.0);
            if rivet.x < 0.12 && rivet.y < 0.10 { albedo = vec3(0.5,0.48,0.38); }
        } else if hit.material == 9u {
            albedo = vec3(0.035,0.085,0.07);
            let panel = floor(point.xz*20.0);
            let circuit = step(0.8,fract(panel.x/5.0))*step(0.15,fract(panel.y/9.0));
            emission = vec3(0.396,0.718,0.686)*circuit*0.4;
        }
        let sun = normalize(vec3(-0.4,0.8,-0.6));
        let shadow = trace(point+hit.normal*0.015,sun).distance;
        let daylight = max(dot(hit.normal,sun),0.0)*select(0.06,0.60,shadow>40.0);
        let furnace = vec3(-2.1,0.7,-0.7)-point;
        let crystal = vec3(2.1,2.3,-0.12)-point;
        let warmth = max(dot(hit.normal,normalize(furnace)),0.0)*4.5/(1.0+dot(furnace,furnace));
        let cool = max(dot(hit.normal,normalize(crystal)),0.0)*2.2/(1.0+dot(crystal,crystal));
        let lighting = vec3(0.16,0.21,0.25)+vec3(0.65,0.78,0.85)*daylight
            + vec3(1.0,0.31,0.075)*warmth + vec3(0.18,0.72,0.65)*cool;
        color = srgb_decode(albedo)*lighting + emission*0.55;
        let spec = pow(max(dot(reflect(-sun,hit.normal),-rd),0.0),24.0);
        if hit.material == 4u || hit.material == 3u { color += vec3(0.38,0.28,0.13)*spec*daylight; }
        color = mix(color,vec3(0.05,0.08,0.09),1.0-exp(-hit.distance*0.009));
    }
    color = srgb_encode(color);
    // A reserved quiet left side makes type legible without glass UI cards.
    let shade = mix(0.18,1.0,smoothstep(0.15,0.62,uv.x));
    let vignette = 1.0-0.30*dot(uv-vec2(0.5),uv-vec2(0.5));
    color *= shade*vignette;
    return vec4(color,1.0);
}
