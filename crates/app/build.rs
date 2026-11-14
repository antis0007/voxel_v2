//! Validate the same composed WGSL programs consumed by the runtime.
#[path = "src/shader_sources.rs"]
mod shader_sources;
mod network_build;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(axiom_native_edit_review)");
    // Winit runs renderer and UI initialization on the Windows main thread.
    // Reserve headroom for that path; RUST_MIN_STACK only affects spawned threads.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bin=minecraft_style_voxel_prototype=/STACK:8388608");
    }
    // Cargo can reuse a build-script executable when a frozen source copy has
    // older timestamps. Text replacement can refresh WGSL bodies, but cannot
    // repair a stale composition function or input table. Reject that case
    // before issuing a misleading successful validation receipt.
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    if std::env::var_os("CARGO_FEATURE_MULTIPLAYER_SDR").is_some(){
        network_build::emit(&manifest.join("../..").canonicalize().unwrap());
    }
    for (path, embedded) in [
        ("build.rs", include_str!("build.rs")),
        ("src/shader_sources.rs", include_str!("src/shader_sources.rs")),
    ] {
        let current = std::fs::read_to_string(manifest.join(path))
            .unwrap_or_else(|error| panic!("cannot read shader validation source {path}: {error}"));
        assert!(current == embedded,
            "Cargo reused stale shader composition code ({path}); update build.rs and src/shader_sources.rs timestamps and rebuild using the same target directory");
    }
    for path in [
        "build.rs",
        "src/shader_sources.rs",
        "src/shaders",
        "src/fluid_compute.wgsl",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    for path in shader_sources::shader_input_paths() { println!("cargo:rerun-if-changed={path}"); }
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
        ("world hardware filtering", world_shader_source_for_filtering(true)),
        ("diffuse gather", indirect_shader_source(false)),
        ("diffuse resolve", indirect_shader_source(true)),
        ("sky", sky_shader_source()),
        ("sky hardware filtering", sky_shader_source_for_filtering(true)),
        ("sky probe", sky_probe_shader_source()),
        ("posed assets", posed_assets_shader_source()),
        ("globe", globe_shader_source()),
        ("globe hardware filtering", globe_shader_source_for_filtering(true)),
        ("cloud", cloud_shader_source()),
        (
            "cloud hardware filtering",
            cloud_shader_source_for_filtering(true),
        ),
        ("cloud composite", cloud_composite_shader_source()),
        ("cloud temporal", cloud_temporal_shader_source()),
        ("atmosphere LUT", ATMOSPHERE_LUT_GENERATION_WGSL.to_owned()),
        ("outline", LINE_SHADER.to_owned()),
        ("shadow", SHADOW_SHADER.to_owned()),
        ("UI", UI_SHADER.to_owned()),
        ("sky volume bake", SKY_VOLUME_BAKE_SHADER.to_owned()),
        ("UI magic", UI_MAGIC_SHADER.to_owned()),
        ("Portal composite", PORTAL_COMPOSITE_SHADER.to_owned()),
        ("UI prism", UI_PRISM_SHADER.to_owned()),
        ("Title vista", MENU_SCENE_SHADER.to_owned()),
        ("Meridian pixels", MENU_PRESENT_SHADER.to_owned()),
        ("Loading globe", loading_globe_shader()),
        ("postprocess", POSTPROCESS_SHADER.to_owned()),
        ("present", PRESENT_SHADER.to_owned()),
        ("fluid", FLUID_SHADER.to_owned()),
    ];
    let manifest=std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let programs=programs.map(|(name,source)|(name,current_validation_source(source,&manifest).expect("cannot read current shader input")));
    let world_fingerprint=source_fingerprint(&programs[0].1);
    let filtered_world_fingerprint=source_fingerprint(&programs[1].1);
    let count=programs.len();
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
    println!("cargo:rustc-env=VOXEL_VALIDATED_WORLD_FNV={world_fingerprint:016x}");
    println!("cargo:rustc-env=VOXEL_VALIDATED_FILTERED_WORLD_FNV={filtered_world_fingerprint:016x}");
    println!("cargo:warning=Validated {count} composed WGSL programs; world_fnv1a64={world_fingerprint:016x}");
}
