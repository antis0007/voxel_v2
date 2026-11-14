//! Server administration: the settings file and the access lists.
//!
//! Both are plain `key=value` text so an operator can edit them by hand, keep them
//! in version control, or copy them between worlds. Parsing never fails hard: a
//! bad line is reported and the previous value stands, so one typo cannot lock
//! everyone out.

use crate::commands::GameMode;
use crate::protocol::MAX_PLAYERS;
use crate::social;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub fn label(self) -> &'static str {
        match self {
            Self::Peaceful => "peaceful",
            Self::Easy => "easy",
            Self::Normal => "normal",
            Self::Hard => "hard",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "peaceful" => Some(Self::Peaceful),
            "easy" => Some(Self::Easy),
            "normal" => Some(Self::Normal),
            "hard" => Some(Self::Hard),
            _ => None,
        }
    }
}

/// The server settings file.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerSettings {
    pub motd: String,
    pub max_players: usize,
    pub gamemode: GameMode,
    pub difficulty: Difficulty,
    pub pvp: bool,
    pub whitelist: bool,
    /// Radius in metres around world spawn where building is refused.
    pub spawn_protection: u32,
    /// Chunk view distance handed to clients.
    pub view_distance: u32,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            motd: "A shared world".to_owned(),
            max_players: MAX_PLAYERS,
            gamemode: GameMode::Survival,
            difficulty: Difficulty::Normal,
            pvp: true,
            whitelist: false,
            spawn_protection: 16,
            view_distance: 6,
        }
    }
}

impl ServerSettings {
    /// Parse a settings file. Returns the settings and every line that was ignored.
    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let mut settings = Self::default();
        let mut problems = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                problems.push(format!("line {}: expected key=value", index + 1));
                continue;
            };
            if let Err(problem) = settings.apply(key.trim(), value.trim()) {
                problems.push(format!("line {}: {problem}", index + 1));
            }
        }
        (settings, problems)
    }

    fn apply(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key.to_ascii_lowercase().as_str() {
            "motd" => {
                let text = social::sanitize_chat(value).map_err(str::to_owned)?;
                self.motd = text;
            }
            "max-players" => {
                let parsed: usize = value.parse().map_err(|_| "max-players is not a number")?;
                if !(1..=MAX_PLAYERS).contains(&parsed) {
                    return Err(format!("max-players must be 1 to {MAX_PLAYERS}"));
                }
                self.max_players = parsed;
            }
            "gamemode" => {
                self.gamemode = match value.to_ascii_lowercase().as_str() {
                    "creative" | "c" => GameMode::Creative,
                    "survival" | "s" => GameMode::Survival,
                    _ => return Err("gamemode is survival or creative".into()),
                };
            }
            "difficulty" => {
                self.difficulty =
                    Difficulty::parse(value).ok_or("difficulty is peaceful, easy, normal or hard")?;
            }
            "pvp" => self.pvp = parse_bool(value)?,
            "whitelist" => self.whitelist = parse_bool(value)?,
            "spawn-protection" => {
                self.spawn_protection =
                    value.parse().map_err(|_| "spawn-protection is not a number")?;
            }
            "view-distance" => {
                let parsed: u32 = value.parse().map_err(|_| "view-distance is not a number")?;
                if !(2..=32).contains(&parsed) {
                    return Err("view-distance must be 2 to 32".into());
                }
                self.view_distance = parsed;
            }
            other => return Err(format!("unknown setting {other}")),
        }
        Ok(())
    }

    pub fn to_text(&self) -> String {
        format!(
            "# Server settings. Edit with care; the host reloads this on start.\nmotd={}\nmax-players={}\ngamemode={}\ndifficulty={}\npvp={}\nwhitelist={}\nspawn-protection={}\nview-distance={}\n",
            self.motd,
            self.max_players,
            self.gamemode.label(),
            self.difficulty.label(),
            self.pvp,
            self.whitelist,
            self.spawn_protection,
            self.view_distance,
        )
    }
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Ok(true),
        "false" | "off" | "no" | "0" => Ok(false),
        _ => Err(format!("{value} is not true or false")),
    }
}

