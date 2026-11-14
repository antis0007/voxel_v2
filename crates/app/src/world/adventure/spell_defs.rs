//! The spell table owns base numbers, names, colour, sound, class and unlocks.
//! Functional copy lives in the shared, validated description catalog.
//!
//! The old per-field `match` functions (`spell_name`, `spell_cost`,
//! `spell_cooldown`, ...) now look rows up here. Adding a spell is one new row,
//! kept sorted by id, plus any new behaviour in the cast and flight code. Tests
//! check that ids are sorted and unique and that every row is complete.
use super::*;
pub mod card;

pub use engine_content::spells::{Class, Palette, Unlock};
pub type SpellDef = engine_content::spells::SpellDef<Cue, &'static str>;

/// Shown for ids that are not in the table.
const UNKNOWN_HINT: &str = "Empty wand slot.";

/// A validated draft table that replaces the compiled one. Only the dev
/// panel's headless Wand Lab sets it, before anything reads `SPELLS`.
pub static SPELLS_OVERRIDE: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

/// Every spell, sorted by id, loaded once from `assets/spells.ron`.
pub static SPELLS: std::sync::LazyLock<Vec<SpellDef>> = std::sync::LazyLock::new(|| {
    engine_content::spells::parse::<Cue>(SPELLS_OVERRIDE.get().copied().unwrap_or(SPELLS_RON))
        .unwrap_or_else(|errors| panic!("assets/spells.ron: {errors:?}"))
        .into_iter()
        .map(engine_content::spells::SpellDef::into_runtime)
        .collect()
});
const SPELLS_RON: &str = include_str!("../../../../../assets/spells.ron");
#[cfg(test)]
const SPELLS_HEADER: &str = "// The spell table. One entry per spell, sorted by id.
// Edit here, not in Rust. `cargo test spells_ron_round_trips` checks the format.
// color is linear RGB; audio is (cue, gain); class and unlock are enum names from spell_defs.rs.
";

/// The row for a spell id, if it has one.
pub fn def(id: u32) -> Option<&'static SpellDef> {
    SPELLS
        .binary_search_by_key(&id, |d| d.id)
        .ok()
        .map(|i| &SPELLS[i])
}

pub fn spell_name(spell: u32) -> &'static str {
    def(spell).map_or("Unknown spell", |d| d.name)
}

pub fn spell_hint(spell: u32) -> &'static str {
    card::description(spell).map_or(UNKNOWN_HINT, |entry| entry.text.as_str())
}

pub fn spell_kind_tag(spell: u32) -> &'static str {
    def(spell).map_or("PROJECTILE", |d| d.tag)
}

pub fn spell_color(spell: u32) -> [f32; 3] {
    def(spell).map_or(PALETTE_AMBER, |d| d.color)
}

pub fn spell_cost(spell: u32) -> f32 {
    def(spell).map_or(12.0, |d| d.cost)
}

pub fn spell_cooldown(spell: u32) -> f32 {
    def(spell).map_or(0.28, |d| d.cooldown)
}

pub fn spell_cast_delay_delta(spell: u32) -> f32 {
    def(spell).map_or(0.0, |d| d.cast_delay)
}

pub fn spell_recharge_delta(spell: u32) -> f32 {
    def(spell).map_or(0.0, |d| d.recharge)
}

pub fn spell_spread_delta(spell: u32) -> f32 {
    def(spell).map_or(0.0, |d| d.spread)
}

pub fn spell_base_damage(spell: u32) -> f32 {
    def(spell).map_or(0.0, |d| d.damage)
}

/// Damage a bolt deals on a direct hit. Older spells keep a floor of 22 so a
/// missing table value never makes a bolt toothless; the deliberately weak
/// spam and utility bolts (Chainsaw, Bounce orb, Lodestone, Dig bolt, Scatter
/// shot pellets) are allowed to be below it.
pub fn spell_hit_damage(spell: u32) -> f32 {
    let base = spell_base_damage(spell);
    if matches!(spell, 71 | 76 | 84 | 86 | 87) {
        base
    } else {
        base.max(22.0)
    }
}

pub fn spell_base_speed(spell: u32) -> f32 {
    def(spell).map_or(26.0, |d| d.speed)
}

/// Sound family and pitch for a spell's cast and bounce.
pub fn spell_audio(spell: u32) -> (Cue, f32) {
    def(spell).map_or((Cue::Spark, 1.0), |d| d.audio)
}

pub fn spell_class(spell: u32) -> Class {
    def(spell).map_or(Class::Other, |d| d.class)
}

