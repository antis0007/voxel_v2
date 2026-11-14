//! The host's chat and command loop: typed lines in, chat and effects out.
//!
//! Access effects are applied and saved here. Effects on gameplay state come back
//! as `Deferred` for the session, which owns the players, the clock and the save.
use super::{link, server_files, social::Row};
use crate::world;
use engine_net::{
    commands::{self, Effect, GameMode, ServerView, WorldChange},
    protocol::{ChatKind, Packet, PlayerId},
    social::ChatBudget,
};
use std::{collections::BTreeMap, time::Instant};

/// The id the host's own player uses in chat and commands.
pub const HOST: PlayerId = 0;

pub enum Deferred {
    Gamemode(PlayerId, bool),
    Teleport { player: PlayerId, to: PlayerId },
    SetTime(f32),
    Save,
    Stop,
    /// Applied by the session, which holds the bodies; the reply goes to `actor`.
    World { actor: PlayerId, target: PlayerId, change: WorldChange },
}

/// What the loop reads from the session.
pub struct Context<'a> {
    pub seed: u64,
    pub time: f32,
    /// The host's own name; `None` for a dedicated host with no player.
    pub host_name: Option<&'a str>,
    /// Creative players; everyone else is in survival.
    pub creative: &'a dyn Fn(PlayerId) -> bool,
    /// The world has a save file, so `/save-all` can do something.
    pub can_save: bool,
}

/// Lines for the host's own chat or console: those meant for everyone or for the host.
pub type Shown = Vec<(ChatKind, PlayerId, String)>;

/// Every player as the command layer and the player list see them: the host first.
pub fn names(socket: &link::Host, host_name: Option<&str>) -> Vec<(PlayerId, String)> {
    host_name.map(|n| (HOST, n.to_owned())).into_iter().chain(socket.roster()).collect()
}

pub fn roster(socket: &link::Host, ctx: &Context) -> Vec<Row> {
    let view = view(socket, ctx);
    view.names
        .iter()
        .map(|(id, name)| Row { id: *id, name: name.clone(), op: view.is_op(*id), creative: (ctx.creative)(*id) })
        .collect()
}

fn view(socket: &link::Host, ctx: &Context) -> AdminView {
    let loaded = server_files::load(&world::storage::save_directory());
    let names = names(socket, ctx.host_name);
    // Rights follow the proven key, never a typed name. The host always holds them.
    AdminView {
        op_holders: std::iter::once(HOST).chain(names.iter().map(|(id, _)| *id).filter(|id| socket.holds_op(*id))).collect(),
        creative: names.iter().filter(|(id, _)| (ctx.creative)(*id)).map(|(id, _)| *id).collect(),
        names,
        whitelisted: loaded.access.whitelist().map(str::to_owned).collect(),
        bans: loaded.access.bans().map(|(name, reason)| (name.to_owned(), reason.to_owned())).collect(),
        whitelist_on: loaded.settings.whitelist,
        time: ctx.time,
        seed: ctx.seed,
    }
}

