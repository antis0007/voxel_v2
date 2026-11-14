//! Friends panel: open the current world, show its session code, join or leave by code.
//! The panel only records requests; the game loop owns sessions and reports their state.
use super::front_end::action;
use egui::{Color32, RichText};

const MUTED: Color32 = Color32::from_rgb(156, 148, 170);
const BRIGHT: Color32 = Color32::from_rgb(236, 226, 204);
const WARN: Color32 = Color32::from_rgb(230, 130, 110);

/// Vector controls stay legible with every UI font and scale.
#[derive(Clone, Copy)]
pub(super) enum Glyph { Refresh, Copy, Check, Join, Players, Play, World, Plus, Settings, Exit }

pub(super) fn glyph(ui: &mut egui::Ui, glyph: Glyph, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    paint_glyph(ui.painter(), rect, glyph, color);
}

pub(super) fn icon_button(ui: &mut egui::Ui, glyph: Glyph, label: &str) -> egui::Response {
    let response = ui.add(egui::Button::new("").min_size(egui::vec2(30.0, 28.0)));
    paint_glyph(ui.painter(), response.rect.shrink(6.0), glyph, ui.visuals().text_color());
    response.clone().on_hover_text(label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));
    response
}

pub(super) fn paint_glyph(painter: &egui::Painter, rect: egui::Rect, glyph: Glyph, color: Color32) {
    let p = |x: f32, y: f32| rect.min + rect.size() * egui::vec2(x, y);
    let stroke = egui::Stroke::new(1.6, color);
    let line = |a, b| { painter.line_segment([a, b], stroke); };
    match glyph {
        Glyph::Play => { painter.add(egui::Shape::convex_polygon(vec![p(0.2,0.0),p(1.0,0.5),p(0.2,1.0)], color, egui::Stroke::NONE)); }
        Glyph::Plus => { line(p(0.5,0.0),p(0.5,1.0)); line(p(0.0,0.5),p(1.0,0.5)); }
        Glyph::World => {
            painter.rect_stroke(rect.shrink(1.0), 1.0, stroke, egui::StrokeKind::Inside);
            line(p(0.0,0.35),p(1.0,0.35)); line(p(0.0,0.7),p(1.0,0.7));
            line(p(0.35,0.0),p(0.35,1.0)); line(p(0.7,0.0),p(0.7,1.0));
        }
        Glyph::Settings => {
            for (x,y) in [(0.15,0.3),(0.5,0.7),(0.85,0.45)] {
                line(p(x,0.0),p(x,1.0));
                painter.rect_filled(egui::Rect::from_min_max(p(x-0.1,y-0.08),p(x+0.1,y+0.08)), 1.0, color);
            }
        }
        Glyph::Exit => {
            line(p(0.4,0.0),p(0.0,0.0)); line(p(0.0,0.0),p(0.0,1.0)); line(p(0.0,1.0),p(0.4,1.0));
            line(p(0.4,0.5),p(1.0,0.5)); line(p(0.7,0.2),p(1.0,0.5)); line(p(0.7,0.8),p(1.0,0.5));
        }
        Glyph::Join => {
            line(p(0.0,0.5),p(0.8,0.5)); line(p(0.5,0.2),p(0.8,0.5)); line(p(0.5,0.8),p(0.8,0.5));
            line(p(1.0,0.0),p(1.0,1.0));
        }
        Glyph::Check => { line(p(0.0,0.5),p(0.35,0.85)); line(p(0.35,0.85),p(1.0,0.15)); }
        Glyph::Copy => {
            painter.rect_stroke(egui::Rect::from_min_max(p(0.3,0.3),p(1.0,1.0)), 1.0, stroke, egui::StrokeKind::Inside);
            line(p(0.0,0.7),p(0.0,0.0)); line(p(0.0,0.0),p(0.7,0.0));
        }
        Glyph::Refresh => {
            let points: Vec<_> = (0..=16).map(|i| {
                let angle = 0.4 + i as f32 * 5.2 / 16.0;
                p(0.5 + angle.cos()*0.42, 0.5 + angle.sin()*0.42)
            }).collect();
            painter.add(egui::Shape::line(points, stroke));
            line(p(0.83,0.23),p(0.95,0.02)); line(p(0.83,0.23),p(0.58,0.16));
        }
        Glyph::Players => {
            painter.circle_stroke(p(0.35,0.25), rect.width()*0.15, stroke);
            painter.circle_stroke(p(0.8,0.3), rect.width()*0.12, stroke);
            line(p(0.05,0.95),p(0.1,0.62)); line(p(0.1,0.62),p(0.6,0.62)); line(p(0.6,0.62),p(0.65,0.95));
            line(p(0.75,0.65),p(0.95,0.65)); line(p(0.95,0.65),p(1.0,0.95));
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    OpenNative { address: String },
    #[cfg(feature = "multiplayer-sdr")]
    InviteFriends(String),
    /// Hosts the open world. An empty name keeps the session out of the browser.
    Open { name: String, listed: bool },
    Close,
    Join { code: String },
    AcceptAnswer(String),
    Leave,
    Cancel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// No world is open.
    #[default]
    Menu,
    Single,
    Opening,
    Hosting,
    Connecting,
    Joined,
    /// A command-line session; the panel shows its code but cannot close it.
    Fixed,
}

/// What the game loop reports each frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub phase: Phase,
    pub code: Option<String>,
    pub players: usize,
    pub status: Option<String>,
}

#[derive(Default)]
pub struct PanelState {
    relay: bool,
    bind: String,
    pub view: View,
    code: String,
    answer: String,
    name: String,
    listed: bool,
    clipboard_checked: bool,
    paste: Option<PasteTarget>,
    pasted_invite: bool,
    pasted_reply: bool,
    clipboard_error: Option<&'static str>,
}

enum PasteTarget { Invite, Reply }

fn punch_token<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.split_ascii_whitespace().map(|word| word.trim_matches(['`', '<', '>']))
        .find(|word| word.starts_with(prefix) && word.len() <= engine_net::punch::CODE_LIMIT)
}

