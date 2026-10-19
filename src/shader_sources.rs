//! Shader composition shared by the renderer and compile-time WGSL validation.
const CAMERA_LIGHTING_WGSL: &str = include_str!("shaders/camera_lighting_shared.wgsl");
const LIGHTING_WGSL: &str = include_str!("shaders/lighting_shared.wgsl");
const ATMOSPHERE_RUNTIME_WGSL: &str = include_str!("shaders/atmosphere_runtime_shared.wgsl");
const ATMOSPHERE_WGSL: &str = include_str!("shaders/atmosphere_shared.wgsl");
pub(crate) const ATMOSPHERE_LUT_GENERATION_WGSL: &str =
    include_str!("shaders/atmosphere_lut_generation.wgsl");
const CLOUD_SHARED_WGSL: &str = include_str!("shaders/cloud_shared.wgsl");
const CLOUD_MORPHOLOGY_WGSL: &str = include_str!("shaders/cloud_morphology.wgsl");
const WEATHER_VOLUME_WGSL: &str = include_str!("shaders/weather_volume.wgsl");

const WORLD_SHADER_BODY: &str = include_str!("shaders/world_shader.wgsl");
const LOCAL_MIST_WGSL: &str = include_str!("shaders/local_mist_shared.wgsl");
const CLOUD_SHADER_BODY: &str = include_str!("shaders/cloud_shader.wgsl");
const CLOUD_COMPOSITE_SHADER_BODY: &str = include_str!("shaders/cloud_composite.wgsl");
const SKY_SHADER_BODY: &str = include_str!("shaders/sky_shader.wgsl");
const CLOUD_TEMPORAL_SHADER_BODY: &str = include_str!("shaders/cloud_temporal.wgsl");

pub(crate) fn world_shader_source() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        CAMERA_LIGHTING_WGSL,
        ATMOSPHERE_RUNTIME_WGSL,
        ATMOSPHERE_WGSL,
        CLOUD_SHARED_WGSL,
        LOCAL_MIST_WGSL,
        WORLD_SHADER_BODY
    )
}

pub(crate) fn sky_shader_source() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}",
        CAMERA_LIGHTING_WGSL, ATMOSPHERE_RUNTIME_WGSL, ATMOSPHERE_WGSL, LOCAL_MIST_WGSL, SKY_SHADER_BODY
    )
}

pub(crate) fn cloud_shader_source() -> String {
    cloud_shader_source_for_filtering(false)
}

/// Keep float32 precision and identical bilinear sampling. Hardware filtering
/// replaces four loads + address arithmetic where the adapter supports it.
/// Unsupported devices retain the explicit four-tap implementation.
pub(crate) fn cloud_shader_source_for_filtering(hardware_filtering: bool) -> String {
    let sampling = if hardware_filtering {
        r#"fn sample_weather_bilinear(tex: texture_2d_array<f32>, uv: vec2<f32>, level: i32) -> vec4<f32> {
            let dims = vec2<f32>(textureDimensions(tex));
            let clamped = clamp(uv, vec2<f32>(0.5)/dims, vec2<f32>(1.0)-vec2<f32>(0.5)/dims);
            return textureSampleLevel(tex, cloud_field_sampler, clamped, level, 0.0);
        }"#
    } else {
        include_str!("shaders/cloud_weather_sampling.wgsl")
    };
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        CAMERA_LIGHTING_WGSL,
        ATMOSPHERE_RUNTIME_WGSL,
        ATMOSPHERE_WGSL,
        CLOUD_SHARED_WGSL,
        CLOUD_MORPHOLOGY_WGSL,
        WEATHER_VOLUME_WGSL,
        sampling,
        CLOUD_SHADER_BODY
    )
}

pub(crate) fn cloud_composite_shader_source() -> String {
    format!(
        "{}\n{}\n{}",
        LIGHTING_WGSL, ATMOSPHERE_RUNTIME_WGSL, CLOUD_COMPOSITE_SHADER_BODY
    )
}

pub(crate) fn cloud_temporal_shader_source() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}",
        CAMERA_LIGHTING_WGSL,
        ATMOSPHERE_RUNTIME_WGSL,
        ATMOSPHERE_WGSL,
        CLOUD_SHARED_WGSL,
        CLOUD_TEMPORAL_SHADER_BODY
    )
}

pub(crate) const LINE_SHADER: &str = include_str!("shaders/outline_shader.wgsl");
pub(crate) const SHADOW_SHADER: &str = include_str!("shaders/shadow_shader.wgsl");
pub(crate) const UI_SHADER: &str = include_str!("shaders/ui_shader.wgsl");
pub(crate) const POSTPROCESS_SHADER: &str = include_str!("shaders/postprocess.wgsl");
pub(crate) const TERRAIN_COMPUTE_SHADER: &str = include_str!("terrain_compute.wgsl");
pub(crate) const FLUID_SHADER: &str = include_str!("fluid_compute.wgsl");

pub(crate) fn indirect_shader_source(composite: bool) -> String {
    let camera = CAMERA_LIGHTING_WGSL
        .split("struct Lighting")
        .next()
        .unwrap();
    format!(
        "{}\n{}\n{}",
        camera,
        include_str!("shaders/indirect_shared.wgsl"),
        if composite {
            include_str!("shaders/indirect_composite.wgsl")
        } else {
            include_str!("shaders/indirect_gather.wgsl")
        }
    )
}
