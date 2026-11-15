//! Online session browser: search the lobby, join by code, host a session. Lookups run on one
//! worker thread that keeps only the newest query, so a slow lobby never stalls a frame.
//! Join swaps the running game into the session; New Window joins from a second process, which
//! is how one machine tests two players. Hosting starts from an open world: the Friends page.
use super::front_end::{action, heading};
use super::coop_panel::{Glyph, glyph, icon_button};
use engine_net::lobby::{
    DEFAULT_LOBBY,
    relay,
    wire::{Entry, MAX_PAGE, Mode, Query, Sort},
};
use egui::{Color32, RichText};
use std::{
    net::SocketAddr,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

/// Seconds a changed search waits before it is sent, so typing sends one query.
const DEBOUNCE: f64 = 0.25;
/// Seconds between automatic refreshes of an unchanged search.
const REFRESH: f64 = 15.0;
const MUTED: Color32 = Color32::from_rgb(156, 148, 170);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ModeFilter {
    #[default]
    Any,
    Survival,
    Creative,
}

type Reply = (Query, Result<(u32, Vec<Entry>), String>);

pub struct BrowserState {
    search: String,
    mode: ModeFilter,
    include_full: bool,
    sort: Sort,
    /// Name shown to other players, remembered between sessions.
    player: String,
    listing: Option<(u32, Vec<Entry>)>,
    error: Option<String>,
    notice: Option<String>,
    /// Query that the UI last saw change, and when.
    seen: Option<Query>,
    changed_at: f64,
    /// Query last sent to the worker, and when.
    asked: Option<Query>,
    asked_at: f64,
    updated_at: Option<f64>,
    requests: Option<Sender<Query>>,
    replies: Option<Receiver<Reply>>,
    lobby: SocketAddr,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            search: String::new(),
            mode: ModeFilter::Any,
            include_full: false,
            sort: Sort::Players,
            player: std::fs::read_to_string(name_file()).map(|n| n.trim().to_owned()).unwrap_or_default(),
            listing: None,
            error: None,
            notice: None,
            seen: None,
            changed_at: 0.0,
            asked: None,
            asked_at: f64::NEG_INFINITY,
            updated_at: None,
            requests: None,
            replies: None,
            lobby: lobby_address(),
        }
    }
}

/// `--multiplayer-lobby=ADDR` from this process, or the public lobby.
fn lobby_address() -> SocketAddr {
    std::env::args()
        .find_map(|a| a.strip_prefix("--multiplayer-lobby=").map(str::to_owned))
        .and_then(|text| text.parse().ok())
        .unwrap_or_else(|| DEFAULT_LOBBY.parse().unwrap_or(SocketAddr::from(([127, 0, 0, 1], 27021))))
}

fn query(state: &BrowserState) -> Query {
    Query {
        text: state.search.trim().to_owned(),
        mode: match state.mode {
            ModeFilter::Any => None,
            ModeFilter::Survival => Some(Mode::Survival),
            ModeFilter::Creative => Some(Mode::Creative),
        },
        version: None,
        include_full: state.include_full,
        sort: state.sort,
        offset: 0,
        limit: MAX_PAGE,
    }
}

impl BrowserState {
    /// Starts the worker on first use. Each request waits for the newest one that arrived.
    fn ensure_worker(&mut self) {
        if self.requests.is_some() {
            return;
        }
        let (request_tx, request_rx) = mpsc::channel::<Query>();
        let (reply_tx, reply_rx) = mpsc::channel::<Reply>();
        let lobby = self.lobby;
        let spawned = thread::Builder::new().name("lobby-browse".into()).spawn(move || {
            while let Ok(mut query) = request_rx.recv() {
                while let Ok(newer) = request_rx.try_recv() {
                    query = newer;
                }
                let result = relay::browse(lobby, &query).map_err(|error| error.to_string());
                if reply_tx.send((query, result)).is_err() {
                    break;
                }
            }
        });
        match spawned {
            Ok(_) => {
                self.requests = Some(request_tx);
                self.replies = Some(reply_rx);
            }
            Err(error) => self.error = Some(format!("Browser thread failed: {error}")),
        }
    }

