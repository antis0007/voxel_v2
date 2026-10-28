//! Player-visible text rules (UI_RULES.md), enforced on files that already
//! follow them. Add a file to `CHECKED` once it is clean; the full report
//! over every UI file is `just lint-ui`.
use std::path::Path;

const CHECKED: &[&str] = &[
    "crates/app/src/egui_ui/chip_canvas.rs",
    "crates/app/src/egui_ui/chip_editor.rs",
    "crates/app/src/egui_ui/chip_fab.rs",
    "crates/app/src/egui_ui/chip_panel.rs",
    "crates/app/src/egui_ui/chip_report.rs",
    "crates/app/src/egui_ui/chip_tools.rs",
    "crates/app/src/egui_ui/debug_overlay.rs",
    "crates/app/src/egui_ui/map.rs",
    "crates/app/src/egui_ui/map/minimap.rs",
    "crates/app/src/egui_ui/map/panel.rs",
    "crates/app/src/egui_ui/map/waypoints.rs",
    "crates/app/src/egui_ui/world_rules.rs",
    "crates/app/src/egui_ui/blueprints.rs",
    "crates/app/src/creative/blueprint.rs",
    "crates/app/src/world/blueprint/format.rs",
    "crates/app/src/world/blueprint/library.rs",
    "crates/app/src/world/factory/chips.rs",
    "crates/app/src/world/factory/chips/actions.rs",
    "crates/app/src/world/factory/overview.rs",
    "crates/app/src/world/factory/recipes.rs",
    "crates/app/src/world/undercrown/interact.rs",
    "crates/app/src/world/adventure/reliquaries/access.rs",
    "crates/chip/src/characterize.rs",
    "crates/chip/src/compile.rs",
    "crates/chip/src/fab.rs",
    "crates/chip/src/firmware.rs",
    "crates/chip/src/lot.rs",
    "crates/chip/src/loops.rs",
    "assets/chip_modules.ron",
];

const BANNED: &[&str] = &[
    "amazing", "powerful", "ultimate", "epic", "seamless", "unleash", "journey", "immersive", "explore",
    "harness", "mysterious", "ancient", "forgotten", "echo", "essence", "robust", "leverage", "delve",
    "elevate", "comprehensive", "crucial", "ensure", "streamline", "elegant", "you", "your", "yourself",
];
/// Item and material names that contain a banned word.
const NAMES: &[&str] = &["ancient circuit", "wild essence", "echo spindle", "cinder echo"];
/// Lines whose literals are identifiers or logs, not player text.
const SKIP: &[&str] = &["println!", "eprintln!", "log::", "Id::new", "id_salt", "env::var", "include_str", "#[path", "panic!", "module(", "port(", "id: \""];

fn literals(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('"') {
        let tail = &rest[start + 1..];
        let mut end = None;
        let mut escaped = false;
        for (i, c) in tail.char_indices() {
            match c {
                '\\' if !escaped => escaped = true,
                '"' if !escaped => {
                    end = Some(i);
                    break;
                }
                _ => escaped = false,
            }
        }
        let Some(end) = end else { break };
        out.push(&tail[..end]);
        rest = &tail[end + 1..];
    }
    out
}

fn problems(text: &str) -> Vec<String> {
    let mut lower = text.to_lowercase();
    for name in NAMES {
        lower = lower.replace(name, "");
    }
    // Drop format arguments: `{name}` is code, not text.
    let mut plain = String::new();
    let mut depth = 0;
    for c in lower.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for word in plain.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '\'')) {
        if BANNED.contains(&word) {
            found.push(format!("word '{word}'"));
        }
        if word.contains('_') && word.chars().any(char::is_alphabetic) {
            found.push(format!("dev text '{word}'"));
        }
    }
    if plain.replace("!=", "").contains('!') {
        found.push("exclamation".into());
    }
    if plain.contains("::") {
        found.push("dev text '::'".into());
    }
    found
}

#[test]
fn ui_text_rules_hold_in_checked_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut failures = Vec::new();
    for file in CHECKED {
        let text = std::fs::read_to_string(root.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        for (n, line) in text.lines().enumerate() {
            if line.contains("#[cfg(test)]") {
                break;
            }
            let code = line.trim_start();
            if code.starts_with("//") || SKIP.iter().any(|s| line.contains(s)) {
                continue;
            }
            for lit in literals(line).into_iter().filter(|l| l.contains(' ') || l.chars().next().is_some_and(char::is_uppercase)) {
                for p in problems(lit) {
                    failures.push(format!("{file}:{}: {p}: \"{lit}\"", n + 1));
                }
            }
        }
    }
    assert!(failures.is_empty(), "UI_RULES.md violations:\n{}", failures.join("\n"));
}

#[test]
fn the_text_check_catches_slop() {
    assert!(!problems("Unleash your seamless journey!").is_empty());
    assert!(!problems("Press F to open the dev_panel").is_empty());
    assert!(problems("Output Full: 3/12 Casings").is_empty());
    assert!(problems("1 crystal + 2 biomass → 1 wild essence").is_empty());
    assert!(problems("Wake {busy_us:.0} µs > {} ms").is_empty());
}
