//! Sparse functional copy. Missing entries intentionally have no description.
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum Tier {
    #[default]
    Standard,
    Behavior,
    Mechanic,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Description {
    pub key: String,
    #[serde(default)]
    pub tier: Tier,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub codex: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keyword {
    pub name: String,
    pub definition: String,
}

/// One optional line of colour under an item's stats, keyed by display name.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flavour {
    pub name: String,
    pub line: String,
}

/// One plain line saying what a named trait or modifier means, keyed by display name.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hint {
    pub name: String,
    pub line: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub entries: Vec<Description>,
    pub keywords: Vec<Keyword>,
    #[serde(default)]
    pub flavours: Vec<Flavour>,
    #[serde(default)]
    pub hints: Vec<Hint>,
}

const SOURCE: &str = include_str!("../../../assets/descriptions.ron");
const SHARED_KEYWORDS: [&str; 4] = ["Burning", "Wet", "Charged", "Sealed"];
static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    // Editorial copy is optional presentation, never gameplay authority.
    // Authoring validation (`parse`) stays strict, but a live session keeps
    // every valid line: one flawed entry must not blank the whole catalog.
    let catalog: Catalog = match ron::from_str(SOURCE) {
        Ok(catalog) => catalog,
        Err(error) => {
            eprintln!("[descriptions] Copy omitted: {error}");
            return Catalog::default();
        }
    };
    let errors = validate(&catalog);
    if !errors.is_empty() {
        eprintln!("[descriptions] Skipped flawed lines only: {}", errors.join("; "));
    }
    usable(catalog)
});

/// The catalog minus every entry, keyword and flavour that fails its own
/// checks; the first of any duplicate key wins.
pub fn usable(catalog: Catalog) -> Catalog {
    let mut keys = HashSet::new();
    let entries = catalog.entries.into_iter().filter(|entry| {
        let mut errors = Vec::new();
        check_entry(entry, &mut errors);
        errors.is_empty() && !entry.key.is_empty() && keys.insert(entry.key.clone())
    }).collect();
    let mut names = HashSet::new();
    let keywords = catalog.keywords.into_iter().filter(|keyword| {
        let mut errors = Vec::new();
        check_keyword(keyword, &mut errors);
        errors.is_empty() && names.insert(keyword.name.clone())
    }).collect();
    let mut lines = HashSet::new();
    let flavours = catalog.flavours.into_iter().filter(|flavour| {
        let mut errors = Vec::new();
        check_flavour(flavour, &mut errors);
        errors.is_empty() && !flavour.name.is_empty() && lines.insert(flavour.name.clone())
    }).collect();
    Catalog { entries, keywords, flavours, hints: catalog.hints }
}

/// Shared entry point for runtime loading and the author-facing lint command.
pub fn parse(source: &str) -> Result<Catalog, Vec<String>> {
    let catalog: Catalog = ron::from_str(source).map_err(|error| vec![error.to_string()])?;
    let errors = validate(&catalog);
    if errors.is_empty() { Ok(catalog) } else { Err(errors) }
}

pub fn get(key: &str) -> Option<&'static Description> {
    CATALOG.entries.iter().find(|entry| entry.key == key)
}

/// Flavour for the thing a player sees under this display name, if any.
pub fn flavour(name: &str) -> Option<&'static str> {
    CATALOG.flavours.iter().find(|f| f.name == name).map(|f| f.line.as_str())
}

/// Hints load on their own: a flawed entry elsewhere in the file must not hide
/// these lines, so each is checked individually and bad ones are skipped.
static HINTS: LazyLock<Vec<Hint>> = LazyLock::new(|| {
    let Ok(catalog) = ron::from_str::<Catalog>(SOURCE) else { return Vec::new() };
    catalog.hints.into_iter().filter(|hint| {
        let mut errors = Vec::new();
        check_hint(hint, &mut errors);
        errors.is_empty()
    }).collect()
});

/// Plain explanation of a trait or modifier shown under its display name, if any.
pub fn hint(name: &str) -> Option<&'static str> {
    HINTS.iter().find(|h| h.name == name).map(|h| h.line.as_str())
}

/// Definitions are attached only to keywords actually present in this copy.
pub fn keywords(text: &str) -> impl Iterator<Item = &'static Keyword> + '_ {
    CATALOG.keywords.iter().filter(move |keyword| {
        words(text).any(|word| word == keyword.name)
    })
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|word| !word.is_empty())
}

/// Content validation, not a grammar parser: semantic adjectives and hidden
/// mechanics still require editorial review. Numeric prose is never accepted.
pub fn validate(catalog: &Catalog) -> Vec<String> {
    let mut errors = Vec::new();
    let mut keys = HashSet::new();
    let mut keyword_names = HashSet::new();
    for keyword in &catalog.keywords {
        if !keyword_names.insert(keyword.name.as_str()) {
            errors.push(format!("keyword {}: duplicate definition", keyword.name));
        }
        check_keyword(keyword, &mut errors);
    }
    for shared in SHARED_KEYWORDS {
        if !keyword_names.contains(shared) {
            errors.push(format!("keyword {shared}: missing shared definition"));
        }
    }
    for entry in &catalog.entries {
        let key = entry.key.as_str();
        if key.is_empty() || !keys.insert(key) {
            errors.push(format!("{key}: empty or duplicate entry key"));
        }
        check_entry(entry, &mut errors);
    }
    let mut names = HashSet::new();
    for flavour in &catalog.flavours {
        let name = flavour.name.as_str();
        if name.is_empty() || !names.insert(name) {
            errors.push(format!("flavour {name}: empty or duplicate name"));
        }
        check_flavour(flavour, &mut errors);
    }
    // Hints explain a name in plain words: one short clause, no numbers (check_copy).
    let mut hint_names = HashSet::new();
    for hint in &catalog.hints {
        if !hint_names.insert(hint.name.as_str()) {
            errors.push(format!("hint {}: duplicate name", hint.name));
        }
        check_hint(hint, &mut errors);
    }
    errors
}

