// ============================================================
// cloud_temporal.wgsl — Conservative temporal accumulation for clouds
//
// This pass is intentionally conservative.
//
// Why:
//   Volumetric clouds are not surfaces. Reprojecting history from an estimated
//   "first cloud hit point" works tolerably for distant exterior views, but it
//   becomes unreliable when:
//     - the camera is inside a cloud layer,
//     - the ray grazes or crosses a thick volume,
//     - the dominant scattering point shifts within the volume,
//     - cloud silhouettes are thin or rapidly changing.
//
// Design:
//   - For distant exterior views, use a slab-entry reprojection anchor.
//   - When the camera is inside any cloud slab, do NOT use world-space
//     reprojection. Fall back to same-UV history only, with much lower weight.
//   - Neighborhood clamp and alpha disagreement reject stale history.
//   - This pass should smooth undersampling, not invent detail.
//
// Notes:
//   - At full-resolution cloud rendering, temporal accumulation should usually
//     be disabled on the Rust side.
//   - History still must be invalidated on resize and large camera cuts.
// ============================================================

@group(2) @binding(0) var current_tex: texture_2d<f32>;
@group(2) @binding(1) var history_tex: texture_2d<f32>;
@group(2) @binding(2) var tex_sampler: sampler;

const DEBUG_VIEW_CLOUD_TEMPORAL_WEIGHT : u32 = 15u;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var out: VsOut;
    let pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0),
    );
    out.pos = vec4<f32>(pos[vi], 0.0, 1.0);
    return out;
}

fn uv_from_frag_coord(frag_xy: vec2<f32>, tex_size: vec2<f32>) -> vec2<f32> {
    return frag_xy / tex_size;
}

fn ndc_from_uv(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
}

fn sky_dir_from_uv(uv: vec2<f32>, ivp: mat4x4<f32>, cam_pos: vec3<f32>) -> vec3<f32> {
    let ndc = ndc_from_uv(uv);
    let clip = vec4<f32>(ndc, 1.0, 1.0);
    let world = ivp * clip;

    // Preserve direction continuity even if projected w changes sign.
    let inv_w = sign(world.w) / max(abs(world.w), 1e-5);
    let wp = world.xyz * inv_w;
    return normalize(wp - cam_pos);
}

fn slab_intersection(ro: vec3<f32>, rd: vec3<f32>, y0: f32, y1: f32) -> vec2<f32> {
    if abs(rd.y) < 1e-5 {
        return vec2<f32>(1e9, -1e9);
    }
    let t0 = (y0 - ro.y) / rd.y;
    let t1 = (y1 - ro.y) / rd.y;
    return vec2<f32>(min(t0, t1), max(t0, t1));
}

// Match the cloud shader's march volume padding (30% below, 50% above)
// so temporal "inside cloud" detection aligns with actual rendering.
// Old values (80% below, 120% above) caused the temporal pass to treat
// cameras far outside clouds as "inside", forcing the conservative blend
// cap and producing flickering at clear/cloudy transitions.
fn padded_layer_bounds(y0: f32, y1: f32) -> vec2<f32> {
    let thickness = max(y1 - y0, 50.0);
    return vec2<f32>(y0 - thickness * 0.30, y1 + thickness * 0.50);
}

fn camera_inside_any_cloud_layer(y: f32) -> bool {
    let b = cloud_layer_bounds();
    // Use padded bounds so that cameras in the volume envelope
    // region are treated as "inside" for reprojection purposes.
    let low  = padded_layer_bounds(b.low_base,  b.low_top);
    let mid  = padded_layer_bounds(b.mid_base,  b.mid_top);
    let high = padded_layer_bounds(b.high_base, b.high_top);
    return
        (y >= low.x  && y <= low.y)  ||
        (y >= mid.x  && y <= mid.y)  ||
        (y >= high.x && y <= high.y);
}

