//! Content overrides from `<save dir>/datapack/`, a data-pack equivalent.
//!
//! Only `recipes.ron` is read. The recipe tables are built once on first use,
//! so [`install`] must run at startup before anything reads a recipe; a later
//! call is refused with a log line and the built-in table stays.
use super::{RecipeRow, RecipeTableRows};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub const DIR: &str = "datapack";
pub const RECIPES_FILE: &str = "recipes.ron";

static PENDING: Mutex<Option<RecipeTableRows>> = Mutex::new(None);
static TAKEN: AtomicBool = AtomicBool::new(false);

/// Reads overrides under `save_dir` and logs which loaded or were refused.
pub fn install(save_dir: &Path) {
    let path = save_dir.join(DIR).join(RECIPES_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => return eprintln!("[datapack] refused {}: {e}", path.display()),
    };
    match parse_recipes(&text) {
        Ok(_) if TAKEN.load(Ordering::Acquire) => {
            eprintln!("[datapack] refused {}: recipes already built", path.display());
        }
        Ok(rows) => {
            let count = rows.hand.len() + rows.bench.len() + rows.crystal_tech.len();
            if let Ok(mut pending) = PENDING.lock() {
                *pending = Some(rows);
                eprintln!("[datapack] loaded {} ({count} recipes)", path.display());
            }
        }
        Err(e) => eprintln!("[datapack] refused {}: {e}", path.display()),
    }
}

/// The override for the recipe table build, once; `None` keeps the built-in file.
pub(super) fn take_recipes() -> Option<RecipeTableRows> {
    TAKEN.store(true, Ordering::Release);
    PENDING.lock().ok().and_then(|mut p| p.take())
}

/// Parses `recipes.ron` text with the built-in row types and checks pattern keys.
pub(super) fn parse_recipes(text: &str) -> Result<RecipeTableRows, String> {
    let rows: RecipeTableRows = ron::from_str(text).map_err(|e| e.to_string())?;
    for row in rows.hand.iter().chain(&rows.bench).chain(&rows.crystal_tech) {
        check(row)?;
    }
    Ok(rows)
}

fn check(row: &RecipeRow) -> Result<(), String> {
    if row.output.count == 0 {
        return Err(format!("{}: output count is 0", row.name));
    }
    if row.pattern.is_empty() && row.ingredients.is_empty() {
        return Err(format!("{}: no pattern or ingredients", row.name));
    }
    for ch in row.pattern.iter().flat_map(|r| r.chars()).filter(|&ch| ch != ' ') {
        if !row.key.iter().any(|(k, _)| *k == ch) {
            return Err(format!("{}: pattern key '{ch}' is not defined", row.name));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_recipes_parse_as_an_override() {
        let rows = parse_recipes(super::super::RECIPES_RON).unwrap();
        assert!(!rows.hand.is_empty() && !rows.bench.is_empty());
    }

    #[test]
    fn broken_overrides_are_refused() {
        assert!(parse_recipes("(hand: [], bench: [").is_err());
        let undefined_key = r##"(hand: [(name: "X", output: (item: Block(Stone), count: 1),
            pattern: ["#"], description: "")], bench: [], crystal_tech: [])"##;
        let err = parse_recipes(undefined_key).err().unwrap_or_default();
        assert!(err.contains("pattern key"), "{err}");
    }

    #[test]
    fn missing_folder_installs_nothing() {
        let dir = std::env::temp_dir().join(format!("voxel-datapack-none-{}", std::process::id()));
        install(&dir);
        assert!(PENDING.lock().unwrap().is_none());
    }
}