    fn collect(&mut self, now: f64) {
        let Some(replies) = self.replies.take() else { return };
        while let Ok((reply_query, result)) = replies.try_recv() {
            // Replies to searches the user has moved past are dropped.
            if self.asked.as_ref() != Some(&reply_query) || reply_query != query(self) {
                continue;
            }
            match result {
                Ok(listing) => {
                    self.listing = Some(listing);
                    self.error = None;
                    self.updated_at = Some(now);
                }
                Err(error) => self.error = Some(error),
            }
        }
        self.replies = Some(replies);
    }

    fn request(&mut self, query: Query, now: f64) {
        self.asked_at = now;
        self.ensure_worker();
        if let Some(requests) = &self.requests {
            if requests.send(query.clone()).is_ok() {
                self.asked = Some(query);
                self.asked_at = now;
            } else {
                self.error = Some("Browser thread stopped".into());
                self.requests = None;
                self.replies = None;
            }
        }
    }

    /// Saves the typed player name for this and later sessions. False if it is not valid.
    fn remember_name(&mut self) -> bool {
        if self.player.trim().is_empty() {
            return true;
        }
        match engine_net::social::sanitize_name(&self.player) {
            Ok(name) => {
                let _ = std::fs::create_dir_all(crate::world::storage::save_directory());
                let _ = std::fs::write(name_file(), &name);
                true
            }
            Err(reason) => {
                self.notice = Some(format!("Player Name: {reason}"));
                false
            }
        }
    }

    /// Joins in this window.
    fn join(&mut self, code: &str) -> super::UiAction {
        if !self.remember_name() {
            return super::UiAction::None;
        }
        super::UiAction::Coop(super::coop_panel::Request::Join { code: code.trim().to_owned() })
    }

    /// Opens a second game window with `args`, for two players on one machine.
    fn launch(&mut self, mut args: Vec<String>) {
        args.push(format!("--multiplayer-lobby={}", self.lobby));
        if !self.remember_name() {
            return;
        }
        if let Ok(name) = engine_net::social::sanitize_name(&self.player) {
            args.push(format!("--multiplayer-name={name}"));
        }
        match std::env::current_exe().and_then(|exe| std::process::Command::new(exe).args(args).spawn()) {
            Ok(_) => self.notice = Some("New Window Open".into()),
            Err(error) => self.notice = Some(format!("New Window Failed: {error}")),
        }
    }
}

fn name_file() -> std::path::PathBuf {
    crate::world::storage::save_directory().join("multiplayer-name")
}

/// Offline layout fixture for the production menu exporter; never opens a lobby connection.
pub(super) fn preview(root: &mut egui::Ui) {
    let now = root.ctx().input(|input| input.time);
    let mut state = BrowserState::default();
    state.player = "Player".into();
    let current = query(&state);
    state.seen = Some(current.clone());
    state.asked = Some(current);
    state.asked_at = now;
    state.updated_at = Some(now);
    state.listing = Some((2, vec![
        Entry { code: "ABCD-EFGH".into(), name: "Riverside".into(), world: "Hills · 0.5 m".into(), mode: Mode::Survival, version: 65, players: 3, max_players: 8, age_secs: 90, motd: String::new() },
        Entry { code: "WXYZ-2345".into(), name: "Workshop".into(), world: "Superflat · 1 m".into(), mode: Mode::Creative, version: 65, players: 1, max_players: 8, age_secs: 30, motd: String::new() },
    ]));
    let mut coop = super::coop_panel::PanelState::default();
    egui::CentralPanel::default().show(root, |ui| {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width((ui.available_width()-24.0).clamp(220.0,760.0));
            super::front_end::surface(ui.ctx()).show(ui, |ui| {
                let _ = draw(ui, &mut state, &mut coop);
                let _ = host_block(ui, Some("latest"), false);
            });
        });
    });
}

