//! Shader composition shared by the renderer and compile-time WGSL validation.
/// Stable diagnostic receipt, not a cryptographic content hash.
pub(crate) fn source_fingerprint(source: &str) -> u64 {
    source.bytes().fold(0xcbf29ce484222325u64, |hash, byte|
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
}

macro_rules! shader_inputs {
    ($($path:literal),* $(,)?) => { &[$((concat!("src/",$path),include_str!($path))),*] };
}
const SHADER_INPUTS: &[(&str,&str)] = shader_inputs![
    "shaders/menu_scene.wgsl", "shaders/menu_present.wgsl",
    "shaders/ui_magic.wgsl", "shaders/camera_lighting_shared.wgsl", "shaders/lighting_shared.wgsl",
    "shaders/atmosphere_runtime_shared.wgsl", "shaders/atmosphere_shared.wgsl", "shaders/atmosphere_lut_generation.wgsl", "shaders/sky_probe.wgsl",
    "shaders/cloud_shared.wgsl", "shaders/cloud_morphology.wgsl", "shaders/weather_volume.wgsl",
    "shaders/underwater_shared.wgsl", "shaders/mood_shared.wgsl", "shaders/terrain_detail_shared.wgsl",
    "shaders/enchantment_shared.wgsl", "shaders/terrain_surface.wgsl", "shaders/fluid_surface_shared.wgsl", "shaders/world_shader.wgsl", "shaders/local_mist_shared.wgsl",
    "shaders/cloud_shader.wgsl", "shaders/cloud_composite.wgsl", "shaders/sky_shader.wgsl",
    "shaders/cloud_temporal.wgsl", "shaders/cloud_weather_sampling.wgsl", "shaders/outline_shader.wgsl",
    "shaders/shadow_shader.wgsl", "shaders/ui_shader.wgsl", "shaders/postprocess.wgsl",
    "shaders/present.wgsl", "shaders/color_shared.wgsl", "shaders/weather_clip_shared.wgsl", "fluid_compute.wgsl", "shaders/indirect_shared.wgsl",
    "shaders/indirect_composite.wgsl", "shaders/indirect_gather.wgsl", "shaders/posed_assets.wgsl",
];

#[allow(dead_code)] // Called only by the build-script crate, never by rendering.
pub(crate) fn shader_input_paths() -> impl Iterator<Item=&'static str> { SHADER_INPUTS.iter().map(|p|p.0) }

/// A cached build-script executable may embed older shader text. Validate fresh
/// files through the SAME composition, without changing runtime file loading.
#[allow(dead_code)]
pub(crate) fn current_validation_source(source: String, manifest: &std::path::Path) -> std::io::Result<String> {
    let mut inputs=SHADER_INPUTS.iter().map(|&(path,embedded)|
        Ok((embedded.to_owned(),std::fs::read_to_string(manifest.join(path))?)))
        .collect::<std::io::Result<Vec<_>>>()?;
    let current_camera=std::fs::read_to_string(manifest.join("src/shaders/camera_lighting_shared.wgsl"))?;
    // Indirect programs use only the camera prefix of the combined declaration.
    inputs.push((CAMERA_LIGHTING_WGSL.split("struct Lighting").next().unwrap().to_owned(),
        current_camera.split("struct Lighting").next().unwrap().to_owned()));
    let mut output=String::with_capacity(source.len()); let mut cursor=0;
    while cursor<source.len() {
        let next=inputs.iter().filter(|(old,_)| !old.is_empty()).filter_map(|(old,new)|
            source[cursor..].find(old).map(|offset|(cursor+offset,old,new)))
            .min_by(|a,b|a.0.cmp(&b.0).then_with(||b.1.len().cmp(&a.1.len())));
        let Some((start,old,new))=next else { output.push_str(&source[cursor..]); break; };
        output.push_str(&source[cursor..start]); output.push_str(new); cursor=start+old.len();
    }
    Ok(output)
}
pub(crate) const UI_MAGIC_SHADER: &str = include_str!("shaders/ui_magic.wgsl");
pub(crate) const MENU_SCENE_SHADER: &str = concat!(include_str!("shaders/color_shared.wgsl"), "\n", include_str!("shaders/menu_scene.wgsl"));
pub(crate) const MENU_PRESENT_SHADER: &str = include_str!("shaders/menu_present.wgsl");
const CAMERA_LIGHTING_WGSL: &str = include_str!("shaders/camera_lighting_shared.wgsl");
const LIGHTING_WGSL: &str = include_str!("shaders/lighting_shared.wgsl");
const ATMOSPHERE_RUNTIME_WGSL: &str = include_str!("shaders/atmosphere_runtime_shared.wgsl");
const ATMOSPHERE_WGSL: &str = include_str!("shaders/atmosphere_shared.wgsl");
pub(crate) const ATMOSPHERE_LUT_GENERATION_WGSL: &str =
    include_str!("shaders/atmosphere_lut_generation.wgsl");
const CLOUD_SHARED_WGSL: &str = include_str!("shaders/cloud_shared.wgsl");
const CLOUD_MORPHOLOGY_WGSL: &str = include_str!("shaders/cloud_morphology.wgsl");
const WEATHER_VOLUME_WGSL: &str = include_str!("shaders/weather_volume.wgsl");
const UNDERWATER_WGSL: &str = include_str!("shaders/underwater_shared.wgsl");
const MOOD_WGSL: &str = include_str!("shaders/mood_shared.wgsl");

const WORLD_SHADER_BODY: &str = concat!(include_str!("shaders/terrain_detail_shared.wgsl"), "\n", include_str!("shaders/enchantment_shared.wgsl"), "\n", include_str!("shaders/terrain_surface.wgsl"), "\n", include_str!("shaders/fluid_surface_shared.wgsl"), "\n", include_str!("shaders/world_shader.wgsl"));
const LOCAL_MIST_WGSL: &str = include_str!("shaders/local_mist_shared.wgsl");
const CLOUD_SHADER_BODY: &str = include_str!("shaders/cloud_shader.wgsl");
const CLOUD_COMPOSITE_SHADER_BODY: &str = include_str!("shaders/cloud_composite.wgsl");
const SKY_SHADER_BODY: &str = include_str!("shaders/sky_shader.wgsl");
const SKY_PROBE_BODY: &str = include_str!("shaders/sky_probe.wgsl");
const POSED_ASSETS_BODY: &str = include_str!("shaders/posed_assets.wgsl");

/// Poses resident rigid scene assets (`render/posed_assets.rs`).
pub(crate) fn posed_assets_shader_source() -> String {
    POSED_ASSETS_BODY.to_owned()
}
const CLOUD_TEMPORAL_SHADER_BODY: &str = include_str!("shaders/cloud_temporal.wgsl");
const CLOUD_WEATHER_SAMPLING_WGSL: &str = include_str!("shaders/cloud_weather_sampling.wgsl");
const WEATHER_CLIP_WGSL: &str = include_str!("shaders/weather_clip_shared.wgsl");

pub(crate) fn world_shader_source() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        CAMERA_LIGHTING_WGSL,
        ATMOSPHERE_RUNTIME_WGSL,
        ATMOSPHERE_WGSL,
        CLOUD_SHARED_WGSL,
        LOCAL_MIST_WGSL,
        UNDERWATER_WGSL,
        MOOD_WGSL,
        WEATHER_CLIP_WGSL,
        CLOUD_WEATHER_SAMPLING_WGSL,
        WORLD_SHADER_BODY
    )
}

