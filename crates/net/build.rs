fn main() {
    println!("cargo:rerun-if-env-changed=GNS_NATIVE_LIB_DIR");
    if std::env::var_os("CARGO_FEATURE_STEAM").is_none()
        || std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
    {
        return;
    }
    // The upstream 0.3.0 build script omits native system dependencies from its
    // static-library link metadata. Keep those requirements with the transport.
    for library in ["bcrypt", "crypt32", "ws2_32", "iphlpapi", "winmm"] {
        println!("cargo:rustc-link-lib={library}");
    }
    // Optional workaround for upstream's Debug/RelWithDebInfo search-path mismatch.
    if let Some(directory) = std::env::var_os("GNS_NATIVE_LIB_DIR") {
        let directory = std::path::PathBuf::from(directory);
        assert!(
            directory.join("GameNetworkingSockets_s.lib").is_file(),
            "GNS_NATIVE_LIB_DIR must contain GameNetworkingSockets_s.lib"
        );
        println!("cargo:rustc-link-search=native={}", directory.display());
    }
}
