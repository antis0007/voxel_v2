@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var<uniform> resolve: vec4<f32>; // exposure, saturation, AA, debug

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(p[index], 0.0, 1.0);
}

// Adapted to WGSL from KhronosGroup/ToneMapping, PBR Neutral.
// Copyright 2024 The Khronos Group, Inc.
// SPDX-License-Identifier: Apache-2.0
// See docs/licenses/Khronos-ToneMapping-Apache-2.0.txt.
fn neutral_tonemap(input: vec3<f32>) -> vec3<f32> {
    let floor_channel = min(input.r, min(input.g, input.b));
    let offset = select(0.04, floor_channel - 6.25 * floor_channel * floor_channel, floor_channel < 0.08);
    var color = input - vec3<f32>(offset);
    let peak = max(color.r, max(color.g, color.b));
    if peak < 0.76 { return color; }
    let compressed_peak = 1.0 - 0.0576 / (peak - 0.52);
    color *= compressed_peak / peak;
    let white_mix = 1.0 - 1.0 / (0.15 * (peak - compressed_peak) + 1.0);
    return mix(color, vec3<f32>(compressed_peak), white_mix);
}

fn sample_color(uv: vec2<f32>) -> vec3<f32> {
    let hdr = max(textureSampleLevel(scene, linear_sampler, uv, 0.0).rgb, vec3<f32>(0.0));
    if resolve.w > 0.5 { return hdr; }
    let c = hdr * resolve.x;
    let mapped = neutral_tonemap(c);
    let grey = dot(mapped, vec3<f32>(0.2126, 0.7152, 0.0722));
    return mix(vec3<f32>(grey), mapped, resolve.y);
}

fn luma(c: vec3<f32>) -> f32 {
    // Perceptual contrast from the linear scene sample.
    return sqrt(max(dot(c, vec3<f32>(0.299, 0.587, 0.114)), 0.0));
}

fn display_color(color: vec3<f32>, position: vec2<f32>) -> vec4<f32> {
    if resolve.w > 0.5 { return vec4<f32>(color, 1.0); }
    // Fixed, zero-mean sub-code-value dither prevents the 8-bit swapchain from
    // turning smooth sunset/haze gradients into bands. No animated grain.
    let p = vec2<u32>(position);
    var h = p.x * 1664525u + p.y * 1013904223u;
    h = (h ^ (h >> 16u)) * 2246822519u;
    h = (h ^ (h >> 13u)) * 3266489917u;
    h = h ^ (h >> 16u);
    let noise = f32(h & 65535u) / 65536.0 - 0.5;
    // Approximate one sRGB code value in linear space, before hardware encode.
    let slope = select(vec3<f32>(1.0 / 12.92),
        (2.4 / 1.055) * pow(max(color, vec3<f32>(0.0031308)), vec3<f32>(0.5833333)),
        color > vec3<f32>(0.0031308));
    return vec4<f32>(clamp(color + slope * (noise / 255.0), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = 1.0 / vec2<f32>(textureDimensions(scene));
    let uv = position.xy * pixel;
    let center = sample_color(uv);
    if resolve.z < 0.5 { return display_color(center, position.xy); }
    let m = luma(center);
    let north = sample_color(uv - vec2<f32>(0.0, pixel.y));
    let south = sample_color(uv + vec2<f32>(0.0, pixel.y));
    let west = sample_color(uv - vec2<f32>(pixel.x, 0.0));
    let east = sample_color(uv + vec2<f32>(pixel.x, 0.0));
    let n = luma(north); let s = luma(south);
    let w = luma(west); let e = luma(east);
    let low = min(m, min(min(n, s), min(w, e)));
    let high = max(m, max(max(n, s), max(w, e)));
    let contrast = high - low;
    if contrast < max(0.055, high * 0.16) { return display_color(center, position.xy); }
    // Restrict reconstruction to the adjacent pixel at an actual contour.
    // Long directional taps smeared bark, tiny leaves and surface painting.
    let horizontal = abs(n + s - 2.0 * m) > abs(w + e - 2.0 * m);
    let neighbor = select(select(west, east, abs(e-m) > abs(w-m)),
                          select(north, south, abs(s-m) > abs(n-m)), horizontal);
    let average = (n + s + w + e) * 0.25;
    let subpixel = smoothstep(0.10, 0.65, abs(average-m) / max(contrast, 0.001));
    return display_color(mix(center, neighbor, subpixel * 0.22), position.xy);
}
