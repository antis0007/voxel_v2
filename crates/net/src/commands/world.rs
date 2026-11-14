//! Grammar for commands that change the world or a player's body.
//!
//! Item, creature, rule and status names stay strings: the application owns
//! those tables, validates them and words the reply.

use super::TimeOp;

/// Ticks per day; tick 0 is sunrise, as on the clock players know.
pub const DAY_TICKS: f64 = 24_000.0;
const SUNRISE: f64 = 0.25;
/// Largest coordinate, in metres, a teleport accepts.
pub const MAX_COORD: f32 = 1_000_000.0;
pub const MAX_GIVE: u32 = 1024;
pub const BORDER_METRES: std::ops::RangeInclusive<u32> = 16..=1_000_000;
const MAX_NAME: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weather {
    Clear,
    Rain,
    Thunder,
}

impl Weather {
    pub fn label(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Rain => "rain",
            Self::Thunder => "thunder",
        }
    }
}

/// One teleport axis in metres: absolute, or an offset from the player (`~`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coord {
    pub relative: bool,
    pub value: f32,
}

impl Coord {
    pub fn resolve(self, current: f32) -> f32 {
        if self.relative { current + self.value } else { self.value }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StatusOp {
    Give { name: String, seconds: f32, level: u8 },
    Clear,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorldChange {
    TeleportTo([Coord; 3]),
    Weather(Weather),
    /// `None` reads the rule.
    Gamerule { rule: String, value: Option<bool> },
    Give { item: String, count: u32 },
    Summon(String),
    /// Metres; 0 is off.
    WorldBorder(u32),
    Status(StatusOp),
}

/// Clock position of a tick count, as a fraction of the day.
pub fn day_fraction(ticks: f64) -> f32 {
    (SUNRISE + ticks / DAY_TICKS).rem_euclid(1.0) as f32
}

pub fn ticks(time_of_day: f32) -> f32 {
    ((f64::from(time_of_day) - SUNRISE).rem_euclid(1.0) * DAY_TICKS) as f32
}

pub(super) fn time(rest: &[&str]) -> Result<TimeOp, String> {
    let op = rest.first().map(|v| v.to_lowercase());
    match (op.as_deref(), rest.len()) {
        (None | Some("query"), 0 | 1) => Ok(TimeOp::Query),
        (Some(op @ ("set" | "add")), 2) => {
            let named = match rest[1].to_lowercase().as_str() {
                "day" => Some(1_000.0),
                "noon" => Some(6_000.0),
                "night" => Some(13_000.0),
                "midnight" => Some(18_000.0),
                _ => None,
            };
            let ticks = match named {
                Some(ticks) if op == "set" => ticks,
                _ => rest[1]
                    .parse::<f64>()
                    .ok()
                    .filter(|t| t.is_finite() && t.abs() <= 1e9)
                    .ok_or("time takes day, noon, night, midnight or a tick count")?,
            };
            Ok(if op == "set" {
                TimeOp::Set(day_fraction(ticks))
            } else {
                TimeOp::Add((ticks / DAY_TICKS).rem_euclid(1.0) as f32)
            })
        }
        _ => Err("time takes set, add or query".into()),
    }
}

/// Three axes such as `10 ~ ~-4`, or `None` if any word is not one.
pub(super) fn coords(words: &[&str]) -> Option<[Coord; 3]> {
    let [x, y, z] = words else { return None };
    let axis = |word: &str| {
        let (relative, number) = match word.strip_prefix('~') {
            Some("") => return Some(Coord { relative: true, value: 0.0 }),
            Some(rest) => (true, rest),
            None => (false, word),
        };
        let value = number.parse::<f32>().ok().filter(|v| v.is_finite() && v.abs() <= MAX_COORD)?;
        Some(Coord { relative, value })
    };
    Some([axis(x)?, axis(y)?, axis(z)?])
}

pub(super) fn weather(rest: &[&str]) -> Result<WorldChange, String> {
    let weather = match rest {
        [word] => match word.to_lowercase().as_str() {
            "clear" | "sun" => Weather::Clear,
            "rain" => Weather::Rain,
            "thunder" | "storm" => Weather::Thunder,
            _ => return Err("weather takes clear, rain or thunder".into()),
        },
        _ => return Err("weather takes clear, rain or thunder".into()),
    };
    Ok(WorldChange::Weather(weather))
}

pub(super) fn gamerule(rest: &[&str]) -> Result<WorldChange, String> {
    let (rule, value) = match rest {
        [rule] => (rule, None),
        [rule, value] => {
            let value = match value.to_lowercase().as_str() {
                "true" | "on" => true,
                "false" | "off" => false,
                _ => return Err("rule values are true or false".into()),
            };
            (rule, Some(value))
        }
        _ => return Err("gamerule takes a rule and an optional true or false".into()),
    };
    Ok(WorldChange::Gamerule { rule: name(rule)?, value })
}

/// `<item words> [count]`; the caller has already taken any player name.
pub(super) fn give(rest: &[&str]) -> Result<WorldChange, String> {
    let (words, count) = match rest.split_last() {
        Some((last, words)) if !words.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) => {
            (words, last.parse::<u32>().unwrap_or(0))
        }
        _ => (rest, 1),
    };
    if words.is_empty() {
        return Err("give takes an item and an optional count".into());
    }
    if !(1..=MAX_GIVE).contains(&count) {
        return Err(format!("count is 1 to {MAX_GIVE}"));
    }
    Ok(WorldChange::Give { item: name(&words.join(" "))?, count })
}

pub(super) fn summon(rest: &[&str]) -> Result<WorldChange, String> {
    if rest.is_empty() {
        return Err("summon takes a creature name".into());
    }
    Ok(WorldChange::Summon(name(&rest.join(" "))?))
}

pub(super) fn border(rest: &[&str]) -> Result<WorldChange, String> {
    let range = || format!("world border is off or {} to {} m", BORDER_METRES.start(), BORDER_METRES.end());
    let metres = match rest {
        [word] if word.eq_ignore_ascii_case("off") || *word == "0" => 0,
        [word] => word.parse::<u32>().ok().filter(|m| BORDER_METRES.contains(m)).ok_or_else(range)?,
        _ => return Err(range()),
    };
    Ok(WorldChange::WorldBorder(metres))
}

pub(super) fn status(rest: &[&str]) -> Result<WorldChange, String> {
    let usage = "effect takes give <name> <seconds> [level] or clear";
    let op = match rest {
        [clear] if clear.eq_ignore_ascii_case("clear") => StatusOp::Clear,
        [give, kind, seconds, level @ ..] if give.eq_ignore_ascii_case("give") && level.len() <= 1 => {
            let seconds = seconds
                .parse::<f32>()
                .ok()
                .filter(|s| s.is_finite() && *s > 0.0)
                .ok_or("effect seconds must be above 0")?;
            let level = match level.first() {
                Some(level) => level.parse::<u8>().ok().filter(|l| *l >= 1).ok_or("effect level is 1 or more")?,
                None => 1,
            };
            StatusOp::Give { name: name(kind)?, seconds, level }
        }
        _ => return Err(usage.into()),
    };
    Ok(WorldChange::Status(op))
}

/// A table key typed by a player: bounded and printable, so replies can echo it.
fn name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_NAME || trimmed.chars().any(char::is_control) {
        return Err("that name is empty, too long or not printable".into());
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_times_and_ticks_map_onto_the_day() {
        assert_eq!(time(&["set", "noon"]), Ok(TimeOp::Set(0.5)));
        assert_eq!(time(&["set", "0"]), Ok(TimeOp::Set(0.25)));
        assert_eq!(time(&["set", "18000"]), Ok(TimeOp::Set(0.0)));
        assert_eq!(time(&["add", "6000"]), Ok(TimeOp::Add(0.25)));
        assert_eq!(time(&["add", "-6000"]), Ok(TimeOp::Add(0.75)));
        assert_eq!(time(&[]), Ok(TimeOp::Query));
        assert!(time(&["add", "noon"]).is_err(), "named times only set");
        assert!(time(&["set", "dusk"]).is_err());
        assert!(time(&["set", "1e12"]).is_err());
        assert!((ticks(0.5) - 6_000.0).abs() < 1e-2);
        // Large tick counts keep their position in the day.
        assert!((ticks(day_fraction(41_666.0 * DAY_TICKS + 6_000.0)) - 6_000.0).abs() < 1.0);
    }

    #[test]
    fn coordinates_accept_relative_axes() {
        let at = coords(&["10", "~", "~-4.5"]).unwrap();
        assert_eq!(at[0], Coord { relative: false, value: 10.0 });
        assert_eq!(at[1].resolve(70.0), 70.0);
        assert_eq!(at[2].resolve(1.0), -3.5);
        assert!(coords(&["1", "2"]).is_none());
        assert!(coords(&["1", "2", "x"]).is_none());
        assert!(coords(&["1", "2", "NaN"]).is_none());
        assert!(coords(&["1", "2", "2000000"]).is_none());
    }

    #[test]
    fn give_takes_multi_word_items_and_a_count() {
        assert_eq!(give(&["steel", "sword", "2"]), Ok(WorldChange::Give { item: "steel sword".into(), count: 2 }));
        assert_eq!(give(&["stone"]), Ok(WorldChange::Give { item: "stone".into(), count: 1 }));
        assert!(give(&["stone", "0"]).is_err());
        assert!(give(&["stone", "1025"]).is_err());
        assert!(give(&[]).is_err());
    }

    #[test]
    fn rules_borders_and_effects_validate_their_values() {
        assert_eq!(
            gamerule(&["keepInventory", "true"]),
            Ok(WorldChange::Gamerule { rule: "keepInventory".into(), value: Some(true) })
        );
        assert_eq!(gamerule(&["pvp"]), Ok(WorldChange::Gamerule { rule: "pvp".into(), value: None }));
        assert!(gamerule(&["pvp", "maybe"]).is_err());
        assert_eq!(border(&["off"]), Ok(WorldChange::WorldBorder(0)));
        assert_eq!(border(&["200"]), Ok(WorldChange::WorldBorder(200)));
        assert!(border(&["3"]).is_err());
        assert_eq!(
            status(&["give", "speed", "30", "2"]),
            Ok(WorldChange::Status(StatusOp::Give { name: "speed".into(), seconds: 30.0, level: 2 }))
        );
        assert_eq!(status(&["clear"]), Ok(WorldChange::Status(StatusOp::Clear)));
        assert!(status(&["give", "speed", "-1"]).is_err());
        assert!(status(&["give", "speed", "30", "0"]).is_err());
        assert_eq!(weather(&["Rain"]), Ok(WorldChange::Weather(Weather::Rain)));
        assert!(weather(&["snow"]).is_err());
        assert!(summon(&[]).is_err());
    }
}
