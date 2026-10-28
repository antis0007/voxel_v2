//! Repo-layout guards. A scratch file at the repo root or a WGSL file that no
//! program includes is picked up by nobody, but agents still read and edit it.
//! These tests fail loudly instead. See CLAUDE.md, "Don't".

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    /// Everything allowed at the repo root. Add a real project entry here on
    /// purpose; scratch files, logs, zips, patches and launchers stay outside.
    const ROOT_FILES: &[&str] = &[
        ".gitattributes",
        ".gitignore",
        "CLAUDE.md",
        "Cargo.toml",
        "Cargo.lock",
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

    /// Root dirs that hold user data or tool scratch stay gitignored.
    #[test]
    fn untracked_root_dirs_are_gitignored() {
        let ignore = fs::read_to_string(repo_root().join(".gitignore")).expect("read .gitignore");
        let lines: Vec<&str> = ignore.lines().map(str::trim).collect();
        let unignored: Vec<&str> = ["saves", "downloads", "target", ".code-advisor", ".superpowers"]
            .into_iter()
            .filter(|dir| !lines.iter().any(|line| line.trim_end_matches('/') == *dir))
            .collect();
        assert!(unignored.is_empty(), "root dirs {unignored:?} must be listed in .gitignore (saves hold world data)");
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

    /// GPU tests get their adapter from `engine_gpu::test_adapter()`, so
    /// `just test-gpu` controls backend selection and fails without a GPU.
    /// Only that helper, the windowed renderer and the menu art export (a runtime tool, not a test) may
    /// request an adapter.
    #[test]
    fn only_engine_gpu_and_the_renderer_request_an_adapter() {
        let root = repo_root();
        let allowed = [
            root.join("crates/gpu/src/lib.rs"),
            root.join("crates/app/src/render.rs"),
            root.join("crates/app/src/egui_ui/menu_capture.rs"),
        ];
        let mut stack = vec![root.join("crates")];
        let mut offenders = Vec::new();
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).expect("read crates dir") {
                let path = entry.expect("crates entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && !allowed.contains(&path)
                    && fs::read_to_string(&path).expect("read source").contains(concat!("request", "_adapter("))
                {
                    offenders.push(path.strip_prefix(&root).unwrap_or(&path).display().to_string());
                }
            }
        }
        assert!(offenders.is_empty(), "use engine_gpu::test_adapter() instead of request_adapter in {offenders:?}");
    }

    /// The cellular simulation reads the packed material table and nothing else,
    /// so no rule can depend on which block it is (docs/adr/0007-bricks-and-scale.md).
    /// Dev-dependencies only drive tests (the lockstep proof uses the netcode link
    /// simulator), so only the sections that link into the library are checked.
    #[test]
    fn engine_sim_depends_on_no_engine_crate_but_core() {
        let manifest = fs::read_to_string(repo_root().join("crates/sim/Cargo.toml")).expect("sim manifest");
        let mut linked = false;
        let mut deps = Vec::new();
        for line in manifest.lines().map(str::trim) {
            if line.starts_with('[') {
                linked = matches!(line, "[dependencies]" | "[build-dependencies]")
                    || line.starts_with("[target.") && line.ends_with(".dependencies]");
            } else if linked
                && line.starts_with("engine_")
                && !line.starts_with("engine_core")
                && !line.starts_with("engine_sim")
            {
                deps.push(line);
            }
        }
        assert!(deps.is_empty(), "engine_sim may depend only on engine_core, found {deps:?}");
    }

    /// Every repo path and `just` command named in the agent-facing docs
    /// exists. Paths are tokens starting with `crates/`, `assets/`, `docs/`,
    /// `scripts/`, `tools/` or `.claude/`; a token containing `*`, `<` or `{`
    /// is a pattern and is skipped.
    #[test]
    fn agent_docs_name_only_existing_paths_and_commands() {
        let root = repo_root();
        let mut files = vec![root.join("CLAUDE.md")];
        for dir in ["docs", "docs/adr", ".claude/skills"] {
            for entry in fs::read_dir(root.join(dir)).expect("docs dir") {
                let path = entry.expect("doc entry").path();
                if path.is_dir() {
                    let skill = path.join("SKILL.md");
                    if skill.is_file() {
                        files.push(skill);
                    }
                } else if path.extension().is_some_and(|e| e == "md")
                    // Roadmaps name recipes and paths that do not exist yet.
                    && path.file_name().is_some_and(|n| !["AUDIT.md", "PLAN2.md", "ENGINE_CORE.md", "MASTER_PLAN.md"].contains(&n.to_str().unwrap_or("")))
                    // So do the proposed `*_V2.md` specifications.
                    && path.file_name().is_some_and(|n| !n.to_str().unwrap_or("").ends_with("_V2.md"))
                {
                    files.push(path);
                }
            }
        }
        let justfile = fs::read_to_string(root.join("justfile")).expect("justfile");
        let recipes: Vec<String> = justfile
            .lines()
            .filter(|l| !l.starts_with([' ', '#', '\t']) && l.contains(':') && !l.starts_with("set "))
            .filter_map(|l| l.split([' ', ':']).next().map(str::to_owned))
            .filter(|n| !n.is_empty() && !n.contains('='))
            .collect();
        let mut missing = Vec::new();
        for file in files {
            let text = fs::read_to_string(&file).expect("read doc");
            let name = file.strip_prefix(&root).unwrap_or(&file).display().to_string();
            for raw in text.split(|c: char| c.is_whitespace() || "`()[],;:\"'<>|".contains(c)) {
                let token = raw.trim_end_matches(['.', '*']);
                let is_path = ["crates/", "assets/", "docs/", "scripts/", "tools/", ".claude/"]
                    .iter()
                    .any(|p| token.starts_with(p));
                if is_path && !token.contains(['*', '{', '…']) && !root.join(token).exists() {
                    missing.push(format!("{name}: path `{token}`"));
                }
            }
            let (mut open, mut fenced) = (false, false);
            for line in text.lines() {
                if let Some(tag) = line.trim_start().strip_prefix("```") {
                    open = !open;
                    // Text and markdown fences hold prose, not commands.
                    fenced = open && !["text", "md", "markdown"].contains(&tag.trim());
                    continue;
                }
                let mut rest = line;
                while let Some(i) = rest.find("just ") {
                    // A command opens inline code or a fenced code line; prose "just" does neither.
                    let command = rest[..i].ends_with('`')
                        || (fenced && line.len() - rest.len() + i == line.len() - line.trim_start().len());
                    rest = &rest[i + 5..];
                    if !command {
                        continue;
                    }
                    let cmd: String =
                        rest.chars().take_while(|c| c.is_ascii_lowercase() || *c == '_' || *c == '-').collect();
                    let cmd = cmd.trim_end_matches('-');
                    if !cmd.is_empty() && !recipes.iter().any(|r| r == cmd) {
                        missing.push(format!("{name}: `just {cmd}` is not a recipe"));
                    }
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "docs name things that do not exist:\n{}", missing.join("\n"));
    }

}