/// Runs the lines typed since the last tick. `local` holds lines that reached the
/// host outside the session queue: its own player's, the console's and joiners' chat.
pub fn run(
    socket: &mut link::Host,
    ctx: &Context,
    local: Vec<(PlayerId, String)>,
    budgets: &mut BTreeMap<PlayerId, ChatBudget>,
    host_name: &mut String,
) -> (Vec<Deferred>, Shown) {
    let mut lines = socket.take_commands();
    lines.splice(0..0, local);
    let mut deferred = Vec::new();
    let mut shown = Shown::new();
    if lines.is_empty() {
        return (deferred, shown);
    }
    let dir = world::storage::save_directory();
    let loaded = server_files::load(&dir);
    let view = view(socket, ctx);
    let mut access_changed = false;
    let mut whitelist_toggle: Option<bool> = None;
    // (target, kind, speaker, text); `None` reaches everyone.
    let mut replies: Vec<(Option<PlayerId>, ChatKind, PlayerId, String)> = Vec::new();
    let online: Vec<String> = view.names.iter().map(|(_, name)| name.clone()).collect();
    let now = Instant::now();
    budgets.retain(|id, _| view.names.iter().any(|(n, _)| n == id));
    for (actor, line) in lines {
        if actor != HOST && !budgets.entry(actor).or_insert_with(|| ChatBudget::new(now)).allow(now) {
            replies.push((Some(actor), ChatKind::System, HOST, "too many messages; wait a moment".into()));
            continue;
        }
        if !line.starts_with('/') {
            match engine_net::social::sanitize_chat(&line) {
                Ok(text) => replies.push((None, ChatKind::Say, actor, text)),
                Err(reason) => replies.push((Some(actor), ChatKind::System, HOST, reason.to_owned())),
            }
            continue;
        }
        let outcome = match commands::parse_for(&line, &online) {
            Ok(command) => commands::execute(actor, &command, &view),
            Err(reason) => commands::Outcome { private: vec![reason], ..Default::default() },
        };
        let has_lists = socket.access().is_some();
        let mut private = outcome.private;
        for effect in &outcome.effects {
            match effect {
                Effect::Ban { .. }
                | Effect::Pardon { .. }
                | Effect::Whitelist { .. }
                | Effect::SetWhitelist(_)
                | Effect::SetOp { .. }
                    if !has_lists =>
                {
                    private.push("access lists unavailable on this transport".to_owned());
                }
                Effect::Ban { name, reason } => {
                    let online = view.resolve(name).filter(|(id, _)| *id != HOST).map(|(id, _)| id);
                    let key = online.and_then(|id| socket.fingerprint(id));
                    if let Some(access) = socket.access_mut() {
                        let _ = access.ban(name, reason);
                        // A name ban alone is beaten by renaming.
                        if let Some(key) = &key {
                            let _ = access.ban_key(key, reason);
                        }
                        access_changed = true;
                    }
                    // A banned player who is online leaves now, not at the next join.
                    if let Some(id) = online {
                        socket.disconnect(id);
                    }
                }
                Effect::Pardon { name } => {
                    if let Some(access) = socket.access_mut() {
                        let _ = access.pardon(name);
                        access_changed = true;
                    }
                }
                Effect::Whitelist { name, add } => {
                    let key = view.resolve(name).and_then(|(id, _)| socket.fingerprint(id));
                    if let Some(access) = socket.access_mut() {
                        if *add {
                            let _ = access.whitelist_add(name);
                        } else {
                            let _ = access.whitelist_remove(name);
                        }
                        // Binding keeps the name reserved for the player who holds it now.
                        if let Some(key) = &key {
                            if *add {
                                let _ = access.bind_whitelist(name, key);
                            } else {
                                let _ = access.whitelist_key(key, false);
                            }
                        }
                        access_changed = true;
                    }
                }
                Effect::SetWhitelist(on) => {
                    whitelist_toggle = Some(*on);
                    access_changed = true;
                }
                Effect::SetOp { player, op } => {
                    let target = view.names.iter().find(|(id, _)| id == player).map(|(_, name)| name.clone());
                    if *player == HOST {
                        private.push("the host is always an operator".to_owned());
                    } else if let Some(name) = target {
                        let key = socket.fingerprint(*player);
                        if let Some(access) = socket.access_mut() {
                            let _ = access.set_op(&name, *op);
                            // Rights follow the target's proven key, not the name it typed.
                            if *op && let Some(key) = &key {
                                let _ = access.bind_op(&name, key);
                            }
                            access_changed = true;
                        }
                    }
                }
                Effect::Kick { player, .. } if *player == HOST => {
                    private.push("the host cannot be kicked".to_owned());
                }
                Effect::Kick { player, .. } => socket.disconnect(*player),
                Effect::Rename { player, name } if *player == HOST => {
                    *host_name = engine_net::social::unique_name(name, |c| {
                        view.names.iter().any(|(id, n)| *id != HOST && n.eq_ignore_ascii_case(c))
                    });
                    replies.push((None, ChatKind::System, HOST, format!("{} is now {host_name}", view.name_of(HOST))));
                }
                Effect::Rename { player, name } => match socket.rename(*player, name) {
                    Ok(assigned) => replies.push((
                        None,
                        ChatKind::System,
                        HOST,
                        format!("{} is now {assigned}", view.name_of(*player)),
                    )),
                    Err(error) => private.push(format!("name not changed: {error}")),
                },
                Effect::Gamemode { player, mode } => {
                    deferred.push(Deferred::Gamemode(*player, *mode == GameMode::Creative));
                }
                Effect::Teleport { player, to } => deferred.push(Deferred::Teleport { player: *player, to: *to }),
                Effect::SetTime(time) => deferred.push(Deferred::SetTime(*time)),
                Effect::Save if ctx.can_save => deferred.push(Deferred::Save),
                Effect::Save => {}
                Effect::Stop => deferred.push(Deferred::Stop),
                Effect::World { actor, target, change } => {
                    deferred.push(Deferred::World { actor: *actor, target: *target, change: change.clone() });
                }
            }
        }
        let mut broadcast = outcome.broadcast;
        if outcome.effects.contains(&Effect::Save) && !ctx.can_save {
            broadcast.clear();
            private.push("this world has no save file".to_owned());
        }
        replies.extend(private.into_iter().map(|text| (Some(actor), ChatKind::System, HOST, text)));
        replies.extend(broadcast.into_iter().map(|text| (None, ChatKind::System, HOST, text)));
        // Whispers go only to their target.
        replies.extend(outcome.direct.into_iter().map(|(target, text)| (Some(target), ChatKind::Whisper, HOST, text)));
    }
    if access_changed && let Some(access) = socket.access() {
        let mut settings = loaded.settings.clone();
        if let Some(on) = whitelist_toggle {
            settings.whitelist = on;
            if let Err(error) = socket.set_access(access.clone(), on) {
                eprintln!("[multiplayer] access lists not applied: {error}");
            }
        }
        if let Err(error) = server_files::save(&dir, &settings, &access) {
            eprintln!("[multiplayer] server files not saved: {error}");
        }
    }
    let roster = Packet::Roster { players: names(socket, ctx.host_name) };
    for (target, kind, from, text) in replies {
        if target.is_none_or(|t| t == HOST) {
            shown.push((kind, from, text.clone()));
        }
        let packet = Packet::Chat { kind, from, text };
        let targets = match target {
            Some(HOST) => Vec::new(),
            Some(id) => vec![id],
            None => socket.players(),
        };
        for id in targets {
            // A chat line may precede the next periodic game roster after a join.
            // Send names first on the same ordered channel so no speaker is '?'.
            if matches!(socket.send_packet(id, &roster), Ok(true)) {
                let _ = socket.send_packet(id, &packet);
            }
        }
    }
    (deferred, shown)
}

