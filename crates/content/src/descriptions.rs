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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub entries: Vec<Description>,
    pub keywords: Vec<Keyword>,
}

const SOURCE: &str = include_str!("../../../assets/descriptions.ron");
const SHARED_KEYWORDS: [&str; 4] = ["Burning", "Wet", "Charged", "Sealed"];
static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    parse(SOURCE).unwrap_or_else(|errors| panic!("assets/descriptions.ron:\n{}", errors.join("\n")))
});

/// Shared entry point for runtime loading and the author-facing lint command.
pub fn parse(source: &str) -> Result<Catalog, Vec<String>> {
    let catalog: Catalog = ron::from_str(source).map_err(|error| vec![error.to_string()])?;
    let errors = validate(&catalog);
    if errors.is_empty() { Ok(catalog) } else { Err(errors) }
}

pub fn get(key: &str) -> Option<&'static Description> {
    CATALOG.entries.iter().find(|entry| entry.key == key)
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
        if !SHARED_KEYWORDS.contains(&keyword.name.as_str()) {
            errors.push(format!("keyword {}: unknown shared keyword", keyword.name));
        }
        if !keyword_names.insert(keyword.name.as_str()) {
            errors.push(format!("keyword {}: duplicate definition", keyword.name));
        }
        if keyword.definition.trim().is_empty() || words(&keyword.definition).count() > 50 {
            errors.push(format!("keyword {}: definition must contain 1..50 words", keyword.name));
        }
        check_copy(&keyword.name, &keyword.definition, false, &mut errors);
    }
    for shared in SHARED_KEYWORDS {
        if !keyword_names.contains(shared) {
            errors.push(format!("keyword {shared}: missing shared definition"));
        }
    }
    for keyword in &catalog.keywords {
        check_keywords(&keyword.name, &keyword.definition, &mut errors);
    }
    for entry in &catalog.entries {
        let key = entry.key.as_str();
        if key.is_empty() || !keys.insert(key) {
            errors.push(format!("{key}: empty or duplicate entry key"));
        }
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
        check_copy(key, &entry.text, true, &mut errors);
        check_copy(key, &entry.codex, false, &mut errors);
        check_keywords(key, &entry.text, &mut errors);
        check_keywords(key, &entry.codex, &mut errors);
    }
    errors
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
