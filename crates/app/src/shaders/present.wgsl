// ============================================================
// present.wgsl
//
// Nearest-neighbour presentation of the low-resolution stylised resolve.
// The resolve target is rendered at one texel per art pixel, so this pass is
// both the pixel-art upscale and the final surface write. UI is drawn after
// this pass at native resolution.
// ============================================================

@group(0) @binding(0) var source_tex : texture_2d<f32>;
// x: art-pixel size in destination pixels. y: debug bypass.
@group(0) @binding(1) var<uniform> present_params : vec4<f32>;
@group(0) @binding(2) var blue_noise_tex : texture_2d<f32>;

struct VsOut {
    @builtin(position) pos : vec4<f32>,
};

fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    let low = c * 12.92;
    let high = 1.055 * pow(max(c, vec3<f32>(0.0031308)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, c < vec3<f32>(0.0031308));
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c < vec3<f32>(0.04045));
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    let p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(p[index], 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let source_dims = vec2<i32>(textureDimensions(source_tex));
    let scale = max(present_params.x, 1.0);
    // Frag position is the centre of the destination pixel. Every destination
    // block maps to exactly one source texel; textureLoad keeps that mapping
    // integer at any window size, so the upscale cannot blur or shimmer.
    let coord = clamp(
        vec2<i32>(floor(in.pos.xy / scale)),
        vec2<i32>(0),
        source_dims - vec2<i32>(1),
    );
    let hdr = max(textureLoad(source_tex, coord, 0).rgb, vec3<f32>(0.0));
    // Diagnostic images stay undithered and native-resolution.
    if present_params.y > 0.5 {
        return vec4<f32>(hdr, 1.0);
    }
    // Final 8-bit write dither belongs here, at native resolution, not in the
    // low-resolution resolve. This turns smooth sky/fog gradients into a fine
    // one-code stipple instead of broad concentric bands.
    let key = vec2<i32>(i32(in.pos.x), i32(in.pos.y));
    let noise_dims = vec2<i32>(textureDimensions(blue_noise_tex));
    let noise = textureLoad(blue_noise_tex, key % noise_dims, 0).r - 0.5;
    var s = srgb_encode(clamp(hdr, vec3<f32>(0.0), vec3<f32>(1.0)));
    s = clamp(s + vec3<f32>(noise * (1.0 / 255.0)),
        vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(srgb_decode(s), 1.0);
}