/// Why a player was refused, if they were.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admission {
    Allowed,
    NotWhitelisted,
    Banned(String),
}

/// Operators, the whitelist and the ban list, keyed by lower-case name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccessList {
    /// Operator name to the profile token that holds it. Names are self-chosen,
    /// so rights follow the token; `None` is an entry not yet bound.
    ops: BTreeMap<String, Option<String>>,
    /// Whitelisted name to the key that claimed it. A typed name proves nothing,
    /// so the first proven key binds an entry and later keys are refused.
    whitelist: BTreeMap<String, Option<String>>,
    bans: BTreeMap<String, String>,
    /// Key fingerprints, which a renamed player cannot shed.
    key_whitelist: BTreeSet<String>,
    key_bans: BTreeMap<String, String>,
}

impl AccessList {
    /// Parse an access file. Unknown lines are reported and skipped.
    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let mut list = Self::default();
        let mut problems = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                problems.push(format!("line {}: expected key=value", index + 1));
                continue;
            };
            let mut parts = value.splitn(2, '|');
            let name = parts.next().unwrap_or_default().trim();
            let extra = parts.next().map(str::trim);
            let reason = extra.unwrap_or("banned");
            let problem = match key.trim().to_ascii_lowercase().as_str() {
                "op" => list.set_op(name, true).err().or_else(|| {
                    let profile = extra.filter(|p| !p.is_empty())?;
                    list.bind_op(name, profile).err()
                }),
                "white" | "whitelist" => list.whitelist_add(name).err().or_else(|| {
                    let key = extra.filter(|k| !k.is_empty())?;
                    list.bind_whitelist(name, key).err()
                }),
                "ban" => list.ban(name, reason).err(),
                "white-key" => list.whitelist_key(name, true).err(),
                "ban-key" => list.ban_key(name, reason).err(),
                other => Some(format!("unknown list {other}")),
            };
            if let Some(problem) = problem {
                problems.push(format!("line {}: {problem}", index + 1));
            }
        }
        (list, problems)
    }

    pub fn to_text(&self) -> String {
        let mut out = String::from("# Access lists. Names are case-insensitive.\n");
        for (name, profile) in &self.ops {
            match profile {
                Some(profile) => out.push_str(&format!("op={name}|{profile}\n")),
                None => out.push_str(&format!("op={name}\n")),
            }
        }
        for (name, key) in &self.whitelist {
            match key {
                Some(key) => out.push_str(&format!("white={name}|{key}\n")),
                None => out.push_str(&format!("white={name}\n")),
            }
        }
        for (name, reason) in &self.bans {
            out.push_str(&format!("ban={name}|{reason}\n"));
        }
        for key in &self.key_whitelist {
            out.push_str(&format!("white-key={key}\n"));
        }
        for (key, reason) in &self.key_bans {
            out.push_str(&format!("ban-key={key}|{reason}\n"));
        }
        out
    }

    /// Whether `name` is reserved for an operator. Rights need [`Self::op_holds`].
    pub fn is_op(&self, name: &str) -> bool {
        self.ops.contains_key(&normalise(name))
    }

    /// Whether the player using `name` with `profile` has operator rights.
    pub fn op_holds(&self, name: &str, profile: &str) -> bool {
        matches!(self.ops.get(&normalise(name)), Some(Some(bound)) if bound.eq_ignore_ascii_case(profile))
    }

    /// The token bound to an operator name: `None` if not an op, `Some(None)` if unbound.
    pub fn op_binding(&self, name: &str) -> Option<Option<&str>> {
        self.ops.get(&normalise(name)).map(Option::as_deref)
    }

    pub fn set_op(&mut self, name: &str, op: bool) -> Result<(), String> {
        let name = normalise(&checked(name)?);
        if op {
            self.ops.entry(name).or_default();
        } else {
            self.ops.remove(&name);
        }
        Ok(())
    }

    /// Ties an operator name to the key fingerprint that may use it.
    pub fn bind_op(&mut self, name: &str, profile: &str) -> Result<(), String> {
        if !crate::identity::is_fingerprint(profile) {
            return Err("operator key must be 32 hex digits".into());
        }
        let slot = self.ops.get_mut(&normalise(&checked(name)?)).ok_or("not an operator")?;
        *slot = Some(profile.to_ascii_lowercase());
        Ok(())
    }

    pub fn whitelist_add(&mut self, name: &str) -> Result<(), String> {
        let name = normalise(&checked(name)?);
        self.whitelist.entry(name).or_default();
        Ok(())
    }

    pub fn whitelist_remove(&mut self, name: &str) -> Result<bool, String> {
        Ok(self.whitelist.remove(&normalise(&checked(name)?)).is_some())
    }

    /// Whether `name` has a whitelist entry, bound or not.
    pub fn is_whitelisted(&self, name: &str) -> bool {
        self.whitelist.contains_key(&normalise(name))
    }

    /// Ties a whitelisted name to the key that may use it. An entry already
    /// bound to another key is not moved.
    pub fn bind_whitelist(&mut self, name: &str, key: &str) -> Result<(), String> {
        let key = checked_key(key)?;
        let slot = self.whitelist.get_mut(&normalise(&checked(name)?)).ok_or("not whitelisted")?;
        match slot {
            Some(bound) if *bound != key => Err("name is bound to another key".into()),
            _ => {
                *slot = Some(key);
                Ok(())
            }
        }
    }

    pub fn ban(&mut self, name: &str, reason: &str) -> Result<(), String> {
        let name = normalise(&checked(name)?);
        let reason = reason.trim();
        let reason = if reason.is_empty() { "banned" } else { reason };
        self.bans.insert(
            name,
            reason.chars().take(crate::commands::MAX_REASON).collect(),
        );
        Ok(())
    }

    pub fn pardon(&mut self, name: &str) -> Result<bool, String> {
        Ok(self.bans.remove(&normalise(&checked(name)?)).is_some())
    }

    pub fn ban_reason(&self, name: &str) -> Option<&str> {
        self.bans.get(&normalise(name)).map(String::as_str)
    }

    pub fn bans(&self) -> impl Iterator<Item = (&str, &str)> {
        self.bans.iter().map(|(name, reason)| (name.as_str(), reason.as_str()))
    }

    pub fn ops(&self) -> impl Iterator<Item = &str> {
        self.ops.keys().map(String::as_str)
    }

    pub fn whitelist(&self) -> impl Iterator<Item = &str> {
        self.whitelist.keys().map(String::as_str)
    }

    /// Admission for a name and, when proven, its key fingerprint.
    ///
    /// Bans always win, then the whitelist when it is switched on. With the
    /// whitelist on, a peer that proved no key is refused, and a name entry
    /// admits only its bound key or, while unbound, any key: the session then
    /// binds it with [`Self::bind_whitelist`].
    pub fn admits(&self, name: &str, key: Option<&str>, whitelist_on: bool) -> Admission {
        let key = key.map(str::to_ascii_lowercase);
        let key_ban = key.as_ref().and_then(|k| self.key_bans.get(k));
        if let Some(reason) = self.ban_reason(name).or(key_ban.map(String::as_str)) {
            return Admission::Banned(reason.to_owned());
        }
        if !whitelist_on {
            return Admission::Allowed;
        }
        let Some(key) = key else { return Admission::NotWhitelisted };
        let by_key = self.key_whitelist.contains(&key);
        let by_name = match self.whitelist.get(&normalise(name)) {
            Some(Some(bound)) => *bound == key,
            Some(None) => true,
            None => false,
        };
        if by_key || by_name { Admission::Allowed } else { Admission::NotWhitelisted }
    }

    /// Adds or removes a key fingerprint on the whitelist; `Ok(true)` if it changed.
    pub fn whitelist_key(&mut self, key: &str, add: bool) -> Result<bool, String> {
        let key = checked_key(key)?;
        Ok(if add { self.key_whitelist.insert(key) } else { self.key_whitelist.remove(&key) })
    }

    pub fn ban_key(&mut self, key: &str, reason: &str) -> Result<(), String> {
        let key = checked_key(key)?;
        let reason = reason.trim();
        let reason = if reason.is_empty() { "banned" } else { reason };
        self.key_bans.insert(key, reason.chars().take(crate::commands::MAX_REASON).collect());
        Ok(())
    }

    pub fn pardon_key(&mut self, key: &str) -> Result<bool, String> {
        Ok(self.key_bans.remove(&checked_key(key)?).is_some())
    }
}

