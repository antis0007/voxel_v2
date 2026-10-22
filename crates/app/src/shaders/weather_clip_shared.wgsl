// Weather clipmap region for one level, from the Lighting uniform. Shared by
// the world, cloud and cloud-composite programs.
fn weather_clip_region(level: i32) -> vec4<f32> {
    return light.weather_clip_regions[level];
}