fn invite_text(text: &str) -> &str {
    punch_token(text, engine_net::punch::FRIENDS_PREFIX)
        .or_else(|| punch_token(text, engine_net::punch::OFFER_PREFIX)).unwrap_or(text.trim())
}

/// Lobby codes are 8 letters or digits, dashes and spaces ignored.
pub fn looks_like_code(text: &str) -> bool {
    let text = invite_text(text);
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    if engine_net::punch::is_offer(text) {
        return engine_net::punch::Offer::decode(text).is_ok_and(|offer|
            offer.signaling_host().is_none() || cfg!(feature = "multiplayer-sdr"));
    }
    #[cfg(feature="multiplayer-sdr")]
    if text.strip_prefix("steam:").and_then(|id|id.parse::<u64>().ok()).is_some_and(|id|id>0) {return true;}
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    if text.parse::<std::net::SocketAddr>().is_ok()||engine_net::invite::decode(text).is_ok() {return true;}
    text.len() <= 12
        && text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | ' '))
        && text.chars().filter(|c| c.is_ascii_alphanumeric()).count() == 8
}

impl PanelState {
    /// The panel is visible with an empty code field and the clipboard was not read yet.
    pub fn wants_clipboard(&self) -> bool {
        self.paste.is_some() || (!self.clipboard_checked && self.code.is_empty()
            && matches!(self.view.phase, Phase::Menu | Phase::Single))
    }
    pub fn offer_clipboard(&mut self, text: Option<String>) {
        self.clipboard_checked = true;
        let paste = self.paste.take();
        self.clipboard_error = None;
        if matches!(paste, Some(PasteTarget::Reply)) {
            if let Some(reply) = text.as_deref().and_then(|text| punch_token(text, engine_net::punch::ANSWER_PREFIX)) {
                self.answer = reply.to_owned();
                self.pasted_reply = true;
            } else {
                self.clipboard_error = Some("No Reply Code");
            }
        } else if let Some(text) = text.filter(|t| looks_like_code(t)) {
            self.code = invite_text(&text).to_owned();
            self.pasted_invite = matches!(paste, Some(PasteTarget::Invite));
        } else if paste.is_some() {
            self.clipboard_error = Some("No Invite Code");
        }
    }
    pub fn code_text(&self) -> Option<&str> {
        Some(invite_text(&self.code)).filter(|c| !c.is_empty())
    }
    /// Read the clipboard again the next time the panel opens.
    pub fn hidden(&mut self) {
        self.clipboard_checked = false;
        self.paste = None;
        self.pasted_invite = false;
        self.pasted_reply = false;
        self.clipboard_error = None;
    }
}

fn status_line(ui: &mut egui::Ui, view: &View) {
    let pending = matches!(view.phase, Phase::Opening | Phase::Connecting);
    let fallback = if view.phase == Phase::Opening { "Opening" } else { "Connecting" };
    if let Some(status) = view.status.as_deref().or(pending.then_some(fallback)) {
        let pending = pending || matches!(status, "Loading World" | "Preparing Invite" | "Waiting for Host");
        // Keep diagnostic chains available without letting a socket error
        // dominate the panel or push its controls off a narrow screen.
        let label = if status.contains("No direct UDP path") { "Direct Connection Unavailable" }
            else if status.contains("Invite reply expired") { "Reply expired. Join again and send the new reply." }
            else if status.contains("timed out") { "Connection Timed Out" }
            else if status.contains("different game version") || status.contains("different game build") || status.contains("different world content") { "Game Builds Differ" }
            else if status.contains("session is full") { "Session Full" }
            else if status.contains("session is closing") { "Session Closed" }
            else if status.contains("no session has that code") { "Session Code Not Found" }
            else if status.starts_with("Join Failed:") { "Join Failed" }
            else if status.starts_with("Open Failed:") { "Hosting Failed" }
            else { status };
        ui.horizontal_wrapped(|ui| {
            if pending { ui.spinner(); }
            ui.label(RichText::new(label).size(14.0).color(if pending || status == "Reply Accepted" { MUTED } else { WARN }))
                .on_hover_text(status);
        });
    }
}

