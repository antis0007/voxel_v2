// Camera-relative temporal cloud reconstruction. History is sampled only at a
// valid projected position and clipped to this frame's local color/alpha range.
@group(2) @binding(0) var current_tex: texture_2d<f32>;
@group(2) @binding(1) var history_tex: texture_2d<f32>;
@group(2) @binding(2) var tex_sampler: sampler;

struct VsOut { @builtin(position) pos: vec4<f32>, };

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0));
    return VsOut(vec4<f32>(positions[vi], 0.0, 1.0));
}

fn layer_anchor(dir: vec3<f32>, base: f32, top: f32) -> f32 {
    let inside = cam.camera_pos.y >= base && cam.camera_pos.y <= top;
    if inside { return 64.0; }
    if abs(dir.y) < 1e-5 { return 1e9; }
    let t0 = (base - cam.camera_pos.y) / dir.y;
    let t1 = (top - cam.camera_pos.y) / dir.y;
    if max(t0, t1) <= 0.0 { return 1e9; }
    return max(min(t0, t1), 0.0) + abs(t1 - t0) * 0.35;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(current_tex));
    let texel = 1.0 / size;
    let uv = in.pos.xy / size;
    let current = textureSampleLevel(current_tex, tex_sampler, uv, 0.0);
    let debug_view = u32(light.torch_meta.w + 0.5);
    if current.a < 1e-6 { return current; }

    let dir = camera_ray(vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0));
    let b = cloud_layer_bounds();
    var anchor = layer_anchor(dir, b.low_base, b.low_top);
    if max(light.cloud_authoring[0].y, light.cloud_authoring[0].z) > 0.01 {
        anchor = min(anchor, layer_anchor(dir, b.mid_base, b.mid_top));
    }
    if light.weather_globals[3].x > 0.01 {
        anchor = min(anchor, layer_anchor(dir, b.high_base, b.high_top));
    }
    if anchor > 1e8 { return current; }

    // Subtract camera positions before projection, never invert a translated
    // matrix or project a large absolute cloud point against that matrix.
    let delta = cam.camera_pos.xyz - cam.previous_camera_pos.xyz;
    let prev_clip = cam.prev_view_proj * vec4<f32>(delta + dir * anchor, 1.0);
    let prev_ndc = prev_clip.xy / max(prev_clip.w, 1e-5);
    let history_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 0.5 - prev_ndc.y * 0.5);
    let valid = prev_clip.w > 0.0 && all(history_uv >= texel * 0.5)
        && all(history_uv <= vec2<f32>(1.0) - texel * 0.5);
    if !valid { return current; }

    var nmin = current;
    var nmax = current;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let s = textureSampleLevel(current_tex, tex_sampler,
                clamp(uv + vec2<f32>(f32(x), f32(y)) * texel, texel * 0.5, vec2<f32>(1.0) - texel * 0.5), 0.0);
            nmin = min(nmin, s);
            nmax = max(nmax, s);
        }
    }
    let history = textureSampleLevel(history_tex, tex_sampler, history_uv, 0.0);
    let clipped = clamp(history, nmin, nmax);
    let alpha_reject = smoothstep(0.03, 0.20, abs(history.a - current.a));
    let clamp_reject = smoothstep(0.03, 0.25, length(history - clipped));
    var weight = mix(0.72, 0.48, saturate1(cam.world_params.z * 8.0));
    weight *= (1.0 - alpha_reject) * (1.0 - clamp_reject);
    // No reliable depth anchor exists inside a volume during translation.
    let inside = cam.camera_pos.y >= b.low_base && cam.camera_pos.y <= b.high_top;
    if inside { weight *= 1.0 - smoothstep(0.02, 0.25, length(delta)); }
    if debug_view == DEBUG_VIEW_CLOUD_TEMPORAL_WEIGHT {
        return vec4<f32>(weight, 1.0, select(0.0, 1.0, inside), 1.0);
    }
    return mix(current, clipped, weight);
}
