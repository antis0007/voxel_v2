@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
struct ResolveParams {
    // exposure, saturation, AA, debug
    resolve: vec4<f32>,
    // x: pixel block scale, y: dither strength, z: palette levels, w: edge strength
    style: vec4<f32>,
    // x: bloom strength, y: bloom threshold, z: magic time, w: vignette
    style2: vec4<f32>,
    // rgb: active spell colour, w: magic intensity
    magic: vec4<f32>,
    // rgb: sky-event colour multiplier, w: amount (0 = untouched).
    grade: vec4<f32>,
    // Underground zone veil: rgb colour, w amount (0 = outside any zone).
    zone_veil: vec4<f32>,
    // x: desaturation, y: pulse Hz, z: swirl, w: spell gain.
    zone_look: vec4<f32>,
    // x: movement boost 0..1, y: SSAO scale, z: legacy grade,
    // w: vertical art-pixel size in scene texels (set by PostProcess).
    motion: vec4<f32>,
};
@group(0) @binding(2) var<uniform> params: ResolveParams;
// Reverse-Z scene depth (1 near, 0 far), read for screen-space ambient occlusion.
@group(0) @binding(3) var scene_depth: texture_depth_2d;
@group(0) @binding(4) var write_noise: texture_2d<f32>;

// Only used by the headless A/B profiler; normal pipelines use pixel centres.
override REFERENCE_RESOLVE_OFFSET: bool = false;

// How much the nearest contact can darken the base colour (0 = none).
const SSAO_STRENGTH: f32 = 0.6;

// Ordered 8x8 Bayer matrix. The pattern is anchored to output art pixels, so
// it dithers the final write without crawling screen grain. A 4x4 matrix only
// has sixteen thresholds, which is visibly coarse once the palette is removed
// from smooth sky/fog gradients.
const BAYER8: array<f32, 64> = array<f32, 64>(
     0.0, 32.0,  8.0, 40.0,  2.0, 34.0, 10.0, 42.0,
    48.0, 16.0, 56.0, 24.0, 50.0, 18.0, 58.0, 26.0,
    12.0, 44.0,  4.0, 36.0, 14.0, 46.0,  6.0, 38.0,
    60.0, 28.0, 52.0, 20.0, 62.0, 30.0, 54.0, 22.0,
     3.0, 35.0, 11.0, 43.0,  1.0, 33.0,  9.0, 41.0,
    51.0, 19.0, 59.0, 27.0, 49.0, 17.0, 57.0, 25.0,
    15.0, 47.0,  7.0, 39.0, 13.0, 45.0,  5.0, 37.0,
    63.0, 31.0, 55.0, 23.0, 61.0, 29.0, 53.0, 21.0,
);

fn bayer8(p: vec2<u32>) -> f32 {
    return BAYER8[(p.y & 7u) * 8u + (p.x & 7u)] / 64.0 - 0.5;
}

