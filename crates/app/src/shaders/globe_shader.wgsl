// Globe tiles: 65x65 height samples folded onto the cube sphere (render/globe/gpu.rs).

struct GlobeUniform {
    // w: night-light count
    eye_body: vec4<f32>,
    // w: clock seconds wrapped to an hour (globe_driver.rs)
    sun_dir_body: vec4<f32>,
    radius_m: f32,
    sea_level_m: f32,
    focal_px: f32,
    // bit 0 lean tier, bit 1 cloud shadow, bit 2 night lights, bit 3 Hell,
    // bit 4 coastal suspension, bit 5 reference cloud air march, bit 6 planet,
    // bits 8-15 debug view, bits 16-17 surface review mode
    flags: u32,
    // xyz moon direction in body space, w moon intensity (LightingState::moon_intensity)
    moon_body: vec4<f32>,
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
struct GlobeShellBlend {
    weights: array<vec4<f32>, 2>, wind_m: vec4<f32>,
    previous: array<vec4<f32>, 6>, current: array<vec4<f32>, 6>,
    flow: vec4<f32>, flow_shape: vec4<f32>,
    previous_time: array<vec4<f32>, 2>, current_time: array<vec4<f32>, 2>,
    // Column opacity/erosion/detail/scale and stratus/alto/cirrus mixture.
    appearance: vec4<f32>, layers: vec4<f32>,
    // Vortex obstacle weights per lattice cell, 360 = shell_obstacles::CELLS.
    obstacles: array<vec4<f32>, 90>,
};
@group(0) @binding(8) var<uniform> shell_blend: GlobeShellBlend;
struct GlobeLightGroups { bounds: array<vec4<f32>, 32>, };
@group(0) @binding(9) var<uniform> globe_light_groups: GlobeLightGroups;
@group(0) @binding(10) var planet_depth_texture: texture_2d<f32>;
@group(0) @binding(11) var cloud_detail_lattice: texture_3d<f32>;
@group(0) @binding(12) var cloud_detail_sampler: sampler;
@group(0) @binding(13) var normal_atlas: texture_2d<f32>;

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
// chart::HALF_M; a test pins it.
const GLOBE_HALF_M: f32 = 24000.0;
const GLOBE_FRAC_PI_4: f32 = 0.78539816339;
const GLOBE_HEIGHT_BIAS: i32 = 32768;
const GLOBE_WATER_BIT: u32 = 256u;
// Moonlight colour per unit moon intensity: cool and dim, like the ground's night floor.
const GLOBE_MOON: vec3<f32> = vec3<f32>(0.10, 0.13, 0.20);
const GLOBE_SKY: vec3<f32> = vec3<f32>(0.35, 0.55, 0.95);
const GLOBE_SUN: vec3<f32> = vec3<f32>(1.0, 0.96, 0.9);
const GLOBE_AMBIENT: f32 = 0.18;
const GLOBE_NIGHT_FLOOR: vec3<f32> = vec3<f32>(0.016, 0.019, 0.026);
const GLOBE_DEEP_WATER: vec3<f32> = vec3<f32>(0.005, 0.026, 0.071);
const GLOBE_SHALLOW_WATER: vec3<f32> = vec3<f32>(0.03, 0.17, 0.21);
const GLOBE_WATER_ABSORB: vec3<f32> = vec3<f32>(0.12, 0.045, 0.03);
// shell::SHELL_HEIGHT_M
const GLOBE_SHELL_M: f32 = 980.0;
const GLOBE_SHELL_SUN: vec3<f32> = vec3<f32>(1.0, 0.98, 0.95);
const GLOBE_CLOUD_ALBEDO: f32 = 0.8;
// shell::HANDOFF_M; the composite's `orbit_fade` uses the same band.
const GLOBE_HANDOFF_LOW_M: f32 = 3200.0;
const GLOBE_HANDOFF_HIGH_M: f32 = 6000.0;
const GLOBE_CITY: vec3<f32> = vec3<f32>(1.0, 0.78, 0.48);
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
    vec3<f32>(0.1960, 0.0108, 0.0066), // Hellstone
    vec3<f32>(0.0331, 0.0363, 0.0470), // basalt
    vec3<f32>(0.0301, 0.0272, 0.0245), // ash
    vec3<f32>(0.8, 0.12, 0.008),       // lava
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

// The atlas packs inland water depth above the water flag. Ocean has zero
// encoded depth and uses the common sea datum.
fn globe_surface_m(raw: vec2<u32>) -> f32 {
    let h = f32(i32(raw.x) - GLOBE_HEIGHT_BIAS);
    if (raw.y & GLOBE_WATER_BIT) != 0u {
        let inland_depth = raw.y >> 9u;
        if inland_depth > 0u {
            return h + f32(inland_depth);
        }
        return max(h, globe.sea_level_m);
    }
    return h;
}

fn globe_height(slot: u32, i: u32, j: u32) -> f32 {
    return globe_surface_m(textureLoad(height_atlas, globe_texel(slot, i, j), 0).xy);
}

fn globe_lattice_normal(slot: u32, face: u32, q: vec2<u32>, chart_min: vec2<f32>, cell: f32, height: f32) -> vec3<f32> {
    if ((globe.flags >> 16u) & 3u) == 3u {
        let oct = textureLoad(normal_atlas, globe_texel(slot, q.x, q.y), 0).xy * 2.0 - 1.0;
        var normal = vec3<f32>(oct, 1.0 - abs(oct.x) - abs(oct.y));
        let fold = clamp(-normal.z, 0.0, 1.0);
        normal.x += select(fold, -fold, normal.x >= 0.0);
        normal.y += select(fold, -fold, normal.y >= 0.0);
        return normalize(normal);
    }
    let il = max(q.x, 1u) - 1u;
    let ir = min(q.x + 1u, GLOBE_LAST);
    let jl = max(q.y, 1u) - 1u;
    let jr = min(q.y + 1u, GLOBE_LAST);
    let dx = (globe_height(slot,ir,q.y)-globe_height(slot,il,q.y))/(f32(ir-il)*cell);
    let dz = (globe_height(slot,q.x,jr)-globe_height(slot,q.x,jl))/(f32(jr-jl)*cell);
    let chart = chart_min + vec2<f32>(q)*cell;
    // Differentiate the tangent cube-sphere projection in body metres.
    // Its axes shear and change scale away from the middle of a face.
    let a = globe_axes(face);
    let radians_per_m = GLOBE_FRAC_PI_4 / GLOBE_HALF_M;
    let st = tan(chart * radians_per_m);
    let cube = a[1] + st.x * a[0] + st.y * a[2];
    let inverse_length = inverseSqrt(dot(cube, cube));
    let up = cube * inverse_length;
    let scale = (globe.radius_m + height) * radians_per_m * inverse_length;
    let along_x = (a[0] - up * dot(a[0], up)) * (scale * (1.0 + st.x * st.x)) + up * dx;
    let along_z = (a[2] - up * dot(a[2], up)) * (scale * (1.0 + st.y * st.y)) + up * dz;
    return normalize(cross(along_z, along_x));
}

struct GlobeVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) rel: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(4) @interpolate(flat) material: u32,
    // Chart x/z in metres, which equal world metres; the resident mask reads it.
    @location(5) chart: vec2<f32>,
    // Continuous lattice coordinate; the fragment blends the four samples around it.
    @location(7) lattice: vec2<f32>,
    @location(8) @interpolate(flat) slot: u32,
    @location(9) @interpolate(flat) cell_m: f32,
    @location(10) parent_lattice: vec2<f32>,
    @location(11) @interpolate(flat) parent_slot: u32,
    @location(12) @interpolate(flat) detail: f32,
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
    let x = tile.chart_min.x + f32(i) * tile.cell_m;
    let z = tile.chart_min.y + f32(j) * tile.cell_m;
    let has_parent = tile.pad != 0u;
    let detail = select(1.0,clamp(tile.origin_body.w,0.0,1.0),has_parent);
    let parent_slot = select(tile.slot,tile.pad-1u,has_parent);
    let parent_cell = tile.cell_m*2.0;
    let parent_width = parent_cell*f32(GLOBE_LAST);
    let parent_min = floor((tile.chart_min+GLOBE_HALF_M)/parent_width)*parent_width-GLOBE_HALF_M;
    let parent_lattice = clamp((vec2<f32>(x,z)-parent_min)/parent_cell,vec2<f32>(0.0),vec2<f32>(64.0));
    var height = globe_surface_m(raw);
    var normal = globe_lattice_normal(tile.slot,tile.face,vec2<u32>(i,j),tile.chart_min,tile.cell_m,height);
    if has_parent && detail < 1.0 {
        let base = min(vec2<u32>(floor(parent_lattice)),vec2<u32>(GLOBE_LAST-1u));
        let f = parent_lattice-vec2<f32>(base);
        // Match the parent mesh's two triangles, including their diagonal.
        let weights = select(vec4<f32>(1.0-f.x-f.y,f.x,f.y,0.0),
            vec4<f32>(0.0,1.0-f.y,1.0-f.x,f.x+f.y-1.0),f.x+f.y>1.0);
        var coarse_height = 0.0;
        var coarse_normal = vec3<f32>(0.0);
        for (var k=0u;k<4u;k++) {
            let w = weights[k];
            if w == 0.0 { continue; }
            let q = base+vec2<u32>(k&1u,k>>1u);
            let sample = textureLoad(height_atlas,globe_texel(parent_slot,q.x,q.y),0).xy;
            let surface_m = globe_surface_m(sample);
            coarse_height += surface_m*w;
            coarse_normal += globe_lattice_normal(parent_slot,tile.face,q,parent_min,parent_cell,surface_m)*w;
        }
        height = mix(coarse_height,height,detail);
        normal = mix(coarse_normal,normal,detail);
    }
    let h = height-select(0.0,mix(4.0,2.0,detail)*tile.cell_m,skirt);
    // Both folds are near the tile centre, so their difference keeps f32 precision.
    let local = globe_fold(tile.face, x, z, h) - globe_fold(tile.face, tile.centre.x, tile.centre.y, 0.0);
    let rel = local + tile.origin_body.xyz;


    var out: GlobeVsOut;
    out.clip = globe.view_proj * vec4<f32>(rel, 1.0);
    out.rel = rel;
    out.normal = normal;
    out.material = raw.y & 255u;
    out.chart = vec2<f32>(f32(tile.face) * GLOBE_PITCH_M + x, z);
    if globe_column_owned(out.chart) {
        out.clip.z -= max(out.clip.z, 0.0) * 0.0015;
    }
    out.lattice = vec2<f32>(f32(i), f32(j));
    out.slot = tile.slot;
    out.cell_m = tile.cell_m;
    out.parent_lattice = parent_lattice;
    out.parent_slot = parent_slot;
    out.detail = detail;
    return out;
}

