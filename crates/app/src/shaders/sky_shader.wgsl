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

@group(2) @binding(7) var planet_depth_texture: texture_2d<f32>;

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

// `RenderDebugView::SkyGalaxy` / `SkyStars` in `light.torch_meta.w`.
const SKY_DEBUG_GALAXY: i32 = 34;
const SKY_DEBUG_STARS: i32 = 35;

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

// Box-filter a star sprite in grid coordinates, keeping square pixel edges.
fn star_rect_coverage(offset: vec2<f32>, half_size: vec2<f32>, footprint: vec2<f32>) -> f32 {
    let half_pixel = footprint * 0.5;
    let lo = max(offset - half_pixel, -half_size);
    let hi = min(offset + half_pixel, half_size);
    let overlap = max(hi - lo, vec2<f32>(0.0)) / footprint;
    return overlap.x * overlap.y;
}

// Procedural star field with per-star colour temperature and twinkle.
// `horizon` is 1 where a ground horizon hides the lower sky, 0 in orbit where
// the planet body itself occludes whatever lies below.
fn star_field(view: vec3<f32>, night: f32, cloud_cover: f32, horizon: f32, vacuum: f32) -> vec3<f32> {
    // Stars only visible at night and, near the ground, above the horizon.
    let above = mix(1.0, smoothstep(-0.02, 0.24, view.y), horizon);
    let fade = night * night * above
               * (1.0 - cloud_cover * 0.96);
    if fade <= 1e-4 {
        return vec3<f32>(0.0);
    }

    let dir = sky_fixed(view);
    // Octahedral projection: compress polar distortion for uniform star density.
    let uv   = octa_map(normalize(vec3<f32>(dir.x, dir.y * 1.10, dir.z)));
    let grid = uv * 820.0;  // slightly higher density than before
    let footprint = max(fwidth(grid), vec2<f32>(0.001));
    let cell  = floor(grid);
    let local = fract(grid) - vec2<f32>(0.5);
    // Integer hash: the sin hash forms lines at these cell magnitudes, and the
    // 0.4% presence threshold turns those lines into visible star grids.
    let icell = vec2<i32>(cell);
    let seed  = hash_cell1(icell, 101);

    // Only a small fraction of cells contain a star; more along the galaxy
    // band, and more again above the air.
    let threshold = 0.9972 - 0.0075 * cel_galaxy_band(dir) - 0.0025 * vacuum;
    let present = smoothstep(threshold, 1.0, seed);
    if present <= 0.0 {
        return vec3<f32>(0.0);
    }

    // Sub-cell jitter so stars don't sit on a grid.
    let jitter = hash_cell2(icell, 211) - vec2<f32>(0.5);

    let bright = seed > 0.9985;
    let off = local - jitter * select(0.44, 0.28, bright);
    var shape = star_rect_coverage(off, vec2<f32>(0.28), footprint);
    if bright {
        // Keep the cross inside its cell. Subtract the shared center once.
        shape = star_rect_coverage(off, vec2<f32>(0.34, 0.21), footprint)
            + star_rect_coverage(off, vec2<f32>(0.21, 0.34), footprint)
            - star_rect_coverage(off, vec2<f32>(0.21), footprint);
    }
    if shape <= 0.0 { return vec3<f32>(0.0); }

    // Populations: 60% neutral, 25% blue-white, 12% ivory, 3% orange.
    let hue = hash_cell1(icell, 307);
    var star_color = vec3<f32>(0.92, 0.95, 1.00);
    if hue > 0.97 {
        star_color = vec3<f32>(1.00, 0.68, 0.42);
    } else if hue > 0.85 {
        star_color = vec3<f32>(1.00, 0.91, 0.78);
    } else if hue > 0.60 {
        star_color = vec3<f32>(0.68, 0.79, 1.00);
    }

    // Twinkling: slow atmospheric shimmer.
    var twinkle = 0.92;
    if vacuum < 0.999 {
        let twinkle_phase = hash_cell1(icell, 401) * 6.28318;
        twinkle += 0.08 * (1.0 - vacuum) * sin(light.weather_globals[0].x * 0.7 + twinkle_phase);
    }

    // Long-tailed magnitudes: most stars sit at the edge of visibility, a few
    // hundredths are crisp, and only the crossed ones are bright.
    var magnitude = 0.10 + 1.6 * pow(hash_cell1(icell, 503), 9.0);
    if bright { magnitude = max(magnitude, 1.4); }
    // Dust lanes hide the stars behind them as well as the diffuse band.
    let lat = dot(dir, CEL_GALAXY_POLE);
    var dust = 1.0;
    if abs(lat) < 0.25 { dust = cel_galaxy_dust(dir, lat, cel_galaxy_warp(dir)); }
    return star_color * (present * shape * twinkle * fade * magnitude * dust);
}

