// Globe tiles: 65x65 height samples folded onto the cube sphere (render/globe/gpu.rs).

struct GlobeUniform {
    // w: night-light count
    eye_body: vec4<f32>,
    sun_dir_body: vec4<f32>,
    radius_m: f32,
    sea_level_m: f32,
    focal_px: f32,
    // bit 0 lean tier, bit 1 cloud shadow, bit 2 night lights
    flags: u32,
    // rgb glow colour, w strength
    limb: vec4<f32>,
    // Camera-relative body-space view-projection, reverse-Z.
    view_proj: mat4x4<f32>,
};

struct TileInstance {
    // xyz: tile centre minus eye in body space, subtracted in f64 on the CPU.
    origin_body: vec4<f32>,
    chart_min: vec2<f32>,
    cell_m: f32,
    face: u32,
    // Face-local chart point of the tile centre.
    centre: vec2<f32>,
    slot: u32,
    pad: u32,
};

@group(0) @binding(0) var<uniform> globe: GlobeUniform;
@group(0) @binding(1) var<storage, read> tiles: array<TileInstance>;
@group(0) @binding(2) var height_atlas: texture_2d<u32>;
@group(0) @binding(3) var climate_atlas: texture_2d<f32>;
// Cloud coverage per face, rows along chart z (render/globe/shell.rs).
@group(0) @binding(4) var shell_coverage: texture_2d_array<f32>;
@group(0) @binding(5) var shell_sampler: sampler;
@group(0) @binding(6) var<storage, read> night_lights: array<GlobeLight>;

struct GlobeLight {
    body: vec3<f32>,
    radius_m: f32,
    intensity: f32,
    pad0: f32,
    pad1: f32,
    pad2: f32,
};

const GLOBE_SAMPLES: u32 = 65u;
const GLOBE_LAST: u32 = 64u;
// The 65x65 lattice plus one skirt ring; gpu::SKIRT_SIDE.
const GLOBE_SKIRT_SIDE: u32 = 67u;
const GLOBE_PITCH_M: f32 = 65536.0;
const GLOBE_ATLAS_COLUMNS: u32 = 46u;
const GLOBE_HALF_M: f32 = 16000.0;
const GLOBE_FRAC_PI_4: f32 = 0.78539816339;
const GLOBE_HEIGHT_BIAS: i32 = 32768;
const GLOBE_WATER_BIT: u32 = 256u;
// Atmosphere shell used for the limb approximation.
const GLOBE_ATMOSPHERE_M: f32 = 10000.0;
const GLOBE_SCATTER_M: f32 = 60000.0;
const GLOBE_SKY: vec3<f32> = vec3<f32>(0.35, 0.55, 0.95);
const GLOBE_SUN: vec3<f32> = vec3<f32>(1.0, 0.96, 0.9);
const GLOBE_AMBIENT: f32 = 0.18;
const GLOBE_NIGHT_FLOOR: vec3<f32> = vec3<f32>(0.016, 0.019, 0.026);
const GLOBE_DEEP_WATER: vec3<f32> = vec3<f32>(0.02, 0.07, 0.14);
// shell::SHELL_HEIGHT_M
const GLOBE_SHELL_M: f32 = 1600.0;
const GLOBE_SHELL_SUN: vec3<f32> = vec3<f32>(1.0, 0.98, 0.95);
const GLOBE_CITY: vec3<f32> = vec3<f32>(1.0, 0.72, 0.38);
// Linear radiance per unit light intensity at the splat centre.
const GLOBE_CITY_GAIN: f32 = 0.03;

// Linear `render.color.base` of the materials in bake::material_index order; rest grey.
// The last slot draws bake::UNMAPPED; bake_palette_tests pins both lengths.
const GLOBE_FALLBACK: u32 = 17u;
const GLOBE_PALETTE: array<vec3<f32>, 18> = array<vec3<f32>, 18>(
    vec3<f32>(0.1065, 0.2049, 0.0732), // grass
    vec3<f32>(0.1193, 0.0637, 0.0363), // dirt
    vec3<f32>(0.1626, 0.1789, 0.1873), // stone
    vec3<f32>(0.4480, 0.3424, 0.1789), // sand
    vec3<f32>(0.5705, 0.6383, 0.6921), // snow grass
    vec3<f32>(0.2330, 0.2330, 0.2140), // gravel
    vec3<f32>(0.1065, 0.0684, 0.0331), // mud, clay
    vec3<f32>(0.3303, 0.2140, 0.1065), // sandstone
    vec3<f32>(0.0732, 0.1549, 0.0509), // leaves
    vec3<f32>(0.1193, 0.0684, 0.0397), // log
    vec3<f32>(0.3424, 0.5705, 0.8689), // ice
    vec3<f32>(0.2140, 0.2140, 0.2140),
    vec3<f32>(0.2140, 0.2140, 0.2140),
    vec3<f32>(0.2140, 0.2140, 0.2140),
    vec3<f32>(0.2140, 0.2140, 0.2140),
    vec3<f32>(0.4200, 0.3801, 0.2429), // limestone
    vec3<f32>(0.0509, 0.1065, 0.0331), // grove moss
    vec3<f32>(0.2140, 0.2140, 0.2140),
);