// The sky's air model (planet_air_shared.wgsl) on the eye-to-surface segment.
fn globe_air(rel: vec3<f32>, jitter: f32) -> PlanetAir {
    let dist = length(rel);
    let steps = select(10, 6, (globe.flags & 1u) != 0u);
    return planet_air_segment(globe.eye_body.xyz, rel / max(dist, 1e-3), normalize(globe.sun_dir_body.xyz),
        globe.radius_m, dist, steps, jitter);
}

// Sunlight left after the air between a surface point and the sun; reddens the terminator.
fn globe_sun_through(p: vec3<f32>, sun: vec3<f32>) -> vec3<f32> {
    let r = length(p);
    return cel_sun_transmittance(r, dot(p, sun) / max(r, 1.0), globe.radius_m);
}

// Moonlight on a surface with normal `n`; zero when the moon is set below that horizon.
fn globe_moonlight(n: vec3<f32>) -> vec3<f32> {
    let moon = globe.moon_body;
    if dot(moon.xyz, moon.xyz) < 0.25 { return vec3<f32>(0.0); }
    return GLOBE_MOON * moon.w * max(dot(n, normalize(moon.xyz)), 0.0);
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

// Fract-only hash: chart cells reach 1e4, past where GPU sin() stays accurate.
fn globe_cell_hash(c: vec2<f32>) -> f32 {
    var p = fract(vec3<f32>(c.x, c.y, c.x) * 0.1031);
    p += dot(p, p.yzx + 33.33);
    return fract((p.x + p.y) * p.z);
}

fn globe_cell_noise(p: vec2<f32>) -> f32 {
    let c = floor(p);
    let f = p - c;
    let s = f * f * (3.0 - 2.0 * f);
    return mix(mix(globe_cell_hash(c), globe_cell_hash(c + vec2<f32>(1.0, 0.0)), s.x),
        mix(globe_cell_hash(c + vec2<f32>(0.0, 1.0)), globe_cell_hash(c + vec2<f32>(1.0, 1.0)), s.x), s.y);
}

fn globe_body_hash(c: vec3<f32>) -> f32 {
    var p = fract(c * 0.1031);
    p += dot(p, p.zyx + 31.32);
    return fract((p.x + p.y) * p.z);
}

fn globe_body_noise(p: vec3<f32>) -> f32 {
    let c = floor(p);
    let f = p - c;
    let s = f * f * (3.0 - 2.0 * f);
    let x = vec3<f32>(1.0, 0.0, 0.0);
    let y = vec3<f32>(0.0, 1.0, 0.0);
    let z = vec3<f32>(0.0, 0.0, 1.0);
    let near = mix(mix(globe_body_hash(c), globe_body_hash(c + x), s.x),
        mix(globe_body_hash(c + y), globe_body_hash(c + x + y), s.x), s.y);
    let far = mix(mix(globe_body_hash(c + z), globe_body_hash(c + x + z), s.x),
        mix(globe_body_hash(c + y + z), globe_body_hash(c + x + y + z), s.x), s.y);
    return mix(near, far, s.z);
}

// Bare ground had one palette colour per material, so a desert read as a flat
// shape from altitude. The tone drifts over kilometres in body space, which has
// no cube-face seams; the finer octave fades once it spans under two pixels.
fn globe_bare_tone(body: vec3<f32>, footprint_m: f32) -> vec3<f32> {
    let q = body / 2600.0;
    let broad = globe_body_noise(q) * 0.6 + globe_body_noise(q * 2.3 + vec3<f32>(17.0, -5.0, 9.0)) * 0.4;
    let detail = 1.0 - smoothstep(320.0, 1280.0, footprint_m);
    let fine = mix(0.5, globe_body_noise(body / 640.0 + vec3<f32>(-23.0, 11.0, 3.0)), detail);
    let v = smoothstep(0.2, 0.8, broad * 0.75 + fine * 0.25);
    return mix(vec3<f32>(0.80, 0.70, 0.62), vec3<f32>(1.10, 1.08, 1.05), v);
}

// Stone, sand, gravel, sandstone and limestone take the tone; damp ground is darker.
fn globe_bare_albedo(albedo: vec3<f32>, material: u32, wet: f32, tone: vec3<f32>) -> vec3<f32> {
    let bare = material == 2u || material == 3u || material == 5u || material == 7u || material == 15u;
    if !bare { return albedo; }
    return albedo * tone * (1.0 - 0.2 * smoothstep(0.1, 0.45, wet));
}

struct GlobeSurfaceSample {
    ground: vec4<f32>,
    // Signed ocean depth, inland coverage, and inland depth times coverage.
    water: vec3<f32>,
    suspended: f32,
    // Canopy coverage, heat, local sky visibility, exposed mineral fraction.
    fields: vec4<f32>,
};

// Albedo from the four lattice samples around the fragment. A flat per-vertex
// material took the provoking vertex's id, so every material border followed
// the mesh diagonals and read as triangles from orbit. Weights are warped by
// cell-scale noise and sharpened to the screen footprint: borders stay crisp
// and irregular up close and resolve to a blend once a cell is under a pixel.
// RGB is reflected colour. Alpha is the fraction of the footprint emitting lava light.
fn globe_surface_sample(lattice: vec2<f32>, slot: u32, cell_m: f32, chart: vec2<f32>, footprint_px: f32, tone: vec3<f32>, orbital_blend: f32) -> GlobeSurfaceSample {
    let base = min(vec2<u32>(floor(lattice)), vec2<u32>(GLOBE_LAST - 1u));
    let f = clamp(lattice - vec2<f32>(base), vec2<f32>(0.0), vec2<f32>(1.0));
    // At orbital distances a hard material transition exposes the atlas grid.
    // Blend the existing taps across the cell to keep broad biome shapes.
    var w = f;
    if orbital_blend < 1.0 {
        // Chart metres in cell units keep the warp continuous across tile borders.
        let q = chart / max(cell_m, 1.0);
        let warp = vec2<f32>(
            globe_cell_noise(q * 0.9 + vec2<f32>(17.0, 3.0)) * 0.65 + globe_cell_noise(q * 2.7 + vec2<f32>(-5.0, 41.0)) * 0.35,
            globe_cell_noise(q * 0.9 + vec2<f32>(-29.0, 11.0)) * 0.65 + globe_cell_noise(q * 2.7 + vec2<f32>(23.0, -7.0)) * 0.35) - vec2<f32>(0.5);
        let edge = clamp(footprint_px * 0.75, 0.06, 0.5);
        let crisp = smoothstep(vec2<f32>(0.5 - edge), vec2<f32>(0.5 + edge), f + warp * 0.55);
        w = mix(crisp, f, orbital_blend);
    }
    var surface = GlobeSurfaceSample(vec4<f32>(0.0), vec3<f32>(0.0), 0.0, vec4<f32>(0.0));
    for (var k = 0u; k < 4u; k += 1u) {
        let o = vec2<u32>(k & 1u, k >> 1u);
        let wx = select(1.0 - w.x, w.x, o.x == 1u);
        let wz = select(1.0 - w.y, w.y, o.y == 1u);
        let weight = wx * wz;
        let water_weight = select(1.0 - f.x, f.x, o.x == 1u) * select(1.0 - f.y, f.y, o.y == 1u);
        if weight == 0.0 && water_weight == 0.0 { continue; }
        let texel = globe_texel(slot, base.x + o.x, base.y + o.y);
        let raw = textureLoad(height_atlas, texel, 0).xy;
        let flooded = (raw.y & GLOBE_WATER_BIT) != 0u;
        let has_suspension = flooded && (globe.flags & 16u) != 0u;
        if weight > 0.0 || water_weight > 0.0 {
            let climate = textureLoad(climate_atlas, texel, 0);
            let material = raw.y & 255u;
            if weight > 0.0 {
                let albedo = globe_bare_albedo(globe_albedo(material, climate.y), material, climate.y, tone);
                surface.ground += vec4<f32>(albedo, select(0.0, 1.0, material == 14u)) * weight;
            }
            // Canopy, heat and sky visibility are continuous fields. The
            // sharpened material weights cut them into dark squares the size
            // of a tile cell wherever a cell spanned several pixels.
            let mineral = material == 1u || material == 2u || material == 3u || material == 5u || material == 7u || material == 15u;
            surface.fields += vec4<f32>(climate.z, climate.x, climate.w, select(0.0, 1.0, mineral)) * water_weight;
            if has_suspension { surface.suspended += climate.x * water_weight; }
        }
        // Water uses the same unwarped bilinear field on both triangles. Keep
        // the dry side signed so a wet/dry edge crosses at sea level.
        let inland_depth = select(0.0, f32(raw.y >> 9u), flooded);
        let sea_depth = globe.sea_level_m - f32(i32(raw.x) - GLOBE_HEIGHT_BIAS);
        let ocean = flooded && (inland_depth == 0.0 || sea_depth > 0.0);
        // The atlas rounds heights to metres. Preserve the wet/dry sign at
        // zero and keep below-sea ice on the dry side.
        let signed_depth = select(-max(abs(sea_depth), 0.125), max(sea_depth, 0.125), ocean);
        surface.water += vec3<f32>(signed_depth, select(0.0, 1.0, inland_depth > 0.0), inland_depth) * water_weight;
    }
    return surface;
}

// Appearance detail is filtered to the visible footprint. The already-resident
// 256 KiB noise lattice replaces 24+ scalar hashes; no new texture is uploaded.
// It modulates reflectance, never coastlines, relief, river paths or snow extent.
fn globe_surface_detail(body: vec3<f32>, footprint_m: f32) -> vec2<f32> {
    let broad = globe_cloud_noise(body / 420.0);
    let medium_weight = 1.0 - smoothstep(22.0, 80.0, footprint_m);
    let fine_weight = 1.0 - smoothstep(5.0, 20.0, footprint_m);
    var medium = 0.5;
    var fine = 0.5;
    if medium_weight > 0.0 { medium = globe_cloud_noise(body / 110.0 + vec3<f32>(7.3, 19.7, -11.4)); }
    if fine_weight > 0.0 { fine = globe_cloud_noise(body / 27.0 + vec3<f32>(-13.7, 9.3, 31.1)); }
    return vec2<f32>(broad - 0.5, (medium - 0.5) * medium_weight + (fine - 0.5) * fine_weight * 0.35);
}

fn globe_ecological_albedo(surface: GlobeSurfaceSample, body: vec3<f32>, footprint_m: f32, distance_m: f32) -> vec3<f32> {
    let detail = globe_surface_detail(body, footprint_m);
    let heat = surface.fields.y;
    // These remain within the ground material families. Broadleaf woodland is
    // warmer than conifer cover; moisture is already in the sampled soil colour.
    let conifer = vec3<f32>(0.040, 0.095, 0.070);
    let broadleaf = vec3<f32>(0.068, 0.155, 0.052);
    let canopy_color = mix(conifer, broadleaf, smoothstep(0.28, 0.70, heat));
    // A tile cell stores one mean canopy, which drew each stand as a dark
    // slab with the cell's outline. Stands are ragged inside that mean:
    // clearings, edges and age classes, at scales the body-space lattice
    // carries until they fall under two pixels.
    var stand = globe_cloud_noise(body / 190.0 + vec3<f32>(2.1, -6.3, 4.7));
    let crown_weight = 1.0 - smoothstep(12.0, 40.0, footprint_m);
    if crown_weight > 0.0 {
        stand = mix(stand, globe_cloud_noise(body / 60.0 + vec3<f32>(-8.4, 1.9, 3.3)), 0.4 * crown_weight);
    }
    let canopy = clamp(surface.fields.x * (0.55 + 0.9 * stand), 0.0, 1.0) * smoothstep(300.0, 1500.0, distance_m);
    let ground = mix(surface.ground.rgb, canopy_color, canopy * 0.80);
    let mineral = surface.fields.w * (1.0 - canopy);
    let variation = 1.0 + detail.x * mix(0.18, 0.30, mineral) + detail.y * mix(0.22, 0.34, mineral);
    return ground * variation;
}

// Distance grade for land albedo: pull chroma toward luminance and deepen
// vegetation greens, leaving snow and sand nearly unchanged. Bare rock is a
// cool blue-grey up close; weathered ranges seen from orbit are warmer.
fn globe_orbital_grade(albedo: vec3<f32>, amount: f32) -> vec3<f32> {
    let luma = dot(albedo, vec3<f32>(0.2126, 0.7152, 0.0722));
    let green = clamp((albedo.g - max(albedo.r, albedo.b)) / max(albedo.g, 1e-3), 0.0, 1.0);
    var muted = mix(vec3<f32>(luma), albedo, 0.72 - 0.20 * green) * (1.0 - 0.30 * green);
    let chroma = (max(albedo.r, max(albedo.g, albedo.b)) - min(albedo.r, min(albedo.g, albedo.b))) / max(luma, 1e-3);
    let grey_rock = (1.0 - smoothstep(0.10, 0.30, chroma)) * (1.0 - smoothstep(0.30, 0.50, luma));
    muted = mix(muted, luma * vec3<f32>(1.12, 1.0, 0.84), 0.55 * grey_rock);
    return mix(albedo, muted, amount);
}

// Land is mottled at every scale: geology, soil moisture, stands of
// different age. Two body-space octaves the size of provinces and valleys
// carry that from orbit, where the finer detail octaves have filtered away.
// Returns a brightness factor and a warm/cool hue lean, each around zero.
fn globe_province_mottle(body: vec3<f32>, footprint_m: f32) -> vec2<f32> {
    let province = globe_cloud_noise(body / 5200.0 + vec3<f32>(3.7, -8.1, 5.3));
    let valley_weight = 1.0 - smoothstep(350.0, 1400.0, footprint_m);
    var valley = 0.5;
    if valley_weight > 0.0 { valley = globe_cloud_noise(body / 1500.0 + vec3<f32>(-6.2, 4.4, 9.9)); }
    let hue = globe_cloud_noise(body / 3300.0 + vec3<f32>(12.5, 2.2, -4.8));
    return vec2<f32>((province - 0.5) * 0.34 + (valley - 0.5) * 0.26 * valley_weight, hue - 0.5);
}

fn color_of_debug_view(view: u32, albedo: vec3<f32>, n: vec3<f32>, ndl: f32, material: u32, rel: vec3<f32>) -> vec3<f32> {
    switch view {
        // Atmosphere transmittance: what the air leaves of the surface.
        case 7u: { return globe_air(rel, 0.5).transmittance; }
        case 18u: { return albedo; }
        case 19u: { return 0.15 + 0.75 * fract(f32(material + 1u) * vec3<f32>(0.618034, 0.381966, 0.236068)); }
        case 20u, 21u: { return n * 0.5 + 0.5; }
        case 23u: { return vec3<f32>(ndl) * GLOBE_SUN; }
        case 24u: { return vec3<f32>(GLOBE_AMBIENT) * GLOBE_SKY; }
        // The globe bakes no occlusion, so these read as open sky.
        case 11u, 28u, 29u: { return vec3<f32>(1.0); }
        case 26u: { return albedo * (ndl * GLOBE_SUN + GLOBE_AMBIENT * GLOBE_SKY); }
        default: { return vec3<f32>(1.0, 0.0, 1.0); }
    }
}

struct GlobeOutput {
    @location(0) color: vec4<f32>,
    // The terrain pass's reflection receiver target; the globe takes no indirect light.
    @location(1) receiver: vec4<f32>,
};

fn gs_hash(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.3183099 + vec3<f32>(0.1, 0.2, 0.3)) * 17.0;
    return fract(q.x * q.y * q.z * (q.x + q.y + q.z));
}