fn first_hit_t_for_slab(ro: vec3<f32>, rd: vec3<f32>, y0: f32, y1: f32) -> vec2<f32> {
    // Use padded bounds for hit testing
    let padded = padded_layer_bounds(y0, y1);
    let inside = ro.y >= padded.x && ro.y <= padded.y;

    if inside {
        // Camera already inside this volume.
        // Reprojection from a fake slab-entry point is not meaningful here.
        return vec2<f32>(0.0, 1.0);
    }

    let hit = slab_intersection(ro, rd, padded.x, padded.y);
    if hit.y > max(hit.x, 0.0) {
        return vec2<f32>(max(hit.x, 0.0), 1.0);
    }
    return vec2<f32>(1e9, 0.0);
}

fn first_cloud_hit_t(dir: vec3<f32>) -> vec2<f32> {
    let b = cloud_layer_bounds();

    var best_t = 1e9;
    var valid = 0.0;

    let low_hit = first_hit_t_for_slab(cam.camera_pos.xyz, dir, b.low_base, b.low_top);
    if low_hit.y > 0.5 && low_hit.x < best_t {
        best_t = low_hit.x;
        valid = 1.0;
    }

    let mid_hit = first_hit_t_for_slab(cam.camera_pos.xyz, dir, b.mid_base, b.mid_top);
    if mid_hit.y > 0.5 && mid_hit.x < best_t {
        best_t = mid_hit.x;
        valid = 1.0;
    }

    let high_hit = first_hit_t_for_slab(cam.camera_pos.xyz, dir, b.high_base, b.high_top);
    if high_hit.y > 0.5 && high_hit.x < best_t {
        best_t = high_hit.x;
        valid = 1.0;
    }

    return vec2<f32>(best_t, valid);
}

