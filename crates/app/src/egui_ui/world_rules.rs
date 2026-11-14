//! World page rules for the single-player owner of the open world.
use super::UiAction;
use crate::world::rules::{GameMode, MAX_SPAWN_PROTECTION_M, Rules};

pub(super) fn draw(ui: &mut egui::Ui, current: Rules, action: &mut UiAction) {
    let mut next = current;
    ui.label("Game Mode");
    ui.horizontal_wrapped(|ui| {
        for mode in GameMode::ALL {
            let button = egui::Button::selectable(next.mode == mode, mode.label());
            if ui.add(button).on_hover_text(mode.hint()).clicked() {
                next.mode = mode;
            }
        }
    });
    ui.checkbox(&mut next.keep_inventory, "Keep Inventory");
    ui.checkbox(&mut next.mob_spawning, "Mob Spawning");
    ui.checkbox(&mut next.daylight_cycle, "Daylight Cycle");
    ui.checkbox(&mut next.pvp, "PvP");
    ui.horizontal_wrapped(|ui| {
        ui.label("Stacks");
        for value in crate::world::inventory::stacks::Balance::ALL {
            ui.selectable_value(&mut next.stacks, value, value.name());
        }
    });
    ui.add(
        egui::Slider::new(&mut next.spawn_protection_m, 0..=MAX_SPAWN_PROTECTION_M)
            .text("Spawn Protection")
            .suffix(" m"),
    );
    let off: Vec<&str> = crate::world::systems::catalog().iter()
        .filter(|d| !current.systems.enabled(&d.id)).map(|d| d.label.as_str()).collect();
    if !off.is_empty() {
        ui.label(egui::RichText::new(format!("Systems Off · {}", off.join(" · "))).small())
            .on_hover_text("Chosen when the world was created");
    }
    if next != current {
        *action = UiAction::SetRules(next);
    }
    ui.separator();
}
