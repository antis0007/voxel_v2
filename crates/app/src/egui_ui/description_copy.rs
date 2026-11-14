//! Validated functional copy shared by item, recipe and spell hover cards.
use super::presentation;
use egui::RichText;
use engine_content::descriptions::{self, Description, Tier};

pub(super) fn item(ui: &mut egui::Ui, item: crate::world::inventory::Item) {
    let expanded = ui.input(|input| input.modifiers.shift);
    if let crate::world::inventory::Item::Assembled{id,..}=item {
        if let Some(tool)=super::held_art::tool(id) {
            if let Ok(stats)=tool.stats() {
                ui.label(format!("Durability: {} / {}",tool.durability_left,stats.durability));
                ui.label(format!("Mining: {:.1} · Tier: {}",stats.mining_speed,stats.mining_level));
                ui.label(format!("Damage: {:.1} · Slots: {} / {}",crate::world::tool_workshop::attack_damage(&tool),tool.modifiers.len(),tool.modifier_slots().unwrap_or(0)));
            }
            if expanded {
                for (part,material) in tool.resolved().unwrap_or_default() {
                    ui.label(format!("{}: {}",part.display_name,material.display_name));
                }
                for applied in tool.traits().unwrap_or_default() {
                    if let Some(rule)=engine_tools::TRAITS.iter().find(|r|r.effect==applied.effect) {ui.label(format!("{} ×{}",rule.display_name,applied.stacks));}
                }
                for row in engine_tools::modifiers::ROWS.iter() {
                    let rank=tool.modifiers.iter().filter(|m|**m==row.id).count();
                    if rank>0 {ui.label(format!("{}: {}",row.display_name,rank));}
                }
            }
            return;
        }
    }
    let stats = item.stats_line();
    if !stats.is_empty() {
        ui.label(RichText::new(stats).size(13.0).color(presentation::PAPER));
    }
    for line in crate::world::mining::stat_lines(item) {
        ui.label(RichText::new(line).size(13.0).color(presentation::PAPER));
    }
    body(ui, item.description_entry(), &item.cost_line(), expanded,
        descriptions::flavour(item.name()));
    if expanded {
        ui.label(RichText::new(item.category()).small().color(presentation::MUTED));
        if let Some(block) = item.placeable_block() {
            ui.label(RichText::new(format!("Density {:.0} kg/m³",
                block.material().physics.density)).size(13.0));
        }
    }
}

pub(super) fn show(
    ui: &mut egui::Ui,
    entry: Option<&Description>,
    cost: &str,
    expanded: bool,
) {
    body(ui, entry, cost, expanded, None);
}

fn body(
    ui: &mut egui::Ui,
    entry: Option<&Description>,
    cost: &str,
    expanded: bool,
    flavour: Option<&str>,
) {
    if let Some(entry) = entry
        && entry.tier != Tier::Standard && !entry.text.is_empty() {
            line(ui, &entry.text);
        }
    if !cost.is_empty() {
        ui.label(RichText::new(cost).size(13.0).color(presentation::MANA));
    }
    // Flavour sits after everything a player acts on and before the codex.
    if let Some(flavour) = flavour {
        ui.label(RichText::new(flavour).size(12.5).italics()
            .color(presentation::MUTED.gamma_multiply(0.85)));
    }
    if expanded
        && let Some(entry) = entry.filter(|entry| !entry.codex.is_empty()) {
            ui.separator();
            line(ui, &entry.codex);
        }
}

fn line(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(13.0).color(presentation::MUTED))
        .on_hover_ui(|ui| {
            for keyword in descriptions::keywords(text) {
                ui.strong(&keyword.name);
                ui.label(&keyword.definition);
            }
        });
}