fn safe_sample(tex: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    return textureSample(tex, tex_sampler, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let td = textureDimensions(current_tex);
    let tex_size = vec2<f32>(f32(td.x), f32(td.y));
    let texel = 1.0 / tex_size;

    // Fragment position is in framebuffer pixel coordinates.
    let uv = uv_from_frag_coord(in.pos.xy, tex_size);

    let current = safe_sample(current_tex, uv);
    let motion = clamp(cam.world_params.z, 0.0, 1.0);
    let debug_view = u32(light.torch_meta.w + 0.5);

    // Only skip temporal blending for completely empty pixels.
    // Thin clouds (0 < alpha < 0.001) should still get some temporal smoothing
    // to avoid flickering from jitter. Allow history to blend in even for very thin clouds.
    let is_empty = current.a < 1e-6;
    if is_empty {
        return current;
    }

    let dir = sky_dir_from_uv(uv, cam.inv_view_proj, cam.camera_pos.xyz);
    let hit = first_cloud_hit_t(dir);
    let hit_t = hit.x;
    let has_cloud_hit = hit.y > 0.5;

    let camera_inside_cloud = camera_inside_any_cloud_layer(cam.camera_pos.y);

    // Default to same-UV history. This is the only safe choice for interior views.
    var history_uv = uv;
    var reproj_valid = false;

    if !camera_inside_cloud && has_cloud_hit {
        // For exterior views only, use a slab-entry anchor far enough from the camera
        // that the reprojection point is not sitting on the near plane.
        let reproj_t = max(hit_t, 64.0 * cam.world_params.x);
        let reproj_world = cam.camera_pos.xyz + dir * reproj_t;

        let prev_clip = cam.prev_view_proj * vec4<f32>(reproj_world, 1.0);
        let prev_valid_w = abs(prev_clip.w) > 1e-5;
        let prev_ndc = prev_clip.xy / max(abs(prev_clip.w), 1e-5) * sign(prev_clip.w);
        let prev_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 0.5 - prev_ndc.y * 0.5);

        let border = texel * 1.5;
        let in_bounds =
            all(prev_uv >= border) &&
            all(prev_uv <= vec2<f32>(1.0) - border);

        reproj_valid = prev_valid_w && in_bounds;
        if reproj_valid {
            history_uv = prev_uv;
        }
    }

    let history = safe_sample(history_tex, history_uv);

    // Neighborhood clamp in current-frame space.
    var nmin = current;
    var nmax = current;

    let offsets = array<vec2<f32>, 8>(
        vec2<f32>(-1.0,  0.0), vec2<f32>( 1.0,  0.0),
        vec2<f32>( 0.0, -1.0), vec2<f32>( 0.0,  1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0), vec2<f32>( 1.0,  1.0),
    );

    for (var i: i32 = 0; i < 8; i = i + 1) {
        let s = safe_sample(current_tex, uv + offsets[i] * texel);
        nmin = min(nmin, s);
        nmax = max(nmax, s);
    }

    let rgb_range = nmax.rgb - nmin.rgb;
    let a_range   = nmax.a   - nmin.a;

    // At interior/exterior slab boundaries, local variance is very high because half
    // the neighborhood pixels see cloud and half see sky. The standard clamp margins
    // amplify this mismatch into flickering banding. Reduce clamp margins proportional
    // to local variance so the history is held more loosely at boundary pixels.
    let local_variance_pre = max(length(rgb_range), a_range);
    let boundary_softness  = 1.0 - smoothstep(0.20, 0.65, local_variance_pre);
    let clamp_margin_rgb = rgb_range * mix(0.08, 0.22, boundary_softness);
    let clamp_margin_a   = a_range   * mix(0.10, 0.28, boundary_softness);

    let clamped_history = vec4<f32>(
        clamp(history.rgb, nmin.rgb - clamp_margin_rgb, nmax.rgb + clamp_margin_rgb),
        clamp(history.a,   nmin.a   - clamp_margin_a,   nmax.a   + clamp_margin_a)
    );

    // Clouds now use more footprint-aware filtering, so temporal can be gentler.
    // Conservative blending reduces volumetric dragging and altitude shimmer.
    let thin_cloud_boost = (1.0 - current.a) * 0.04;
    let static_weight = 0.60 + thin_cloud_boost;
    let moving_weight = 0.32;
    let motion_factor = smoothstep(0.0, 0.03, motion);
    var blend_weight = mix(static_weight, moving_weight, motion_factor);

    // High local variance means the pixel is unstable or near a silhouette/interior transition.
    let local_variance = max(length(rgb_range), a_range);
    let variance_reject = smoothstep(0.20, 0.65, local_variance);
    blend_weight *= (1.0 - variance_reject * 0.55);

    // Reject history based on clamp distance and alpha disagreement.
    let clamp_dist_rgb = length(history.rgb - clamped_history.rgb);
    let clamp_dist_a   = abs(history.a - clamped_history.a);
    let alpha_span = nmax.a - nmin.a;
    let neighborhood_instability = smoothstep(0.08, 0.35, alpha_span);

    let clamp_rejection = max(
        smoothstep(0.05, 0.18, clamp_dist_rgb),
        smoothstep(0.04, 0.14, clamp_dist_a)
    );

    blend_weight *= (1.0 - clamp_rejection * 0.82);
    blend_weight *= (1.0 - neighborhood_instability * 0.45);

    // Prevent thick old history from dragging across newly thin/empty pixels.
    // Relax the disocclusion penalty for thin clouds to prevent flickering.
    // Only fully apply disocclusion when current is truly empty (alpha < 0.02).
    let disocclusion =
        smoothstep(0.18, 0.75, history.a - current.a) *
        smoothstep(0.02, 0.0, current.a);

    blend_weight *= (1.0 - disocclusion * 0.65);

    // If exterior reprojection is invalid, only allow a tiny same-UV accumulation
    // in very static shots. Otherwise drop history entirely.
    if !reproj_valid && !camera_inside_cloud {
        blend_weight *= smoothstep(0.02, 0.0, motion) * 0.15;
    }

    // Interior views are especially sensitive: cap blend further.
    if camera_inside_cloud {
        blend_weight = min(blend_weight, 0.14);
    }

    blend_weight = clamp(blend_weight, 0.0, 1.0);

    if debug_view == DEBUG_VIEW_CLOUD_TEMPORAL_WEIGHT {
        return vec4<f32>(
            vec3<f32>(
                blend_weight,
                select(0.0, 1.0, reproj_valid),
                select(0.0, 1.0, camera_inside_cloud)
            ),
            1.0,
        );
    }

    return mix(current, clamped_history, blend_weight);
}