fn normalise(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn checked_key(key: &str) -> Result<String, String> {
    let key = key.trim();
    if !crate::identity::is_fingerprint(key) {
        return Err("key must be 32 hex digits".into());
    }
    Ok(key.to_ascii_lowercase())
}

fn checked(name: &str) -> Result<String, String> {
    social::sanitize_name(name).map_err(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_the_file() {
        let settings = ServerSettings {
            motd: "Quiet valley".to_owned(),
            max_players: 8,
            gamemode: GameMode::Creative,
            difficulty: Difficulty::Hard,
            pvp: false,
            whitelist: true,
            spawn_protection: 24,
            view_distance: 10,
        };
        let (parsed, problems) = ServerSettings::parse(&settings.to_text());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(parsed, settings);
    }

    #[test]
    fn bad_setting_lines_are_reported_and_defaults_stand() {
        let text = "max-players=99\ndifficulty=extreme\nnonsense=1\nnot-a-line\n";
        let (parsed, problems) = ServerSettings::parse(text);
        assert_eq!(parsed.max_players, MAX_PLAYERS);
        assert_eq!(parsed.difficulty, Difficulty::Normal);
        assert_eq!(problems.len(), 4, "{problems:?}");
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let (parsed, problems) = ServerSettings::parse("# note\n\n// other\npvp=off\n");
        assert!(problems.is_empty());
        assert!(!parsed.pvp);
    }

    #[test]
    fn access_lists_round_trip_and_stay_sorted() {
        let mut list = AccessList::default();
        list.set_op("Bo", true).unwrap();
        list.set_op("ana", true).unwrap();
        list.whitelist_add("Cy").unwrap();
        list.whitelist_add("Dee").unwrap();
        list.bind_whitelist("Dee", &"a".repeat(32)).unwrap();
        list.whitelist_key(&"b".repeat(32), true).unwrap();
        list.ban_key(&"c".repeat(32), "alt").unwrap();
        list.ban("Bad Actor", "grief").unwrap();
        let text = list.to_text();
        let (parsed, problems) = AccessList::parse(&text);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(parsed, list);
        assert!(text.find("ana").unwrap() < text.find("bo").unwrap());
    }

    #[test]
    fn bans_win_over_the_whitelist() {
        let mut list = AccessList::default();
        list.whitelist_add("Ana").unwrap();
        list.ban("Ana", "grief").unwrap();
        assert_eq!(list.admits("Ana", None, true), Admission::Banned("grief".to_owned()));
        list.pardon("ANA").unwrap();
        assert_eq!(list.admits("ana", Some(&"0".repeat(32)), true), Admission::Allowed);
    }

    #[test]
    fn the_whitelist_only_gates_when_switched_on() {
        let mut list = AccessList::default();
        list.whitelist_add("Ana").unwrap();
        assert_eq!(list.admits("Bo", None, true), Admission::NotWhitelisted);
        assert_eq!(list.admits("Bo", None, false), Admission::Allowed);
        let (first, second) = ("0".repeat(32), "1".repeat(32));
        assert_eq!(list.admits("ANA", None, true), Admission::NotWhitelisted, "a typed name proves nothing");
        assert_eq!(list.admits("ANA", Some(&first), true), Admission::Allowed, "unbound entry");
        list.bind_whitelist("Ana", &first).unwrap();
        assert_eq!(list.admits("ana", Some(&second), true), Admission::NotWhitelisted);
        assert!(list.bind_whitelist("Ana", &second).is_err(), "a bound entry does not move");
        list.whitelist_key(&second, true).unwrap();
        assert_eq!(list.admits("Bo", Some(&second), true), Admission::Allowed, "key entry");
    }

    #[test]
    fn invalid_names_are_refused_when_editing_lists() {
        let mut list = AccessList::default();
        assert!(list.whitelist_add("bad\nname").is_err());
        assert!(list.ban("", "x").is_err());
        assert!(list.set_op("seventeen_chars_x", true).unwrap_err().contains("longer"));
    }
}

