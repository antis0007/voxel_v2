// Continuous kilometer-scale organization shared by cloud banks and rain.
// Hash noise avoids a repeating volume-texture period across the horizon.
fn weather_bank_noise(p: vec2<f32>) -> f32 {
    let cell = vec2<i32>(floor(p));
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    return mix(mix(hash_cell1(cell, 4127), hash_cell1(cell + vec2<i32>(1, 0), 4127), u.x),
               mix(hash_cell1(cell + vec2<i32>(0, 1), 4127),
                   hash_cell1(cell + vec2<i32>(1, 1), 4127), u.x), u.y);
}

fn weather_bank(xz: vec2<f32>) -> f32 {
    let q = vec2<f32>(xz.x * 0.82 + xz.y * 0.57, -xz.x * 0.57 + xz.y * 0.82);
    return weather_bank_noise(q / vec2<f32>(2400.0, 1500.0)) * 0.72
        + weather_bank_noise(q / 670.0 + vec2<f32>(31.7, -19.3)) * 0.28;
}

// Polynomial smooth maximum for blending overlapping cloud thermals.
// This keeps the low-cost implicit field, but removes hard seams that make
// each updraft read as a stack of separate balls.
fn cloud_smooth_max(a: f32, b: f32, radius: f32) -> f32 {
    let h = max(radius - abs(a - b), 0.0) / max(radius, 1e-4);
    return max(a, b) + h * h * radius * 0.25;
}

