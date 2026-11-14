fn weather_bank(xz: vec2<f32>) -> f32 {
    let q = vec2<f32>(xz.x * 0.82 + xz.y * 0.57, -xz.x * 0.57 + xz.y * 0.82);
    return weather_bank_noise(q / vec2<f32>(2400.0, 1500.0)) * 0.72
        + weather_bank_noise(q / 670.0 + vec2<f32>(31.7, -19.3)) * 0.28;
}

fn photographic_low_density(
    p: vec3<f32>, base_y: f32, top_y: f32, footprint: f32,
    weather: CellWeather, shadow_lod: bool,
) -> f32 {
    if p.y <= base_y || p.y >= top_y { return 0.0; }
    if weather.coverage < 0.025 { return 0.0; }
    let h = (p.y - base_y) / max(top_y - base_y, 1.0);
    let wind = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let side = vec2<f32>(-wind.y, wind.x);
    let shear = light.weather_globals[1].z;
    let flow = cloud_flow_warp(p.xz, h, LAYER_LOW);
    let xz = p.xz + layer_wind_advection_offset(LAYER_LOW)
        + side * h * h * shear * 95.0 + flow.xy;
    let bank = weather_bank(xz);
    // The shared field places weather systems; volume noise only shapes their clouds.
    let coverage = weather.coverage;
    let world_scale = cloud_horizontal_scale();
    let size = mix(1120.0, 640.0, saturate1(light.weather_globals[2].x)) * world_scale;
    let storm = weather.storm;
    let convective = smoothstep(0.12, 0.55, weather.cloud_type);

    // Both view and shadow rays resolve the same medium-scale lobes. Smaller
    // texture periods provide actual meter-scale detail instead of magnifying
    // a blurred 64-texel volume across a kilometer.
    let shape_freq = 0.0018 / world_scale;
    let shape_lod = footprint_texel_lod(footprint, shape_freq, SHAPE_TEX_SIZE, 6.0);
    let time = light.weather_globals[0].x;
    let wspd = max(light.weather_globals[0].y, 0.0);
    // Advect the same 3D medium through height as wind moves it across the
    // map. Slow vertical evolution produces swelling/rising edges, not a flat
    // texture sliding under a fixed cloud silhouette.
    let evolving_y = p.y + flow.z + time * wspd * (0.045 + weather.convection * 0.07);
    let shape = sample_shape_global(xz, evolving_y, shape_freq, shape_freq, shape_lod);
    let warp = (shape.gb - vec2<f32>(0.5)) * 185.0 * world_scale;
    // Broad continuous deformation breaks repeated texture alignment without
    // a cell lattice or a camera-dependent neighbourhood.
    let organization = vec2<f32>(weather_bank_noise(xz / (1800.0 * world_scale) + vec2<f32>(17.3, -8.1)),
        weather_bank_noise(xz / (1800.0 * world_scale) + vec2<f32>(-31.8, 42.5))) - vec2<f32>(0.5);
    // The same advected 3D field lifts and sinks broad parts of each cloud,
    // while the kilometer-scale organization bends whole banks. Apply this
    // to the density masks too; warping only the interior left flat layer caps.
    let height_warp = (shape.b - 0.5) * 0.38 + organization.y * 0.20;
    // Cumulus condense at a shared lifting level, so a field has one flat
    // underside and differs only in how far each tower climbs. Warping the
    // whole profile floated each cloud at its own height (±30% of the slab).
    // The warp fades in above the base and keeps full strength in the tops.
    let warp_weight = smoothstep(0.02, 0.40, h);
    let shaped_h = clamp(h + flow.z / max(top_y - base_y, 1.0)
        + height_warp * convective * warp_weight - mammatus_carve(xz, h, storm), 0.0, 1.0);
    // Pure cumulus has no density below its condensation floor or above its
    // cap. Resolve that mask before the broad condensate lookup.
    let floor_noise = weather_bank_noise(xz / 150.0 + vec2<f32>(3.3, 8.1));
    let floor_lift = (floor_noise * 0.7 + shape.r * 0.3) * 0.12 * convective;
    if convective == 1.0 && (shaped_h <= floor_lift || shaped_h >= 1.0) { return 0.0; }
    var field = 0.0;
    if convective > 0.0 {
        // Density is a continuous, advected medium, not a smooth union of
        // ellipsoids. The old broad mass overrode gaps between every smaller
        // lobe, so adding detail could never break its inflated outline.
        // Low-frequency 3D condensate supplies connected, asymmetric bodies;
        // the existing shape texture cuts shoulders and feeder gaps into them.
        let macro_freq = 0.25 / size;
        let macro_y_freq = 0.28 / max(top_y - base_y, 160.0);
        let macro_lod = footprint_texel_lod(footprint,
            max(macro_freq, macro_y_freq), SHAPE_TEX_SIZE, 6.0);
        // The nonperiodic bank warp already breaks tiling at this scale.
        // One sample retains its broad contrast; averaging two unrelated
        // volumes flattened it and doubled this new lookup's bandwidth.
        let condensate = textureSampleLevel(shape_noise_tex, cloud_field_sampler,
            stable_noise_uvw(xz + organization * size * 0.70,
                evolving_y, macro_freq, macro_y_freq), macro_lod);
        let moisture = (condensate.r - 0.48) * 2.20 + (shape.r - 0.48) * 0.95
            + (coverage - 0.67) * 0.90 + (bank - 0.5) * 0.65;
        let entrainment = (1.0 - smoothstep(0.27, 0.62, condensate.g)) * 0.18
            + (1.0 - smoothstep(0.30, 0.64, shape.g)) * 0.10;
        // Convection permits deeper growth; the actual top is still decided
        // by condensate, so it does not repeat a cap in every lattice cell.
        let dilution = smoothstep(0.22, 1.0, shaped_h) * mix(0.50, 0.30, storm);
        field = moisture - entrainment - dilution;
    }

    // Broad, scalloped decks preserve weather variety without forcing every
    // cumulus through the same flat top and bottom height mask.
    // A sheet still needs varying water content and thickness: the old field
    // saturated everywhere in overcast weather, erasing its lighting structure.
    // Rolls and holes at a scale between the banks and the puff grid, so an
    // overcast deck is a field of uneven lumps rather than one ceiling.
    var rolls = 0.0;
    var deck = 0.0;
    // mix() evaluates both operands. Branch at its exact endpoints so fair
    // weather cumulus does not evaluate discarded sheet noise, and vice versa.
    if convective < 1.0 || storm > 0.001 {
        rolls = weather_bank_noise(xz / vec2<f32>(520.0, 240.0) + vec2<f32>(11.0, -6.0));
        deck = (coverage - 0.38) * 2.0 + (shape.r - 0.5) * 0.85
            + (rolls - 0.55) * 0.9
            - smoothstep(0.35, 0.95, h) * 1.2;
        field = mix(deck, field, convective);
    }
    // Fair-weather cumulus must not acquire a second blanket underneath it.
    if storm > 0.001 {
        let anvil_bank = weather_bank(xz + wind * h * h * shear * 600.0);
        let anvil = smoothstep(0.56, 0.78, shaped_h) * (1.0 - smoothstep(0.86, 1.0, shaped_h))
            * storm * authored_anvil_spread() * smoothstep(0.24, 0.68, anvil_bank) * 1.8;
        field = max(field, deck * 0.65 + anvil - 0.60 - (1.0 - storm) * 2.0);
    }
    let softness = 0.08 + min(footprint / max(size, 1.0), 0.18);
    let sheet_ramp = (1.0 - convective) * 0.55;
    // Everything below only removes density. Reject at the actual filtered
    // threshold, so broad sheet edges are not clipped by a fixed cutoff.
    if field <= -softness - sheet_ramp * 0.4 { return 0.0; }

    // Fix the physical scale. Only its mip changes with distance; changing
    // frequency with the camera made cloud edges swell into different shapes.
    let detail_freq = 0.012 / world_scale;
    let detail_lod = footprint_texel_lod(footprint, detail_freq, DETAIL_TEX_SIZE, 5.0);
    let detail_keep = 1.0 - smoothstep(32.0, 140.0, footprint);
    var detail = vec4<f32>(0.5);
    if detail_keep > 0.01 && !shadow_lod {
        detail = sample_detail_global(xz + warp, evolving_y, detail_freq, detail_freq, detail_lod);
    }
    // Preserve the mean erosion when its fine variation becomes unresolved.
    // Fading the whole subtraction to zero reinflated clouds at a distance.
    let erosion_noise = mix(0.5, dot(detail.rgb, vec3<f32>(0.62, 0.30, 0.08)), detail_keep);
    field -= (1.0 - erosion_noise) * light.weather_globals[1].w * light.weather_globals[2].y * 0.34;
    // The coarse condensate stays resolved after fine erosion filters away.
    // No camera-dependent 2D noise is needed to disguise an ellipsoid beneath.
    // Sheets thin out over hundreds of metres; a narrow ramp cut stratus into
    // flat, hard-edged cut-outs that read as a map pasted on the sky.
    let body = smoothstep(-softness - sheet_ramp * 0.4, softness + 0.14 + sheet_ramp, field);
    // Condensation-floor relief lives at a broader scale than edge turbulence.
    // Driving it from the fine Worley channel flattened the whole underside
    // as its mip level increased, leaving a visible plane across distant decks.
    let density_h = mix(h, shaped_h, convective);
    // A ragged condensation floor: the lifting level wanders by a few percent
    // of the slab, so undersides seen edge-on are not one razor-flat plane.
    // Hashed, not texture-driven: the shape sample is mip-blurred to a
    // constant at range, which is exactly where flat bottoms show.
    var base = smoothstep(floor_lift, floor_lift + 0.07, shaped_h);
    var water_content = 1.0;
    if convective < 1.0 {
        let base_freq = 0.0006;
        let base_lod = footprint_texel_lod(footprint, base_freq, SHAPE_TEX_SIZE, 6.0);
        // This floor lookup contributes only to the sheet component.
        let base_shape = textureSampleLevel(shape_noise_tex, cloud_field_sampler,
            stable_noise_uvw(rotate2(xz, 0.91) + organization * 600.0,
                evolving_y, base_freq, base_freq), base_lod).r;
        let base_height = mix(0.015, 0.13, 1.0 - base_shape) + (1.0 - shape.b) * 0.012
            + storm * (1.0 - bank) * 0.055;
        let deck_base = smoothstep(base_height, base_height + 0.075, density_h);
        base = mix(deck_base, base, convective);
        let sheet_field = (coverage - 0.50) * 1.8 + (shape.r - 0.5) * 1.35
            + (bank - 0.5) * 1.15 + (rolls - 0.5) * 1.25
            - smoothstep(0.18, 0.95, h) * 1.1;
        let sheet_water = mix(0.28, 1.08, smoothstep(-0.08, 0.85, sheet_field));
        water_content = mix(sheet_water, 1.0, convective);
    }
    let cap = 1.0 - smoothstep(mix(0.70, 0.94, shape.g), 1.0, density_h);
    let folds = mix(0.68, 1.08, shape.g) * mix(0.87, 1.0, detail.r);
    // Apply the authored wind-aligned rolls to both view and shadow density.
    // The helper was composed into the shader but never sampled.
    let streets = cloud_street_rolls(xz, wind, weather.convection);
    return body * base * cap * folds * water_content * mix(0.45, 1.60, authored_cloud_density())
        * streets * smoothstep(0.01, 0.12, weather.low_amount);
}