pub(super) fn draw(ui: &mut egui::Ui, state: &mut BrowserState, coop: &mut super::coop_panel::PanelState) -> super::UiAction {
    let mut result = super::UiAction::None;
    heading(ui, "Multiplayer", "");
    ui.horizontal_wrapped(|ui| {
        glyph(ui, Glyph::Players, MUTED);
        ui.add(egui::TextEdit::singleline(&mut state.player).hint_text("Player Name").char_limit(16).desired_width(160.0))
            .on_hover_text("Player Name");
    });
    if let Some(request) = super::coop_panel::menu_join(ui, coop) {
        result = match request {
            super::coop_panel::Request::Join { code } => state.join(&code),
            other => super::UiAction::Coop(other),
        };
    }
    if matches!(coop.view.phase, super::coop_panel::Phase::Menu | super::coop_panel::Phase::Single) {
        ui.separator();
        let sessions = ui.collapsing("Public Sessions", |ui| public_sessions(ui, state));
        if let Some(code) = sessions.body_returned.flatten() { result = state.join(&code); }
    }
    ui.add_space(10.0);
    if let Some(code) = coop.code_text().filter(|c| super::coop_panel::looks_like_code(c)) {
        let code = code.to_owned();
        ui.collapsing("Advanced", |ui| {
            if action(ui, "New Window", false).clicked() {
                let flag=if code.parse::<SocketAddr>().is_ok()||code.starts_with("steam:"){"--multiplayer-join="}else{"--multiplayer-join-code="};
                state.launch(vec![flag.to_owned()+&code]);
            }
        });
    }
    if let Some(notice) = &state.notice {
        ui.label(RichText::new(notice).small().color(MUTED));
    }
    result
}

fn public_sessions(ui: &mut egui::Ui, state: &mut BrowserState) -> Option<String> {
    let now = ui.ctx().input(|input| input.time);
    state.collect(now);
    let current = query(state);
    if state.seen.as_ref() != Some(&current) {
        state.seen = Some(current.clone());
        state.changed_at = now;
        state.listing = None;
        state.updated_at = None;
        state.error = None;
    }
    let due = state.asked.as_ref() != Some(&current) && now - state.changed_at >= DEBOUNCE
        && (state.error.is_none() || now - state.asked_at >= REFRESH);
    let refresh = state.asked.as_ref() == Some(&current) && now - state.asked_at >= REFRESH;
    if due || refresh {
        state.request(current, now);
    }
    ui.ctx().request_repaint_after(Duration::from_millis(200));

    ui.horizontal_wrapped(|ui| {
        ui.add(egui::TextEdit::singleline(&mut state.search).hint_text("Search Sessions").desired_width((ui.available_width()-210.0).clamp(100.0,360.0)));
        if icon_button(ui, Glyph::Refresh, "Refresh").clicked() {
            state.asked = None;
            state.asked_at = f64::NEG_INFINITY;
            state.error = None;
        }
        egui::ComboBox::from_id_salt("session_mode").width(90.0)
            .selected_text(match state.mode { ModeFilter::Any => "All Modes", ModeFilter::Survival => "Survival", ModeFilter::Creative => "Creative" })
            .show_ui(ui, |ui| {
                for (filter, label) in [(ModeFilter::Any, "All Modes"), (ModeFilter::Survival, "Survival"), (ModeFilter::Creative, "Creative")] {
                    ui.selectable_value(&mut state.mode, filter, label);
                }
            });
        ui.menu_button("Filters", |ui| {
            ui.checkbox(&mut state.include_full, "Show Full");
            ui.separator();
            for (sort, label) in [(Sort::Players, "Players"), (Sort::Newest, "Newest"), (Sort::Name, "Name")] {
                ui.selectable_value(&mut state.sort, sort, label);
            }
        });
    });
    let status = match (&state.error, &state.listing, state.updated_at) {
        (Some(_), _, _) => RichText::new("Lobby Offline").color(Color32::from_rgb(230, 130, 110)),
        (None, Some((total, _)), Some(_)) => RichText::new(format!("{total} Sessions")).color(MUTED),
        _ => RichText::new("Searching").color(MUTED),
    };
    let status_response = ui.label(status.size(13.0));
    if let Some(error) = &state.error { status_response.on_hover_text(error); }
    ui.add_space(6.0);

    let mut join_code = None;
    let rows = state.listing.as_ref().map(|(_, entries)| entries.as_slice()).unwrap_or_default();
    egui::ScrollArea::vertical().id_salt("online_sessions").max_height(260.0).show(ui, |ui| {
        if rows.is_empty() && state.listing.is_some() {
            ui.label(RichText::new("No sessions match").color(MUTED));
        }
        for entry in rows {
            ui.add_enabled_ui(entry.players < entry.max_players, |ui| {
                if let Some(code) = session_row(ui, entry) { join_code = Some(code); }
            });
        }
    });
    join_code
}

