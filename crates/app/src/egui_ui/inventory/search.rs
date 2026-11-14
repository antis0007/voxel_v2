//! Locate carried and open-chest stacks without rearranging inventory.
use crate::world::{inventory::{Item, SurvivalInventory}, technology};
use egui::{Color32, Id, Painter, Rect, Response, RichText, Stroke, StrokeKind, vec2};
use std::collections::HashMap;

const STATE: &str = "inventory_locate";

#[derive(Clone, Default)]
struct Search {
    query: String,
    enabled: bool,
    matches: HashMap<Item, bool>,
}

impl Search {
    fn update(&mut self, query: &str, enabled: bool) {
        let query = query.trim().to_lowercase();
        if self.query != query {
            self.query = query;
            self.matches.clear();
        }
        self.enabled = enabled && !self.query.is_empty();
    }

    fn matches(&mut self, item: Item) -> Option<bool> {
        self.enabled.then(|| *self.matches.entry(item)
            .or_insert_with(|| technology::matches(item, &self.query)))
    }

    fn counts(&mut self, inventory: &SurvivalInventory) -> (usize, usize) {
        let slots = inventory.pack.iter().chain(&inventory.hotbar)
            .chain(inventory.container.iter().flat_map(|(_, slots)| slots));
        let mut found = 0;
        let mut occupied = 0;
        for stack in slots.flatten() {
            occupied += 1;
            found += usize::from(self.matches(stack.item) == Some(true));
        }
        (found, occupied)
    }
}

fn with_search<R>(ctx: &egui::Context, f: impl FnOnce(&mut Search) -> R) -> R {
    ctx.data_mut(|data| f(data.get_temp_mut_or_default::<Search>(Id::unique(STATE))))
}

/// Query, clear, and explicit inventory locate controls share one row.
pub(in crate::egui_ui) fn row(ui: &mut egui::Ui, query: &mut String, width: f32) -> [Response; 3] {
    let mut enabled = with_search(ui.ctx(), |search| search.enabled);
    let responses = ui.horizontal(|ui| {
        let clear_width = ui.spacing().interact_size.y;
        let locate_width = ui.painter().layout_no_wrap("Locate".into(),
            egui::TextStyle::Button.resolve(ui.style()), ui.visuals().text_color()).size().x
            + 2.0 * ui.spacing().button_padding.x;
        let edit_width = (width - clear_width - locate_width - 2.0 * ui.spacing().item_spacing.x).max(1.0);
        let edit = ui.add(egui::TextEdit::singleline(query).hint_text("Search").desired_width(edit_width))
            .on_hover_text("#Category · ^Station · $Material · @Source");
        let clear = ui.add_enabled(!query.is_empty(), egui::Button::new("x").min_size(vec2(clear_width, clear_width)))
            .on_hover_text("Clear Search");
        if clear.clicked() {
            query.clear();
            enabled = false;
            edit.request_focus();
        }
        let locate = ui.add_enabled(!query.trim().is_empty(),
            egui::Button::new("Locate").selected(enabled).min_size(vec2(locate_width, clear_width)))
            .on_hover_text("Matching Stacks · Pack + Hotbar + Chest");
        if locate.clicked() { enabled = !enabled; }
        [edit, clear, locate]
    }).inner;
    with_search(ui.ctx(), |search| search.update(query, enabled));
    responses
}

/// Matching occupied slots, excluding crafting, equipment, cursor and overflow.
pub(in crate::egui_ui) fn summary(ui: &mut egui::Ui, inventory: &SurvivalInventory) {
    let counts = with_search(ui.ctx(), |search| search.enabled.then(|| search.counts(inventory)));
    if let Some((found, occupied)) = counts {
        let colour = if found == 0 { super::super::presentation::AMBER }
            else { super::super::presentation::JADE };
        ui.label(RichText::new(format!("{found}/{occupied}")).size(13.0).color(colour))
            .on_hover_text("Matching Stacks · Pack + Hotbar + Chest");
    }
}

/// Nonmatches stay readable and interactive; matches gain an inset corner mark.
pub(super) fn paint(painter: &Painter, rect: Rect, unit: f32, item: Option<Item>) {
    let Some(matches) = item.and_then(|item| with_search(painter.ctx(), |search| search.matches(item)))
        else { return };
    let inner = rect.shrink(2.0 * unit);
    if matches {
        painter.rect_stroke(inner, 0, Stroke::new(unit.max(1.0), super::super::presentation::JADE), StrokeKind::Inside);
        painter.rect_filled(Rect::from_min_size(inner.right_top() - vec2(3.0 * unit, 0.0), vec2(3.0 * unit, 3.0 * unit)),
            0, super::super::presentation::JADE);
    } else {
        painter.rect_filled(inner, 0, Color32::from_black_alpha(150));
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