fn code_row(ui: &mut egui::Ui, code: &str) {
    ui.horizontal_wrapped(|ui| {
        let id = egui::Id::new(("copied_session", code));
        let now = ui.input(|i| i.time);
        let copied = ui.ctx().data(|d| d.get_temp::<f64>(id)).is_some_and(|until| until > now);
        let copy = if engine_net::punch::is_offer(code) { Some("Copy Invite") }
            else if code.starts_with(engine_net::punch::ANSWER_PREFIX) { Some("Copy Reply") }
            else { None };
        let clicked = if let Some(label) = copy {
            action(ui, if copied { "Copied" } else { label }, true).clicked()
        } else {
            ui.label(RichText::new(code).monospace().size(22.0).strong().color(BRIGHT));
            icon_button(ui, if copied { Glyph::Check } else { Glyph::Copy }, if copied { "Copied" } else { "Copy Code" }).clicked()
        };
        if clicked {
            ui.ctx().copy_text(code.to_owned());
            ui.ctx().data_mut(|d| d.insert_temp(id, now + 2.0));
            ui.ctx().request_repaint_after(std::time::Duration::from_secs(2));
        }
    });
}

fn join_row(ui: &mut egui::Ui, state: &mut PanelState, request: &mut Option<Request>) {
    let pasted_invite = std::mem::take(&mut state.pasted_invite);
    ui.horizontal_wrapped(|ui| {
        let width = ui.available_width().min(160.0);
        let field = ui.add(egui::TextEdit::singleline(&mut state.code).hint_text("Invite Code")
            .char_limit(engine_net::punch::CODE_LIMIT).desired_width(width));
        if field.changed() { state.clipboard_error = None; }
        if ui.button("Paste & Join").clicked() { state.paste = Some(PasteTarget::Invite); }
        let submit = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let valid = looks_like_code(&state.code);
        if ui.add_enabled(valid, egui::Button::new("Join Friend")).clicked() || (valid && (submit || pasted_invite)) {
            *request = Some(Request::Join { code: invite_text(&state.code).to_owned() });
        }
    });
    if let Some(error) = state.clipboard_error { ui.label(RichText::new(error).color(WARN)); }
}

fn host_options(ui: &mut egui::Ui, state: &mut PanelState, request: &mut Option<Request>) {
    #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
    {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.relay, false, "Direct Address");
            ui.selectable_value(&mut state.relay, true, "Public Lobby");
        });
        if !state.relay {
            ui.add(egui::TextEdit::singleline(&mut state.bind).hint_text("0.0.0.0:27020")
                .desired_width(180.0).char_limit(96));
            if action(ui, "Host by Address", false).clicked() {
                let address = state.bind.trim();
                *request = Some(Request::OpenNative { address: if address.is_empty() { "0.0.0.0:27020" } else { address }.to_owned() });
            }
            return;
        }
    }
    ui.checkbox(&mut state.listed, "Listed");
    if state.listed {
        ui.add(egui::TextEdit::singleline(&mut state.name).hint_text("Session Name")
            .char_limit(engine_net::lobby::wire::MAX_NAME).desired_width(240.0));
    }
    if action(ui, "Host in Lobby", false).clicked() {
        *request = Some(Request::Open { name: state.name.trim().to_owned(), listed: state.listed });
    }
}

fn connecting(ui: &mut egui::Ui, view: &View) -> Option<Request> {
    if let Some(code) = view.code.as_deref() {
        code_row(ui, code);
        if code.starts_with(engine_net::punch::ANSWER_PREFIX) {
            ui.label(RichText::new("Send the reply to the host. They choose Paste & Connect in Friends.").color(MUTED));
        }
    }
    status_line(ui, view);
    action(ui, "Cancel", false).clicked().then_some(Request::Cancel)
}

