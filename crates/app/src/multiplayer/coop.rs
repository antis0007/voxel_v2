//! Switching a running game between single-player and a shared session.
//! `Mode` is the one place the game loop asks what a session changes.
use super::open::{HostOpening, JoinAttempt, OpenOptions};
use super::*;
use crate::egui_ui::{UiAction, coop_panel::{Phase, Request, View}};

/// What kind of session the running game is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Single,
    /// A saved single-player world hosted in place; it keeps its own save.
    Opened,
    /// A command-line building host with its own building save.
    Hosted,
    Joined,
}

impl Mode {
    pub fn of(session: Option<&Session>) -> Self {
        session.map_or(Self::Single, Session::mode)
    }
    /// The session, not the local loop, steps the world.
    pub fn network(self) -> bool {
        self != Self::Single
    }
    /// The world's own checkpoint saves and the preference file keep running.
    pub fn local_saves(self) -> bool {
        matches!(self, Self::Single | Self::Opened)
    }
    /// UI actions that still apply while a session owns the world. Others act on
    /// state the host owns, so they arrive as session commands or not at all.
    pub fn allows(self, action: &UiAction, host_weather: bool) -> bool {
        !self.network()
            || host_weather
            || matches!(
                action,
                UiAction::None | UiAction::Resume | UiAction::Quit | UiAction::SaveWorld | UiAction::BackToWorldSelect | UiAction::Coop(_)
            )
    }
}

/// Where the game stands after a session switch.
pub enum Event {
    Hosted(Session),
    SaveBeforeJoin,
    Joined(Box<(Session, World, Player, DayNightCycle)>),
}

/// Lobby work started from the UI, polled once a frame.
#[derive(Default)]
pub struct Driver {
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    native_opening: Option<String>,
    opening: Option<HostOpening>,
    joining: Option<JoinAttempt>,
    join_checkpoint: bool,
    /// Single-player world to reopen after leaving a joined session.
    pub return_to: Option<String>,
    pub status: Option<String>,
}

impl Driver {
    pub fn busy(&self) -> bool {
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        if self.native_opening.is_some() {return true;}
        self.opening.is_some() || self.joining.is_some()
    }
    pub fn open(&mut self, world: &World, name: String, listed: bool, creative: bool) {
        if self.busy() {
            return;
        }
        self.status = None;
        match HostOpening::start(world, OpenOptions { name, listed, creative }) {
            Ok(opening) => self.opening = Some(opening),
            Err(error) => self.status = Some(format!("Open Failed: {error:#}")),
        }
    }
    pub fn join(&mut self, code: &str) {
        if self.busy() {
            return;
        }
        self.status = None;
        self.join_checkpoint = false;
        match JoinAttempt::start(code) {
            Ok(attempt) => self.joining = Some(attempt),
            Err(error) => self.status = Some(format!("Join Failed: {error:#}")),
        }
    }
    pub fn cancel(&mut self) {
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        {self.native_opening=None;}
        self.opening = None;
        self.joining = None;
        self.join_checkpoint = false;
        self.return_to = None;
        self.status = None;
    }
    /// `world`: the open world, if any; `saved`: its last checkpoint finished, so it may be replaced.
    pub fn poll(&mut self, world: Option<&mut World>, saved: bool, save_failed: bool) -> Option<Event> {
        #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
        if let Some(address)=self.native_opening.take() {
            return match world.context("no world is open").and_then(|world|Session::host_direct(world,&address)) {
                Ok(session)=>Some(Event::Hosted(session)),
                Err(error)=>{self.status=Some(format!("Open Failed: {error:#}"));None}
            };
        }
        if let Some(opening) = &self.opening
            && let Some(result) = opening.poll()
        {
            let options = opening.options.clone();
            self.opening = None;
            let opened = result.map_err(anyhow::Error::msg).and_then(|relay| {
                let world = world.context("no world is open")?;
                Session::host_world(world, relay, &options)
            });
            return match opened {
                Ok(session) => Some(Event::Hosted(session)),
                Err(error) => {
                    self.status = Some(format!("Open Failed: {error:#}"));
                    None
                }
            };
        }
        if self.joining.is_some() && self.join_checkpoint && save_failed {
            self.cancel();
            self.status = Some("World Save Failed".into());
            return None;
        }
        let joining = self.joining.as_mut()?;
        let needs_checkpoint = world.is_some() && !self.join_checkpoint;
        let had_reply = joining.reply().is_some();
        let result = joining.poll(saved && !needs_checkpoint);
        if !had_reply && let Some(reply) = joining.reply() && matches!(review(), Some(Review::Join(_))) {
            eprintln!("[multiplayer-review] punch reply={reply}");
        }
        let result = match result {
            Some(result) => result,
            None if needs_checkpoint && joining.prepared() => {
                self.join_checkpoint = true;
                return Some(Event::SaveBeforeJoin);
            }
            None => return None,
        };
        self.joining = None;
        match result {
            Ok(joined) => Some(Event::Joined(Box::new(joined))),
            Err(error) => {
                self.status = Some(format!("Join Failed: {error:#}"));
                self.return_to = None;
                None
            }
        }
    }
    pub fn view(&self, session: Option<&Session>, world_open: bool) -> View {
        let phase = if self.busy() && self.joining.is_none() {
            Phase::Opening
        } else if self.joining.is_some() {
            Phase::Connecting
        } else {
            match Mode::of(session) {
                Mode::Single if world_open => Phase::Single,
                Mode::Single => Phase::Menu,
                Mode::Opened => Phase::Hosting,
                Mode::Hosted => Phase::Fixed,
                Mode::Joined => Phase::Joined,
            }
        };
        let failure = session.filter(|s| s.failed()).map(|_| "Disconnected".to_owned());
        let code = self.joining.as_ref().and_then(JoinAttempt::reply)
            .or_else(|| session.and_then(Session::code)).map(str::to_owned);
        let preparing_invite = phase == Phase::Hosting && code.is_none();
        View {
            phase,
            code,
            players: session.map_or(0, Session::others),
            status: failure.or_else(|| self.status.clone())
                .or_else(|| self.joining.as_ref().map(|join| join.status().to_owned()))
                .or_else(|| preparing_invite.then(|| "Preparing Invite".to_owned()))
                .or_else(|| session.filter(|s| !s.ready()).map(|s| {
                    if s.code().is_some_and(|code| code.starts_with(engine_net::punch::ANSWER_PREFIX)) {
                        "Waiting for Host".to_owned()
                    } else {
                        "Loading World".to_owned()
                    }
                })),
        }
    }
    /// Turns a panel request into the action the game loop handles, or handles it here.
    pub fn request(&mut self, request: Request, world: Option<&World>, mode: Mode) -> Option<Request> {
        match request {
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            Request::OpenNative {address}=>{
                if self.busy()||mode!=Mode::Single||world.is_none() {self.status=Some("No Open World".into());}
                else {match super::direct_address(&address) {
                    Ok(_)=>{self.status=None;self.native_opening=Some(address);}
                    Err(_)=>self.status=Some("Invalid Address".into()),
                }}
                None
            }
            Request::Open { name, listed } => {
                match world {
                    // The session's mode is the world's game mode.
                    Some(world) if mode == Mode::Single => self.open(world, name, listed, world.rules.mode.creative_tools()),
                    _ => self.status = Some("No Open World".into()),
                }
                None
            }
            Request::Cancel => {
                self.cancel();
                None
            }
            Request::Join { .. } if mode != Mode::Single => {
                self.status = Some("Already Connected".into());
                None
            }
            other => Some(other),
        }
    }
}