// Disc radiance relative to the terrain's direct-sun irradiance scale. The
// disc covers ~1e-4 sr, so it must be far brighter than the sky to read as the
// source; tone mapping then holds a pale-gold core with a coloured limb.
const SUN_DISC_GAIN: f32 = 24.0;

// The visible sun: one disc and one tight glare skirt. Its colour and
// brightness are the source times the same transmittance that lights the
// terrain, so disc, ground and clouds agree. The broad aureole is the sky
// LUT's aerosol scattering; clouds occlude this per pixel in the composite,
// so nothing here is dimmed by a global cloud-cover number.
fn sun_disk(dir: vec3<f32>, sun_dir: vec3<f32>,
            sun_color: vec3<f32>, sun_intensity: f32) -> vec3<f32> {
    let cos_angle = clamp(dot(dir, sun_dir), -1.0, 1.0);
    // Small-angle separation in radians; exact enough below a few degrees.
    let theta = sqrt(max(2.0 * (1.0 - cos_angle), 0.0));
    let radius = sqrt(max(2.0 * (1.0 - light.sky_art[0].x), 1e-8));
    // One-pixel antialiased limb at any resolution or field of view.
    let aa = max(fwidth(theta), radius * 0.03);
    let disk = 1.0 - smoothstep(radius - aa, radius + aa, theta);
    let x = saturate1(theta / radius);
    let mu = sqrt(max(1.0 - x * x, 0.0));
    // Limb darkening u ~ 0.6; the limb also loses blue faster than the core.
    let limb_dark = 1.0 - 0.6 * (1.0 - mu);

    let trans = sky_transmittance_to_sun_at_altitude(sun_dir.y, max(cam.camera_pos.y, 0.0));
    let source = max(sun_color, vec3<f32>(0.0)) * trans * sun_intensity;
    // Photographic core: the centre saturates toward pale gold before the
    // limb does, so a low sun reads as a luminous gold point in amber air.
    let hue = radiance_chroma(max(source, vec3<f32>(1e-6)));
    let core_hue = mix(hue, vec3<f32>(1.0, 0.93, 0.78) * 1.06, 0.55 * mu);
    let disc = core_hue * sky_luma(source) * limb_dark * SUN_DISC_GAIN;
    // Forward-scatter glare: falls by e every 1.3 radii outside the limb.
    let skirt = exp(-max(theta - radius, 0.0) / (radius * 1.3)) * (1.0 - disk);
    let glare = source * light.sky_art[0].y * SUN_DISC_GAIN * 0.25 * skirt;
    return disc * disk + glare;
}

// Stars appear only where the sky behind them is dark at this exposure, so a
// twilight dome hides them and they come through as blue hour deepens.
fn star_visibility(background: vec3<f32>) -> f32 {
    let exposure = max(light.ambient_exposure.w, 0.01);
    return 1.0 - smoothstep(0.02, 0.07, sky_luma(background) * exposure);
}

// Moon angular radius: 2.2 degrees, planet.rs MIN_MOON_RADIUS.
const MOON_COS_RADIUS: f32 = 0.999263;

// The moon as a lit sphere plus its aerosol halo; a is the sky it hides.
fn moon_disk(dir: vec3<f32>, moon_dir: vec3<f32>, sun_dir: vec3<f32>,
             moon_color: vec3<f32>, moon_intensity: f32,
             cloud_cover: f32, night: f32) -> vec4<f32> {
    let cos_angle = dot(dir, moon_dir);
    // Moon is slightly cooler / bluer than pure white.
    let moon_tint = mix(moon_color, vec3<f32>(0.76, 0.82, 0.98), 0.35);
    let body = cel_moon(dir, moon_dir, MOON_COS_RADIUS, sun_dir, moon_tint, 0.0);
    // Halo scales with the lit fraction, so a crescent glows less than a full moon.
    let lit_fraction = 0.5 + 0.5 * dot(sun_dir, -moon_dir);
    let halo  = pow(max(cos_angle, 0.0), 280.0) * 0.12 * (1.0 - body.a);
    let corona = pow(max(cos_angle, 0.0), 18.0)  * 0.022 * (1.0 - cloud_cover * 0.55);
    let glow = moon_tint * (halo + corona) * lit_fraction;
    let scale = moon_intensity * night * (1.0 - cloud_cover * 0.58);
    return vec4<f32>((body.rgb + glow) * scale, body.a);
}

