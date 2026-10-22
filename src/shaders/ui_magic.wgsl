// Native sprite coordinates, stepped time and silhouette mask. Never samples the world.
struct Magic {
    state: vec4<u32>, mask: array<vec4<u32>, 2>,
    pink: vec4<f32>, violet: vec4<f32>, mint: vec4<f32>,
    flow: vec4<f32>, detail: vec4<f32>,
}
@group(0) @binding(0) var<uniform> magic: Magic;
struct Vertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return Vertex(vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0), uv);
}
@fragment fn fs_main(v: Vertex) -> @location(0) vec4<f32> {
    let p = min(vec2<u32>(v.uv * vec2<f32>(magic.state.zw)), magic.state.zw - vec2<u32>(1u));
    let bit = p.y * 16u + p.x;
    let word = bit / 32u;
    if (magic.mask[word / 4u][word % 4u] & (1u << (bit % 32u))) == 0u { discard; }
    let time = f32(magic.state.x) * 0.125 * magic.flow.y;
    let phase = u32(floor(time * 8.0));
    let pixel = vec2<f32>(p);
    let direction = vec2<f32>(cos(magic.detail.y), sin(magic.detail.y));
    let across = vec2<f32>(-direction.y, direction.x);
    let width = magic.flow.z;
    let grain = fract(sin(dot(pixel, vec2<f32>(12.9898,78.233))) * 43758.5453);
    var color = magic.pink.rgb;
    var alpha = 0.0;
    if magic.state.y == 1u {
        // Crossed ribbons retain the authored sprite silhouette and body colour.
        let band = abs(fract(dot(pixel, direction) * magic.detail.x / 18.0 - time * 0.13) - 0.5) * 18.0;
        let crossing = abs(fract(dot(pixel, across) * magic.detail.x / 23.0 + time * 0.09 + 0.27) - 0.5) * 23.0;
        let ribbon = clamp(width + 0.5 - band, 0.0, 1.0);
        let counter = clamp(width * 0.7 + 0.5 - crossing, 0.0, 1.0);
        let a = mix(magic.pink.rgb, magic.violet.rgb, 0.5 + 0.5 * sin(time * 0.7 + dot(pixel,direction)*0.22));
        let b = mix(magic.violet.rgb, magic.mint.rgb, 0.5 + 0.5 * sin(time * 0.55 + pixel.y*0.18));
        color = mix(a, b, counter * 0.75);
        // A hint of colour separation within the ribbon, never outside the mask.
        let fringe = clamp(width + 1.5 - band, 0.0, 1.0) - ribbon;
        color = mix(color, mix(magic.mint.rgb, magic.pink.rgb, step(0.5,fract(dot(pixel,direction)*0.08-time*0.13))), fringe * magic.flow.w);
        alpha = max(ribbon * 0.68, counter * 0.46) + fringe * magic.flow.w * 0.22;
        let glint = step(0.98 - magic.detail.z * 0.08, grain) * step(0.88, sin(time * 1.7 + grain * 40.0));
        color = mix(color, vec3<f32>(0.98,0.94,1.0), glint * 0.8);
        alpha = max(alpha, glint * magic.detail.z * 0.8);
    } else if magic.state.y == 2u {
        // A broken ember seam denotes the existing unstable/volatile modifier.
        color = mix(magic.pink.rgb, magic.violet.rgb, grain);
        if (p.x + 2u * p.y + phase / 2u) % 13u < u32(ceil(width)) { alpha = 0.65; }
    } else {
        // Curse vocabulary: downward violet stitch, deliberately unlike enchantment.
        color = mix(magic.pink.rgb, magic.mint.rgb, grain * magic.flow.w);
        if (p.y + 12u - (phase / 2u) % 12u) % 12u < u32(ceil(width)) && (p.x + p.y / 3u) % 4u == 0u { alpha = 0.75; }
    }
    alpha = clamp(alpha * magic.flow.x, 0.0, 0.84);
    return vec4<f32>(color * alpha, alpha);
}
