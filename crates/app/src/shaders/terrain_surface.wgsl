// Shared near/horizon surface shading. Metre-space bands retain their scale
// while unresolved contrast transfers to roughness. These small shading
// normals supplement geometry; they are not reconstructed terrain heights.
var<private> natural_relief_strength: vec4<f32>;

struct TerrainSurfaceRelief {
    normal: vec3<f32>,
    variance: f32,
}

fn terrain_surface_relief(n: vec3<f32>, p: vec3<f32>, footprint_m: f32) -> TerrainSurfaceRelief {
    // A fixed oblique world field gives cliffs vertical variation too. Do not
    // choose projection axes from the view or a changing triangle normal.
    let uv = p.xz + p.y * vec2<f32>(0.31, 0.23);
    let wavelengths = vec4<f32>(48.0, 12.0, 3.0, 0.85);
    var gradient = vec2<f32>(0.0);
    var variance = 0.0;
    for (var i = 0u; i < 4u; i += 1u) {
        let resolved = surface_detail_weight(footprint_m / wavelengths[i]);
        let strength = natural_relief_strength[i];
        variance += 0.20 * strength * strength * (1.0 - resolved * resolved);
        if resolved > 0.0 {
            let offset = vec2<f32>(13.7, -29.4) + f32(i) * vec2<f32>(-31.0, 37.5);
            gradient += surface_noise_grad(uv / wavelengths[i] + offset).yz * strength * resolved;
        }
    }
    let g = vec3<f32>(gradient.x, dot(gradient, vec2<f32>(0.31, 0.23)), gradient.y);
    let tangent = g - n * dot(n, g);
    return TerrainSurfaceRelief(normalize(n - tangent), variance);
}

// Same climate stages and seed salts as landscape.rs / terrain_variety.rs.
// The shader lacks erosion sediment, so this returns rainfall-derived moisture.
// Keeping regional belts and small ecotones here prevents a hot desert from
// receiving temperate surface tint just because it lies far from the origin.
fn terrain_surface_climate(p: vec3<f32>, fp: f32) -> vec2<f32> {
    let seed = cam.scene_seed.x;
    let version = cam.scene_seed.y;
    let warped = p.xz + vec2<f32>(
        climate_noise_filtered(p.xz / 420.0, seed + 7u, fp / 420.0) - 0.5,
        climate_noise_filtered(p.xz / 390.0, seed + 9u, fp / 390.0) - 0.5) * 180.0;
    var heat = climate_noise_filtered(warped / 700.0, seed + 11u, fp * (2.5 / 700.0));
    var wet = climate_noise_filtered(warped / 520.0, seed + 23u, fp * (2.5 / 520.0));
    if version >= 5u {
        heat = clamp((heat - 0.5) * 1.55 + 0.5, 0.0, 1.0);
        wet = clamp((wet - 0.5) * 1.4 + 0.5, 0.0, 1.0);
    }
    if version >= 15u {
        let ecotone = vec2<f32>(
            climate_noise_filtered(p.xz / 52.0, seed + 9601u, fp / 52.0),
            climate_noise_filtered(p.xz / 47.0, seed + 9603u, fp / 47.0)) - vec2<f32>(0.5);
        let fine = vec2<f32>(
            climate_noise_filtered(p.xz / 13.0, seed + 9607u, fp / 13.0),
            climate_noise_filtered(p.xz / 12.0, seed + 9611u, fp / 12.0)) - vec2<f32>(0.5);
        heat = clamp((heat - 0.5) * 1.12 + 0.5 + ecotone.x * 0.14 + fine.x * 0.05, 0.0, 1.0);
        wet = clamp((wet - 0.5) * 1.10 + 0.5 + ecotone.y * 0.14 + fine.y * 0.05, 0.0, 1.0);
    }
    if version >= 33u {
        let reach = smoothstep(350.0, 1600.0, length(p.xz));
        let latitude = p.z + (climate_noise_filtered(p.xz / 1900.0, seed + 9701u, fp / 1900.0) - 0.5) * 1800.0;
        let belt = sin(latitude * (6.28318530718 / 7200.0)) * surface_detail_weight(fp * (2.0 / 7200.0));
        let region_t = climate_noise_filtered(p.xz / 2600.0, seed + 9703u, fp / 2600.0) - 0.5;
        let region_m = climate_noise_filtered(p.xz / 2200.0, seed + 9707u, fp / 2200.0) - 0.5;
        heat = clamp(heat + (belt * 0.13 + region_t * 0.26) * reach, 0.0, 1.0);
        wet = clamp(wet + region_m * 0.30 * reach, 0.0, 1.0);
    }
    let lapse = select(0.0035, 0.0018, version >= 7u);
    return vec2<f32>(clamp(heat - max(p.y - 30.0, 0.0) * lapse, 0.0, 1.0), wet);
}

fn terrain_surface_strength(weights: vec4<f32>, heat: f32, wet: f32) -> vec4<f32> {
    // Meadow -> forest floor: stronger clods, but still much softer than rock.
    let turf = mix(vec4<f32>(0.040, 0.024, 0.042, 0.020),
        vec4<f32>(0.050, 0.038, 0.065, 0.032), smoothstep(0.46, 0.72, wet));
    let rock = vec4<f32>(0.075, 0.060, 0.110, 0.060);
    let sand = vec4<f32>(0.035, 0.025, 0.024, 0.009);
    let snow = vec4<f32>(0.025, 0.018, 0.015, 0.006);
    let cold = (1.0 - smoothstep(0.12, 0.30, heat)) * smoothstep(0.32, 0.62, wet);
    return mix(turf, snow, cold * 0.55) * weights.x
        + rock * weights.y + sand * weights.z + snow * weights.w;
}