/// Full page for the home navigation: the browser inside the same surface as the other menu pages.
pub(super) fn page(root: &mut egui::Ui, owner: &mut super::EguiUi) -> super::UiAction {
    let mut result = super::UiAction::None;
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
        egui::ScrollArea::vertical().id_salt("online_screen").show(ui, |ui| {
            ui.add_space(22.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width((ui.available_width() - 32.0).clamp(220.0, 760.0));
                super::front_end::surface(ui.ctx()).show(ui, |ui| {
                    result = draw(ui, &mut owner.online, &mut owner.coop);
                    if matches!(owner.coop.view.phase, super::coop_panel::Phase::Menu | super::coop_panel::Phase::Single) {
                        let recent = owner.library_entries.iter().filter(|e| e.saved > 0).max_by_key(|e| e.saved);
                        if let Some(id) = host_block(ui, recent.map(|e| e.id.as_str()), owner.library_loading) {
                            result = super::UiAction::HostWorld(id);
                        }
                    }
                });
            });
        });
    });
    result
}

/// Host path: returns the latest saved world's id when Host Latest World is pressed.
fn host_block(ui: &mut egui::Ui, latest: Option<&str>, loading: bool) -> Option<String> {
    ui.separator();
    caption_host(ui);
    let mut open = None;
    ui.add_enabled_ui(latest.is_some() && !loading, |ui| {
        if action(ui, "Host Latest World", true).clicked() { open = latest.map(str::to_owned); }
    });
    open
}

fn caption_host(ui: &mut egui::Ui) {
    super::front_end::caption(ui, "INVITE A FRIEND");
    ui.label(RichText::new("Opens your latest world to friends once it has loaded.").small().color(MUTED));
}

/// One listing row; returns the code when Join is pressed.
fn session_row(ui: &mut egui::Ui, entry: &Entry) -> Option<String> {
    let mut joined = None;
    egui::Frame::new().fill(super::themes::current(ui.ctx()).recess)
        .inner_margin(egui::Margin::symmetric(10, 8)).corner_radius(4.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(&entry.name).strong()).on_hover_ui(|ui| {
                        ui.label(format!("Generator {}", entry.version));
                        if !entry.motd.is_empty() { ui.label(&entry.motd); }
                    });
                    let mode = match entry.mode { Mode::Survival => "Survival", Mode::Creative => "Creative" };
                    ui.label(RichText::new(format!("{} · {mode}", entry.world)).small().color(MUTED));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, Glyph::Join, "Join Session").clicked() { joined = Some(entry.code.clone()); }
                    ui.label(RichText::new(format!("{}/{}", entry.players, entry.max_players)).color(MUTED));
                    glyph(ui, Glyph::Players, MUTED);
                });
            });
        });
    ui.add_space(4.0);
    joined
}
