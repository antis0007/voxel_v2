//! Chat history, the player list and typed lines waiting to be sent.
//! Pure data: the session fills it from packets and the HUD draws it.
use engine_net::protocol::{ChatKind, PlayerId};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, time::Instant};

/// Lines kept for scrollback; older lines are dropped.
pub const HISTORY: usize = 100;
/// Lines a player may queue before the session sends them.
const OUTBOX: usize = 16;

pub struct Line {
    pub kind: ChatKind,
    pub text: String,
    pub at: Instant,
}

/// One player list entry, as the host sends it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: PlayerId,
    pub name: String,
    pub op: bool,
    pub creative: bool,
}

#[derive(Default)]
pub struct Social {
    lines: VecDeque<Line>,
    roster: Vec<Row>,
    names: std::collections::BTreeMap<PlayerId, String>,
    outbox: Vec<String>,
    /// Player id this side plays as; `None` until the host assigns one.
    pub own: Option<PlayerId>,
    /// Lines pushed since `take_unseen`, for a console with no overlay.
    unseen: usize,
    /// Round trip, jitter and stalls per player, from the host's echo probes.
    pub quality: std::collections::BTreeMap<PlayerId, engine_netcode::quality::Quality>,
    pub board: engine_netcode::scoreboard::Board,
    /// The session's lobby code, for inviting: the host learns it from the
    /// lobby, a joiner keeps the one it used. `None` for direct-address sessions.
    pub code: Option<String>,
}

impl Social {
    pub fn lines(&self) -> impl DoubleEndedIterator<Item = &Line> + ExactSizeIterator {
        self.lines.iter()
    }

    pub fn roster(&self) -> &[Row] {
        &self.roster
    }

    pub fn name_of(&self, id: PlayerId) -> Option<&str> {
        self.roster.iter().find(|r| r.id == id).map(|r| r.name.as_str())
            .or_else(|| self.names.get(&id).map(String::as_str))
    }
    pub fn learn_names(&mut self, names: Vec<(PlayerId,String)>) {
        self.names = names.into_iter().take(17).collect();
    }

    pub fn own_row(&self) -> Option<&Row> {
        self.own.and_then(|id| self.roster.iter().find(|r| r.id == id))
    }

    /// Records a received line. Player chat is prefixed with the speaker's name;
    /// the command layer already words every other kind.
    pub fn push(&mut self, kind: ChatKind, from: PlayerId, text: &str) {
        let text = match kind {
            ChatKind::Say => format!("<{}> {text}", self.name_of(from).unwrap_or("?")),
            _ => text.to_owned(),
        };
        self.push_text(kind, text);
    }

    pub fn push_text(&mut self, kind: ChatKind, text: String) {
        if self.lines.len() >= HISTORY {
            self.lines.pop_front();
        }
        self.lines.push_back(Line { kind, text, at: Instant::now() });
        self.unseen = (self.unseen + 1).min(HISTORY);
    }

    /// Texts of the lines added since the last call.
    pub fn take_unseen(&mut self) -> Vec<String> {
        let skip = self.lines.len() - self.unseen.min(self.lines.len());
        self.unseen = 0;
        self.lines.iter().skip(skip).map(|l| l.text.clone()).collect()
    }

    /// Replaces the player list and announces arrivals and departures.
    pub fn set_roster(&mut self, roster: Vec<Row>) {
        let previous = std::mem::replace(&mut self.roster, roster);
        if previous.is_empty() {
            return;
        }
        let joined: Vec<_> = self.roster.iter().filter(|r| !previous.iter().any(|p| p.id == r.id)).map(|r| format!("{} joined", r.name)).collect();
        let left: Vec<_> = previous.iter().filter(|p| !self.roster.iter().any(|r| r.id == p.id)).map(|p| format!("{} left", p.name)).collect();
        for text in joined.into_iter().chain(left) {
            self.push_text(ChatKind::System, text);
        }
    }

    /// Queues a typed line. Blank lines are ignored; a full outbox drops the line.
    pub fn send(&mut self, line: &str) {
        let line = line.trim();
        if !line.is_empty() && self.outbox.len() < OUTBOX {
            self.outbox.push(line.to_owned());
        }
    }

    pub fn take_outbox(&mut self) -> Vec<String> {
        std::mem::take(&mut self.outbox)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: PlayerId, name: &str) -> Row {
        Row { id, name: name.into(), op: false, creative: false }
    }

    #[test]
    fn say_lines_carry_the_speaker_and_history_is_bounded() {
        let mut social = Social::default();
        social.set_roster(vec![row(0, "Ana"), row(3, "Bo")]);
        social.push(ChatKind::Say, 3, "hi");
        social.push(ChatKind::System, 0, "Bo was kicked: spam");
        let texts: Vec<_> = social.lines().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["<Bo> hi", "Bo was kicked: spam"]);
        for i in 0..HISTORY + 5 {
            social.push_text(ChatKind::System, i.to_string());
        }
        assert_eq!(social.lines().len(), HISTORY);
        assert_eq!(social.lines().last().map(|l| l.text.as_str()), Some("104"));
    }

    #[test]
    fn roster_changes_are_announced_after_the_first_list() {
        let mut social = Social::default();
        social.set_roster(vec![row(0, "Ana")]);
        assert_eq!(social.lines().len(), 0);
        social.set_roster(vec![row(0, "Ana"), row(4, "Cy")]);
        social.set_roster(vec![row(4, "Cy")]);
        let texts: Vec<_> = social.lines().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["Cy joined", "Ana left"]);
        assert_eq!(social.take_unseen(), ["Cy joined", "Ana left"]);
        assert!(social.take_unseen().is_empty());
    }

    #[test]
    fn outbox_trims_ignores_blank_and_is_bounded() {
        let mut social = Social::default();
        social.send("   ");
        social.send(" /list ");
        for _ in 0..40 {
            social.send("x");
        }
        let sent = social.take_outbox();
        assert_eq!(sent.len(), OUTBOX);
        assert_eq!(sent[0], "/list");
        assert!(social.take_outbox().is_empty());
    }
}
