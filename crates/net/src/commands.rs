//! Chat commands in the shape players expect from a hosted server.
//!
//! The crate owns parsing, permission checks and user-facing wording. Anything
//! that touches the world comes back as an [`Effect`] for the application to
//! apply, so this module stays independent of the renderer and the save format.

use crate::protocol::PlayerId;
use crate::social;

mod world;
pub use world::{Coord, DAY_TICKS, StatusOp, Weather, WorldChange, day_fraction, ticks};

pub const MAX_REASON: usize = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Survival => "survival",
            Self::Creative => "creative",
            Self::Adventure => "adventure",
            Self::Spectator => "spectator",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimeOp {
    Set(f32),
    Add(f32),
    Query,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Help,
    List,
    Msg { to: String, text: String },
    Me(String),
    Name(String),
    Gamemode(GameMode),
    GamemodeOf { mode: GameMode, to: String },
    Kick { to: String, reason: String },
    Time(TimeOp),
    Seed,
    Tp { to: String },
    Op { to: String },
    Deop { to: String },
    Whitelist { action: WhitelistAction, name: Option<String> },
    Ban { to: String, reason: String },
    Pardon { to: String },
    Banlist,
    SaveAll,
    Stop,
    /// `target` names another player; `None` is the one who typed it.
    World { target: Option<String>, change: WorldChange },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhitelistAction {
    Add,
    Remove,
    On,
    Off,
    List,
}

impl Command {
    /// Commands that change what other players can do stay with the host.
    fn needs_op(&self) -> bool {
        match self {
            Self::Gamemode(_) | Self::GamemodeOf { .. } | Self::Kick { .. } | Self::Tp { .. } | Self::Op { .. }
            | Self::Deop { .. } | Self::Whitelist { .. } | Self::Ban { .. }
            | Self::Pardon { .. } | Self::Banlist | Self::SaveAll | Self::Stop
            | Self::World { .. } => true,
            Self::Time(TimeOp::Query) => false,
            Self::Time(_) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Gamemode { player: PlayerId, mode: GameMode },
    Kick { player: PlayerId, reason: String },
    Teleport { player: PlayerId, to: PlayerId },
    SetTime(f32),
    SetOp { player: PlayerId, op: bool },
    /// Apply through `HostSession::rename`, which runs the admission checks.
    Rename { player: PlayerId, name: String },
    Whitelist { name: String, add: bool },
    SetWhitelist(bool),
    Ban { name: String, reason: String },
    Pardon { name: String },
    Save,
    Stop,
    /// Applied and answered by the application, which owns the tables it names.
    World { actor: PlayerId, target: PlayerId, change: WorldChange },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// Said to every player.
    pub broadcast: Vec<String>,
    /// Said only to the player who typed the command.
    pub private: Vec<String>,
    /// Said only to the named player; never relayed to anyone else.
    pub direct: Vec<(PlayerId, String)>,
    pub effects: Vec<Effect>,
}

/// What the command layer needs to know about the running session.
pub trait ServerView {
    fn players(&self) -> Vec<(PlayerId, String)>;
    fn is_op(&self, player: PlayerId) -> bool;
    fn gamemode(&self, player: PlayerId) -> GameMode;
    fn time_of_day(&self) -> f32;
    fn seed(&self) -> u64;

    /// Case-insensitive name lookup, so `/msg ana` reaches `Ana`.
    fn resolve(&self, name: &str) -> Option<(PlayerId, String)> {
        let wanted = name.to_lowercase();
        self.players()
            .into_iter()
            .find(|(_, candidate)| candidate.to_lowercase() == wanted)
    }

    fn name_of(&self, player: PlayerId) -> String {
        self.players()
            .into_iter()
            .find(|(id, _)| *id == player)
            .map(|(_, name)| name)
            .unwrap_or_default()
    }

    /// Ban entries as `(name, reason)`. Defaults to empty for a private session.
    fn banned(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// Whitelisted names. Defaults to empty for a private session.
    fn whitelist(&self) -> Vec<String> {
        Vec::new()
    }

    fn whitelist_enabled(&self) -> bool {
        false
    }

    /// Online play keeps one creative flag per player.
    fn supports(&self, mode: GameMode) -> bool {
        matches!(mode, GameMode::Survival | GameMode::Creative)
    }
}

pub fn parse(line: &str) -> Result<Command, String> {
    parse_for(line, &[])
}

/// Leading words naming a player, and how many words that took.
///
/// Names may contain spaces (`Ana (2)`), so the longest run of words matching a
/// name in `names` wins; otherwise the first word is the target.
fn target(rest: &[&str], names: &[String]) -> Option<(String, usize)> {
    let first = rest.first()?;
    Some(match named(rest, names) {
        Some(n) => (rest[..n].join(" "), n),
        None => (first.to_string(), 1),
    })
}

/// Words in the longest leading run that is an online name.
fn named(rest: &[&str], names: &[String]) -> Option<usize> {
    (1..=rest.len()).rev().find(|&n| {
        let candidate = rest[..n].join(" ");
        names.iter().any(|name| name.eq_ignore_ascii_case(&candidate))
    })
}

/// [`parse`] with the online player names, so targets with spaces resolve.
pub fn parse_for(line: &str, names: &[String]) -> Result<Command, String> {
    let body = line.trim().strip_prefix('/').ok_or("commands start with /")?;
    let mut parts = body.split_whitespace();
    let head = parts.next().ok_or("no command given")?.to_lowercase();
    let rest: Vec<&str> = parts.collect();
    let joined = rest.join(" ");
    match head.as_str() {
        "help" | "?" => Ok(Command::Help),
        "list" | "who" => Ok(Command::List),
        "seed" => Ok(Command::Seed),
        "me" => {
            let text = social::sanitize_chat(&joined).map_err(str::to_owned)?;
            Ok(Command::Me(text))
        }
        "msg" | "tell" | "w" => {
            let (to, used) = target(&rest, names).ok_or("who should the message go to?")?;
            let text = social::sanitize_chat(&rest[used..].join(" "))
                .map_err(|_| "the message is empty or too long")?;
            Ok(Command::Msg { to, text })
        }
        "name" | "nick" => {
            let wanted = social::sanitize_name(&joined).map_err(str::to_owned)?;
            Ok(Command::Name(wanted))
        }
        "gamemode" | "gm" => {
            let mode = match rest.first().map(|v| v.to_lowercase()).as_deref() {
                Some("survival" | "s" | "0") => GameMode::Survival,
                Some("creative" | "c" | "1") => GameMode::Creative,
                Some("adventure" | "a" | "2") => GameMode::Adventure,
                Some("spectator" | "sp" | "3") => GameMode::Spectator,
                _ => return Err("gamemode takes survival, creative, adventure or spectator".into()),
            };
            // The whole remainder is the name, as for `/tp`.
            match rest[1..].join(" ") {
                to if to.is_empty() => Ok(Command::Gamemode(mode)),
                to => Ok(Command::GamemodeOf { mode, to }),
            }
        }
        "kick" => {
            let (to, used) = target(&rest, names).ok_or("who should be kicked?")?;
            let given = rest[used..].join(" ");
            let reason = if given.is_empty() { "kicked by the host".to_owned() } else { given };
            if reason.len() > MAX_REASON {
                return Err("the kick reason is too long".into());
            }
            Ok(Command::Kick { to, reason })
        }
        "time" | "daylight" => Ok(Command::Time(world::time(&rest)?)),
        "tp" | "teleport" => {
            // The whole remainder is the name, so `Ana (2)` never resolves to `Ana`.
            if joined.is_empty() {
                return Err("who should be teleported to?".into());
            }
            // A whole-line name wins, so a player called `1 2 3` stays reachable.
            let split = rest.len().saturating_sub(3);
            match world::coords(&rest[split..]) {
                Some(at) if !names.iter().any(|n| n.eq_ignore_ascii_case(&joined)) => {
                    let who = rest[..split].join(" ");
                    Ok(Command::World { target: (!who.is_empty()).then_some(who), change: WorldChange::TeleportTo(at) })
                }
                _ => Ok(Command::Tp { to: joined }),
            }
        }
        "weather" => Ok(Command::World { target: None, change: world::weather(&rest)? }),
        "gamerule" => Ok(Command::World { target: None, change: world::gamerule(&rest)? }),
        "summon" => Ok(Command::World { target: None, change: world::summon(&rest)? }),
        "worldborder" => Ok(Command::World { target: None, change: world::border(&rest)? }),
        "effect" => Ok(Command::World { target: None, change: world::status(&rest)? }),
        "give" => {
            // `/give <player> <item>` when the leading words name someone online.
            let used = named(&rest, names).filter(|&n| n < rest.len()).unwrap_or(0);
            let target = (used > 0).then(|| rest[..used].join(" "));
            Ok(Command::World { target, change: world::give(&rest[used..])? })
        }
        "op" if joined.is_empty() => Err("who should become an operator?".into()),
        "op" => Ok(Command::Op { to: joined }),
        "deop" if joined.is_empty() => Err("who should lose operator?".into()),
        "deop" => Ok(Command::Deop { to: joined }),
        "whitelist" | "wl" => {
            let action = match rest.first().map(|v| v.to_lowercase()).as_deref() {
                Some("add") => WhitelistAction::Add,
                Some("remove") | Some("rm") => WhitelistAction::Remove,
                Some("on") => WhitelistAction::On,
                Some("off") => WhitelistAction::Off,
                None | Some("list") => WhitelistAction::List,
                Some(other) => return Err(format!("whitelist does not take {other}")),
            };
            let name = match action {
                WhitelistAction::Add | WhitelistAction::Remove => {
                    if rest.len() < 2 {
                        return Err("which name?".into());
                    }
                    Some(social::sanitize_name(&rest[1..].join(" ")).map_err(str::to_owned)?)
                }
                _ => None,
            };
            Ok(Command::Whitelist { action, name })
        }
        "ban" => {
            let (to, used) = target(&rest, names).ok_or("who should be banned?")?;
            let given = rest[used..].join(" ");
            let reason = if given.is_empty() { "banned".to_owned() } else { given };
            if reason.len() > MAX_REASON {
                return Err("the ban reason is too long".into());
            }
            social::sanitize_name(&to).map_err(str::to_owned)?;
            Ok(Command::Ban { to, reason })
        }
        "pardon" | "unban" => {
            if joined.is_empty() {
                return Err("who should be pardoned?".into());
            }
            let to = social::sanitize_name(&joined).map_err(str::to_owned)?;
            Ok(Command::Pardon { to })
        }
        "banlist" | "bans" => Ok(Command::Banlist),
        "save-all" | "save" => Ok(Command::SaveAll),
        "stop" | "shutdown" => Ok(Command::Stop),
        other => Err(format!("there is no command called {other}")),
    }
}

pub fn execute(actor: PlayerId, command: &Command, view: &dyn ServerView) -> Outcome {
    let mut outcome = Outcome::default();
    if command.needs_op() && !view.is_op(actor) {
        outcome.private.push("only an operator can run that".to_owned());
        return outcome;
    }
    let actor_name = view.name_of(actor);
    match command {
        Command::Help => {
            outcome.private.push(
                "/list /msg <player> <text> /me <action> /name <name> /seed /time query"
                    .to_owned(),
            );
            if view.is_op(actor) {
                outcome.private.push(
                    "/gamemode <mode> [player] /time <set|add> <day|noon|night|midnight|ticks> /tp <player> | [player] <x y z> (~ is relative)"
                        .to_owned(),
                );
                outcome.private.push(
                    "/weather <clear|rain|thunder> /gamerule <rule> [true|false] /give [player] <item> [count] /summon <creature> /worldborder <metres|off> /effect <give <name> <seconds> [level]|clear>"
                        .to_owned(),
                );
                outcome.private.push(
                    "/kick <player> [reason] /op <player> /deop <player> /whitelist <add|remove|on|off|list> [name] /ban <player> [reason] /pardon <player> /banlist /save-all /stop"
                        .to_owned(),
                );
            }
        }
        Command::List => {
            let players = view.players();
            let roster = players
                .iter()
                .map(|(id, name)| format!("{name} ({})", view.gamemode(*id).label()))
                .collect::<Vec<_>>()
                .join(", ");
            outcome.private.push(format!(
                "{} of {} playing: {roster}",
                players.len(),
                crate::protocol::MAX_PLAYERS
            ));
        }
        Command::Msg { to, text } => match view.resolve(to) {
            Some((target, name)) => {
                outcome.private.push(format!("to {name}: {text}"));
                if target != actor {
                    outcome.direct.push((target, format!("from {actor_name}: {text}")));
                }
            }
            None => outcome.private.push(format!("no player called {to}")),
        },
        Command::Me(action) => outcome.broadcast.push(format!("* {actor_name} {action}")),
        Command::Name(wanted) => match social::sanitize_name(wanted) {
            Ok(name) => outcome.effects.push(Effect::Rename { player: actor, name }),
            Err(reason) => outcome.private.push(reason.to_owned()),
        },
        Command::Gamemode(mode) if !view.supports(*mode) => {
            outcome.private.push(format!("{} is not available in this session", mode.label()));
        }
        Command::Gamemode(mode) => {
            outcome.effects.push(Effect::Gamemode { player: actor, mode: *mode });
            outcome.broadcast.push(format!("{actor_name} set their gamemode to {}", mode.label()));
        }
        Command::GamemodeOf { mode, .. } if !view.supports(*mode) => {
            outcome.private.push(format!("{} is not available in this session", mode.label()));
        }
        Command::GamemodeOf { mode, to } => match view.resolve(to) {
            Some((target, name)) => {
                outcome.effects.push(Effect::Gamemode { player: target, mode: *mode });
                outcome.broadcast.push(format!("{actor_name} set the gamemode of {name} to {}", mode.label()));
            }
            None => outcome.private.push(format!("no player called {to}")),
        },
        Command::Kick { to, reason } => match view.resolve(to) {
            Some((target, name)) => {
                outcome.effects.push(Effect::Kick { player: target, reason: reason.clone() });
                outcome.broadcast.push(format!("{name} was kicked: {reason}"));
            }
            None => outcome.private.push(format!("no player called {to}")),
        },
        Command::Time(TimeOp::Query) => {
            outcome.private.push(format!("time is {:.0} ticks", ticks(view.time_of_day())));
        }
        Command::Time(TimeOp::Set(value)) => {
            outcome.effects.push(Effect::SetTime(*value));
            outcome.broadcast.push(format!("time set to {:.0} ticks", ticks(*value)));
        }
        Command::Time(TimeOp::Add(value)) => {
            let next = (view.time_of_day() + value).rem_euclid(1.0);
            outcome.effects.push(Effect::SetTime(next));
            outcome.broadcast.push(format!("time moved to {:.0} ticks", ticks(next)));
        }
        Command::Seed => outcome.private.push(format!("seed is {}", view.seed())),
        Command::Tp { to } => match view.resolve(to) {
            Some((target, name)) if target != actor => {
                outcome.effects.push(Effect::Teleport { player: actor, to: target });
                outcome.broadcast.push(format!("{actor_name} teleported to {name}"));
            }
            Some(_) => outcome.private.push("you are already there".to_owned()),
            None => outcome.private.push(format!("no player called {to}")),
        },
        Command::Op { to } => match view.resolve(to) {
            Some((target, name)) => {
                outcome.effects.push(Effect::SetOp { player: target, op: true });
                outcome.broadcast.push(format!("{name} is now an operator"));
            }
            None => outcome.private.push(format!("no player called {to}")),
        },
        Command::Deop { to } => match view.resolve(to) {
            Some((target, name)) => {
                outcome.effects.push(Effect::SetOp { player: target, op: false });
                outcome.broadcast.push(format!("{name} is no longer an operator"));
            }
            None => outcome.private.push(format!("no player called {to}")),
        },
        Command::Whitelist { action, name } => match action {
            WhitelistAction::Add | WhitelistAction::Remove => {
                let add = *action == WhitelistAction::Add;
                if let Some(name) = name {
                    outcome.effects.push(Effect::Whitelist { name: name.clone(), add });
                    outcome.broadcast.push(if add {
                        format!("{name} added to the whitelist")
                    } else {
                        format!("{name} removed from the whitelist")
                    });
                }
            }
            WhitelistAction::On | WhitelistAction::Off => {
                let on = *action == WhitelistAction::On;
                outcome.effects.push(Effect::SetWhitelist(on));
                outcome.broadcast.push(if on {
                    "whitelist is on".to_owned()
                } else {
                    "whitelist is off".to_owned()
                });
            }
            WhitelistAction::List => {
                let names = view.whitelist();
                if !view.whitelist_enabled() {
                    outcome.private.push("whitelist is off".to_owned());
                }
                if names.is_empty() {
                    outcome.private.push("no names on the whitelist".to_owned());
                } else {
                    outcome.private.push(format!("whitelist: {}", names.join(", ")));
                }
            }
        },
        Command::Ban { to, reason } => {
            outcome.effects.push(Effect::Ban { name: to.clone(), reason: reason.clone() });
            outcome.broadcast.push(format!("{to} was banned: {reason}"));
        }
        Command::Pardon { to } => {
            outcome.effects.push(Effect::Pardon { name: to.clone() });
            outcome.broadcast.push(format!("{to} was pardoned"));
        }
        Command::Banlist => {
            let bans = view.banned();
            if bans.is_empty() {
                outcome.private.push("no players are banned".to_owned());
            } else {
                let listed = bans
                    .iter()
                    .map(|(name, reason)| format!("{name} ({reason})"))
                    .collect::<Vec<_>>()
                    .join(", ");
                outcome.private.push(format!("{} banned: {listed}", bans.len()));
            }
        }
        Command::SaveAll => {
            outcome.effects.push(Effect::Save);
            outcome.broadcast.push("world saved".to_owned());
        }
        Command::Stop => {
            outcome.effects.push(Effect::Stop);
            outcome.broadcast.push("server stopping".to_owned());
        }
        Command::World { target, change } => {
            let resolved = match target {
                None => Some(actor),
                Some(name) => view.resolve(name).map(|(id, _)| id),
            };
            match resolved {
                Some(target) => outcome.effects.push(Effect::World { actor, target, change: change.clone() }),
                None => outcome.private.push(format!("no player called {}", target.as_deref().unwrap_or_default())),
            }
        }
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        players: Vec<(PlayerId, String)>,
        ops: Vec<PlayerId>,
        mode: GameMode,
        time: f32,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                players: vec![
                    (1, "Ana".to_owned()),
                    (2, "Bo".to_owned()),
                    (4, "Cy".to_owned()),
                ],
                ops: vec![1],
                mode: GameMode::Survival,
                time: 0.25,
            }
        }
    }

    impl ServerView for Fake {
        fn players(&self) -> Vec<(PlayerId, String)> {
            self.players.clone()
        }
        fn is_op(&self, player: PlayerId) -> bool {
            self.ops.contains(&player)
        }
        fn gamemode(&self, _player: PlayerId) -> GameMode {
            self.mode
        }
        fn time_of_day(&self) -> f32 {
            self.time
        }
        fn seed(&self) -> u64 {
            2738958700
        }
        fn banned(&self) -> Vec<(String, String)> {
            vec![("bo".to_owned(), "grief".to_owned())]
        }
        fn whitelist(&self) -> Vec<String> {
            vec!["ana".to_owned()]
        }
        fn whitelist_enabled(&self) -> bool {
            true
        }
    }

    #[test]
    fn parses_the_common_commands() {
        assert_eq!(parse("/help"), Ok(Command::Help));
        assert_eq!(parse("/list"), Ok(Command::List));
        assert_eq!(
            parse("/msg Ana good run"),
            Ok(Command::Msg { to: "Ana".into(), text: "good run".into() })
        );
        assert_eq!(parse("/gm c"), Ok(Command::Gamemode(GameMode::Creative)));
        assert_eq!(
            parse("/kick Bo spam"),
            Ok(Command::Kick { to: "Bo".into(), reason: "spam".into() })
        );
        assert_eq!(parse("/time set noon"), Ok(Command::Time(TimeOp::Set(0.5))));
        assert_eq!(parse("/time"), Ok(Command::Time(TimeOp::Query)));
        assert_eq!(parse("/seed"), Ok(Command::Seed));
        assert!(parse("/nope").is_err());
        assert!(parse("list").is_err());
        assert!(parse("/gamemode").is_err());
        assert!(parse("/time set dusk").is_err());
    }

    #[test]
    fn targets_with_spaces_resolve_to_the_whole_name() {
        let names = vec!["Ana".to_owned(), "Ana (2)".to_owned()];
        assert_eq!(parse_for("/op Ana (2)", &names), Ok(Command::Op { to: "Ana (2)".into() }));
        assert_eq!(parse("/op Ana (2)"), Ok(Command::Op { to: "Ana (2)".into() }));
        assert_eq!(
            parse_for("/msg ana (2) hello there", &names),
            Ok(Command::Msg { to: "ana (2)".into(), text: "hello there".into() })
        );
        assert_eq!(
            parse_for("/kick Ana (2) spam", &names),
            Ok(Command::Kick { to: "Ana (2)".into(), reason: "spam".into() })
        );
        assert_eq!(
            parse_for("/msg Ana hi", &names),
            Ok(Command::Msg { to: "Ana".into(), text: "hi".into() })
        );
        assert!(parse("/op").is_err());
    }

    #[test]
    fn operators_can_change_the_session() {
        let view = Fake::new();
        let outcome = execute(1, &Command::Gamemode(GameMode::Creative), &view);
        assert_eq!(
            outcome.effects,
            vec![Effect::Gamemode { player: 1, mode: GameMode::Creative }]
        );
    }

    #[test]
    fn players_without_operator_are_refused() {
        let view = Fake::new();
        let outcome = execute(2, &Command::Kick { to: "Ana".into(), reason: "x".into() }, &view);
        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.private, vec!["only an operator can run that"]);
    }

    #[test]
    fn list_reports_count_and_gamemodes() {
        let view = Fake::new();
        let outcome = execute(2, &Command::List, &view);
        assert!(
            outcome.private[0].starts_with("3 of 16 playing: Ana (survival)"),
            "{}",
            outcome.private[0]
        );
    }

    #[test]
    fn whispers_reach_the_named_player_case_insensitively() {
        let view = Fake::new();
        let outcome = execute(1, &Command::Msg { to: "bo".into(), text: "hi".into() }, &view);
        assert_eq!(outcome.private, vec!["to Bo: hi"]);
        assert_eq!(outcome.direct, vec![(2, "from Ana: hi".to_owned())]);
        assert!(outcome.broadcast.is_empty(), "a whisper must not reach other players");
    }

    #[test]
    fn time_add_wraps_inside_the_day() {
        let view = Fake::new();
        let outcome = execute(1, &Command::Time(TimeOp::Add(0.9)), &view);
        match outcome.effects.as_slice() {
            [Effect::SetTime(value)] => assert!((value - 0.15).abs() < 1e-4, "{value}"),
            other => panic!("unexpected effects {other:?}"),
        }
    }

    #[test]
    fn teleporting_to_yourself_is_refused() {
        let view = Fake::new();
        let outcome = execute(1, &Command::Tp { to: "Ana".into() }, &view);
        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.private, vec!["you are already there"]);
    }

    #[test]
    fn renaming_validates_the_request() {
        let view = Fake::new();
        let ok = execute(2, &Command::Name("Bo".into()), &view);
        assert_eq!(ok.effects, vec![Effect::Rename { player: 2, name: "Bo".into() }]);
        let bad = execute(2, &Command::Name("bad\nname".into()), &view);
        assert!(bad.effects.is_empty());
    }

    #[test]
    fn admin_commands_parse() {
        assert_eq!(
            parse("/whitelist add Ana"),
            Ok(Command::Whitelist { action: WhitelistAction::Add, name: Some("Ana".into()) })
        );
        assert_eq!(
            parse("/wl off"),
            Ok(Command::Whitelist { action: WhitelistAction::Off, name: None })
        );
        assert_eq!(
            parse("/whitelist"),
            Ok(Command::Whitelist { action: WhitelistAction::List, name: None })
        );
        assert_eq!(
            parse("/ban Bo grief"),
            Ok(Command::Ban { to: "Bo".into(), reason: "grief".into() })
        );
        assert_eq!(parse("/pardon Bo"), Ok(Command::Pardon { to: "Bo".into() }));
        assert_eq!(parse("/banlist"), Ok(Command::Banlist));
        assert_eq!(parse("/save-all"), Ok(Command::SaveAll));
        assert_eq!(parse("/stop"), Ok(Command::Stop));
        assert!(parse("/whitelist add").is_err(), "add needs a name");
        assert!(parse("/ban bad@name").is_err(), "the name alphabet is enforced");
    }

    #[test]
    fn whitelist_and_ban_effects_reach_the_host() {
        let view = Fake::new();
        let add = execute(
            1,
            &Command::Whitelist { action: WhitelistAction::Add, name: Some("Cy".into()) },
            &view,
        );
        assert_eq!(add.effects, vec![Effect::Whitelist { name: "Cy".into(), add: true }]);

        let on = execute(1, &Command::Whitelist { action: WhitelistAction::On, name: None }, &view);
        assert_eq!(on.effects, vec![Effect::SetWhitelist(true)]);

        let ban = execute(1, &Command::Ban { to: "Cy".into(), reason: "spam".into() }, &view);
        assert_eq!(
            ban.effects,
            vec![Effect::Ban { name: "Cy".into(), reason: "spam".into() }]
        );

        let pardon = execute(1, &Command::Pardon { to: "Cy".into() }, &view);
        assert_eq!(pardon.effects, vec![Effect::Pardon { name: "Cy".into() }]);

        let save = execute(1, &Command::SaveAll, &view);
        assert_eq!(save.effects, vec![Effect::Save]);
        let stop = execute(1, &Command::Stop, &view);
        assert_eq!(stop.effects, vec![Effect::Stop]);
    }

    #[test]
    fn listings_read_from_the_session() {
        let view = Fake::new();
        let bans = execute(1, &Command::Banlist, &view);
        assert_eq!(bans.private, vec!["1 banned: bo (grief)"]);
        let names = execute(
            1,
            &Command::Whitelist { action: WhitelistAction::List, name: None },
            &view,
        );
        assert_eq!(names.private, vec!["whitelist: ana"]);
    }

    #[test]
    fn teleport_reads_coordinates_unless_the_line_is_a_name() {
        let names = vec!["Ana".to_owned(), "1 2 3".to_owned()];
        let at = |x: f32, relative: bool| Coord { relative, value: x };
        assert_eq!(
            parse_for("/tp 10 ~ ~-2", &names),
            Ok(Command::World {
                target: None,
                change: WorldChange::TeleportTo([at(10.0, false), at(0.0, true), at(-2.0, true)]),
            })
        );
        assert_eq!(
            parse_for("/tp Ana 0 64 0", &names),
            Ok(Command::World {
                target: Some("Ana".into()),
                change: WorldChange::TeleportTo([at(0.0, false), at(64.0, false), at(0.0, false)]),
            })
        );
        assert_eq!(parse_for("/tp 1 2 3", &names), Ok(Command::Tp { to: "1 2 3".into() }));
        assert_eq!(parse_for("/tp Ana", &names), Ok(Command::Tp { to: "Ana".into() }));
    }

    #[test]
    fn give_targets_an_online_player_only_when_named() {
        let names = vec!["Bo".to_owned()];
        assert_eq!(
            parse_for("/give Bo steel sword 2", &names),
            Ok(Command::World {
                target: Some("Bo".into()),
                change: WorldChange::Give { item: "steel sword".into(), count: 2 },
            })
        );
        assert_eq!(
            parse_for("/give Bo", &names),
            Ok(Command::World { target: None, change: WorldChange::Give { item: "Bo".into(), count: 1 } }),
            "a lone name is an item"
        );
        let view = Fake::new();
        let given = execute(1, &parse_for("/give Bo stone", &names).unwrap(), &view);
        assert_eq!(
            given.effects,
            vec![Effect::World { actor: 1, target: 2, change: WorldChange::Give { item: "stone".into(), count: 1 } }]
        );
        let missing = execute(1, &Command::World { target: Some("Zed".into()), change: WorldChange::Summon("x".into()) }, &view);
        assert!(missing.effects.is_empty());
        assert_eq!(missing.private, vec!["no player called Zed"]);
    }

    #[test]
    fn world_commands_need_an_operator_and_a_supported_mode() {
        let view = Fake::new();
        let weather = parse("/weather rain").unwrap();
        assert!(execute(2, &weather, &view).effects.is_empty());
        assert_eq!(
            execute(1, &weather, &view).effects,
            vec![Effect::World { actor: 1, target: 1, change: WorldChange::Weather(Weather::Rain) }]
        );
        assert_eq!(parse("/gm sp"), Ok(Command::Gamemode(GameMode::Spectator)));
        assert_eq!(parse("/gm c Ana (2)"), Ok(Command::GamemodeOf { mode: GameMode::Creative, to: "Ana (2)".into() }));
        assert_eq!(
            execute(1, &Command::GamemodeOf { mode: GameMode::Creative, to: "bo".into() }, &view).effects,
            vec![Effect::Gamemode { player: 2, mode: GameMode::Creative }]
        );
        assert!(execute(2, &Command::GamemodeOf { mode: GameMode::Creative, to: "Ana".into() }, &view).effects.is_empty());
        let refused = execute(1, &Command::Gamemode(GameMode::Spectator), &view);
        assert!(refused.effects.is_empty() && refused.broadcast.is_empty());
        assert_eq!(execute(1, &Command::Time(TimeOp::Query), &view).private, vec!["time is 0 ticks"]);
    }

    #[test]
    fn players_without_operator_cannot_ban() {
        let view = Fake::new();
        let outcome = execute(2, &Command::Ban { to: "Ana".into(), reason: "x".into() }, &view);
        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.private, vec!["only an operator can run that"]);
    }
}