// Cloud geometry in meters. Overlapping, tilted thermals form a connected
// three-dimensional field. A thermal has a ragged foot, offset mid-level
// shoulders and an independently lifting crown instead of a single ball.
fn cumulus_envelope(xz: vec2<f32>, h: f32, coverage: f32, bank: f32, storm: f32, size: f32) -> f32 {
    let time = light.weather_globals[0].x;
    let wspd = max(light.weather_globals[0].y, 0.0);
    // The field below is rotated into the cloud-bank frame at its call site.
    // Rotate the wind too so the cell's shear and the advected density agree.
    let wind = rotate2(safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4)), 0.43);
    // Use the existing bank field to group growth into broad weather systems.
    // This breaks the one-puff-per-cell rhythm without another texture read.
    let cluster = smoothstep(0.24, 0.78, bank);
    let cell_coverage = saturate1(coverage * mix(0.82, 1.16, cluster) + storm * 0.05);
    let cell = vec2<i32>(floor(xz / size));
    var envelope = -2.0;
    for (var z = -1; z <= 1; z += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let id = cell + vec2<i32>(x, z);
            let seed = hash_cell1(id, 137);
            let jitter = hash_cell2(id, 731);
            // Full-cell placement avoids a regularly spaced empty gutter
            // around every cell. Compact support below is exactly one cell,
            // so an omitted cell outside this 3x3 cannot contribute.
            let center = (vec2<f32>(id) + jitter) * size;
            let presence = smoothstep(seed - 0.12, seed + 0.12,
                cell_coverage * 1.12 + storm * 0.30);
            // No possible contribution: avoid axes, turrets and divisions for
            // absent cells and cells beyond their compact support.
            let local = xz - center;
            if presence <= 0.001 || dot(local, local) >= size * size { continue; }
            let radius = size * mix(0.20, 0.42, hash_cell1(id, 947))
                * mix(0.84, 1.08, cluster) * mix(0.78, 1.12, presence)
                * (1.0 + storm * 0.34);
            let random_axis = safe_normalize2(jitter - vec2<f32>(0.5), vec2<f32>(1.0, 0.0));
            // Use a wind-aligned long axis with per-cell angular variation.
            // The wider flow-aligned lobes make neighboring thermals join into
            // sculpted banks instead of a sky full of detached round puffs.
            let crosswind = vec2<f32>(-wind.y, wind.x);
            let axis = safe_normalize2(mix(random_axis, crosswind, 0.36), random_axis);
            let side = vec2<f32>(-axis.y, axis.x);
            // Tilt and twist each tower as it rises. The offset is continuous
            // in height, so the silhouette never changes with camera motion.
            let base_angle = (seed - 0.5) * 0.75;
            // Local phase is continuous across the map; advection makes each
            // cell's slow rotation follow the wind without camera-relative drift.
            let local_phase = dot(local, wind) * 0.0014;
            // Silhouettes evolve slowly while the advected medium moves with
            // the wind. Faster phase rotation looked like jittering blobs.
            let flow_phase = time * wspd * 0.0035 + seed * 6.2831853 + local_phase;
            let roll = sin(flow_phase + h * 4.7 + jitter.y * 2.7) * 0.44
                + sin(flow_phase * 0.37 - h * 7.4 + jitter.x * 5.0) * 0.19;
            let twist = base_angle + (h - 0.38) * (mix(-2.0, 2.0, jitter.x) + roll)
                + sin(h * 7.3 + seed * 19.0 + flow_phase * 0.35) * 0.48;
            let bend = (h - 0.35) * (0.20 + storm * 0.28)
                + sin(flow_phase + h * 3.6 + jitter.x * 3.0) * 0.16;
            // Vertical wind shear pushes the crown downwind, while a slower
            // cross-flow roll curls it back around the rising thermal. Keep the
            // deformation cell-local: a world-origin rotation would amplify
            // tiny time changes into large motion far from the origin.
            let updraft = smoothstep(0.08, 0.92, h) * size * (0.12 + storm * 0.13);
            let cross_roll = sin(flow_phase * 0.29 + h * 3.7 + jitter.y * 4.9) * 0.62;
            let shear = wind * updraft + side * (updraft * cross_roll + size * roll * 0.055);
            let twisted = rotate2(local - axis * (size * bend) - shear, twist);
            let delta = vec2<f32>(dot(twisted, axis), dot(twisted, side))
                / vec2<f32>(radius, radius * mix(0.60, 1.70, jitter.y));
            let growth = hash_cell1(id, 1733);
            let height = mix(0.38, 0.98, growth) + cluster * 0.08
                + storm * smoothstep(0.42, 0.90, growth) * 0.25;
            // Keep the lifting condensation level coherent across a bank; large
            // per-cell foot offsets made each thermal read as a floating ball.
            let foot = hash_cell1(id, 2309) * mix(0.035, 0.12, 1.0 - storm);
            let local_h = h - foot;
            // A narrow rising core opens into a wider shoulder, then separates
            // into a lopsided crown. The vertical slices overlap smoothly.
            // Reduce the repeated ball-on-ball rhythm while retaining layered
            // growth; cell phase and advected noise supply irregular billows.
            let waist = 1.0 + 0.21 * sin(local_h * 6.2 + seed * 21.0)
                + 0.11 * sin(local_h * 11.3 + jitter.y * 13.0);
            // Break the ellipsoid cross-section with rotating, height-correlated
            // lobes. This keeps the large cloud bank coherent while giving its
            // sides torn cauliflower folds instead of a stack of smooth balls.
            let azimuth = atan2(delta.y, delta.x);
            let radial_lobes = 1.0
                + 0.13 * sin(azimuth * 3.2 + local_h * 10.0 + flow_phase)
                + 0.07 * sin(azimuth * 5.3 - local_h * 16.0 + jitter.y * 6.2831853);
            let tower_xy = delta / (waist * radial_lobes);
            // Close each thermal below its shoulder. A core centred at zero
            // extended below the layer and was sliced into a horizontal shelf.
            let shoulder_h = (local_h - height * mix(0.44, 0.55, jitter.y))
                / (height * mix(0.28, 0.36, jitter.x));
            let tower_h = (local_h - height * 0.48) / max(height * 0.40, 0.12);
            let crown_h = (local_h - height * mix(0.58, 0.74, growth))
                / (height * mix(0.31, 0.44, jitter.y));
            let shoulder_xy = rotate2(tower_xy - vec2<f32>((jitter.x - 0.5) * 0.72,
                (seed - 0.5) * 0.48), roll * 0.30 + (jitter.y - 0.5) * 0.34)
                / vec2<f32>(1.12, 0.77);
            let crown_xy = rotate2(tower_xy + vec2<f32>((seed - 0.5) * 0.90,
                (jitter.y - 0.5) * 0.48), -roll * 0.38 + (seed - 0.5) * 0.52)
                / vec2<f32>(0.80, 0.64);
            let tower = 1.0 - dot(tower_xy * vec2<f32>(1.0, 0.78), tower_xy * vec2<f32>(1.0, 0.78))
                - tower_h * tower_h;
            let shoulder = 1.0 - dot(shoulder_xy, shoulder_xy) - shoulder_h * shoulder_h;
            let crown = 1.0 - dot(crown_xy, crown_xy) - crown_h * crown_h;
            var dome = cloud_smooth_max(tower, shoulder, 0.22);
            dome = cloud_smooth_max(dome, crown, 0.24);
            // A narrow, irregular condensation foot softens each base without
            // bridging whole cells into the repeated horizontal pancake bands.
            let base_xy = tower_xy / vec2<f32>(1.45, 1.18);
            let base_h = (local_h - height * mix(0.23, 0.33, jitter.x))
                / (height * mix(0.17, 0.22, jitter.y));
            let base_shelf = 1.0 - dot(base_xy, base_xy) - base_h * base_h;
            // Preserve negative distance outside the foot. Scaling this signed
            // field toward zero let later positive noise inflate empty space
            // into a broad pale skirt beneath every thermal.
            dome = cloud_smooth_max(dome, base_shelf - 0.35, 0.18);
            // A few cells grow oblique feeder arms. Their angular and vertical
            // offsets vary by cell, breaking the repeated cauliflower rhythm.
            let arm_h = (local_h - height * mix(0.38, 0.64, jitter.x)) / (height * 0.25);
            let arm_xy = (tower_xy - vec2<f32>(mix(0.42, 0.92, seed),
                (jitter.y - 0.5) * 0.82)) / vec2<f32>(0.92, 0.32);
            let arm = 1.0 - dot(arm_xy, arm_xy) - arm_h * arm_h;
            // A disabled signed-distance lobe must be negative, not zero:
            // multiplying by zero manufactured a flat slab outside the tower.
            dome = cloud_smooth_max(dome,
                arm - (1.0 - smoothstep(0.42, 0.78, growth)) * 4.0, 0.13);
            // Compact support keeps even noise-inflated storm lobes inside
            // the 3x3 neighborhood, so changing cells cannot drop a lobe.
            // Keep the support compact at the actual cell edge, but let the
            // broad base bridge across most of the Voronoi spacing first.
            dome -= smoothstep(0.84, 0.99, length(local) / size) * 4.0;
            envelope = max(envelope, dome - (1.0 - presence) * 3.0);
        }
    }
    return envelope;
}