/// Per-frame sky and sun terms shared by every terrain pixel (`SkyProbe`).
pub(crate) fn sky_probe_shader_source() -> String {
    format!(
        "{}
{}
{}
{}",
        CAMERA_LIGHTING_WGSL,
        ATMOSPHERE_RUNTIME_WGSL,
        ATMOSPHERE_WGSL,
        SKY_PROBE_BODY
    )
}

pub(crate) fn sky_shader_source() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
        CAMERA_LIGHTING_WGSL,
        ATMOSPHERE_RUNTIME_WGSL,
        ATMOSPHERE_WGSL,
        LOCAL_MIST_WGSL,
        UNDERWATER_WGSL,
        MOOD_WGSL,
        SKY_SHADER_BODY
    )
}

// Build-time validation entry (build.rs). The runtime calls the filtering variant.
#[allow(dead_code)]
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
        CLOUD_WEATHER_SAMPLING_WGSL
    };
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        CAMERA_LIGHTING_WGSL,
        ATMOSPHERE_RUNTIME_WGSL,
        ATMOSPHERE_WGSL,
        CLOUD_SHARED_WGSL,
        CLOUD_MORPHOLOGY_WGSL,
        WEATHER_VOLUME_WGSL,
        WEATHER_CLIP_WGSL,
        sampling,
        CLOUD_SHADER_BODY
    )
}

pub(crate) fn cloud_composite_shader_source() -> String {
    // Reuse the camera declaration in the existing texture group, keeping the
    // composite within four bind groups. Depth classification needs the exact
    // projection coefficients at every world scale.
    let camera = CAMERA_LIGHTING_WGSL.split("struct Lighting").next().unwrap()
        .replace("@group(0) @binding(0)", "@group(1) @binding(2)");
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
        camera, LIGHTING_WGSL, ATMOSPHERE_RUNTIME_WGSL, WEATHER_CLIP_WGSL, CLOUD_WEATHER_SAMPLING_WGSL, CLOUD_SHARED_WGSL, CLOUD_COMPOSITE_SHADER_BODY
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
pub(crate) const SHADOW_SHADER: &str = concat!(include_str!("shaders/terrain_detail_shared.wgsl"), "\n", include_str!("shaders/shadow_shader.wgsl"));
pub(crate) const UI_SHADER: &str = include_str!("shaders/ui_shader.wgsl");
pub(crate) const POSTPROCESS_SHADER: &str = concat!(include_str!("shaders/color_shared.wgsl"), "\n", include_str!("shaders/postprocess.wgsl"));
pub(crate) const PRESENT_SHADER: &str = concat!(include_str!("shaders/color_shared.wgsl"), "\n", include_str!("shaders/present.wgsl"));
// Kept on purpose (docs/design/EXPEDITION_DEPTH_DESIGN.md):
// validated by build.rs, not yet dispatched by the runtime.
#[allow(dead_code)]
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

#[cfg(test)]
mod validation_receipt {
    #[test]
    fn embedded_world_matches_the_build_validator_source() {
        assert_eq!(format!("{:016x}",super::source_fingerprint(&super::world_shader_source())),
            env!("VOXEL_VALIDATED_WORLD_FNV"),"copied source timestamps reused a stale shader validation result");
    }
}