/// A system line for one player only: the host's own chat, or the joiner's.
pub fn tell(socket: &mut link::Host, host_name: Option<&str>, to: PlayerId, text: String, shown: &mut Shown) {
    if to == HOST {
        shown.push((ChatKind::System, HOST, text));
        return;
    }
    // Names first on the same ordered channel, as in `run`.
    let roster = Packet::Roster { players: names(socket, host_name) };
    if matches!(socket.send_packet(to, &roster), Ok(true)) {
        let _ = socket.send_packet(to, &Packet::Chat { kind: ChatKind::System, from: HOST, text });
    }
}

/// Live session state as the command layer sees it.
struct AdminView {
    names: Vec<(PlayerId, String)>,
    /// The host, plus players whose profile token holds an operator entry under their name.
    op_holders: Vec<PlayerId>,
    creative: Vec<PlayerId>,
    whitelisted: Vec<String>,
    bans: Vec<(String, String)>,
    whitelist_on: bool,
    time: f32,
    seed: u64,
}

impl ServerView for AdminView {
    fn players(&self) -> Vec<(PlayerId, String)> {
        self.names.clone()
    }
    /// A dedicated host's console is not a player, so it speaks as "Server".
    fn name_of(&self, player: PlayerId) -> String {
        match self.names.iter().find(|(id, _)| *id == player) {
            Some((_, name)) => name.clone(),
            None if player == HOST => "Server".to_owned(),
            None => String::new(),
        }
    }
    fn is_op(&self, player: PlayerId) -> bool {
        self.op_holders.contains(&player)
    }
    fn gamemode(&self, player: PlayerId) -> GameMode {
        if self.creative.contains(&player) { GameMode::Creative } else { GameMode::Survival }
    }
    fn time_of_day(&self) -> f32 {
        self.time
    }
    fn seed(&self) -> u64 {
        self.seed
    }
    fn banned(&self) -> Vec<(String, String)> {
        self.bans.clone()
    }
    fn whitelist(&self) -> Vec<String> {
        self.whitelisted.clone()
    }
    fn whitelist_enabled(&self) -> bool {
        self.whitelist_on
    }
}