// Cloud uniforms adapt to the same flow field used by the orbital shell.
fn cloud_flow_warp(xz: vec2<f32>, h: f32, layer_id: i32) -> vec3<f32> {
    let wind = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let wspd = max(light.weather_globals[0].y, 0.0);
    return cloud_flow_warp_at(xz, h, layer_id, wind, wspd, light.weather_globals[0].x,
        saturate1(light.weather_globals[1].z), wspd * cloud_layer_speed_mult(layer_id));
}
// Extra cloud forms chosen from existing weather channels and hashed noise,
// so no weather texture channel or save field is added.

// Weights for fibres, cirrocumulus grain and a cirrostratus veil per ice bank.
fn cirrus_form_weights(xz: vec2<f32>) -> vec3<f32> {
    let f = weather_bank_noise(xz / 21000.0 + vec2<f32>(41.3, -7.7));
    let veil = smoothstep(0.62, 0.76, f);
    let grain = smoothstep(0.30, 0.46, f) * (1.0 - veil);
    return vec3<f32>(1.0 - smoothstep(0.30, 0.46, f), grain, veil);
}

// Mackerel sky: small ripples across the wind, averaged out at range.
fn cirrocumulus_grain(along: f32, perp: f32, footprint: f32) -> f32 {
    let n1 = weather_bank_noise(vec2<f32>(along / 210.0, perp / 95.0));
    let n2 = weather_bank_noise(vec2<f32>(along / 83.0 + 7.1, perp / 46.0 - 3.3));
    let grain = smoothstep(0.50, 0.70, n1 * 0.65 + n2 * 0.35);
    return mix(0.36, grain, 1.0 - smoothstep(30.0, 90.0, footprint));
}

