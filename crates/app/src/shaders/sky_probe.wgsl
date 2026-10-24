// Evaluates the per-frame sky and sun terms of world_shader.wgsl once, with
// the same functions and inputs the terrain fragment used to call per pixel.
// The renderer copies `probe_out` into the atmosphere group's `sky_probe`.
@group(3) @binding(0) var<storage, read_write> probe_out : SkyProbe;

fn probe_unit3(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    // Same as world_shader's terrain_safe_unit3.
    let l2 = dot(v, v);
    return select(fallback, v * inverseSqrt(max(l2, 1e-20)), l2 > 1e-20);
}

@compute @workgroup_size(1)
fn cs_sky_probe() {
    let sun_dir = probe_unit3(light.sun_dir_intensity.xyz, vec3<f32>(0.0, 1.0, 0.0));
    let moon_dir = probe_unit3(light.moon_dir_intensity.xyz, vec3<f32>(0.0, -1.0, 0.0));
    let sun_az = safe_normalize2(sun_dir.xz, vec2<f32>(1.0, 0.0));
    probe_out.terms[SKY_PROBE_SUN_TRANSMITTANCE] = vec4<f32>(sky_transmittance_to_sun(sun_dir.y), 0.0);
    probe_out.terms[SKY_PROBE_MOON_TRANSMITTANCE] = vec4<f32>(sky_transmittance_to_sun(moon_dir.y), 0.0);
    probe_out.terms[SKY_PROBE_ZENITH] = vec4<f32>(atmosphere_color(vec3<f32>(0.0, 1.0, 0.0)), 0.0);
    probe_out.terms[SKY_PROBE_SOLAR_HORIZON] =
        vec4<f32>(atmosphere_color(normalize(vec3<f32>(sun_az.x, 0.16, sun_az.y))), 0.0);
    probe_out.terms[SKY_PROBE_ANTISOLAR_HORIZON] =
        vec4<f32>(atmosphere_color(normalize(vec3<f32>(-sun_az.x, 0.16, -sun_az.y))), 0.0);
    probe_out.terms[SKY_PROBE_LOW_SOLAR] =
        vec4<f32>(atmosphere_color(normalize(vec3<f32>(sun_az.x, 0.035, sun_az.y))), 0.0);
    probe_out.terms[SKY_PROBE_OPPOSITE_SUN] =
        vec4<f32>(atmosphere_color(normalize(vec3<f32>(-sun_dir.x, 0.55, -sun_dir.z))), 0.0);
    probe_out.terms[7] = vec4<f32>(0.0);
}