/// Can a wand of this tier hold the spell, given what rare magic is found?
pub fn spell_available(spell: u32, wand: u32, rare: u32) -> bool {
    if wand == 0 || spell == wands::EMPTY || spell > MAX_SPELL_ID {
        return false;
    }
    let Some(d) = def(spell) else {
        return false;
    };
    match d.unlock {
        Unlock::FirstWand => wand >= 1,
        Unlock::SecondWandOrRare => wand >= 2 || rare != 0,
        Unlock::ThirdWandOrTwoRare => wand >= 3 || rare.count_ones() >= 2,
        Unlock::ThirdWand => wand >= 3,
        Unlock::RareBit => rare.checked_shr(spell).is_some_and(|bits| bits & 1 != 0),
        Unlock::Never => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_sorted_by_id_with_no_duplicates() {
        for pair in SPELLS.windows(2) {
            assert!(
                pair[0].id < pair[1].id,
                "{} ({}) must come before {} ({})",
                pair[0].name,
                pair[0].id,
                pair[1].name,
                pair[1].id
            );
        }
    }

    #[test]
    fn every_row_is_inside_the_id_space_and_complete() {
        for d in SPELLS.iter() {
            assert!(d.id <= MAX_SPELL_ID, "{} is above MAX_SPELL_ID", d.name);
            assert_eq!(def(d.id).map(|row| row.id), Some(d.id), "lookup of {}", d.name);
            assert!(!d.name.is_empty() && !d.tag.is_empty());
            assert!(d.cost.is_finite() && d.cost >= 0.0, "{} cost", d.name);
            assert!(d.cooldown.is_finite() && d.cooldown >= 0.0, "{} cooldown", d.name);
            assert!(d.speed.is_finite() && d.speed > 0.0, "{} speed", d.name);
            assert!(d.damage.is_finite() && d.damage >= 0.0, "{} damage", d.name);
            assert!(d.audio.1 > 0.0 && d.audio.1.is_finite(), "{} pitch", d.name);
            assert!(
                d.color.iter().all(|c| (0.0..=1.0).contains(c)),
                "{} colour",
                d.name
            );
        }
    }

    #[test]
    fn only_the_empty_slot_is_unobtainable() {
        for d in SPELLS.iter() {
            assert_eq!(
                d.unlock == Unlock::Never,
                d.id == wands::EMPTY,
                "{} ({}) unlock {:?}",
                d.name,
                d.id,
                d.unlock
            );
        }
    }

    #[test]
    fn unknown_ids_fall_back_to_the_documented_defaults() {
        for id in (0..=MAX_SPELL_ID + 1).filter(|id| def(*id).is_none()).chain([4000]) {
            assert!(def(id).is_none());
            assert_eq!(spell_name(id), "Unknown spell");
            assert_eq!(spell_class(id), Class::Other);
            assert_eq!(spell_cost(id), 12.0);
            assert_eq!(spell_cooldown(id), 0.28);
            assert_eq!(spell_base_speed(id), 26.0);
            assert!(!spell_available(id, 3, !0));
        }
    }

    #[test]
    fn classes_agree_with_the_wand_predicates() {
        for d in SPELLS.iter() {
            assert_eq!(wands::projectile(d.id), d.class == Class::Projectile, "{}", d.name);
            assert_eq!(wands::is_modifier(d.id), d.class == Class::Modifier, "{}", d.name);
            assert_eq!(wands::is_effect(d.id), d.class == Class::Effect, "{}", d.name);
        }
    }
}

#[cfg(test)]
mod table_file {
    /// Serialising the loaded table reproduces the file byte for byte, so the
    /// file is the single source of truth and hand edits keep the format.
    #[test]
    fn spells_ron_round_trips() {
        // ron emits the platform newline; compare without carriage returns.
        let text = ron::ser::to_string_pretty(&*super::SPELLS, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        // Skip the comment header: the table starts at the first line that is `[`.
        let file = super::SPELLS_RON.replace('\r', "");
        let body = match file.find("\n[\n") {
            Some(i) => &file[i + 1..],
            None => file.as_str(),
        };
        let (a, b) = (text.trim_end(), body.trim_end());
        if a != b {
            let i = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
            let lo = i.saturating_sub(60);
            panic!(
                "assets/spells.ron differs from the table at byte {i}; run `cargo test dump_spells_ron -- --ignored` to rewrite it.\n table: {:?}\n  file: {:?}",
                &a[lo..(i + 60).min(a.len())],
                &b[lo..(i + 60).min(b.len())]
            );
        }
    }

    /// Rewrites assets/spells.ron from the loaded table (keeps the header).
    #[test]
    #[ignore = "writes assets/spells.ron"]
    fn dump_spells_ron() {
        let text = ron::ser::to_string_pretty(&*super::SPELLS, ron::ser::PrettyConfig::new()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/spells.ron");
        std::fs::write(path, format!("{}{text}", super::SPELLS_HEADER)).unwrap();
    }
}