fn cloud_street_rolls(xz: vec2<f32>, wind: vec2<f32>, convection: f32) -> f32 {
    return cloud_street_rolls_at(xz, wind, convection, max(light.weather_globals[0].y, 0.0));
}

// Mammatus pouches carved into the underside of storm bases.
fn mammatus_carve(xz: vec2<f32>, h: f32, storm: f32) -> f32 {
    let gate = smoothstep(0.45, 0.80, storm) * (1.0 - smoothstep(0.0, 0.26, h));
    if gate <= 0.01 { return 0.0; }
    let lobes = weather_bank_noise(xz / 120.0 + vec2<f32>(14.2, 3.9)) * 0.65
        + weather_bank_noise(xz / 52.0 - vec2<f32>(6.1, 27.0)) * 0.35;
    return (1.0 - smoothstep(0.30, 0.75, lobes)) * 0.11 * gate;
}

// Stationary wave lenticulars: lens bands across the wind that do not drift.
fn lenticular_density(xz: vec2<f32>, wind: vec2<f32>, h: f32, convection: f32, humidity: f32) -> f32 {
    let region = smoothstep(0.60, 0.78, weather_bank_noise(xz / 11000.0 + vec2<f32>(3.7, -12.2)))
        * (1.0 - smoothstep(0.30, 0.60, convection)) * smoothstep(0.25, 0.60, humidity);
    if region <= 0.01 { return 0.0; }
    let along = dot(xz, wind);
    let perp = dot(xz, vec2<f32>(-wind.y, wind.x));
    let bands = 0.5 + 0.5 * sin(along * 6.2831853 / 3600.0
        + weather_bank_noise(vec2<f32>(along / 5200.0, perp / 1500.0)) * 3.0);
    let lens = smoothstep(0.78, 0.96, bands)
        * smoothstep(0.35, 0.70, weather_bank_noise(vec2<f32>(along / 7000.0, perp / 1100.0) + vec2<f32>(8.8, 1.3)));
    let core = sin(3.14159265 * clamp(h, 0.0, 1.0));
    return lens * region * core;
}
