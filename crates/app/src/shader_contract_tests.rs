//! Fast static checks on the WGSL sources. They run in milliseconds and need
//! no GPU. naga validation of every composed program already runs in build.rs;
//! these tests cover what naga cannot see: unused files, the single-pixel-grid
//! rule from .claude/skills/render/SKILL.md, and calls to helpers that were renamed or removed.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn shader_dir() -> PathBuf {
    manifest_dir().join("src").join("shaders")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// Every `.wgsl` file under src/shaders, by file name.
fn shader_files() -> BTreeSet<String> {
    fs::read_dir(shader_dir())
        .expect("src/shaders must exist")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "wgsl"))
        .filter_map(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect()
}

/// Every `shaders/<name>.wgsl` path that src/shader_sources.rs includes.
fn included_shader_files() -> BTreeSet<String> {
    let source = read(&manifest_dir().join("src").join("shader_sources.rs"));
    let mut names = BTreeSet::new();
    for chunk in source.split("include_str!(\"shaders/").skip(1) {
        if let Some(end) = chunk.find('"') {
            names.insert(chunk[..end].to_owned());
        }
    }
    names
}

#[test]
fn every_shader_file_is_included_by_the_build() {
    // CLAUDE.md: a new .wgsl file that is not listed in shader_sources.rs is
    // silently unused. Catch it here instead of at review time.
    let on_disk = shader_files();
    let included = included_shader_files();
    let unused: Vec<_> = on_disk.difference(&included).collect();
    assert!(
        unused.is_empty(),
        "shader files on disk but not included by shader_sources.rs: {unused:?}"
    );
}

#[test]
fn every_included_shader_file_exists() {
    let on_disk = shader_files();
    let included = included_shader_files();
    let missing: Vec<_> = included.difference(&on_disk).collect();
    assert!(
        missing.is_empty(),
        "shader_sources.rs includes files that do not exist: {missing:?}"
    );
}

/// Rule from .claude/skills/render/SKILL.md "One pixel size": procedural detail sits on the single
/// PIXEL_RES lattice. A second cells-per-metre constant, or a multi-texel cell
/// written as `PIXEL_RES * 0.5`, is a second grid. Lines that legitimately
/// need one are listed in ALLOWED_GRID_LINES with a reason.
const FORBIDDEN_GRID_PATTERNS: &[&str] = &["PIXEL_RES * 0.5", "PIXEL_RES*0.5", "* 0.5) * PIXEL_RES"];

/// (file name, substring that must appear on the line) -> reason it is allowed.
const ALLOWED_GRID_LINES: &[(&str, &str, &str)] = &[];

#[test]
fn procedural_detail_uses_one_pixel_grid() {
    let mut violations = Vec::new();
    for name in shader_files() {
        let text = read(&shader_dir().join(&name));
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            let hit = FORBIDDEN_GRID_PATTERNS.iter().any(|pattern| code.contains(pattern));
            if !hit {
                continue;
            }
            let allowed = ALLOWED_GRID_LINES
                .iter()
                .any(|(file, needle, _)| *file == name && code.contains(needle));
            if !allowed {
                violations.push(format!("{name}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "second pixel grid found (render skill rule):\n{}",
        violations.join("\n")
    );
}

/// Helper functions that were renamed or removed by other sessions. A call to
/// one of these means a half-finished edit; naga catches it too, but this test
/// names the stale name directly.
const RETIRED_HELPERS: &[&str] = &["specular_term(", "pixel_blob_legacy(", "texel_block("];

#[test]
fn no_calls_to_retired_helpers() {
    let mut violations = Vec::new();
    for name in shader_files() {
        let text = read(&shader_dir().join(&name));
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for helper in RETIRED_HELPERS {
                if code.contains(helper) {
                    violations.push(format!("{name}:{}: {}", number + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "calls to retired helpers:\n{}",
        violations.join("\n")
    );
}