// Palette quantization only. The resolve target is Rgba16Float, so smooth
// gradients keep their full linear precision here; the present pass performs
// the final 8-bit write dither at native screen resolution. Quantization is
// intentionally skipped on low-contrast areas: a smooth sky/fog gradient is
// exactly where a finite RGB palette becomes visible banding, and the
// pixel-grid art direction already carries the stylisation there.
fn pixel_finish(c: vec3<f32>, position: vec2<f32>, preserve_smooth: bool) -> vec3<f32> {
    let key = vec2<u32>(u32(position.x), u32(position.y));
    if !preserve_smooth {
        let levels = params.style.z;
        if (levels >= 1.5) {
            let s = srgb_encode(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
            let palette_dither = bayer8(key) * clamp(params.style.y, 0.0, 1.5) / levels;
            let q = clamp(floor(s * levels + 0.5 + palette_dither) / levels,
                vec3<f32>(0.0), vec3<f32>(1.0));
            return srgb_decode(q);
        }
    }
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

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
    // Neutral highlights still roll to white. A chromatic sun (gold, orange,
    // rose) keeps its hue, or the disk clips to white inside a colored sky.
    let sat = clamp(1.0 - min(color.r, min(color.g, color.b)) / max(compressed_peak, 1e-4), 0.0, 1.0);
    // Only up to ordinary highlight levels: a saturated sky band (peak 2-5)
    // and fire keep their hue, but the solar disc (peak ~30+) rolls to pale
    // gold. Holding hue at every level mapped disc and sky to one orange.
    let keep_hue = smoothstep(0.28, 0.62, sat) * (1.0 - smoothstep(6.0, 24.0, peak));
    return mix(color, vec3<f32>(compressed_peak), white_mix * (1.0 - keep_hue));
}

// A restrained split-tone gives depth to the neutral tonemap: cool skylight in
// the shadows and a warm solar bias in the highlights. The old frame was
// technically correct but read flat because every luminance band shared the
// same hue.
fn split_tone(c: vec3<f32>) -> vec3<f32> {
    let y = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let shadow = 1.0 - smoothstep(0.06, 0.52, y);
    let highlight = smoothstep(0.42, 0.94, y);
    if params.motion.z > 0.5 {
        return c * (vec3<f32>(1.0) + vec3<f32>(0.035, -0.025, 0.065) * shadow
            + vec3<f32>(0.035, 0.015, -0.025) * highlight);
    }
    let tint = vec3<f32>(1.0)
        + vec3<f32>(-0.034, 0.006, 0.052) * shadow
        + vec3<f32>(0.028, 0.010, -0.018) * highlight;
    return c * tint;
}

fn sample_color(uv: vec2<f32>) -> vec3<f32> {
    let hdr = max(textureSampleLevel(scene, linear_sampler, uv, 0.0).rgb, vec3<f32>(0.0));
    // Debug views expose raw HDR values and must not receive an art grade.
    if params.resolve.w > 0.5 { return hdr; }
    let c = hdr * params.resolve.x;
    let mapped = neutral_tonemap(c);
    let grey = dot(mapped, vec3<f32>(0.2126, 0.7152, 0.0722));
    var graded = split_tone(mix(vec3<f32>(grey), mapped, params.resolve.y));
    // Sky events (blood moon, ember dusk, violet hour) wash the whole frame, so
    // terrain, water and cloud all sit in the same coloured light.
    graded *= mix(vec3<f32>(1.0), max(params.grade.rgb, vec3<f32>(0.0)), clamp(params.grade.w, 0.0, 1.0));
    // Selective vibrance: lift the less-saturated sky and cloud chroma without
    // pushing already vivid grass or spell colours into neon. This is the
    // difference between a flat "all saturation" slider and an art grade.
    let hi = max(graded.r, max(graded.g, graded.b));
    let lo = min(graded.r, min(graded.g, graded.b));
    let sat = (hi - lo) / max(hi, 1e-4);
    let gain = 1.0 + 0.08 * (1.0 - smoothstep(0.18, 0.82, sat));
    return mix(vec3<f32>(dot(graded, vec3<f32>(0.2126, 0.7152, 0.0722))), graded, gain);
}

fn luma(c: vec3<f32>) -> f32 {
    // Perceptual contrast from the linear scene sample.
    return sqrt(max(dot(c, vec3<f32>(0.299, 0.587, 0.114)), 0.0));
}

// Golden-angle disk: angle = i * 2.3999632, radius = sqrt((i + 0.5) / 12).
const SSAO_DISK = array<vec2<f32>, 12>(
    vec2<f32>(0.204124145, 0.000000000),
    vec2<f32>(-0.260699260, 0.238821892),
    vec2<f32>(0.039904174, -0.454687794),
    vec2<f32>(0.328594579, 0.428593361),
    vec2<f32>(-0.603011408, -0.106664154),
    vec2<f32>(0.571224981, -0.363366694),
    vec2<f32>(-0.191063468, 0.710747084),
    vec2<f32>(-0.364379143, -0.701589510),
    vec2<f32>(0.790556741, 0.288709841),
    vec2<f32>(-0.822442395, 0.339492523),
    vec2<f32>(0.396471373, -0.847236951),
    vec2<f32>(0.292982751, 0.934074109),
);

// Screen-space ambient occlusion from the depth buffer. A tap closer to the eye
// than the centre occludes it. The test uses depth ratios, since
// z_tap / z_centre = d_centre / d_tap for reverse-Z, so it needs no near plane
// and is independent of voxel scale. Gaps past the falloff are ignored, so far
// silhouettes cannot halo onto near geometry. `pixel` is in depth-buffer texels.
fn ssao_factor(pixel: vec2<f32>) -> f32 {
    if params.motion.y <= 0.0 { return 1.0; }
    let dims = vec2<i32>(textureDimensions(scene_depth));
    let max_px = dims - vec2<i32>(1);
    let centre = clamp(vec2<i32>(pixel), vec2<i32>(0), max_px);
    let d_centre = textureLoad(scene_depth, centre, 0);
    if (d_centre <= 0.0) {
        return 1.0; // far plane: sky
    }
    let radius = 11.0 * clamp(f32(dims.y) / 1080.0, 0.6, 2.0);
    // Interleaved gradient noise rotates the disc per pixel, so 12 taps average
    // into fine grain instead of a fixed pattern.
    let spin = fract(52.9829189 * fract(0.06711056 * pixel.x + 0.00583715 * pixel.y)) * 6.2831853;
    let axis = vec2<f32>(cos(spin), sin(spin)) * radius;
    let rotation = mat2x2<f32>(axis, vec2<f32>(-axis.y, axis.x));
    var occluded = 0.0;
    for (var i: u32 = 0u; i < 12u; i = i + 1u) {
        let offset = vec2<i32>(rotation * SSAO_DISK[i]);
        let tap = clamp(centre + offset, vec2<i32>(0), max_px);
        let d_tap = textureLoad(scene_depth, tap, 0);
        if (d_tap <= 0.0) {
            continue;
        }
        // Positive when the tap is nearer the eye than the centre.
        let gap = 1.0 - d_centre / d_tap;
        // Thresholds sit above the depth slope of a flat plane seen at a grazing
        // angle, so open ground is not darkened; only real steps occlude.
        occluded += smoothstep(0.02, 0.10, gap) * (1.0 - smoothstep(0.12, 0.35, gap));
    }
    return 1.0 - SSAO_STRENGTH * params.motion.y * occluded / 12.0;
}

fn bloom_weight(c: vec3<f32>, threshold: f32) -> vec3<f32> {
    return c * smoothstep(threshold, threshold + 0.22, luma(c));
}

fn resolve_color(position: vec4<f32>) -> vec4<f32> {
    let scene_size = vec2<f32>(textureDimensions(scene));
    let texel = 1.0 / scene_size;
    let scale = max(params.style.x, 0.1);
    let source_scale = vec2<f32>(scale, select(scale, params.motion.w, params.motion.w > 0.0));
    if params.resolve.w > 0.5 {
        // Debug views stay at native resolution and keep their diagnostic
        // colours. Posterizing them would destroy the very values they expose.
        return vec4<f32>(sample_color(position.xy * texel * source_scale), 1.0);
    }

    // The pass renders one output texel per art pixel. Convert that texel to
    // the centre of its scene block; the present pass then upscales it with an
    // exact integer nearest lookup. UI is composited after presentation, so
    // text and HUD remain native-resolution.
    // Art-pixel size in scene texels. Below 1 the scene is smaller than the window
    // and this pass upsamples it.
    // Fragment position already includes the half-pixel centre. Adding 0.5
    // here blends adjacent texels at native resolution and offsets depth/AO.
    let source_pixel = (position.xy + select(vec2<f32>(0.0), vec2<f32>(0.5),
        REFERENCE_RESOLVE_OFFSET)) * source_scale;
    let screen_uv = clamp(
        source_pixel / scene_size,
        vec2<f32>(0.0),
        vec2<f32>(1.0),
    );
    var uv = screen_uv;
    let aspect = scene_size.x / max(scene_size.y, 1.0);
    let screen_centered = vec2<f32>((screen_uv.x - 0.5) * aspect, screen_uv.y - 0.5);
    let sample_stride = source_scale * texel;
    let zone_amount = clamp(params.zone_veil.w, 0.0, 1.0);
    let magic_amount = clamp(params.magic.w * max(params.zone_look.w, 0.0), 0.0, 1.0);
    let magic_tint = max(params.magic.rgb, vec3<f32>(0.0));
    if (magic_amount > 0.001) {
        // Screen-space arcane lens: the image bends around the centre while
        // keeping every sample snapped to the same pixel block.
        let centered = screen_uv - vec2<f32>(0.5);
        let radius = length(screen_centered);
        let spiral = vec2<f32>(-centered.y, centered.x)
            * sin(radius * 22.0 - params.style2.z * 2.6);
        let ripple = vec2<f32>(
            sin(params.style2.z * 1.7 + uv.y * 15.0),
            cos(params.style2.z * 1.3 + uv.x * 17.0),
        );
        uv = clamp(
            uv + (spiral * 0.0032 + ripple * 0.0013)
                * magic_amount * smoothstep(0.04, 0.70, radius),
            vec2<f32>(0.0),
            vec2<f32>(1.0),
        );
    }
    if (zone_amount > 0.001 && params.zone_look.z > 0.01) {
        // Zone swirl: slow, low-amplitude drift that is stronger at the edges.
        let zr = length(screen_centered);
        let zt = params.style2.z * (0.4 + params.zone_look.y * 0.5);
        uv = clamp(
            uv + vec2<f32>(sin(zt + uv.y * 9.0), cos(zt * 0.8 + uv.x * 11.0))
                * 0.0018 * params.zone_look.z * zone_amount * smoothstep(0.1, 0.8, zr),
            vec2<f32>(0.0),
            vec2<f32>(1.0),
        );
    }
    let vignette_centered = screen_centered;
    var center = sample_color(uv);

    let north = sample_color(clamp(uv - vec2<f32>(0.0, sample_stride.y), vec2<f32>(0.0), vec2<f32>(1.0)));
    let south = sample_color(clamp(uv + vec2<f32>(0.0, sample_stride.y), vec2<f32>(0.0), vec2<f32>(1.0)));
    let west = sample_color(clamp(uv - vec2<f32>(sample_stride.x, 0.0), vec2<f32>(0.0), vec2<f32>(1.0)));
    let east = sample_color(clamp(uv + vec2<f32>(sample_stride.x, 0.0), vec2<f32>(0.0), vec2<f32>(1.0)));

    // Noita-style glow: two stacked halo layers around bright scene light.
    // The tight ring keeps torches and emissive voxels chunky; the wide ring
    // lets spells and lanterns bleed a soft, saturated colour into nearby dark
    // pixels. Each tap is still one block, so the halo never becomes a
    // native-resolution Gaussian smear.
    var bloom = vec3<f32>(0.0);
    let bloom_strength = max(params.style2.x, 0.0);
    if (bloom_strength > 0.0) {
        let threshold = max(params.style2.y, 0.0);
        let tight = (bloom_weight(north, threshold) + bloom_weight(south, threshold)
            + bloom_weight(west, threshold) + bloom_weight(east, threshold)) * 0.25;

        let wide_radius = sample_stride * 4.25;
        let wide_offsets = array<vec2<f32>, 4>(
            vec2<f32>( 0.9,  0.9), vec2<f32>(-0.9,  0.9),
            vec2<f32>( 0.9, -0.9), vec2<f32>(-0.9, -0.9),
        );
        var wide = vec3<f32>(0.0);
        var wide_peak = 0.0;
        var surround = vec3<f32>(0.0);
        for (var i: u32 = 0u; i < 4u; i = i + 1u) {
            let tap_uv = clamp(uv + wide_offsets[i] * wide_radius, vec2<f32>(0.0), vec2<f32>(1.0));
            let tap = sample_color(tap_uv);
            wide += bloom_weight(tap, threshold);
            wide_peak = max(wide_peak, luma(tap));
            surround += tap;
        }
        wide *= 0.25;
        surround *= 0.25;
        // A lone bright block must not become a floating cross. The centre only
        // joins when its neighbourhood already carries light.
        let support = smoothstep(threshold * 0.35, threshold, wide_peak);
        bloom = tight * 0.58 + wide * 0.52 + bloom_weight(center, threshold) * support * 0.22;
        // Glow marks light that stands out from its surroundings (a torch in a
        // cave, a spell in dusk). Where the whole neighbourhood is already
        // bright, as in daylit sky, snow or sand, it only washes the frame out,
        // so fade it down. A small residual keeps the sun disk's halo.
        let ambient_bright = smoothstep(0.50, 0.85, luma(surround));
        bloom *= mix(1.0, 0.12, ambient_bright);
    }
    let n = luma(north);
    let s = luma(south);
    let w = luma(west);
    let e = luma(east);

    // In near-black regions, a single sampled texel can pop between block
    // positions as the camera moves. Blend toward the block average only in
    // the dark; lit surfaces keep their crisp pixel-art colour.
    let dark = 1.0 - smoothstep(0.015, 0.18, luma(center));
    if (dark > 0.0) {
        let block_average = (center + north + south + west + east) * 0.2;
        center = mix(center, block_average, dark * 0.35);
    }
    let m = luma(center);
    let low = min(m, min(min(n, s), min(w, e)));
    let high = max(m, max(max(n, s), max(w, e)));
    let contrast = high - low;

    // A block-scale dark contour gives voxel silhouettes a drawn edge without
    // the smeared halos of native-resolution FXAA.
    let edge = smoothstep(0.12, 0.36, contrast) * clamp(params.style.w, 0.0, 1.0);
    // Screen-blend the glow: it brightens dark pixels fully but approaches 1.0
    // asymptotically on bright ones, so adding bloom can never clip the frame.
    // Occlusion darkens the lit scene only; the bloom glow added below stays intact.
    // Depth follows the same lens/swirl as scene colour, including at reduced
    // scene resolution. Otherwise contact shadows slide away during spells.
    let base_color = center * (1.0 - edge) * ssao_factor(uv * scene_size);
    let glow = clamp(bloom * bloom_strength, vec3<f32>(0.0), vec3<f32>(1.0));
    var color = base_color + glow * (vec3<f32>(1.0) - clamp(base_color, vec3<f32>(0.0), vec3<f32>(1.0)));

    // A restrained vignette keeps the pixelated light sources focused without
    // turning the frame into a dark tunnel.
    let vignette = clamp(params.style2.w, 0.0, 0.5);
    if (vignette > 0.0) {
        let radius = dot(vignette_centered, vignette_centered) * 2.0;
        color *= 1.0 - vignette * smoothstep(0.18, 1.0, radius);
    }

    if (zone_amount > 0.001) {
        let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
        color = mix(color, vec3<f32>(luma), clamp(params.zone_look.x, 0.0, 0.9) * zone_amount);
        // Pulse is time based so cadence is independent of frame rate.
        let pulse = 0.5 + 0.5 * sin(params.style2.z * 6.2832 * params.zone_look.y);
        let veil = max(params.zone_veil.rgb, vec3<f32>(0.0));
        // Multiplicative so bright lights keep their own colour; additive
        // lift keeps pure black from staying dead in a lit layer.
        color = mix(color, color * (0.55 + 0.9 * veil), zone_amount * 0.55);
        color += veil * zone_amount * (0.004 + 0.006 * pulse);
    }

    if (magic_amount > 0.001) {
        // Spell grade: a restrained radial lens, edge-weighted chromatic
        // separation, a slow colour wash, one expanding arcane shock ring and
        // fine radial filaments. Everything is time-based, not frame-based, so
        // the effect keeps the same cadence at low frame rates.
        let radius = length(screen_centered);
        let edge = smoothstep(0.10, 0.68, radius);
        let chroma = sample_stride * (0.55 + magic_amount * 2.1) * mix(0.35, 1.0, edge);
        let red_shift = sample_color(clamp(
            uv + vec2<f32>(chroma.x, chroma.y * 0.30),
            vec2<f32>(0.0), vec2<f32>(1.0))).r;
        let blue_shift = sample_color(clamp(
            uv - vec2<f32>(chroma.x * 0.70, chroma.y),
            vec2<f32>(0.0), vec2<f32>(1.0))).b;
        color.r = mix(color.r, red_shift, magic_amount * (0.26 + edge * 0.24));
        color.b = mix(color.b, blue_shift, magic_amount * (0.26 + edge * 0.24));

        let angle = atan2(screen_centered.y, screen_centered.x);
        let phase = params.style2.z * 2.1 + radius * 13.0;
        let trip = vec3<f32>(
            0.5 + 0.5 * sin(phase),
            0.5 + 0.5 * sin(phase + 2.094),
            0.5 + 0.5 * sin(phase + 4.188),
        );
        color = mix(color, color * (0.80 + 0.44 * trip) + magic_tint * 0.022,
            magic_amount * 0.24);

        // Screen-edge aurora and a single outward ring keep the frame feeling
        // charged without turning into a constant full-screen filter.
        let aura = edge * magic_amount;
        color += magic_tint * aura
            * (0.014 + 0.008 * sin(params.style2.z * 2.4 + radius * 19.0));
        let ring_t = fract(params.style2.z * 0.53);
        let ring = exp(-pow((radius - ring_t * 0.92) * 18.0, 2.0))
            * (1.0 - ring_t) * magic_amount;
        color += magic_tint * ring * 0.045;

        let filaments = pow(max(
            0.0,
            sin(angle * 7.0 + radius * 31.0 - params.style2.z * 3.6)
        ), 6.0);
        color += magic_tint * filaments * edge * magic_amount * 0.022;
        color += magic_tint * exp(-radius * radius * 26.0) * magic_amount * 0.026;
    }

    let boost = clamp(params.motion.x, 0.0, 1.0);
    if (boost > 0.001) {
        // Movement boost screen (after Mycopunk's BoostScreen): radial speed
        // streaks and a red/cyan smear at the frame edge, clear in the centre
        // so aim is never obscured. Streaks are angular bins that scroll
        // outward on the clock, so they read as motion at any frame rate.
        let radius = length(screen_centered);
        let edge = smoothstep(0.22, 0.75, radius);
        let dir = screen_centered / max(radius, 1e-4);
        let smear = dir * sample_stride * (1.5 + 4.5 * boost) * edge;
        let r = sample_color(clamp(uv + smear, vec2<f32>(0.0), vec2<f32>(1.0))).r;
        let b = sample_color(clamp(uv - smear, vec2<f32>(0.0), vec2<f32>(1.0))).b;
        color.r = mix(color.r, r, boost * 0.6);
        color.b = mix(color.b, b, boost * 0.6);
        let angle = atan2(screen_centered.y, screen_centered.x);
        let lane = floor(angle * 28.0);
        let lane_seed = fract(sin(lane * 12.9898) * 43758.5453);
        let streak = smoothstep(0.80, 1.0,
            fract(radius * 2.2 - params.style2.z * (2.0 + lane_seed * 1.5) + lane_seed))
            * step(0.55, lane_seed);
        let line = streak * edge * edge * boost;
        color = mix(color, vec3<f32>(0.92, 0.96, 1.0), line * 0.22);
        color *= 1.0 - edge * boost * 0.10;
    }

    // Optional native-scale reconstruction remains available when pixel scale
    // is 1 and the user has antialiasing enabled.
    if (scale <= 1.5 && params.resolve.z > 0.5 && contrast >= max(0.055, high * 0.16)) {
        let horizontal = abs(n + s - 2.0 * m) > abs(w + e - 2.0 * m);
        let neighbor = select(
            select(west, east, abs(e - m) > abs(w - m)),
            select(north, south, abs(s - m) > abs(n - m)),
            horizontal,
        );
        let average = (n + s + w + e) * 0.25;
        let subpixel = smoothstep(0.10, 0.65, abs(average - m) / max(contrast, 0.001));
        color = mix(color, neighbor, subpixel * 0.22);
    }

    // A smooth local neighbourhood is the signature of sky, horizon haze and
    // soft fog. Preserve those as continuous 8-bit gradients; keep the palette
    // identity on high-contrast surfaces where it reads as deliberate art.
    let preserve_smooth = contrast < 0.075 + bloom_strength * 0.01;
    // Stacked final treatment: block sampling -> two-scale glow -> contour ->
    // vignette -> spell grade -> selective palette -> fine dither.
    return vec4<f32>(pixel_finish(color, position.xy, preserve_smooth), 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return resolve_color(position);
}

@fragment
fn fs_native(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let resolved = resolve_color(position);
    // Match the reference RGBA16Float intermediate's precision without its
    // full-screen store, texture read, allocation or second render pass.
    let rg = unpack2x16float(pack2x16float(resolved.rg));
    let ba = unpack2x16float(pack2x16float(resolved.ba));
    let color = max(vec3<f32>(rg, ba.x), vec3<f32>(0.0));
    if params.resolve.w > 0.5 { return vec4<f32>(color, 1.0); }
    let key = vec2<i32>(position.xy) % vec2<i32>(textureDimensions(write_noise));
    let noise = textureLoad(write_noise, key, 0).r - 0.5;
    return vec4<f32>(dithered_surface_write(color, noise), 1.0);
}
