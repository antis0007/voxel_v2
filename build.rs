//! Validate the same composed WGSL programs consumed by the runtime.
#[path = "src/shader_sources.rs"]
mod shader_sources;

fn main() {
    for path in [
        "build.rs",
        "src/shader_sources.rs",
        "src/shaders",
        "src/terrain_compute.wgsl",
        "src/fluid_compute.wgsl",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    // Naga's parser and validator can require more stack than a build worker.
    std::thread::Builder::new()
        .name("shader-compilation".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(validate)
        .expect("cannot start WGSL compiler")
        .join()
        .expect("WGSL compilation failed");
}

fn validate() {
    use shader_sources::*;
    let programs = [
        ("world", world_shader_source()),
        ("diffuse gather", indirect_shader_source(false)),
        ("diffuse resolve", indirect_shader_source(true)),
        ("sky", sky_shader_source()),
        ("cloud", cloud_shader_source()),
        ("cloud hardware filtering", cloud_shader_source_for_filtering(true)),
        ("cloud composite", cloud_composite_shader_source()),
        ("cloud temporal", cloud_temporal_shader_source()),
        ("atmosphere LUT", ATMOSPHERE_LUT_GENERATION_WGSL.to_owned()),
        ("outline", LINE_SHADER.to_owned()),
        ("shadow", SHADOW_SHADER.to_owned()),
        ("UI", UI_SHADER.to_owned()),
        ("postprocess", POSTPROCESS_SHADER.to_owned()),
        ("terrain", TERRAIN_COMPUTE_SHADER.to_owned()),
        ("fluid", FLUID_SHADER.to_owned()),
    ];
    let mut errors = Vec::new();
    for (name, source) in programs {
        match naga::front::wgsl::parse_str(&source) {
            Err(error) => errors.push(format!("{name}: {}", error.emit_to_string(&source))),
            Ok(module) => {
                let mut validator = naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                );
                if let Err(error) = validator.validate(&module) {
                    errors.push(format!("{name}: {}", error.emit_to_string(&source)));
                }
            }
        }
    }
    assert!(
        errors.is_empty(),
        "Invalid WGSL programs:\n{}",
        errors.join("\n")
    );
}
