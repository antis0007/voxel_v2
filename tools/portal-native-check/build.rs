// Test the actual portal math and adapter without pulling in WGPU/windowing.
// Only app-module declarations are removed; the core implementation is verbatim.
fn main() {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let core = root.join("crates/app/src/world/portals.rs");
    let authored = root
        .join("crates/app/src/world/portals/authored.rs")
        .canonicalize()
        .unwrap();
    println!("cargo:rerun-if-changed={}", core.display());
    println!("cargo:rerun-if-changed={}", authored.display());
    let mut source = String::new();
    for line in std::fs::read_to_string(&core).unwrap().lines() {
        if line.starts_with("pub mod ")
            || line.starts_with("mod gates_tests;")
            || line == "#[cfg(test)]"
        {
            continue;
        }
        source.push_str(
            line.strip_prefix("//!")
                .map_or(line.to_string(), |s| format!("//{s}"))
                .as_str(),
        );
        source.push('\n');
    }
    source.push_str("pub mod gates { pub fn realm() -> super::Realm { super::Realm::Planet(\"local\".into()) } }\n");
    source.push_str(&format!(
        "#[path = {:?}] pub mod authored;\n",
        authored.to_str().unwrap()
    ));
    std::fs::write(
        std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("portal_core.rs"),
        source,
    )
    .unwrap();
}
