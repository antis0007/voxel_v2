//! Repo-layout guards. A scratch file at the repo root or a WGSL file that no
//! program includes is picked up by nobody, but agents still read and edit it.
//! These tests fail loudly instead. See AGENTS.md, "Repo hygiene".

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    /// Everything allowed at the repo root. Add a real project entry here on
    /// purpose; scratch files, logs, zips, patches and launchers stay outside.
    const ROOT_FILES: &[&str] = &[
        ".gitattributes",
        ".gitignore",
        "AGENTS.md",
        "CLAUDE.md",
        "RTK.md",
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "justfile",
        "rust-toolchain.toml",
    ];
    /// `downloads/` and `saves/` are gitignored user data; `.code-advisor/`
    /// and `.superpowers/` are gitignored tool scratch created by hooks.
    const ROOT_DIRS: &[&str] = &[
        ".cargo",
        ".claude",
        ".code-advisor",
        ".codex",
        ".git",
        ".superpowers",
        "crates",
        "scripts",
        "assets",
        "docs",
        "downloads",
        "saves",
        "target",
        "tools",
    ];

    /// Two levels above this crate (`crates/app`).
    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("repo root")
    }

    #[test]
    fn repo_root_holds_only_project_entries() {
        let mut unexpected = Vec::new();
        for entry in fs::read_dir(repo_root()).expect("read repo root") {
            let name = entry.expect("repo root entry").file_name().to_string_lossy().into_owned();
            if !ROOT_FILES.contains(&name.as_str()) && !ROOT_DIRS.contains(&name.as_str()) {
                unexpected.push(name);
            }
        }
        unexpected.sort();
        assert!(
            unexpected.is_empty(),
            "unexpected repo-root entries {unexpected:?}. Move scratch files, logs, \
             launchers, zips and exes out of the repo. If an entry is a real project \
             file, add it to ROOT_FILES or ROOT_DIRS in src/repo_layout.rs."
        );
    }

    #[test]
    fn every_wgsl_file_is_included_by_shader_sources() {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let registry =
            fs::read_to_string(src.join("shader_sources.rs")).expect("read shader_sources.rs");
        let mut unregistered = Vec::new();
        let mut pending = vec![src.clone()];
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(&dir).expect("read src directory") {
                let path = entry.expect("src entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|ext| ext == "wgsl") {
                    let relative = path
                        .strip_prefix(&src)
                        .expect("path under src")
                        .to_string_lossy()
                        .replace('\\', "/");
                    if !registry.contains(&format!("include_str!(\"{relative}\")")) {
                        unregistered.push(relative);
                    }
                }
            }
        }
        unregistered.sort();
        assert!(
            unregistered.is_empty(),
            "WGSL files not included by src/shader_sources.rs (silently unused): {unregistered:?}"
        );
    }
}