/// `VOXEL_REVIEW_COOP=open:<world name>` or `join:<code>` drives the Online panel without
/// clicks, so captures and scripted runs exercise the same switch as the buttons.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Review {
    Open(String),
    Join(String),
}

pub fn review() -> Option<Review> {
    let text = std::env::var("VOXEL_REVIEW_COOP").ok()?;
    match text.split_once(':')? {
        ("open", name) => Some(Review::Open(name.to_owned())),
        ("join", code) => Some(Review::Join(code.to_owned())),
        _ => None,
    }
}

/// Ends the running session, if any, and returns the mode it was in. A hosted world gets
/// its live simulation back; pass `None` only when the game is exiting.
pub fn end(
    slot: &mut Option<Session>,
    world: Option<&mut World>,
    player: &Player,
    day: &DayNightCycle,
    simulation: Option<&mut crate::simulation::WorldSimulation>,
) -> Option<Mode> {
    let mut session = slot.take()?;
    let mode = session.mode();
    if mode == Mode::Opened
        && let Some(simulation) = simulation
    {
        session.exchange_simulation(simulation);
    }
    if let Some(world) = world
        && let Err(error) = session.close(world, player, day)
    {
        world.expedition.say(format!("Save Failed: {error}"));
        eprintln!("[multiplayer] close failed: {error:#}");
    }
    Some(mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_single_player_runs_the_local_loop() {
        assert!(!Mode::Single.network());
        for mode in [Mode::Opened, Mode::Hosted, Mode::Joined] {
            assert!(mode.network(), "{mode:?}");
        }
    }

    #[test]
    fn an_opened_world_keeps_its_own_saves() {
        assert!(Mode::Single.local_saves());
        assert!(Mode::Opened.local_saves());
        assert!(!Mode::Hosted.local_saves(), "a building host has its own save");
        assert!(!Mode::Joined.local_saves(), "a joiner never writes the host's world");
    }

    #[test]
    fn sessions_filter_world_actions_but_keep_leaving_and_friends() {
        let blocked = UiAction::SetDifficulty(Default::default());
        assert!(Mode::Single.allows(&blocked, false));
        for mode in [Mode::Opened, Mode::Hosted, Mode::Joined] {
            assert!(!mode.allows(&blocked, false), "{mode:?}");
            assert!(mode.allows(&UiAction::BackToWorldSelect, false));
            assert!(mode.allows(&UiAction::Coop(Request::Leave), false));
            assert!(mode.allows(&UiAction::RandomizeWeather, true), "host weather");
        }
    }

    #[test]
    fn no_session_reports_single_player_or_the_menu() {
        let driver = Driver::default();
        assert_eq!(Mode::of(None), Mode::Single);
        assert_eq!(driver.view(None, true).phase, Phase::Single);
        assert_eq!(driver.view(None, false).phase, Phase::Menu);
    }

    #[test]
    fn requests_that_need_a_single_player_world_are_refused_with_a_state() {
        let mut driver = Driver::default();
        let open = Request::Open { name: String::new(), listed: false };
        assert_eq!(driver.request(open, None, Mode::Single), None);
        assert_eq!(driver.status.as_deref(), Some("No Open World"));
        assert_eq!(driver.request(Request::Join { code: "ABCD-EFGH".into() }, None, Mode::Joined), None);
        assert_eq!(driver.status.as_deref(), Some("Already Connected"));
        assert_eq!(driver.request(Request::Leave, None, Mode::Joined), Some(Request::Leave));
        assert!(!driver.busy());
    }

    #[test]
    fn a_malformed_code_fails_without_a_worker() {
        let mut driver = Driver::default();
        driver.join("ABC");
        assert!(!driver.busy());
        assert!(driver.status.as_deref().is_some_and(|s| s.starts_with("Join Failed")));
    }
}
