// ============================================================
// sky_shader.wgsl
//
// Full-screen sky pass.
//
// The sky pass no longer owns cloud composition. It runs after terrain and
// cloud media are available, but it still shades only untouched scene depth.
// Cloud compositing stays in cloud_composite.wgsl.
//
// Improvements over previous version:
//   • Sun disk with limb-darkening (darker at edges, matching real photosphere)
//   • Moon with subtle crescent shading and colour temperature
//   • Star field through octahedral mapping (unchanged logic, better density)
//   • ACES tonemap + saturation grading applied to the final composite
// ============================================================

struct VsOut {
    @builtin(position) @invariant pos: vec4<f32>,
    @location(0)       ndc: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var out: VsOut;
    let pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0),
    );
    let ndc = pos[vertex_index];
    out.pos = vec4<f32>(ndc, 0.0, 1.0); // reverse-Z: far plane is depth 0
    out.ndc = ndc;
    return out;
}

// Reconstruct world-space ray direction from NDC.
fn sky_dir(ndc: vec2<f32>) -> vec3<f32> {
    return camera_ray(ndc);
}

// ---- Tonemapping & grading ----

// ACES filmic tonemapping (Narkowicz approximation).
// Maps HDR linear → display-referred [0,1].
fn aces_tonemap(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp(
        (color * (a * color + vec3<f32>(b))) / (color * (c * color + vec3<f32>(d)) + vec3<f32>(e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
}

fn saturate_color(color: vec3<f32>, amount: f32) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    return mix(vec3<f32>(luma), color, amount);
}

// ---- Celestial body helpers ----

// Octahedral map: encodes direction → 2-D UV for efficient star lookup.
fn octa_map(v: vec3<f32>) -> vec2<f32> {
    let a   = abs(v);
    let inv = 1.0 / max(a.x + a.y + a.z, 1e-5);
    var p   = v.xz * inv;
    if v.y < 0.0 {
        let old = p;
        p = vec2<f32>(
            (1.0 - abs(old.y)) * select(-1.0, 1.0, old.x >= 0.0),
            (1.0 - abs(old.x)) * select(-1.0, 1.0, old.y >= 0.0),
        );
    }
    return p;
}

// Procedural star field with per-star colour temperature and twinkle.
fn star_field(dir: vec3<f32>, night: f32, cloud_cover: f32) -> vec3<f32> {
    // Stars only visible at night and above the horizon.
    let fade = night * night * smoothstep(-0.02, 0.24, dir.y)
               * (1.0 - cloud_cover * 0.96);
    if fade <= 1e-4 {
        return vec3<f32>(0.0);
    }

    // Octahedral projection: compress polar distortion for uniform star density.
    let uv   = octa_map(normalize(vec3<f32>(dir.x, dir.y * 1.10, dir.z)));
    let grid = uv * 820.0;  // slightly higher density than before
    let cell  = floor(grid);
    let local = fract(grid) - vec2<f32>(0.5);
    // Integer hash: the sin hash forms lines at these cell magnitudes, and the
    // 0.4% presence threshold turns those lines into visible star grids.
    let icell = vec2<i32>(cell);
    let seed  = hash_cell1(icell, 101);

    // Only a small fraction of cells contain a star.
    let present = smoothstep(0.9960, 1.0, seed);
    if present <= 0.0 {
        return vec3<f32>(0.0);
    }

    // Sub-cell jitter so stars don't sit on a grid.
    let jitter = hash_cell2(icell, 211) - vec2<f32>(0.5);

    let d = length(local - jitter * 0.58);

    // Pixel stars: one hard texel per star. The brightest few (seed above
    // 0.9985) grow a plus-shaped cross, as a pixel-art sprite would.
    let off = local - jitter * 0.58;
    let texel = max(abs(off.x), abs(off.y)) < 0.5 * 0.56;
    let cross = (abs(off.x) < 0.5 * 0.56 && abs(off.y) < 1.5 * 0.56)
             || (abs(off.y) < 0.5 * 0.56 && abs(off.x) < 1.5 * 0.56);
    let shape = select(0.0, 1.0, select(texel, cross, seed > 0.9985));

    // Colour temperature: mix cool blue-white (O/B type) with warm yellow-white (K/G).
    let hue  = hash_cell1(icell, 307);
    let star_color = mix(
        vec3<f32>(0.60, 0.68, 1.00),   // hot blue-white
        vec3<f32>(1.00, 0.92, 0.78),   // warm yellow-white
        smoothstep(0.20, 0.85, hue),
    );

    // Twinkling: slow atmospheric shimmer.
    let twinkle_phase = hash_cell1(icell, 401) * 6.28318;
    // Stepped twinkle: three brightness frames instead of a smooth shimmer.
    let twinkle = 0.84 + 0.16 * floor(0.5 + 0.5 * sin(light.weather_globals[0].x * 0.7 + twinkle_phase) * 2.0) / 2.0;

    return star_color * (present * shape * twinkle * fade);
}

// Sun disk with limb darkening.
// The core stays hotter than the limb. Both follow solar_disk_chroma so a
// gold or rose sky has a sun of the same family, redder than the scattered air.
fn sun_disk(dir: vec3<f32>, sun_dir: vec3<f32>,
            sun_color: vec3<f32>, sun_intensity: f32,
            cloud_cover: f32, daylight: f32) -> vec3<f32> {
    let cos_angle = dot(dir, sun_dir);
    // Angular radius of sun disk: ~0.5° → cos ≈ 1 - 3.8e-5
    // Larger apparent disk (0.9°) is more visually readable in a game.
    let disk_cos  = 0.99998;   // inner disk

    let low = 1.0 - smoothstep(0.02, 0.24, sun_dir.y);
    let chroma = solar_disk_chroma(sun_dir.y);
    // Noon stays the authored near-white source. Low sun replaces it.
    let core = mix(sun_color, chroma, smoothstep(0.08, 0.55, low));
    // Limb and glow lose green and blue first, matching a real photosphere.
    let limb = core * vec3<f32>(1.0, mix(0.94, 0.42, low), mix(0.88, 0.18, low));
    let glow = mix(core, chroma, 0.65 + 0.35 * low);

    // Limb darkening coefficient u ≈ 0.6 (solar observation)
    let mu         = saturate1((cos_angle - disk_cos) / max(1.0 - disk_cos, 1e-6));
    let limb_dark  = 1.0 - 0.6 * (1.0 - sqrt(max(mu, 0.0)));
    let disk_color = mix(limb, core, sqrt(max(mu, 0.0)));

    let disk  = smoothstep(disk_cos - 0.00002, disk_cos, cos_angle) * limb_dark;
    let halo  = pow(max(cos_angle, 0.0), mix(380.0, 220.0, low)) * mix(0.38, 0.55, low);
    let corona = pow(max(cos_angle, 0.0), mix(40.0, 16.0, low))
        * mix(0.06, 0.20, low) * (1.0 - cloud_cover * 0.80);

    let cover = 1.0 - cloud_cover * 0.72;
    return (disk_color * disk + glow * halo + glow * corona) * sun_intensity * cover;
}

// Moon disk with flat shading and subtle blue-grey colour temperature.
fn moon_disk(dir: vec3<f32>, moon_dir: vec3<f32>,
             moon_color: vec3<f32>, moon_intensity: f32,
             cloud_cover: f32, night: f32) -> vec3<f32> {
    let cos_angle = dot(dir, moon_dir);
    // Moon angular radius: ~0.5°, use 0.8° for readability.
    let disk_cos = 0.999900;

    let disk  = smoothstep(disk_cos - 0.00003, disk_cos, cos_angle);
    // Soft halo from lunar atmosphere scatter
    let halo  = pow(max(cos_angle, 0.0), 280.0) * 0.12;
    let corona = pow(max(cos_angle, 0.0), 18.0)  * 0.022 * (1.0 - cloud_cover * 0.55);

    let vis = (disk + halo + corona) * moon_intensity * night;
    // Moon is slightly cooler / bluer than pure white.
    let moon_tint = mix(moon_color, vec3<f32>(0.76, 0.82, 0.98), 0.35);
    return moon_tint * vis * (1.0 - cloud_cover * 0.58);
}

// ============================================================
// FRAGMENT SHADER
// ============================================================

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dir = sky_dir(in.ndc);
    if uw_active() {
        let jitter = fract(dot(floor(in.pos.xy), vec2<f32>(0.754877666, 0.569840296)));
        return vec4<f32>(uw_backdrop(dir, jitter), 1.0);
    }

    let daylight   = saturate1(light.sun_color_daylight.w);
    let twilight   = saturate1(light.moon_color_twilight.w);
    let night      = 1.0 - daylight;
    let exposure   = max(light.ambient_exposure.w, 0.01);
    let saturation = clamp(light.sky_top_saturation.w, 0.45, 1.05);
    let sun_dir    = normalize(light.sun_dir_intensity.xyz);
    let moon_dir   = normalize(light.moon_dir_intensity.xyz);

    let weather_cover = saturate1(light.weather_clip_summaries[0].y);
    let cloud_cover = weather_cover;
    let cloud_cover_stars = cloud_cover;

    // --- Base sky from the shared atmosphere path (analytic fallback or runtime LUT mode) ---
    let background = atmosphere_color(dir);
    var sky = background;

    // --- Sun disk (with limb darkening) ---
    sky += sun_disk(
        dir, sun_dir,
        light.sun_color_daylight.rgb, light.sun_dir_intensity.w,
        cloud_cover, daylight,
    );

    // --- Moon disk ---
    sky += moon_disk(
        dir, moon_dir,
        light.moon_color_twilight.rgb, light.moon_dir_intensity.w,
        cloud_cover, night,
    );

    // --- Star field ---
    sky += star_field(dir, night, cloud_cover_stars);

    // --- Sky events: aurora, meteor shower, rainbow ---
    let event_time = light.torch_meta.y;
    let dark_sky = 0.25 + 0.75 * smoothstep(0.35, 0.95, night);
    if light.mood.y > 0.001 {
        sky += mood_aurora(dir, event_time) * light.mood.y * dark_sky * (1.0 - cloud_cover * 0.75);
    }
    if light.mood.z > 0.001 {
        sky += mood_meteors(dir, event_time) * light.mood.z * smoothstep(0.30, 0.9, night)
            * (1.0 - cloud_cover * 0.85) * smoothstep(0.0, 0.08, dir.y);
    }
    sky += mood_ambience(cam.camera_pos.xyz, dir, 30.0, event_time, light.ambience,
        max(length(dpdx(dir)), length(dpdy(dir)))) * smoothstep(-0.1, 0.1, dir.y + 0.1);
    if light.mood.w > 0.001 {
        // A bow needs rain in front of the viewer and direct sun behind them.
        // The scheduled event alone drew one into clear or fully overcast sky.
        let rain = smoothstep(0.08, 0.35, light.cloud_authoring[1].z);
        let sunlit = 1.0 - smoothstep(0.45, 0.85, cloud_cover);
        sky += mood_rainbow(dir, sun_dir) * light.mood.w * daylight * rain * sunlit;
    }

    // Celestial disks cannot shine through the planet. The horizon dips with
    // altitude; use a smooth angular coverage to avoid a crawling hard line.
    let altitude = max(cam.camera_pos.y, 1.0);
    let radius = 6360000.0;
    let horizon_y = -sqrt(altitude * (2.0 * radius + altitude)) / (radius + altitude);
    let sky_visibility = smoothstep(horizon_y - 0.001, horizon_y + 0.001, dir.y);
    sky = mix(background, sky, sky_visibility);
    let mist = local_mist(dir, MIST_RANGE_M + MIST_NEAR_CLEARANCE_M, 1.0, background);
    sky = sky*mist.a+mist.rgb;
    if light.mood.x > 0.001 {
        sky = mix(sky, mood_cosmos_backdrop(dir, event_time), clamp(light.mood.x, 0.0, 1.0));
    }

    return vec4<f32>(max(sky, vec3<f32>(0.0)), 1.0);
}