fn gs_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(gs_hash(i), gs_hash(i + vec3<f32>(1, 0, 0)), u.x),
                mix(gs_hash(i + vec3<f32>(0, 1, 0)), gs_hash(i + vec3<f32>(1, 1, 0)), u.x), u.y);
    let b = mix(mix(gs_hash(i + vec3<f32>(0, 0, 1)), gs_hash(i + vec3<f32>(1, 0, 1)), u.x),
                mix(gs_hash(i + vec3<f32>(0, 1, 1)), gs_hash(i + vec3<f32>(1, 1, 1)), u.x), u.y);
    return mix(a, b, u.z);
}

struct SeaState {
    slope: vec3<f32>,
    variance: f32,
    foam: f32,
}

// One deep-water swell train along fixed body axis `axis`; xyz slope, w unresolved variance.
fn gs_swell(p: vec3<f32>, axis: vec3<f32>, wavelength: f32, steep: f32, t: f32, fp: f32) -> vec4<f32> {
    let k = 6.2831853 / wavelength;
    let ph = dot(p, axis) * k - sqrt(9.81 * k) * t;
    // Retire coherent wave normals before they become a screen-space lattice.
    // Their energy remains in slope variance, preserving the sun-glint lobe.
    let resolved = exp(-0.5 * pow(fp * k * 0.5, 2.0)) * (1.0 - smoothstep(3.0, 14.0, fp));
    let s = sin(ph) / (1.0 - 0.3 * cos(ph));
    return vec4<f32>(axis * (s * steep * resolved), 0.5 * steep * steep * (1.0 - resolved * resolved));
}

