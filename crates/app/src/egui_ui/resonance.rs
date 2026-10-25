//! Forge controls issue authoritative actions; no run state lives in the UI.
use super::{UiAction, presentation as style};
use crate::world::{adventure::resonance::{Action, BOONS, State}, factory};
use egui::{RichText, Ui};

pub(super) fn draw(ui: &mut Ui, run: &State, bench: bool, action: &mut UiAction) {
    let mut send = |command| *action = UiAction::Factory(factory::Action::Resonance(command));
    ui.heading("Expeditions");
    ui.label(RichText::new(format!("Resonator {} / 5  ·  {} expeditions banked", run.calibration, run.completed)).color(style::MUTED));
    if !run.active {
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(bench, egui::Button::new("Begin"))
                .on_hover_text("1x Crystal + 1x Coil").clicked() { send(Action::Begin); }
            let tier = run.calibration + 1;
            if ui.add_enabled(bench && run.calibration < 5,
                egui::Button::new("Calibrate"))
                .on_hover_text(format!("{}x Circuits + {tier}x Coils + {tier}x Cores\nDamage −2% / Level", tier * 2)).clicked() {
                send(Action::Calibrate);
            }
        });
        if !bench { ui.label(RichText::new("Workbench required").color(style::AMBER)); }
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("Pressure {} / 5", run.pressure())).color(style::AMBER));
        ui.label(format!("{} guardians · {} salvage · {:+.0}% damage taken", run.kills, run.salvage, (run.incoming() - 1.0) * 100.0));
    });
    ui.label(RichText::new("Pressure +1 / 8 Kills · +1 / 90s").color(style::MUTED));
    for (id, &(name, detail)) in BOONS.iter().enumerate() {
        if run.boons[id] > 0 {
            ui.label(format!("{name} ×{}", run.boons[id])).on_hover_text(detail);
        }
    }
    if run.pending > 0 {
        ui.label(RichText::new(format!("Upgrades {}x", run.pending)).color(style::MANA));
        for id in run.offers() {
            let (name, detail) = BOONS[id];
            if ui.button(format!("{name}  →  stack {}", run.boons[id] + 1)).clicked() { send(Action::Choose(id)); }
            ui.label(RichText::new(detail).small().color(style::MUTED));
        }
    }
    ui.label(format!("Bank payout: {} steel, {:.1} coils, {} cores", run.salvage, run.salvage as f32 * 0.5, run.salvage / 8));
    if ui.add_enabled(bench, egui::Button::new("Bank Salvage")).clicked() { send(Action::Bank); }
    if !bench { ui.label(RichText::new("Workbench Required").color(style::AMBER)); }
}