fn photographic_low_density(
    p: vec3<f32>, base_y: f32, top_y: f32, footprint: f32,
    weather: CellWeather, macro_bias: f32, shadow_lod: bool,
) -> f32 {
    if p.y <= base_y || p.y >= top_y { return 0.0; }
    if weather.coverage < 0.025 { return 0.0; }
    let h = (p.y - base_y) / max(top_y - base_y, 1.0);
    let wind = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let side = vec2<f32>(-wind.y, wind.x);
    let shear = light.weather_globals[1].z;
    let xz = p.xz + layer_wind_advection_offset(LAYER_LOW)
        + side * h * h * shear * 95.0;
    let bank = weather_bank(xz);
    // Group updrafts into weather systems with clear corridors between them.
    // Preserve zero coverage and avoid a second uniform blanket in storms.
    let coverage = saturate1(weather.coverage * macro_bias
        + (bank - 0.5) * 0.72 * smoothstep(0.025, 0.20, weather.coverage));
    let size = mix(760.0, 380.0, saturate1(light.weather_globals[2].x));
    let storm = weather.storm;
    let convective = smoothstep(0.12, 0.55, weather.cloud_type);

    // Both view and shadow rays resolve the same medium-scale lobes. Smaller
    // texture periods provide actual meter-scale detail instead of magnifying
    // a blurred 64-texel volume across a kilometer.
    let shape_freq = 0.0038;
    let shape_lod = footprint_texel_lod(footprint, shape_freq, SHAPE_TEX_SIZE, 6.0);
    let time = light.weather_globals[0].x;
    let wspd = max(light.weather_globals[0].y, 0.0);
    // Advect the same 3D medium through height as wind moves it across the
    // map. Slow vertical evolution produces swelling/rising edges, not a flat
    // texture sliding under a fixed cloud silhouette.
    let evolving_y = p.y + time * wspd * (0.045 + weather.convection * 0.07);
    let shape = sample_shape_global(xz, evolving_y, shape_freq, shape_freq, shape_lod);
    let warp = (shape.gb - vec2<f32>(0.5)) * 185.0;
    // Broad continuous deformation breaks perspective rows without changing
    // cell support or choosing a camera-dependent neighbourhood.
    let organization = vec2<f32>(weather_bank_noise(xz / 1800.0 + vec2<f32>(17.3, -8.1)),
        weather_bank_noise(xz / 1800.0 + vec2<f32>(-31.8, 42.5))) - vec2<f32>(0.5);
    // The same advected 3D field lifts and sinks broad parts of each cloud,
    // while the kilometer-scale organization bends whole banks. Apply this
    // to the density masks too; warping only the interior left flat layer caps.
    let height_warp = (shape.b - 0.5) * 0.38 + organization.y * 0.20;
    let shaped_h = clamp(h + height_warp * convective, 0.0, 1.0);
    var envelope = 0.0;
    if convective > 0.001 {
        envelope = cumulus_envelope(rotate2(xz, 0.43) + organization * size * 1.35 + warp,
            shaped_h, coverage, bank, storm, size);
    }
    // Preserve rounded macro volumes at distance: excessive noise inflation
    // pushed the whole body back through the layer's flat altitude bounds.
    let lobes = (shape.r - 0.5) * 1.18 + (shape.g - 0.5) * 0.48;
    var field = envelope + lobes;

    // Broad, scalloped decks preserve weather variety without forcing every
    // cumulus through the same flat top and bottom height mask.
    // A sheet still needs varying water content and thickness: the old field
    // saturated everywhere in overcast weather, erasing its lighting structure.
    let sheet_field = (coverage - 0.50) * 1.8 + (shape.r - 0.5) * 1.2
        + (bank - 0.5) * 0.8 - smoothstep(0.18, 0.95, h) * 1.1;
    let deck = (coverage - 0.32) * 2.0 + (shape.r - 0.5) * 0.65
        - smoothstep(0.35, 0.95, h) * 1.2;
    field = mix(deck, field, convective);
    // Fair-weather cumulus must not acquire a second blanket underneath it.
    if storm > 0.001 {
        let anvil_bank = weather_bank(xz + wind * h * h * shear * 600.0);
        let anvil = smoothstep(0.56, 0.78, shaped_h) * (1.0 - smoothstep(0.86, 1.0, shaped_h))
            * storm * authored_anvil_spread() * smoothstep(0.24, 0.68, anvil_bank) * 1.8;
        field = max(field, deck * 0.65 + anvil - 0.60 - (1.0 - storm) * 2.0);
    }
    if field < -0.45 { return 0.0; }

    let detail_freq = 0.018;
    let detail_lod = footprint_texel_lod(footprint, detail_freq, DETAIL_TEX_SIZE, 5.0);
    // Keep one filtered erosion sample into the middle distance: the old
    // 18 m cutoff left kilometer-scale billows as smooth, identical puffs.
    let detail_keep = 1.0 - smoothstep(8.0, 42.0, footprint);
    var detail = vec4<f32>(0.5);
    if detail_keep > 0.01 && !shadow_lod {
        detail = sample_detail_global(xz + warp, evolving_y, detail_freq, detail_freq, detail_lod);
        let erosion = (1.0 - dot(detail.rgb, vec3<f32>(0.55, 0.30, 0.15)))
            * light.weather_globals[1].w * light.weather_globals[2].y * 0.65;
        field -= erosion * detail_keep;
    }
    let softness = 0.09 + smoothstep(8.0, 90.0, footprint) * 0.20;
    let body = smoothstep(-softness, softness + 0.14, field);
    // Condensation-floor relief lives at a broader scale than edge turbulence.
    // Driving it from the fine Worley channel flattened the whole underside
    // as its mip level increased, leaving a visible plane across distant decks.
    let base_freq = 0.0006;
    let base_lod = footprint_texel_lod(footprint, base_freq, SHAPE_TEX_SIZE, 6.0);
    // Deform the floor with the existing nonperiodic organization field so
    // its single volume lookup does not stamp a grid onto the underside.
    let base_shape = textureSampleLevel(shape_noise_tex, cloud_field_sampler,
        stable_noise_uvw(rotate2(xz, 0.91) + organization * 600.0,
            evolving_y, base_freq, base_freq), base_lod).r;
    let base_height = mix(0.015, 0.13, 1.0 - base_shape) + (1.0 - shape.b) * 0.012
        + storm * (1.0 - bank) * 0.055;
    let density_h = mix(h, shaped_h, convective);
    let deck_base = smoothstep(base_height, base_height + 0.075, density_h);
    let base = mix(deck_base, smoothstep(0.0, 0.045, shaped_h), convective);
    let cap = 1.0 - smoothstep(mix(0.70, 0.94, shape.g), 1.0, density_h);
    let folds = mix(0.68, 1.08, shape.g) * mix(0.87, 1.0, detail.r);
    let sheet_water = mix(0.28, 1.08, smoothstep(-0.08, 0.85, sheet_field));
    let water_content = mix(sheet_water, 1.0, convective);
    return body * base * cap * folds * water_content * mix(0.45, 1.60, authored_cloud_density())
        * smoothstep(0.01, 0.12, weather.low_amount);
}
