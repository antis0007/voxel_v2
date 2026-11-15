struct ResidentCoverage {
    header: vec4<i32>,
    words: array<vec4<u32>, 131>,
    sparse_header: vec4<i32>,
    sparse_keys: array<vec4<i32>, 512>,
};

struct TerrainArrival {
    count: vec4<u32>,
    slots: array<vec4<i32>, 256>,
};

struct Camera {
    view_proj : mat4x4<f32>,
    camera_pos : vec4<f32>,
    inv_view_proj : mat4x4<f32>,
    world_params : vec4<f32>,
    prev_view_proj : mat4x4<f32>,
    camera_forward : vec4<f32>,
    ray_right : vec4<f32>,
    ray_up : vec4<f32>,
    previous_camera_pos : vec4<f32>,
    scene_seed : vec4<u32>,
    // x: smoothed resident detail handoff radius in metres; negative means
    // no override. y: sea level. z: full-detail (LOD 0) ring radius.
    // w: strict GPU-published resident disc radius; zero means no complete disc.
    coverage : vec4<f32>,
    resident_coverage: ResidentCoverage,
    // xyz: render origin in voxels; chunk vertices are drawn relative to it.
    render_origin: vec4<i32>,
    // xyz: eye minus the render origin in metres, less the view matrix residual.
    eye_from_origin: vec4<f32>,
    planet: array<vec4<f32>, 8>,
    // Portal destination views keep dot(xyz, eye-relative position) + w >= 0.
    // All zero in the primary view.
    portal_clip: vec4<f32>,
    // Chart shear undo around the eye (render/planet_frame.rs). [0].xyz and
    // [1].xyz: undo per chart metre on x and z; [0].w near, [1].w far radius;
    // [2]: eye chart x, z on its face, face index. Zero far radius: off.
    planet_flat: array<vec4<f32>, 3>,
    // Rows taking an eye direction to the star system's fixed axes (planet::sky_axes). Zero: identity.
    sky_axes: array<vec4<f32>, 3>,
    terrain_arrival: TerrainArrival,
};

// Star and galaxy lookup direction, fixed in space while the planet turns.
fn sky_fixed(d: vec3<f32>) -> vec3<f32> {
    let r = cam.sky_axes;
    if dot(r[0].xyz, r[0].xyz) < 0.5 { return d; }
    return vec3<f32>(dot(r[0].xyz, d), dot(r[1].xyz, d), dot(r[2].xyz, d));
}

fn portal_view() -> bool {
    return any(cam.portal_clip != vec4<f32>(0.0));
}

@group(0) @binding(0) var<uniform> cam : Camera;

fn planet_basis(face: u32) -> mat3x3<f32> {
    switch face {
        case 0u: { return mat3x3<f32>(vec3<f32>(0,-1,0),vec3<f32>(1,0,0),vec3<f32>(0,0,1)); }
        case 1u: { return mat3x3<f32>(vec3<f32>(0,1,0),vec3<f32>(-1,0,0),vec3<f32>(0,0,1)); }
        case 2u: { return mat3x3<f32>(vec3<f32>(1,0,0),vec3<f32>(0,1,0),vec3<f32>(0,0,1)); }
        case 3u: { return mat3x3<f32>(vec3<f32>(1,0,0),vec3<f32>(0,-1,0),vec3<f32>(0,0,-1)); }
        case 4u: { return mat3x3<f32>(vec3<f32>(1,0,0),vec3<f32>(0,0,1),vec3<f32>(0,-1,0)); }
        default: { return mat3x3<f32>(vec3<f32>(1,0,0),vec3<f32>(0,0,-1),vec3<f32>(0,1,0)); }
    }
}

fn planet_relative(p: vec3<f32>) -> vec3<f32> {
    let params = cam.planet[0];
    let face = u32(clamp(floor((p.x + params.z * 0.5) / params.z), 0.0, 5.0));
    let local = vec3<f32>(p.x - f32(face) * params.z, p.y, p.z);
    return planet_project(face, local);
}

fn planet_project(face: u32, local: vec3<f32>) -> vec3<f32> {
    let params = cam.planet[0];
    // Past a face edge, s leaves [-1, 1] and tan heads for a pole. Pin the
    // cover there; resident chunks do not use this path.
    let u = clamp(local.xz / params.y, vec2<f32>(-1.0), vec2<f32>(1.0));
    let q = tan(u * 0.7853981634);
    let point = normalize(planet_basis(face) * vec3<f32>(q.x,1,q.y)) * (params.x + local.y);
    let delta = point - cam.planet[1].xyz;
    let sphere = vec3<f32>(dot(delta,cam.planet[2].xyz),dot(delta,cam.planet[3].xyz),dot(delta,cam.planet[4].xyz));
    return sphere + planet_flat_undo(point);
}