// Wind sea from fixed-frame swell, gust patches and whitecaps; `fp` is metres per pixel.
fn globe_sea_state(p: vec3<f32>, up: vec3<f32>, t: f32, fp: f32) -> SeaState {
    let warp = gs_noise(p / 2300.0);
    let gust = gs_noise(p / 700.0 + vec3<f32>(t * 0.002, warp * 1.7, 0.0));
    // Weather-scale wind: calm slicks and rough streaks kilometres across
    // give the sun glint its mottled edge from orbit.
    let synoptic = gs_noise(p / 6100.0 + vec3<f32>(3.1, t * 0.0004, -7.7));
    let rough = smoothstep(0.22, 0.78, gust * 0.6 + synoptic * 0.4 + (warp - 0.5) * 0.2);
    let energy = mix(0.30, 1.35, rough);
    if fp >= 14.0 {
        // The four wave trains and whitecaps are entirely unresolved here.
        // Analytic variance avoids both tiling and their trigonometry/noise.
        return SeaState(vec3<f32>(0.0), 0.006 + 0.015225 * energy, 0.022 * rough * rough);
    }
    let q = p + vec3<f32>(warp - 0.5, gust - 0.5, warp - gust) * 46.0;
    let s0 = gs_swell(q, normalize(vec3<f32>(0.83, 0.2, 0.56)), 110.0, 0.035, t, fp);
    let s1 = gs_swell(q, normalize(vec3<f32>(0.24, 0.5, 0.97)), 62.0, 0.040, t, fp);
    let s2 = gs_swell(q, normalize(vec3<f32>(-0.66, 0.3, 0.75)), 34.0, 0.045, t, fp);
    let s3 = gs_swell(q, normalize(vec3<f32>(0.90, -0.4, -0.44)), 19.0, 0.040, t, fp);
    var out: SeaState;
    out.slope = (s0.xyz + s1.xyz + s2.xyz + s3.xyz) * energy;
    // Cox-Munk wind-sea variance (~0.02 at 7 m/s) takes over as texels go sub-pixel.
    out.variance = 0.006 + (s0.w + s1.w + s2.w + s3.w + 0.012 * smoothstep(0.5, 6.0, fp)) * energy;
    let cap = smoothstep(0.80, 0.93, gs_noise(q / 7.0 + vec3<f32>(t * 0.11, 0.0, -t * 0.07)));
    out.foam = mix(0.022, cap * 0.85, 1.0 - smoothstep(1.5, 5.0, fp)) * rough * rough;
    return out;
}

