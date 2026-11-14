//! Player names and chat policy shared by the host and every client.
//!
//! Both travel over the wire and are shown to other players, so both are bounded
//! and stripped of control characters before they reach the session.

use std::time::{Duration, Instant};

/// Longest name a player may ask for, in characters.
pub const MAX_NAME: usize = 16;
/// Longest chat line, in bytes.
pub const MAX_CHAT: usize = 240;
/// Chat messages allowed per peer inside `CHAT_WINDOW`.
pub const CHAT_MESSAGES: u32 = 5;
pub const CHAT_WINDOW: Duration = Duration::from_secs(5);

fn name_character_allowed(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')
}

/// Clean a requested name, or explain why it cannot be used.
///
/// Names are printable ASCII, at most `MAX_NAME` characters, and use single
/// spaces only in the middle. Anything else would either break the wire or make
/// a name impossible to type back in a command.
pub fn sanitize_name(raw: &str) -> Result<String, &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("name is empty");
    }
    if trimmed.chars().count() > MAX_NAME {
        return Err("name is longer than 16 characters");
    }
    if !trimmed.chars().all(|c| name_character_allowed(c) || c == ' ') {
        return Err("name uses a character that is not allowed");
    }
    if trimmed.starts_with(' ') || trimmed.ends_with(' ') || trimmed.contains("  ") {
        return Err("name has stray spaces");
    }
    Ok(trimmed.to_owned())
}

/// First free name in the `Ana`, `Ana (2)`, `Ana (3)` sequence.
///
/// The suffix counts up, so a name stays predictable when the same person
/// reconnects after a drop.
pub fn unique_name(wanted: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(wanted) {
        return wanted.to_owned();
    }
    for suffix in 2..10_000u32 {
        let candidate = format!("{wanted} ({suffix})");
        if !taken(&candidate) {
            return candidate;
        }
    }
    wanted.to_owned()
}

/// Clean a chat line or explain why it cannot be sent.
pub fn sanitize_chat(raw: &str) -> Result<String, &'static str> {
    let stripped: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = stripped.trim();
    if trimmed.is_empty() {
        return Err("message is empty");
    }
    if trimmed.len() > MAX_CHAT {
        return Err("message is too long");
    }
    Ok(trimmed.to_owned())
}

/// Sliding-window chat allowance for one peer.
#[derive(Debug, Clone)]
pub struct ChatBudget {
    window_start: Instant,
    used: u32,
}

impl ChatBudget {
    pub fn new(now: Instant) -> Self {
        Self { window_start: now, used: 0 }
    }

    /// Spends one message. `false` means the peer is talking too fast.
    pub fn allow(&mut self, now: Instant) -> bool {
        if now.duration_since(self.window_start) >= CHAT_WINDOW {
            self.window_start = now;
            self.used = 0;
        }
        if self.used >= CHAT_MESSAGES {
            return false;
        }
        self.used += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_keep_to_the_visible_alphabet() {
        assert_eq!(sanitize_name("  Ana  "), Ok("Ana".to_owned()));
        assert_eq!(sanitize_name("ana_2"), Ok("ana_2".to_owned()));
        assert_eq!(sanitize_name("Big Boss"), Ok("Big Boss".to_owned()));
        assert!(sanitize_name("").is_err());
        assert!(sanitize_name("   ").is_err());
        assert!(sanitize_name("bad\nname").is_err());
        assert!(sanitize_name("double  space").is_err());
        assert!(sanitize_name("seventeen_chars_x").is_err());
        assert_eq!(sanitize_name("sixteen_chars_ab").unwrap().len(), 16);
        assert!(sanitize_name("sixteen_chars_abc").is_err());
    }

    #[test]
    fn duplicate_names_gain_a_counted_suffix() {
        let taken = ["Ana", "Ana (2)"];
        assert_eq!(unique_name("Ana", |name| taken.contains(&name)), "Ana (3)");
        assert_eq!(unique_name("Bo", |name| taken.contains(&name)), "Bo");
    }

    #[test]
    fn chat_is_trimmed_and_loses_control_characters() {
        assert_eq!(sanitize_chat("  hello\nthere  "), Ok("hello there".to_owned()));
        assert!(sanitize_chat("   ").is_err());
        assert!(sanitize_chat(&"x".repeat(MAX_CHAT + 1)).is_err());
        assert!(sanitize_chat(&"x".repeat(MAX_CHAT)).is_ok());
    }

    #[test]
    fn chat_budget_refills_on_the_next_window() {
        let start = Instant::now();
        let mut budget = ChatBudget::new(start);
        for _ in 0..CHAT_MESSAGES {
            assert!(budget.allow(start));
        }
        assert!(!budget.allow(start), "the window must run out");
        assert!(budget.allow(start + CHAT_WINDOW));
    }
}