// Same blend as planet_frame::Eye::relative, read through the eye face's tan chart.
fn planet_flat_undo(body: vec3<f32>) -> vec3<f32> {
    let rows = cam.planet_flat;
    if rows[1].w <= 0.0 { return vec3<f32>(0.0); }
    let b = planet_basis(u32(rows[2].z));
    let dn = dot(body, b[1]);
    if dn <= 0.0 { return vec3<f32>(0.0); }
    let chart = atan(vec2<f32>(dot(body, b[0]), dot(body, b[2])) / dn) / 0.7853981634 * cam.planet[0].y;
    let d = chart - rows[2].xy;
    let reach = log(max(length(d), rows[0].w) / rows[0].w) / log(rows[1].w / rows[0].w);
    let w = 1.0 - smoothstep(0.0, 1.0, reach);
    return (rows[0].xyz * d.x + rows[1].xyz * d.y) * w;
}

// Projection-scaled basis vectors never contain world translation.
fn camera_ray(ndc: vec2<f32>) -> vec3<f32> {
    return normalize(cam.ray_right.xyz * ndc.x + cam.ray_up.xyz * ndc.y + cam.camera_forward.xyz);
}

struct Lighting {
    sun_dir_intensity : vec4<f32>,
    moon_dir_intensity : vec4<f32>,
    sun_color_daylight : vec4<f32>,
    moon_color_twilight : vec4<f32>,
    ambient_exposure : vec4<f32>,
    sky_top_saturation : vec4<f32>,
    // xyz: horizon sky colour. w: distant terrain range in metres, 0 when
    // no backdrop is drawn.
    sky_horizon_reserved : vec4<f32>,
    fog_color_density : vec4<f32>,
    fog_params : vec4<f32>,
    weather_globals : array<vec4<f32>, 4>,
    cloud_authoring : array<vec4<f32>, 2>,
    weather_clip_regions : array<vec4<f32>, 3>,
    weather_clip_summaries : array<vec4<f32>, 3>,
    held_torch_pos_radius : vec4<f32>,
    held_torch_color_strength : vec4<f32>,
    torch_meta : vec4<f32>,
    torch_positions : array<vec4<f32>, 64>,
    torch_colors : array<vec4<f32>, 64>,
    material_tints: array<vec4<f32>,256>,
    material_properties: array<vec4<f32>,256>,
    surface_weather: vec4<f32>,
    // x: cosmos chamber, y: aurora, z: meteor shower, w: rainbow.
    mood: vec4<f32>,
    // x: fireflies, y: pollen, z: diamond dust, w: embers.
    ambience: vec4<f32>,
    // Petals, spores, spray, dry dust.
    biome_ambience: vec4<f32>,
    // Healthy glints, blight, poisonous pressure, mist.
    mana_ambience: vec4<f32>,
    location_mood: vec4<f32>,
    // Palette A/B/C, strength/speed/width/chromatic, density/angle/sparkle.
    magic: array<vec4<f32>,5>,
    // SkyArt (render/atmosphere_params.rs): [disc cos radius, glare, sky grade,
    // sunset saturation], [cloud contrast, exposure gain, -, -].
    sky_art: array<vec4<f32>,2>,
    torch_group_bounds: array<vec4<f32>,16>,
    // Sunless-realm light rig (world.rs LightingState::realm): [0] on, cavern,
    // grotto; [1] fill; [2] air rgb + range; [3..9) glow pools; [9..11) shaft.
    realm: array<vec4<f32>,12>,
};

@group(1) @binding(0) var<uniform> light : Lighting;

// Shared by opaque terrain and clouds: fading terrain must also release its
// occlusion of the background volume, not merely change its surface colour.
fn terrain_streaming_fade(_horizontal_distance: f32) -> f32 {
    // Retained for shader ABI compatibility, but deliberately disabled.
    // Resident terrain is no longer discarded beyond the published coverage,
    // and the depth-tested heightfield now fills any transient missing chunk.
    // Blending valid resident geometry toward sky created a moving transparent
    // ring and let the sky/cloud pass show through ground that was still there.
    return 0.0;
}