@fragment
fn fs_main(in: GlobeVsOut) -> GlobeOutput {
    let fade = globe_mask_alpha(in.chart);
    // Under resident chunks the chunks draw the ground; a faded-out fragment must not write depth.
    if fade < 1.0 / 255.0 {
        discard;
    }
    let n = normalize(in.normal);
    let footprint = max(length(fwidth(in.lattice)),1e-4);
    let surface_mode = (globe.flags >> 16u) & 3u;
    var tone = vec3<f32>(1.0);
    if surface_mode < 2u { tone = globe_bare_tone(globe.eye_body.xyz + in.rel, footprint * in.cell_m); }
    let dist = length(in.rel);
    // Sharpened material borders show the tile lattice once a cell spans
    // several pixels; from a few kilometres the taps blend instead.
    let orbital_blend = smoothstep(2500.0, 12000.0, dist);
    var surface: GlobeSurfaceSample;
    if in.detail <= 0.0 {
        surface = globe_surface_sample(in.parent_lattice,in.parent_slot,in.cell_m*2.0,in.chart,footprint*0.5,tone,orbital_blend);
    } else {
        surface = globe_surface_sample(in.lattice,in.slot,in.cell_m,in.chart,footprint,tone,orbital_blend);
        if in.detail < 1.0 {
            let coarse = globe_surface_sample(in.parent_lattice,in.parent_slot,in.cell_m*2.0,in.chart,footprint*0.5,tone,orbital_blend);
            surface.ground = mix(coarse.ground,surface.ground,in.detail);
            surface.water = mix(coarse.water,surface.water,in.detail);
            surface.suspended = mix(coarse.suspended,surface.suspended,in.detail);
            surface.fields = mix(coarse.fields,surface.fields,in.detail);
        }
    }
    var ground = surface.ground.rgb;
    let water = surface.water;
    let coast_edge = max(0.015, 0.5 * fwidth(water.x));
    let ocean = smoothstep(-coast_edge, coast_edge, water.x);
    let inland_edge = clamp(fwidth(water.y), 0.015, 0.25);
    let inland = smoothstep(0.5 - inland_edge, 0.5 + inland_edge, water.y);
    let wet = max(ocean, inland);
    let view = -in.rel / max(dist, 1e-3);
    let sun = normalize(globe.sun_dir_body.xyz);
    let p = globe.eye_body.xyz + in.rel;
    let up = normalize(p);
    // Projected footprint includes foreshortening at the ocean limb. A plain
    // distance/focal estimate underfiltered grazing waves into repeating dots.
    let sea_footprint = max(length(dpdx(p)), length(dpdy(p)));
    if surface_mode >= 2u && wet < 1.0 {
        ground = globe_ecological_albedo(surface, p, sea_footprint, dist);
    }
    // The palette is the voxel art's colour, right for ground you stand on.
    // Seen through kilometres of canopy gaps, shadow and dry litter, land is
    // darker and far less saturated; grade toward that with distance.
    ground = globe_orbital_grade(ground, orbital_blend);
    if orbital_blend > 0.0 && wet < 1.0 {
        let mottle = globe_province_mottle(p, sea_footprint) * orbital_blend;
        ground *= (1.0 + mottle.x) * mix(vec3<f32>(1.0), vec3<f32>(1.07, 1.0, 0.90), mottle.y);
        if surface_mode == 3u {
            // Valley floors are darker than the ridges beside them: deeper
            // soil, denser growth, less sky. The baked sky visibility is that
            // field at tile resolution, and it draws the drainage from orbit.
            let open = smoothstep(0.84, 1.0, surface.fields.z);
            ground *= mix(1.0, mix(0.66, 1.06, open), orbital_blend);
        }
    }
    let hell = (globe.flags & 8u) != 0u;
    let sun_light = select(GLOBE_SUN, vec3<f32>(1.0, 0.67, 0.38), hell) * globe_sun_through(p, sun);
    // A 500 m range is a few texels from orbit. Lean the shading normal
    // further off the vertical there so ranges keep a lit and a shaded side.
    let relief_n = normalize(n + (n - up * dot(n, up)) * (0.9 * orbital_blend));
    let land_ndl = max(dot(relief_n, sun), 0.0);
    let sea_ndl = max(dot(up, sun), 0.0);
    var sun_visibility = 1.0;
    if (globe.flags & 2u) != 0u && (land_ndl > 0.0 || (wet > 0.0 && sea_ndl > 0.0)) {
        sun_visibility -= 0.6 * globe_shell_alpha_along(p, sun);
    }
    let ndl = land_ndl * sun_visibility;
    // Same ids and encodings as the terrain pass (render/debug.rs), so one
    // capture shows both surfaces in one view. Haze stands in for fog transmittance.
    let debug_view = (globe.flags >> 8u) & 255u;
    if debug_view != 0u {
        let albedo = select(ground, GLOBE_SHALLOW_WATER, wet > 0.5);
        var out = color_of_debug_view(debug_view, albedo, n, ndl, in.material, in.rel);
        return GlobeOutput(vec4<f32>(out, fade), vec4<f32>(0.0));
    }
    let ambient = GLOBE_AMBIENT * select(GLOBE_SKY, vec3<f32>(0.55, 0.25, 0.32), hell)
        * select(1.0, surface.fields.z, surface_mode == 3u);
    var color = ground * (ndl * sun_light + ambient);
    if wet > 0.0 {
        // The lattice normal includes steep land on neighbouring samples.
        // Use the local sea normal for reflections at the shoreline.
        let sea_ndv = clamp(dot(up, view), 0.0, 1.0);
        let fresnel = 0.02 + 0.98 * pow(1.0 - sea_ndv, 5.0);
        // Wind sea: swell trains tilt the normal, gust patches change the
        // slope variance, and the part the pixel footprint cannot resolve widens the lobe.
        let ocean_depth = max(water.x, 0.0);
        let open_sea = ocean * (1.0 - inland) * smoothstep(1.0, 8.0, ocean_depth);
        let sea = globe_sea_state(p, up, globe.sun_dir_body.w, sea_footprint);
        let sea_n = normalize(up + (sea.slope - up * dot(up, sea.slope)) * open_sea);
        let half_vector = view + sun;
        let h = half_vector / max(length(half_vector), 0.0001);
        let ndh = clamp(dot(sea_n, h), 0.0, 1.0);
        let vdh = clamp(dot(view, h), 0.0, 1.0);
        let slope_variance = mix(0.01, sea.variance, open_sea);
        let distribution_denom = ndh * ndh * (slope_variance - 1.0) + 1.0;
        let distribution = slope_variance / (3.14159265 * distribution_denom * distribution_denom);
        let masking_view = sea_ndl * sqrt(sea_ndv * sea_ndv * (1.0 - slope_variance) + slope_variance);
        let masking_sun = sea_ndv * sqrt(sea_ndl * sea_ndl * (1.0 - slope_variance) + slope_variance);
        let visibility = 0.5 / max(masking_view + masking_sun, 0.0001);
        let sun_fresnel = 0.02 + 0.98 * pow(1.0 - vdh, 5.0);
        // `sun_light` is radiance of a white diffuser (irradiance / pi); the
        // specular lobe takes the irradiance itself, hence the factor pi.
        let glint = 3.14159265 * distribution * visibility * sun_fresnel * sea_ndl * sun_visibility;
        // Shelves read turquoise because the floor shows through water that takes red first.
        // Approximate the longer path through water at oblique viewing and sun
        // angles. Refraction limits that increase even near the limb.
        let inland_depth = water.z / max(water.y, 0.01);
        let depth = mix(ocean_depth, max(ocean_depth, inland_depth), inland);
        let optical_depth = depth * (1.0 + 0.35 * (1.0 - sea_ndv) + 0.20 * (1.0 - sea_ndl));
        let through = exp(-optical_depth * GLOBE_WATER_ABSORB);
        let shelf = vec3<f32>(0.010, 0.055, 0.049) * ((1.0 - through.r) * through.g);
        let bed = mix(ground, GLOBE_SHALLOW_WATER, 0.24 * orbital_blend);
        let clear_body = bed * through + GLOBE_DEEP_WATER * (1.0 - through) + shelf;
        // Shelf seas carry plankton in drifting swirls; open ocean stays clear.
        let bloom = smoothstep(0.52, 0.80, gs_noise(p / 7400.0 + vec3<f32>(11.0, 4.0, -9.0))
            * 0.65 + gs_noise(p / 2100.0 + vec3<f32>(-3.0, 8.0, 5.0)) * 0.35)
            * (1.0 - smoothstep(12.0, 55.0, depth)) * smoothstep(1.5, 6.0, depth) * (1.0 - inland);
        let living_body = mix(clear_body, vec3<f32>(0.010, 0.052, 0.040), 0.45 * bloom);
        let body = mix(living_body, vec3<f32>(0.130, 0.109, 0.076), 0.68 * surface.suspended);
        let water_light = sea_ndl * sun_visibility * sun_light + ambient;
        var water_color = body * water_light
            + fresnel * GLOBE_SKY * 0.45 * max(dot(up, sun) + 0.1, 0.0) + glint * sun_light;
        let foam_lit = vec3<f32>(0.80, 0.83, 0.79) * water_light;
        water_color = mix(water_color, foam_lit, sea.foam * open_sea);
        // Surf where the swell shoals: a broken line along exposed shores,
        // absent in the lee of calm patches. Below a pixel it is a faint rim.
        let shoal = ocean * (1.0 - inland) * (1.0 - smoothstep(0.3, 2.6, ocean_depth));
        if shoal > 0.0 {
            let breaks = smoothstep(0.35, 0.75, gs_noise(p / 90.0 + vec3<f32>(0.0, globe.sun_dir_body.w * 0.02, 0.0)));
            let energy = clamp(sea.variance * 40.0, 0.2, 1.0);
            water_color = mix(water_color, foam_lit, 0.55 * shoal * breaks * energy);
        }
        color = mix(color, water_color, wet);
    }
    // Skylight fades over twilight; direct sun already falls off with the air above.
    let day = smoothstep(-0.12, 0.08, dot(up, sun));
    // Moonlight keeps terrain and coastlines readable: a sky floor plus the moon itself.
    let night_albedo = mix(ground, GLOBE_DEEP_WATER * 0.6, wet);
    let moon = night_albedo * ((0.35 + 0.65 * max(dot(n, up), 0.0)) * GLOBE_MOON * 0.5 + globe_moonlight(n));
    color = mix(GLOBE_NIGHT_FLOOR + moon, color, day);
    color += vec3<f32>(1.2, 0.13, 0.008) * surface.ground.a * (1.0 - wet);
    if (globe.flags & 4u) != 0u {
        // Squared so twilight land does not show lights as white blooms.
        let dark = (1.0 - day) * (1.0 - day);
        // Ground illumination belongs to exposed land. Projecting the
        // anchorage light field onto sea fragments made bright city grids in
        // empty oceans; platforms retain their own rendered emissive lights.
        color += globe_city_glow(globe.eye_body.xyz + in.rel) * dark * (1.0 - wet);
    }
    // Aerial perspective from the same integral as the sky's limb, so the disc edge meets it.
    let jitter = fract(dot(floor(in.clip.xy), vec2<f32>(0.754877666, 0.569840296)));
    let air = globe_air(in.rel, jitter);
    color = color * air.transmittance + air.inscatter + air.airglow;
    color += planet_aurora_ray(globe.eye_body.xyz, normalize(in.rel), length(in.rel),
        sun, globe.radius_m, globe_clock());
    return GlobeOutput(vec4<f32>(color, fade), vec4<f32>(0.0));
}
