// Cloud shell, cloud shadow and night lights; appended to globe_shader.wgsl.

// Projection::Tangent::from_sphere: face whose normal is nearest, then face-local chart metres.
fn globe_chart_of(dir: vec3<f32>) -> vec3<f32> {
    var face = 0u;
    var best = -2.0;
    for (var f = 0u; f < 6u; f++) {
        let dn = dot(dir, globe_axes(f)[1]);
        if dn > best {
            best = dn;
            face = f;
        }
    }
    let a = globe_axes(face);
    let dn = max(best, 1e-6);
    let s = atan(dot(dir, a[0]) / dn) / GLOBE_FRAC_PI_4;
    let t = atan(dot(dir, a[2]) / dn) / GLOBE_FRAC_PI_4;
    return vec3<f32>(clamp(s, -1.0, 1.0) * GLOBE_HALF_M, clamp(t, -1.0, 1.0) * GLOBE_HALF_M, f32(face));
}

fn globe_shell_alpha(dir: vec3<f32>) -> f32 {
    let chart = globe_chart_of(dir);
    let uv = (chart.xy + GLOBE_HALF_M) / (2.0 * GLOBE_HALF_M);
    let c = textureSampleLevel(shell_coverage, shell_sampler, uv, i32(chart.z), 0.0).r;
    return smoothstep(0.45, 0.75, c);
}

// Coverage where the ray from `p` toward the sun crosses the shell.
fn globe_shell_alpha_along(p: vec3<f32>, sun: vec3<f32>) -> f32 {
    let r = globe.radius_m + GLOBE_SHELL_M;
    let b = dot(p, sun);
    let c = dot(p, p) - r * r;
    let disc = b * b - c;
    if disc < 0.0 {
        return 0.0;
    }
    let t = -b + sqrt(disc);
    if t < 0.0 {
        return 0.0;
    }
    return globe_shell_alpha(normalize(p + sun * t));
}

fn globe_city_glow(p: vec3<f32>) -> vec3<f32> {
    var glow = 0.0;
    let count = min(u32(globe.eye_body.w), arrayLength(&night_lights));
    for (var i = 0u; i < count; i++) {
        let light = night_lights[i];
        let d = distance(p, light.body);
        glow += light.intensity * smoothstep(light.radius_m, 0.0, d);
    }
    return GLOBE_CITY * glow * GLOBE_CITY_GAIN;
}

struct GlobeShellOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) dir: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) face: u32,
};

// Level-0 root tiles at a constant height; instances follow the opaque tiles.
@vertex
fn vs_shell(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> GlobeShellOut {
    let tile = tiles[instance];
    let x = tile.chart_min.x + f32(vertex % GLOBE_SAMPLES) * tile.cell_m;
    let z = tile.chart_min.y + f32(vertex / GLOBE_SAMPLES) * tile.cell_m;
    let local = globe_fold(tile.face, x, z, GLOBE_SHELL_M) - globe_fold(tile.face, tile.centre.x, tile.centre.y, 0.0);
    var out: GlobeShellOut;
    out.clip = globe.view_proj * vec4<f32>(local + tile.origin_body.xyz, 1.0);
    out.dir = globe_dir(tile.face, x, z);
    out.uv = (vec2<f32>(x, z) + GLOBE_HALF_M) / (2.0 * GLOBE_HALF_M);
    out.face = tile.face;
    return out;
}

@fragment
fn fs_shell(in: GlobeShellOut) -> GlobeOutput {
    let n = normalize(in.dir);
    let sun = normalize(globe.sun_dir_body.xyz);
    let c = textureSample(shell_coverage, shell_sampler, in.uv, i32(in.face)).r;
    let alpha = smoothstep(0.45, 0.75, c);
    // No probe is bound to the globe; GLOBE_SKY stands in for the probe zenith.
    let lit = GLOBE_SHELL_SUN * max(dot(n, sun), 0.0) + GLOBE_SKY * 0.4;
    let day = smoothstep(-0.05, 0.05, dot(n, sun));
    let color = mix(vec3<f32>(0.02), lit, day);
    return GlobeOutput(vec4<f32>(color, alpha), vec4<f32>(0.0));
}
