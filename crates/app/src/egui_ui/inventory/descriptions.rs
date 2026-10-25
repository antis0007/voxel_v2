//! Shared copy presentation for inventory slots, recipes and creative items.
use super::Item;

pub(super) fn details(ui: &mut egui::Ui, item: Item) {
    super::super::description_copy::item(ui, item);
}

pub(super) fn expanded(ui: &egui::Ui) -> bool {
    ui.input(|input| input.modifiers.shift)
}
