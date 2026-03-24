// ============================================================
// cloud_shared.wgsl - Shared cloud layer height definitions
//
// Provides global envelope bounds for early-out and temporal
// reprojection. Per-cell AABB computation lives in cloud_shader.wgsl
// alongside CellWeather (which is not available at this include level).
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

fn cloud_layer_bounds() -> CloudLayerBounds {
    var b: CloudLayerBounds;

    let base_ctl         = clamp(light.weather_globals[1].x, 0.0, 1.0);
    let thick_ctl        = clamp(light.weather_globals[1].y, 0.0, 1.0);
    let mid_height_norm  = clamp(light.weather_globals[3].y, 0.0, 1.0);
    let high_height_norm = clamp(light.weather_globals[2].w, 0.0, 1.0);
    let storm_height_ctl = clamp(light.cloud_authoring[0].w, 0.0, 1.0);
    let altostratus_ctl  = clamp(light.cloud_authoring[0].z, 0.0, 1.0);

    // Keep low, mid, and high layers independently anchored in world space.
    // The old chained bounds forced unrelated genera onto nearly the same
    // effective level once one layer thickened.
    let low_base_center = mix(350.0, 2200.0, base_ctl);
    let low_thickness   = mix(250.0, 1800.0, thick_ctl);
    let storm_extension = mix(600.0, 6000.0, storm_height_ctl);
    b.low_base = low_base_center * 0.82;
    b.low_top  = low_base_center + low_thickness + storm_extension;

    let mid_base_center = mix(2500.0, 7500.0, mid_height_norm);
    let mid_thickness   = mix(500.0, 2200.0, thick_ctl) + altostratus_ctl * 900.0;
    b.mid_base = mid_base_center - 350.0;
    b.mid_top  = mid_base_center + mid_thickness;

    let high_base_center = mix(6500.0, 12500.0, high_height_norm);
    let high_thickness   = mix(350.0, 1400.0, thick_ctl);
    b.high_base = high_base_center - 250.0;
    b.high_top  = high_base_center + high_thickness;

    return b;
}

struct CellAABB {
    y_min : f32,
    y_max : f32,
};

fn cell_aabb_valid(aabb: CellAABB) -> bool {
    return aabb.y_max > aabb.y_min;
}
