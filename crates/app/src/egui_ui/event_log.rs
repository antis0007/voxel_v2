//! Session history of HUD notices, newest first, with repeats collapsed.
//! The world keeps only the current notice; this records each one the HUD sees,
//! so multiplayer clients log what the host replicates as well.
use super::presentation as style;
use crate::world::expedition::Expedition;
use egui::{RichText, Sense};
use glam::Vec3;
use std::collections::VecDeque;

const KEEP: usize = 100;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Entry {
    pub text: String,
    pub detail: String,
    pub at: Option<[f32; 3]>,
    /// World clock when the notice last appeared.
    pub clock: String,
    pub count: u32,
}

#[derive(Default)]
pub(super) struct EventLog {
    entries: VecDeque<Entry>,
    serial: Option<u32>,
    /// Player position in metres on the last observed frame.
    eye: [f32; 3],
    clock: String,
}

impl EventLog {
    /// Records the expedition's notice once per serial; no world clears the log.
    pub fn observe(&mut self, expedition: Option<&Expedition>, clock: &str, eye: [f32; 3]) {
        self.eye = eye;
        clock.clone_into(&mut self.clock);
        let Some(e) = expedition else {
            self.entries.clear();
            self.serial = None;
            return;
        };
        if self.serial == Some(e.serial) {
            return;
        }
        let loading = self.serial.is_none();
        self.serial = Some(e.serial);
        // A world that loads with an expired notice does not log it again.
        if e.message.is_empty() || (loading && e.message_seconds <= 0.0) {
            return;
        }
        self.note(&e.message, &e.detail, e.at);
    }

    /// Records a notice drawn outside the expedition banner, such as a sky event.
    pub fn note(&mut self, text: &str, detail: &str, at: Option<[f32; 3]>) {
        if let Some(last) = self.entries.front_mut()
            && last.text == text
            && last.detail == detail
        {
            last.count += 1;
            last.clock.clone_from(&self.clock);
            last.at = at;
            return;
        }
        self.entries.push_front(Entry {
            text: text.to_owned(),
            detail: detail.to_owned(),
            at,
            clock: self.clock.clone(),
            count: 1,
        });
        self.entries.truncate(KEEP);
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    pub fn eye(&self) -> [f32; 3] {
        self.eye
    }
}

/// `42 m NE`, plus the height difference when it is more than 3 m.
pub(super) fn bearing(eye: [f32; 3], at: [f32; 3]) -> String {
    const NAMES: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let d = Vec3::from(at) - Vec3::from(eye);
    let flat = d.x.hypot(d.z);
    if flat < 2.0 && d.y.abs() <= 3.0 {
        return "Here".into();
    }
    // Yaw 0 looks north (-z) and grows toward east (+x).
    let octant = (d.x.atan2(-d.z).to_degrees().rem_euclid(360.0) / 45.0).round() as usize % 8;
    let mut text = format!("{flat:.0} m {}", NAMES[octant]);
    if d.y.abs() > 3.0 {
        text.push_str(&format!(" · {}{:.0} m", if d.y > 0.0 { "↑" } else { "↓" }, d.y.abs()));
    }
    text
}

/// The pause menu's Log page: one row per notice, detail lines on hover.
pub(super) fn page(ui: &mut egui::Ui, log: &EventLog) {
    super::front_end::heading(ui, "Log", "");
    if log.entries.is_empty() {
        ui.label(RichText::new("No Events").color(style::MUTED));
        return;
    }
    for entry in log.entries() {
        let row = ui
            .horizontal(|ui| {
                ui.label(RichText::new(&entry.clock).monospace().small().color(style::MUTED));
                let mut text = RichText::new(&entry.text).color(style::PAPER);
                if !entry.detail.is_empty() {
                    text = text.underline();
                }
                ui.label(text);
                if entry.count > 1 {
                    ui.label(RichText::new(format!("×{}", entry.count)).small().color(style::AMBER));
                }
                if let Some(at) = entry.at {
                    ui.label(RichText::new(bearing(log.eye, at)).small().color(style::MUTED));
                }
            })
            .response
            .interact(Sense::hover());
        if !entry.detail.is_empty() {
            row.on_hover_ui(|ui| {
                ui.label(RichText::new(&entry.text).strong());
                for line in entry.detail.lines() {
                    ui.label(line);
                }
                if let Some(at) = entry.at {
                    ui.label(RichText::new(format!("{:.0}, {:.0}, {:.0} m", at[0], at[1], at[2])).small().color(style::MUTED));
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_collapse_and_new_text_goes_first() {
        let mut log = EventLog::default();
        let mut e = Expedition::default();
        e.say("Herd Full");
        log.observe(Some(&e), "06:00", [0.0; 3]);
        log.observe(Some(&e), "06:00", [0.0; 3]);
        e.say("Herd Full");
        log.observe(Some(&e), "06:01", [0.0; 3]);
        e.say_at("Treasure Gate Open", "Seal: Lever Pin Pulled", Vec3::new(0.0, 0.0, -40.0));
        log.observe(Some(&e), "06:02", [0.0; 3]);
        let entries: Vec<_> = log.entries().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "Treasure Gate Open");
        assert_eq!(entries[0].detail, "Seal: Lever Pin Pulled");
        assert_eq!((entries[1].text.as_str(), entries[1].count, entries[1].clock.as_str()), ("Herd Full", 2, "06:01"));
    }

    #[test]
    fn plain_say_drops_the_previous_detail() {
        let mut e = Expedition::default();
        e.say_at("Cache Open", "Seal: Cracked Wall Broken", Vec3::ZERO);
        e.say("Lumen Lantern: On");
        assert!(e.detail.is_empty() && e.at.is_none());
    }

    #[test]
    fn leaving_the_world_clears_and_an_expired_notice_is_not_relogged() {
        let mut log = EventLog::default();
        let mut e = Expedition::default();
        e.say("Cache Open");
        log.observe(Some(&e), "06:00", [0.0; 3]);
        log.observe(None, "06:00", [0.0; 3]);
        assert_eq!(log.entries().count(), 0);
        e.message_seconds = 0.0;
        log.observe(Some(&e), "06:00", [0.0; 3]);
        assert_eq!(log.entries().count(), 0);
    }

    #[test]
    fn bearing_names_the_compass_octant_and_height() {
        assert_eq!(bearing([0.0; 3], [0.0, 0.0, -40.0]), "40 m N");
        assert_eq!(bearing([0.0; 3], [30.0, -12.0, 30.0]), "42 m SE · ↓12 m");
        assert_eq!(bearing([0.0; 3], [1.0, 1.0, 0.0]), "Here");
    }
}
