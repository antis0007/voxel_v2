// Distant precipitation is a participating medium, not a screen overlay.
// The source coordinates follow the same advected banks as the low clouds;
// fall distance bends the curtains with the wind. Terrain clips the ray.
fn march_rain_volume(dir: vec3<f32>, max_distance: f32, cone: f32, pixel: vec2<f32>) -> CloudSample {
    var result = CloudSample(vec3<f32>(0.0), 1.0, vec4<f32>(0.0), 0.0);
    let storm_control = clamp(light.cloud_authoring[1].z, 0.0, 1.0);
    if storm_control <= 0.08 { return result; }

    let bounds = cloud_layer_bounds();
    let ceiling = bounds.low_base + 220.0;
    let camera = cam.camera_pos.xyz;
    var entry = 0.0;
    var exit_distance = min(max_distance, 12000.0);
    if abs(dir.y) > 0.00001 {
        let a = (-150.0 - camera.y) / dir.y;
        let b = (ceiling - camera.y) / dir.y;
        entry = max(min(a, b), 0.0);
        exit_distance = min(max(a, b), exit_distance);
    } else if camera.y > ceiling || camera.y < -150.0 {
        return result;
    }
    if exit_distance <= entry { return result; }

    let wind = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let wind_speed = max(light.weather_globals[0].y, 0.0);
    let advection = layer_wind_advection_offset(LAYER_LOW);
    let sun_dir = normalize(light.sun_dir_intensity.xyz);
    let daylight = clamp(light.sun_color_daylight.w, 0.0, 1.0);
    let sky = atmosphere_color(vec3<f32>(0.0, 1.0, 0.0));
    let sky_side = atmosphere_color(normalize(vec3<f32>(-sun_dir.x, 0.5, -sun_dir.z)));
    let ambient = mix(sky_side, sky, 0.65) * 0.30;
    let cos_sun = dot(dir, sun_dir);
    // Broad droplet forward scattering; the narrow sun disk stays in the sky.
    let g = 0.65;
    let phase = (1.0 - g * g) / (12.56637 * pow(max(1.0 + g * g - 2.0 * g * cos_sun, 0.01), 1.5));
    let sun = light.sun_color_daylight.rgb * light.sun_dir_intensity.w * daylight
        * photographic_cloud_sun(sun_dir.y, max(camera.y, 0.0));
    var distance_moment = 0.0;

    let noise_size=vec2<i32>(textureDimensions(blue_noise_tex));
    let rain_phase=textureLoad(blue_noise_tex,vec2<i32>(pixel)%noise_size,0).r;
    // Quadratic spacing resolves nearby curtains without a distance cutoff.
    // Frame-stable stratification avoids coherent horizontal rain slices.
    for (var i = 0; i < 40; i += 1) {
        let u0 = f32(i) / 40.0;
        let u1 = f32(i + 1) / 40.0;
        let t0 = mix(entry, exit_distance, u0 * u0);
        let t1 = mix(entry, exit_distance, u1 * u1);
        let t = mix(t0,t1,fract(rain_phase+f32(i)*0.618033989));
        let p = camera + dir * t;
        let fall = max(bounds.low_base - p.y, 0.0);
        let source_xz = p.xz + advection - wind * fall * wind_speed * 0.22;
        let weather = weather_at_world(source_xz);
        let storm = smoothstep(0.10, 0.65, weather.storm) * weather.low_amount;
        if storm <= 0.001 { continue; }
        let aabb = cell_low_aabb(weather);
        let bank = weather_bank(source_xz);
        let source = smoothstep(0.32, 0.62, bank)
            * smoothstep(0.48, 0.83, weather.coverage);
        // Fade into the condensation floor; do not draw through cloud crowns.
        let top = 1.0 - smoothstep(aabb.y_min - 35.0, aabb.y_min + 100.0, p.y);
        // Long vertical filaments widen into haze as droplets fall and mix.
        let freq = 0.0014;
        let footprint = max(t * cone, (t1 - t0) * 0.20);
        let lod = footprint_texel_lod(footprint, freq, SHAPE_TEX_SIZE, 6.0);
        let filaments = textureSampleLevel(shape_noise_tex, cloud_field_sampler,
            stable_noise_uvw(source_xz, p.y + light.weather_globals[0].x * 45.0, freq, 0.00012), lod).r;
        let streak = mix(smoothstep(0.25, 0.72, filaments), 0.48,
            smoothstep(250.0, 1000.0, fall) * 0.45);
        let evaporation = mix(0.35, 1.0, smoothstep(0.35, 0.80, weather.humidity));
        let sigma = storm * source * top * streak * evaporation * 0.0018;
        let alpha = 1.0 - exp(-sigma * (t1 - t0));
        if alpha > 0.00001 {
            // Rain below the bank receives subdued skylight, while slanted
            // sunlight can illuminate its exposed edge at dawn and sunset.
            let sun_visibility = mix(0.28, 0.05, source * storm);
            let lighting = ambient + sun * phase * sun_visibility;
            let weight = result.transmittance * alpha;
            result.radiance += lighting * weight;
            distance_moment += t * weight;
            result.transmittance *= 1.0 - alpha;
        }
        if result.transmittance < 0.01 { break; }
    }
    result.mean_distance = distance_moment / max(1.0 - result.transmittance, 0.00001);
    return result;
}
