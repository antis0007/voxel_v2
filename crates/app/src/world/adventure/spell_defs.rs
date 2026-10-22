//! The spell table: one row per spell, and the only place a spell's numbers,
//! text, colour, sound, classification and unlock rule are written down.
//!
//! The old per-field `match` functions (`spell_name`, `spell_cost`,
//! `spell_cooldown`, ...) now look rows up here. Adding a spell is one new row,
//! kept sorted by id, plus any new behaviour in the cast and flight code. Tests
//! check that ids are sorted and unique and that every row is complete.
use super::*;

/// What a spell is, for the evaluator and the wand palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Class {
    /// Fires a bolt that flies and impacts.
    Projectile,
    /// A card that changes the next bolt, or the deck's control flow.
    Modifier,
    /// A growth or summon effect delivered by a Touch or a trigger.
    Effect,
    /// Utility casts, delivery cards and the empty slot.
    Other,
}

/// What a wand must be (or what rare magic must be found) to hold a spell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Unlock {
    /// Any wand.
    FirstWand,
    /// A second-tier wand, or any rare-magic progress.
    SecondWandOrRare,
    /// A third-tier wand, or two rare-magic discoveries.
    ThirdWandOrTwoRare,
    /// Only the spell's own rare-magic bit (reliquary spells).
    RareBit,
    /// Never holdable: the empty slot.
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpellDef {
    pub id: u32,
    #[serde(deserialize_with = "leak")]
    pub name: &'static str,
    #[serde(deserialize_with = "leak")]
    pub flavor: &'static str,
    #[serde(deserialize_with = "leak")]
    pub hint: &'static str,
    /// Badge shown on the hover card ("PROJECTILE", "MODIFIER", ...).
    #[serde(deserialize_with = "leak")]
    pub tag: &'static str,
    pub color: [f32; 3],
    pub cost: f32,
    pub cooldown: f32,
    pub cast_delay: f32,
    pub recharge: f32,
    pub spread: f32,
    pub damage: f32,
    pub speed: f32,
    pub audio: (Cue, f32),
    pub class: Class,
    pub unlock: Unlock,
}

/// Shown for ids that are not in the table.
const UNKNOWN_FLAVOR: &str = "A spell. Nobody has written down what it does yet.";
const UNKNOWN_HINT: &str = "Empty wand slot.";

/// Every spell, sorted by id, loaded once from `assets/spells.ron`.
pub static SPELLS: std::sync::LazyLock<Vec<SpellDef>> = std::sync::LazyLock::new(|| {
    ron::from_str(SPELLS_RON).unwrap_or_else(|error| panic!("assets/spells.ron: {error}"))
});
const SPELLS_RON: &str = include_str!("../../../../../assets/spells.ron");
#[cfg(test)]
const SPELLS_HEADER: &str = "// The spell table. One entry per spell, sorted by id; ids 0-95 fit the save format.
// Edit here, not in Rust. `cargo test spells_ron_round_trips` checks the format.
// color is linear RGB; audio is (cue, gain); class and unlock are enum names from spell_defs.rs.
";

/// Table strings live for the whole run; leaking them once keeps `SpellDef` a
/// plain `Copy` row with `&'static str` fields.
fn leak<'de, D: serde::Deserializer<'de>>(d: D) -> Result<&'static str, D::Error> {
    <String as serde::Deserialize>::deserialize(d).map(|s| &*Box::leak(s.into_boxed_str()))
}

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

pub fn spell_flavor(spell: u32) -> &'static str {
    def(spell).map_or(UNKNOWN_FLAVOR, |d| d.flavor)
}

pub fn spell_hint(spell: u32) -> &'static str {
    def(spell).map_or(UNKNOWN_HINT, |d| d.hint)
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
/// missing table value never makes a bolt toothless; the two deliberately weak
/// spam spells (Chainsaw, Bounce orb) are allowed to be below it.
pub fn spell_hit_damage(spell: u32) -> f32 {
    let base = spell_base_damage(spell);
    if matches!(spell, 71 | 76) {
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
        Unlock::RareBit => rare & (1 << spell) != 0,
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
            assert!(!d.name.is_empty() && !d.flavor.is_empty() && !d.tag.is_empty());
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
        for id in [33, 80, 95, 4000] {
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
        let text = ron::ser::to_string_pretty(&*super::SPELLS, ron::ser::PrettyConfig::new()).unwrap().replace('', "");
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