fn check_keyword(keyword: &Keyword, errors: &mut Vec<String>) {
    if !SHARED_KEYWORDS.contains(&keyword.name.as_str()) {
        errors.push(format!("keyword {}: unknown shared keyword", keyword.name));
    }
    if keyword.definition.trim().is_empty() || words(&keyword.definition).count() > 50 {
        errors.push(format!("keyword {}: definition must contain 1..50 words", keyword.name));
    }
    check_copy(&keyword.name, &keyword.definition, false, errors);
    check_keywords(&keyword.name, &keyword.definition, errors);
}

fn check_entry(entry: &Description, errors: &mut Vec<String>) {
    let key = entry.key.as_str();
    let limit = match entry.tier {
        Tier::Standard => 0,
        Tier::Behavior => 8,
        Tier::Mechanic => 14,
    };
    if entry.tier == Tier::Standard && (!entry.text.is_empty() || !entry.codex.is_empty()) {
        errors.push(format!("{key}: Standard items must not contain prose"));
    } else if words(&entry.text).count() > limit {
        errors.push(format!("{key}: {:?} description exceeds {limit} words", entry.tier));
    }
    if words(&entry.codex).count() > 50 {
        errors.push(format!("{key}: codex exceeds 50 words"));
    }
    if entry.text.contains(['\n', '\r', ';'])
        || entry.text.trim_end().strip_suffix('.').unwrap_or(entry.text.trim_end())
            .contains(['.', '!', '?'])
    {
        errors.push(format!("{key}: description must be a single clause or sentence"));
    }
    if !entry.text.is_empty() && entry.text.trim() != entry.text {
        errors.push(format!("{key}: remove surrounding description whitespace"));
    }
    if entry.tier == Tier::Mechanic && !entry.text.is_empty() && !entry.text.ends_with('.') {
        errors.push(format!("{key}: Mechanic description must be a sentence"));
    }
    check_copy(key, &entry.text, true, errors);
    check_copy(key, &entry.codex, false, errors);
    check_keywords(key, &entry.text, errors);
    check_keywords(key, &entry.codex, errors);
}

/// Flavour is never mechanical: no numbers (check_copy), no sentence
/// punctuation, under ten words, and none of the banned mood words.
fn check_flavour(flavour: &Flavour, errors: &mut Vec<String>) {
    let name = flavour.name.as_str();
    let count = words(&flavour.line).count();
    if count == 0 || count > 9 {
        errors.push(format!("flavour {name}: line must contain 1..9 words"));
    }
    if flavour.line.contains(['.', '?', ';', '\n', '\r']) || flavour.line.trim() != flavour.line {
        errors.push(format!("flavour {name}: one unpunctuated line"));
    }
    if words(&flavour.line).any(|word| matches!(word.to_ascii_lowercase().as_str(),
        "amazing" | "powerful" | "ultimate" | "epic" | "seamless" | "unleash" | "journey"
        | "adventure" | "immersive" | "explore" | "harness" | "mysterious" | "ancient"
        | "forgotten" | "echo" | "essence" | "resonance" | "mighty" | "legendary")) {
        errors.push(format!("flavour {name}: remove mood words"));
    }
    check_copy(name, &flavour.line, false, errors);
}

fn check_hint(hint: &Hint, errors: &mut Vec<String>) {
    let name = hint.name.as_str();
    if name.is_empty() {
        errors.push("hint: empty name".to_owned());
    }
    let count = words(&hint.line).count();
    if count == 0 || count > 12 {
        errors.push(format!("hint {name}: line must contain 1..12 words"));
    }
    if hint.line.contains([';', '\n', '\r']) || hint.line.trim() != hint.line {
        errors.push(format!("hint {name}: one plain line"));
    }
    check_copy(name, &hint.line, false, errors);
    check_keywords(name, &hint.line, errors);
}

fn check_keywords(key: &str, text: &str, errors: &mut Vec<String>) {
    for word in words(text) {
        for shared in SHARED_KEYWORDS {
            if word.eq_ignore_ascii_case(shared) && word != shared {
                errors.push(format!("{key}: capitalize shared keyword {shared}"));
            }
        }
    }
}

fn check_copy(key: &str, text: &str, functional: bool, errors: &mut Vec<String>) {
    if text.contains('!') {
        errors.push(format!("{key}: remove exclamation marks"));
    }
    if words(text).any(|word| matches!(word.to_ascii_lowercase().as_str(),
        "you" | "your" | "yours" | "yourself" | "you're" | "you'll" | "you've" | "let's")) {
        errors.push(format!("{key}: remove second-person wording"));
    }
    let has_number = text.chars().any(char::is_numeric) || words(text).any(|word| {
        matches!(word.to_ascii_lowercase().as_str(),
            "zero" | "one" | "two" | "three" | "four" | "five" | "six" | "seven"
            | "eight" | "nine" | "ten" | "half" | "quarter" | "double" | "triple")
    });
    if has_number {
        errors.push(format!("{key}: numbers belong in data-derived stats, not prose"));
    }
    if functional && words(text).any(|word| {
        matches!(word.to_ascii_lowercase().as_str(),
            "powerful" | "ancient" | "mighty" | "mysterious" | "amazing" | "legendary")
    }) {
        errors.push(format!("{key}: remove nonfunctional adjectives"));
    }
}
