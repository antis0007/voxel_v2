// ============================================================
// cloud_shared.wgsl - Shared cloud layer height definitions
//
// Provides global envelope bounds for slab-bounded raymarching
// and temporal reprojection. Per-step AABB computation (from weather)
// lives in cloud_shader.wgsl alongside CellWeather.
//
// Included by cloud_shader.wgsl and cloud_temporal.wgsl via render.rs.
// ============================================================

struct CloudLayerBounds {
    low_base  : f32,
    low_top   : f32,
    mid_base  : f32,
    mid_top   : f32,
    high_base : f32,
    high_top  : f32,
};

// Atmosphere height is independent of the editable voxel world's ceiling.
// Keeping cloud bases at 80-140 m put ordinary hills inside every weather deck.
const CLOUD_LOW_ALTITUDE_OFFSET_M: f32 = 600.0;
const CLOUD_MID_ALTITUDE_OFFSET_M: f32 = 1600.0;
// High ice clouds remain sunlit after the lower liquid decks enter shadow.
const CLOUD_HIGH_ALTITUDE_OFFSET_M: f32 = 7000.0;

fn cloud_layer_bounds() -> CloudLayerBounds {
    var b: CloudLayerBounds;

    // Conservative superset of cell_low/mid/high_aabb, before the altitude
    // offsets below. Keep the thickness controls paired with those functions:
    // fair cumulus needs hundreds of meters of depth for distinct turrets.

    let base_ctl         = clamp(light.weather_globals[1].x, 0.0, 1.0);
    let thick_ctl        = clamp(light.weather_globals[1].y, 0.0, 1.0);
    let mid_height_norm  = clamp(light.weather_globals[3].y, 0.0, 1.0);
    let high_height_norm = clamp(light.weather_globals[2].w, 0.0, 1.0);
    let storm_height_ctl = clamp(light.cloud_authoring[0].w, 0.0, 1.0);
    let altostratus_ctl  = clamp(light.cloud_authoring[0].z, 0.0, 1.0);

    // Low layer — conservative envelope for voxel-world clouds.
    // The bounds are a SUPERSET of what any cell AABB can produce.
    // The AABB dead zone was raised (smoothstep 0.07→0.22 in low_amount),
    // but the global envelope does not need to change — it is a coarse
    // early-reject test and should remain a guaranteed superset.
    let low_base_center  = mix(80.0, 280.0, base_ctl);
    let low_fair_thick   = mix(420.0, 980.0, thick_ctl);
    let low_storm_extra  = mix(400.0, 1800.0, storm_height_ctl);
    let storm_amount = clamp(light.cloud_authoring[1].z, 0.0, 1.0);
    b.low_base = max(low_base_center * 0.85 - 2.0, 0.0);
    b.low_top  = low_base_center * 1.06 + (low_fair_thick + low_storm_extra * storm_amount) * 1.35 + 2.0;

    // Mid layer
    let mid_base_center = mix(220.0, 420.0, mid_height_norm);
    let mid_depth_max   = mix(110.0, 320.0, thick_ctl) + altostratus_ctl * 130.0;
    b.mid_base = max(mid_base_center * 0.94 - mix(20.0, 45.0, altostratus_ctl) - 2.0, 0.0);
    b.mid_top  = mid_base_center + mid_depth_max + 2.0;

    // High layer
    let high_base_center = mix(380.0, 550.0, high_height_norm);
    let high_depth_max   = mix(90.0, 240.0, thick_ctl);
    b.high_base = high_base_center * 0.97 - 22.0;
    b.high_top  = high_base_center - 20.0 + high_depth_max + 2.0;

    b.low_base += CLOUD_LOW_ALTITUDE_OFFSET_M;
    b.low_top += CLOUD_LOW_ALTITUDE_OFFSET_M;
    b.mid_base += CLOUD_MID_ALTITUDE_OFFSET_M;
    b.mid_top += CLOUD_MID_ALTITUDE_OFFSET_M;
    b.high_base += CLOUD_HIGH_ALTITUDE_OFFSET_M;
    b.high_top += CLOUD_HIGH_ALTITUDE_OFFSET_M;
    // Whole-layer compositing is valid only for disjoint altitude intervals.
    // Lift the middle deck above deep low convection, preserving its thickness.
    let mid_lift = max(b.low_top + 120.0 - b.mid_base, 0.0);
    b.mid_base += mid_lift;
    b.mid_top += mid_lift;
    return b;
}


struct CellAABB {
    y_min : f32,
    y_max : f32,
};

fn cell_aabb_valid(aabb: CellAABB) -> bool {
    return aabb.y_max > aabb.y_min;
}