// ============================================================
// FRAGMENT SHADER
// ============================================================

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let cave = clamp(light.sky_art[1].w, 0.0, 1.0);
    if cave >= 0.999 { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
    let dir = camera_ray(in.ndc);
    if uw_active() {
        let jitter = fract(dot(floor(in.pos.xy), vec2<f32>(0.754877666, 0.569840296)));
        return vec4<f32>(uw_backdrop(dir, jitter), 1.0);
    }

    if lava_eye_active() { return vec4<f32>(lava_backdrop(dir), 1.0); }

    if cam.planet[0].w < -0.5 || cam.planet[0].w > 1.5 {
        // The Nether: its own dome, frost-coloured sparks and the living-air motes.
        let t = light.torch_meta.y;
        var realm: vec3<f32>;
        if cam.planet[0].w > 1.5 {
            realm = cel_hell_sky(dir, t);
        } else {
            realm = cel_realm_sky(dir, t);
            realm += star_field(dir, 1.0, 0.0, 0.0, 0.0) * vec3<f32>(0.55, 0.85, 1.0) * 0.45;
        }
        realm += mood_ambience(cam.camera_pos.xyz, dir, 30.0, t, light.ambience,
            max(length(dpdx(dir)), length(dpdy(dir))));
        return vec4<f32>(realm * (1.0 - cave), 1.0);
    }
    if cam.planet[0].x > 0.0 {
        let altitude = max(cam.planet[1].w, 0.0);
        let air = exp(-max(altitude - 150.0, 0.0) / 1400.0)
            * (1.0 - smoothstep(CEL_AIR_TOP_M * 0.6, CEL_AIR_TOP_M, altitude));
        // Flat-chart worlds use the globe with the local day/night lighting.
        // Its body frame is valid even while the legacy mesh flag stays zero.
        let mesh_planet = cam.planet[0].w > 0.5;
        let sun = normalize(select(light.sun_dir_intensity.xyz, cam.planet[5].xyz, mesh_planet));
        let moon = normalize(select(light.moon_dir_intensity.xyz, cam.planet[6].xyz, mesh_planet));
        let giant = normalize(cam.planet[7].xyz);
        let day = smoothstep(-0.12, 0.15, sun.y);
        let vacuum = 1.0 - air;
        var color = atmosphere_color(dir) * air;
        let t = light.torch_meta.y;
        let dark = max(vacuum, 1.0 - day);
        let cosmos_view = i32(light.torch_meta.w);
        var back = vec3<f32>(0.0);
        if cosmos_view != SKY_DEBUG_GALAXY { back = star_field(dir, dark, 0.0, air, vacuum) * (1.0 + vacuum * 0.75); }
        if cosmos_view != SKY_DEBUG_STARS && dark * (1.0 - day * air) > 1e-3 { back += cel_galaxy(sky_fixed(dir)) * dark * (1.0 + vacuum); }
        back *= 1.0 - day * air;
        // Review views: one cosmos layer alone on black, no air, bodies or clouds.
        if cosmos_view == SKY_DEBUG_GALAXY || cosmos_view == SKY_DEBUG_STARS { return vec4<f32>(back, 1.0); }
        // The giant's spin axis in the eye frame (w of the frame rows, planet.rs).
        let ecliptic = vec3<f32>(cam.planet[2].w, cam.planet[3].w, cam.planet[4].w);
        let pole = select(vec3<f32>(0.0, 1.0, 0.0), normalize(ecliptic), dot(ecliptic, ecliptic) > 0.25);
        // Extinction through the air column: bodies redden and dim toward the horizon.
        let extinction = mix(vec3<f32>(1.0), exp(-vec3<f32>(0.02, 0.05, 0.11) / max(dir.y + 0.06, 0.03)), air);
        let giant_px = cel_giant(dir, giant, cam.planet[7].w, normalize(cam.planet[5].xyz), pole, t);
        back = back * (1.0 - giant_px.a) + giant_px.rgb * extinction * (1.0 - day * air * 0.8);
        // Earthshine: the planet's lit face, as seen from the moon, fills its night side.
        let earthshine = 0.10 * (0.5 - 0.5 * dot(sun, moon));
        let moon_px = cel_moon(dir, moon, cam.planet[6].w, sun, vec3<f32>(0.42, 0.44, 0.47), earthshine);
        // Aerosol glow around the moon, scaled by its lit fraction; gone above the air.
        let moon_cos = max(dot(dir, moon), 0.0);
        let moon_glow = (pow(moon_cos, 900.0) * 0.05 + pow(moon_cos, 60.0) * 0.012)
            * (0.5 - 0.5 * dot(sun, moon)) * (1.0 - moon_px.a) * air * (1.0 - day);
        color += back * (1.0 - moon_px.a) + (moon_px.rgb + vec3<f32>(0.62, 0.70, 0.86) * moon_glow) * extinction;
        color += cel_sun_vacuum(dir, sun, cam.planet[5].w, vacuum);
        let radius = cam.planet[0].x;
        // Limb shell: height of the ray's closest approach sets the glow, the
        // sun at that point sets day, dusk or night. Rays into the disc are
        // behind the cover mesh, so only h >= 0 matters.
        let dir_b = planet_wx_body(dir);
        let sun_b = planet_wx_body(sun);
        let eye_b = normalize(cam.planet[1].xyz) * (radius + altitude);
        let tca = -dot(eye_b, dir_b);
        // Single-scattered air along the ray: blue limb, red terminator and the
        // night airglow all come out of one integral. Rays deep inside the disc
        // are behind the cover mesh and skip it.
        let closest = eye_b + dir_b * tca;
        let miss2 = dot(closest, closest);
        let top = radius + CEL_AIR_TOP_M;
        let inner = max(radius - 400.0, 0.0);
        let hits_air = altitude < CEL_AIR_TOP_M || (tca > 0.0 && miss2 < top * top);
        if vacuum > 0.01 && hits_air && (tca <= 0.0 || miss2 > inner * inner) {
            let jitter = fract(dot(floor(in.pos.xy), vec2<f32>(0.754877666, 0.569840296)));
            let limb = cel_limb(eye_b, dir_b, sun_b, radius, jitter);
            // Compose L + T * background; adding only L let stars shine
            // through the dense limb. Reuse the march's existing extinction.
            // Gently soften only the brightest orbital limb radiance.
            let peak = max(limb.inscatter.r, max(limb.inscatter.g, limb.inscatter.b));
            let soften = 0.12 * smoothstep(CEL_AIR_TOP_M, 3.0 * CEL_AIR_TOP_M, altitude)
                * smoothstep(0.12, 0.65, peak);
            color = color * mix(vec3<f32>(1.0), limb.transmittance, vacuum)
                + (limb.inscatter * (1.0 - soften) + limb.airglow) * vacuum;
        }
        // The same raised curtains cross the disc and limb, in the same body
        // frame. Clip against the planet when viewing the sky from its surface.
        var aurora_distance = radius * 4.0;
        if tca > 0.0 && miss2 < radius * radius {
            aurora_distance = max(tca - sqrt(max(radius * radius - miss2, 0.0)), 0.0);
        }
        color += planet_aurora_ray(eye_b, dir_b, aurora_distance, sun_b, radius, t % 3600.0);
        return vec4<f32>(max(color,vec3<f32>(0.0)),1.0);
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
    sky += sun_disk(dir, sun_dir, light.sun_color_daylight.rgb, light.sun_dir_intensity.w);

    // --- Moon disk, in front of the stars and the galaxy band ---
    let moon = moon_disk(
        dir, moon_dir, sun_dir,
        light.moon_color_twilight.rgb, light.moon_dir_intensity.w,
        cloud_cover, night,
    );

    // --- Star field and galaxy band ---
    let star_fade = night * night * smoothstep(-0.02, 0.24, dir.y) * (1.0 - cloud_cover_stars * 0.96);
    let cosmos_view = i32(light.torch_meta.w);
    var stars = vec3<f32>(0.0);
    if cosmos_view != SKY_DEBUG_GALAXY { stars = star_field(dir, night, cloud_cover_stars, 1.0, 0.0); }
    if cosmos_view != SKY_DEBUG_STARS && star_fade > 1e-4 { stars += cel_galaxy(sky_fixed(dir)) * star_fade; }
    if cosmos_view == SKY_DEBUG_GALAXY || cosmos_view == SKY_DEBUG_STARS { return vec4<f32>(stars, 1.0); }
    sky += stars * star_visibility(background) * (1.0 - moon.a) + moon.rgb;

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

    return vec4<f32>(max(sky, vec3<f32>(0.0)) * (1.0-cave), 1.0);
}