// Columns u, n, v of engine_core::cube_sphere::Face::basis, in Face order.
fn globe_axes(face: u32) -> mat3x3<f32> {
    switch face {
        case 0u: { return mat3x3<f32>(vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0)); }
        case 1u: { return mat3x3<f32>(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0)); }
        case 2u: { return mat3x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0)); }
        case 3u: { return mat3x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.0, 0.0, -1.0)); }
        case 4u: { return mat3x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, -1.0, 0.0)); }
        default: { return mat3x3<f32>(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, -1.0), vec3<f32>(0.0, 1.0, 0.0)); }
    }
}

// Projection::Tangent::to_sphere of face-local chart metres.
fn globe_dir(face: u32, x: f32, z: f32) -> vec3<f32> {
    let a = globe_axes(face);
    let s = tan(GLOBE_FRAC_PI_4 * x / GLOBE_HALF_M);
    let t = tan(GLOBE_FRAC_PI_4 * z / GLOBE_HALF_M);
    return normalize(a[1] + s * a[0] + t * a[2]);
}

// chart::to_body of a face-local chart point (x, h, z).
fn globe_fold(face: u32, x: f32, z: f32, h: f32) -> vec3<f32> {
    return globe_dir(face, x, z) * (globe.radius_m + h);
}

fn globe_texel(slot: u32, i: u32, j: u32) -> vec2<i32> {
    let base = vec2<u32>(slot % GLOBE_ATLAS_COLUMNS, slot / GLOBE_ATLAS_COLUMNS) * GLOBE_SAMPLES;
    return vec2<i32>(base + vec2<u32>(i, j));
}

// Water samples sit on the sea surface, not the baked floor.
fn globe_surface_m(raw: vec2<u32>) -> f32 {
    let h = f32(i32(raw.x) - GLOBE_HEIGHT_BIAS);
    if (raw.y & GLOBE_WATER_BIT) != 0u {
        return max(h, globe.sea_level_m);
    }
    return h;
}

fn globe_height(slot: u32, i: u32, j: u32) -> f32 {
    return globe_surface_m(textureLoad(height_atlas, globe_texel(slot, i, j), 0).xy);
}

struct GlobeVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) rel: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) wet: f32,
    @location(3) water: f32,
    @location(4) @interpolate(flat) material: u32,
    // Chart x/z in metres, which equal world metres; the resident mask reads it.
    @location(5) chart: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> GlobeVsOut {
    let tile = tiles[instance];
    let si = vertex % GLOBE_SKIRT_SIDE;
    let sj = vertex / GLOBE_SKIRT_SIDE;
    let i = clamp(si, 1u, GLOBE_SAMPLES) - 1u;
    let j = clamp(sj, 1u, GLOBE_SAMPLES) - 1u;
    // The skirt ring hangs below the edge, so a coarser neighbour's T-junction never opens onto the sky.
    let skirt = si == 0u || sj == 0u || si == GLOBE_SKIRT_SIDE - 1u || sj == GLOBE_SKIRT_SIDE - 1u;
    let raw = textureLoad(height_atlas, globe_texel(tile.slot, i, j), 0).xy;
    let h = globe_surface_m(raw) - select(0.0, 2.0 * tile.cell_m, skirt);
    let x = tile.chart_min.x + f32(i) * tile.cell_m;
    let z = tile.chart_min.y + f32(j) * tile.cell_m;
    // Both folds are near the tile centre, so their difference keeps f32 precision.
    let local = globe_fold(tile.face, x, z, h) - globe_fold(tile.face, tile.centre.x, tile.centre.y, 0.0);
    let rel = local + tile.origin_body.xyz;

    let il = max(i, 1u) - 1u;
    let ir = min(i + 1u, GLOBE_LAST);
    let jl = max(j, 1u) - 1u;
    let jr = min(j + 1u, GLOBE_LAST);
    let dhdx = (globe_height(tile.slot, ir, j) - globe_height(tile.slot, il, j)) / (f32(ir - il) * tile.cell_m);
    let dhdz = (globe_height(tile.slot, i, jr) - globe_height(tile.slot, i, jl)) / (f32(jr - jl) * tile.cell_m);
    // local_frame: east along d dir / ds, which lies in the plane of up and u.
    let up = globe_dir(tile.face, x, z);
    let u = globe_axes(tile.face)[0];
    let east = normalize(u - up * dot(u, up));
    let north = cross(east, up);

    var out: GlobeVsOut;
    out.clip = globe.view_proj * vec4<f32>(rel, 1.0);
    out.rel = rel;
    out.normal = normalize(up - east * dhdx - north * dhdz);
    out.wet = textureLoad(climate_atlas, globe_texel(tile.slot, i, j), 0).y;
    out.water = select(0.0, 1.0, (raw.y & GLOBE_WATER_BIT) != 0u);
    out.material = raw.y & 255u;
    out.chart = vec2<f32>(f32(tile.face) * GLOBE_PITCH_M + x, z);
    return out;
}

