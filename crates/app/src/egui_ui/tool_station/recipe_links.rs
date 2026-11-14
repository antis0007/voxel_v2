//! Forge inspection uses the shared recipe browser, never crafting commands.
use super::*;

pub(super) fn item(ui: &mut egui::Ui, response: &egui::Response, item: Item) -> bool {
    let tooltip = |ui: &mut egui::Ui| {
        ui.label(item.name());
        ui.weak("[R] Sources · [U] Uses");
    };
    response.clone().on_hover_ui(tooltip).on_disabled_hover_ui(tooltip);
    #[cfg(test)]
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("forge_recipe_link", item)), response.rect));
    let pointed = response.hovered() || (!response.enabled() && response.contains_pointer());
    if !pointed || ui.ctx().text_edit_focused() { return false; }
    let uses = ui.input(|i| i.events.iter().find_map(|event| match event {
        egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. }
            if !modifiers.ctrl && !modifiers.command && !modifiers.alt => match key {
                egui::Key::R => Some(false),
                egui::Key::U => Some(true),
                _ => None,
            },
        _ => None,
    }));
    if let Some(uses) = uses {
        super::super::recipe_book::request(ui.ctx(), item, uses);
        true
    } else { false }
}

/// Resolve the actual inventory identity; previews never invent a tool-bank id.
pub(super) fn owned(ui: &mut egui::Ui, response: &egui::Response, inv: &SurvivalInventory, id: u16) -> bool {
    let actual = inv.hotbar.iter().chain(&inv.pack).chain(std::iter::once(&inv.offhand))
        .chain(std::iter::once(&inv.cursor)).flatten()
        .find_map(|stack| matches!(stack.item, Item::Assembled { id: found, .. } if found == id).then_some(stack.item));
    actual.is_some_and(|actual| item(ui, response, actual))
}