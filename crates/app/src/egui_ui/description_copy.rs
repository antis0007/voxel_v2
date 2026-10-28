//! Validated functional copy shared by item, recipe and spell hover cards.
use super::presentation;
use egui::RichText;
use engine_content::descriptions::{self, Description, Tier};

pub(super) fn item(ui: &mut egui::Ui, item: crate::world::inventory::Item) {
    let expanded = ui.input(|input| input.modifiers.shift);
    let stats = item.stats_line();
    if !stats.is_empty() {
        ui.label(RichText::new(stats).size(11.0).color(presentation::PAPER));
    }
    for line in crate::world::mining::stat_lines(item) {
        ui.label(RichText::new(line).size(11.0).color(presentation::PAPER));
    }
    show(ui, item.description_entry(), &item.cost_line(), expanded);
    if expanded {
        ui.label(RichText::new(item.category()).small().color(presentation::MUTED));
        if let Some(block) = item.placeable_block() {
            ui.label(RichText::new(format!("Density {:.0} kg/m³",
                block.material().physics.density)).size(11.0));
        }
    }
}

pub(super) fn show(
    ui: &mut egui::Ui,
    entry: Option<&Description>,
    cost: &str,
    expanded: bool,
) {
    if let Some(entry) = entry
        && entry.tier != Tier::Standard && !entry.text.is_empty() {
            line(ui, &entry.text);
        }
    if !cost.is_empty() {
        ui.label(RichText::new(cost).size(11.0).color(presentation::MANA));
    }
    if expanded
        && let Some(entry) = entry.filter(|entry| !entry.codex.is_empty()) {
            ui.separator();
            line(ui, &entry.codex);
        }
}

fn line(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(11.0).color(presentation::MUTED))
        .on_hover_ui(|ui| {
            for keyword in descriptions::keywords(text) {
                ui.strong(&keyword.name);
                ui.label(&keyword.definition);
            }
        });
}