/// Pause-menu page. Returns the request made this frame.
pub fn pause_page(ui: &mut egui::Ui, state: &mut PanelState) -> Option<Request> {
    let mut request = None;
    let pasted_reply = std::mem::take(&mut state.pasted_reply);
    let view = state.view.clone();
    match view.phase {
        Phase::Menu => join_row(ui, state, &mut request),
        Phase::Single => {
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            if action(ui, "Play With Friends", true).clicked() {
                let address = if cfg!(feature = "multiplayer-sdr") { "friends" } else { "punch" };
                return Some(Request::OpenNative { address: address.into() });
            }
            ui.separator();
            join_row(ui, state, &mut request);
            #[cfg(any(feature = "multiplayer-steam", feature = "multiplayer-sdr"))]
            ui.collapsing("Advanced", |ui| host_options(ui, state, &mut request));
            #[cfg(not(any(feature = "multiplayer-steam", feature = "multiplayer-sdr")))]
            host_options(ui, state, &mut request);
            status_line(ui, &view);
        }
        Phase::Opening | Phase::Connecting => return connecting(ui, &view),
        Phase::Hosting | Phase::Fixed => {
            if let Some(code) = &view.code {
                code_row(ui, code);
                #[cfg(feature = "multiplayer-sdr")]
                if code.starts_with(engine_net::punch::FRIENDS_PREFIX) && ui.button("Invite on Steam").clicked() {
                    request = Some(Request::InviteFriends(code.clone()));
                }
                if code.starts_with(engine_net::punch::OFFER_PREFIX) {
                    ui.horizontal_wrapped(|ui| {
                        let field = ui.add(egui::TextEdit::singleline(&mut state.answer).hint_text("Reply Code")
                            .char_limit(engine_net::punch::CODE_LIMIT).desired_width(160.0));
                        if field.changed() { state.clipboard_error = None; }
                        if ui.button("Paste & Connect").clicked() { state.paste = Some(PasteTarget::Reply); }
                        let reply = punch_token(&state.answer, engine_net::punch::ANSWER_PREFIX);
                        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let clicked = ui.add_enabled(reply.is_some(), egui::Button::new("Connect Friend")).clicked();
                        if (pasted_reply || clicked || enter) && let Some(reply) = reply {
                            request = Some(Request::AcceptAnswer(reply.to_owned()));
                            state.answer.clear();
                        }
                    });
                    if let Some(error) = state.clipboard_error { ui.label(RichText::new(error).color(WARN)); }
                }
            }
            ui.label(RichText::new(format!("Players {}", view.players + 1)).color(MUTED));
            if view.phase == Phase::Hosting && action(ui, "Stop Hosting", false).clicked() {
                request = Some(Request::Close);
            }
        }
        Phase::Joined => {
            if let Some(code) = &view.code {
                code_row(ui, code);
            }
            ui.label(RichText::new(format!("Players {}", view.players + 1)).color(MUTED));
            if action(ui, "Leave", false).clicked() {
                request = Some(Request::Leave);
            }
        }
    }
    if matches!(view.phase, Phase::Menu | Phase::Hosting | Phase::Joined | Phase::Fixed) {
        status_line(ui, &view);
    }
    request
}

/// Game-page summary: the code while in a session, a way to the Online page otherwise.
/// Returns true when the Online page should open.
pub fn summary(ui: &mut egui::Ui, state: &PanelState) -> bool {
    match state.view.phase {
        Phase::Single => action(ui, "Friends", false).clicked(),
        Phase::Hosting | Phase::Joined | Phase::Fixed => {
            if let Some(code) = &state.view.code {
                code_row(ui, code);
            }
            false
        }
        Phase::Opening | Phase::Connecting => {
            ui.label(RichText::new(if state.view.phase == Phase::Opening { "Opening" } else { "Connecting" }).color(MUTED));
            false
        }
        Phase::Menu => false,
    }
}

/// Main-menu join: the code field, its state line and Cancel while connecting.
pub fn menu_join(ui: &mut egui::Ui, state: &mut PanelState) -> Option<Request> {
    let mut request = None;
    if state.view.phase == Phase::Connecting {
        return connecting(ui, &state.view);
    } else if state.view.phase == Phase::Menu {
        join_row(ui, state, &mut request);
    } else {
        return pause_page(ui, state);
    }
    status_line(ui, &state.view);
    request
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_recognised_from_pasted_text() {
        assert!(looks_like_code("ABCD-EFGH"));
        assert!(looks_like_code(" abcd efgh \n"));
        assert!(looks_like_code("A1B2C3D4"));
        assert!(!looks_like_code("ABCD-EFG"));
        assert!(!looks_like_code("hello world, how are you"));
        assert!(!looks_like_code("ABCD/EFGH"));
    }

    #[test]
    fn clipboard_fills_an_empty_code_field_once() {
        let mut state = PanelState::default();
        state.view.phase = Phase::Single;
        assert!(state.wants_clipboard());
        state.offer_clipboard(Some("not a code at all".into()));
        assert!(!state.wants_clipboard() && state.code.is_empty());
        state.hidden();
        state.offer_clipboard(Some("WXYZ-2345".into()));
        assert_eq!(state.code, "WXYZ-2345");
        assert!(!state.wants_clipboard());
    }
}