// Length of the eye-to-surface segment inside the atmosphere shell.
fn globe_air_path(rel: vec3<f32>) -> f32 {
    let top = globe.radius_m + GLOBE_ATMOSPHERE_M;
    let dist = length(rel);
    let dir = rel / max(dist, 1e-3);
    let b = dot(globe.eye_body.xyz, dir);
    let c = dot(globe.eye_body.xyz, globe.eye_body.xyz) - top * top;
    // Inside the shell the segment starts at the eye; otherwise at the near crossing.
    var enter = 0.0;
    if c > 0.0 {
        enter = -b - sqrt(max(b * b - c, 0.0));
    }
    return max(dist - clamp(enter, 0.0, dist), 0.0);
}

fn globe_albedo(material: u32, wet: f32) -> vec3<f32> {
    var albedo = GLOBE_PALETTE[min(material, GLOBE_FALLBACK)];
    if material == 0u || material == 8u {
        let dry = clamp((0.3 - wet) / 0.3, 0.0, 1.0) * 0.5;
        let lush = clamp((wet - 0.7) / 0.3, 0.0, 1.0) * 0.5;
        albedo = mix(albedo, vec3<f32>(0.33, 0.27, 0.07), dry);
        albedo = mix(albedo, vec3<f32>(0.04, 0.15, 0.11), lush);
    }
    return albedo;
}

struct GlobeOutput {
    @location(0) color: vec4<f32>,
    // The terrain pass's reflection receiver target; the globe takes no indirect light.
    @location(1) receiver: vec4<f32>,
};

@fragment
fn fs_main(in: GlobeVsOut) -> GlobeOutput {
    let fade = globe_mask_alpha(in.chart);
    // Under resident chunks the chunks draw the ground; a faded-out fragment must not write depth.
    if fade < 1.0 / 255.0 {
        discard;
    }
    let n = normalize(in.normal);
    let dist = length(in.rel);
    let view = -in.rel / max(dist, 1e-3);
    let sun = normalize(globe.sun_dir_body.xyz);
    let up = normalize(globe.eye_body.xyz + in.rel);
    var ndl = max(dot(n, sun), 0.0);
    if (globe.flags & 2u) != 0u {
        ndl *= 1.0 - 0.6 * globe_shell_alpha_along(globe.eye_body.xyz + in.rel, sun);
    }
    var color: vec3<f32>;
    if in.water > 0.5 {
        let fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(n, view), 0.0), 5.0);
        let glint = pow(max(dot(reflect(-view, n), sun), 0.0), 400.0) * 4.0;
        color = GLOBE_DEEP_WATER * (ndl * GLOBE_SUN + GLOBE_AMBIENT * GLOBE_SKY)
            + fresnel * (GLOBE_SKY * 0.3 + glint * GLOBE_SUN);
    } else {
        color = globe_albedo(in.material, in.wet) * (ndl * GLOBE_SUN + GLOBE_AMBIENT * GLOBE_SKY);
    }
    let day = smoothstep(-0.05, 0.05, dot(up, sun));
    color = mix(GLOBE_NIGHT_FLOOR, color, day);
    if (globe.flags & 4u) != 0u {
        color += globe_city_glow(globe.eye_body.xyz + in.rel) * (1.0 - day);
    }
    let path = globe_air_path(in.rel);
    let haze = 1.0 - exp(-path / GLOBE_SCATTER_M);
    color = color * (1.0 - haze) + globe.limb.rgb * globe.limb.w * haze * day;
    return GlobeOutput(vec4<f32>(color, fade), vec4<f32>(0.0));
}
