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
fn climate_turn(p: vec2<f32>, cs: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(p.x * cs.x - p.y * cs.y, p.x * cs.y + p.y * cs.x);
}

// Generator 53 kilometre-scale climate; mirrors climate_regional in landscape.rs.
fn terrain_climate_regional(xz: vec2<f32>, seed: u32, fp: f32) -> vec2<f32> {
    let a = vec2<f32>(0.82, 0.57);
    let b = vec2<f32>(0.39, -0.92);
    let c = vec2<f32>(-0.66, 0.75);
    let d = vec2<f32>(0.95, 0.31);
    let warp = vec2<f32>(
        (climate_noise_filtered(climate_turn(xz, a) / 1300.0, seed + 7u, fp / 1300.0) - 0.5) * 560.0
            + (climate_noise_filtered(climate_turn(xz, b) / 410.0, seed + 8u, fp / 410.0) - 0.5) * 120.0,
        (climate_noise_filtered(climate_turn(xz, c) / 1210.0, seed + 9u, fp / 1210.0) - 0.5) * 560.0
            + (climate_noise_filtered(climate_turn(xz, d) / 380.0, seed + 10u, fp / 380.0) - 0.5) * 120.0);
    let w = xz + warp;
    let t = climate_noise_filtered(climate_turn(w, b) / 2100.0, seed + 11u, fp * (2.5 / 2100.0)) * 0.72
        + climate_noise_filtered(climate_turn(w, c) / 820.0, seed + 12u, fp * (2.5 / 820.0)) * 0.28;
    let r = climate_noise_filtered(climate_turn(w, d) / 1600.0, seed + 23u, fp * (2.5 / 1600.0)) * 0.72
        + climate_noise_filtered(climate_turn(w, a) / 640.0, seed + 24u, fp * (2.5 / 640.0)) * 0.28;
    return clamp((vec2<f32>(t, r) - vec2<f32>(0.5)) * 1.3 + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0));
}

fn terrain_surface_climate(p: vec3<f32>, fp: f32) -> vec2<f32> {
    let seed = cam.scene_seed.x;
    let version = cam.scene_seed.y;
    var heat: f32;
    var wet: f32;
    if version >= 53u {
        let regional = terrain_climate_regional(p.xz, seed, fp);
        heat = regional.x;
        wet = regional.y;
    } else {
        let warped = p.xz + vec2<f32>(
            climate_noise_filtered(p.xz / 420.0, seed + 7u, fp / 420.0) - 0.5,
            climate_noise_filtered(p.xz / 390.0, seed + 9u, fp / 390.0) - 0.5) * 180.0;
        heat = climate_noise_filtered(warped / 700.0, seed + 11u, fp * (2.5 / 700.0));
        wet = climate_noise_filtered(warped / 520.0, seed + 23u, fp * (2.5 / 520.0));
    }
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
        let ecotone_amp = select(0.14, 0.05, version >= 53u);
        let speck = select(0.05, 0.015, version >= 53u);
        heat = clamp((heat - 0.5) * 1.12 + 0.5 + ecotone.x * ecotone_amp + fine.x * speck, 0.0, 1.0);
        wet = clamp((wet - 0.5) * 1.10 + 0.5 + ecotone.y * ecotone_amp + fine.y * speck, 0.0, 1.0);
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
