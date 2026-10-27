//! Controls screen: camera comfort scales and the rebindable action table.

use crate::keybinds::{self, Action, Bindings, SLOTS};
use egui::RichText;

pub(super) fn comfort(ui: &mut egui::Ui) {
    let mut motion = crate::comfort::camera_motion_percent();
    if ui.add(egui::Slider::new(&mut motion, 0..=100).suffix("%").text("Camera Motion")).changed() {
        crate::comfort::set_camera_motion_percent(motion);
    }
    let mut fov = crate::comfort::fov_effects_percent();
    if ui.add(egui::Slider::new(&mut fov, 0..=100).suffix("%").text("FOV Effects")).changed() {
        crate::comfort::set_fov_effects_percent(fov);
    }
}

fn matches(action: Action, bindings: &Bindings, needle: &str) -> bool {
    needle.is_empty()
        || action.label().to_lowercase().contains(needle)
        || action.group().to_lowercase().contains(needle)
        || bindings.slots(action).iter().flatten().any(|b| b.label().to_lowercase().contains(needle))
}

pub(super) fn bindings(ui: &mut egui::Ui) {
    keybinds::refresh_capture();
    let search_id = egui::Id::new("keybind_search");
    let mut search: String = ui.ctx().data_mut(|d| d.get_temp(search_id).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut search).hint_text("Search").desired_width(180.0));
        if ui.button("Reset").clicked() {
            keybinds::cancel_capture();
            keybinds::apply(Bindings::defaults());
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(search_id, search.clone()));
    if let Some(status) = keybinds::status() {
        ui.label(RichText::new(status).small());
    }
    let current = keybinds::current();
    let defaults = Bindings::defaults();
    let capture = keybinds::capturing();
    let needle = search.trim().to_lowercase();
    let accent = ui.visuals().selection.bg_fill;
    let mut group = "";
    egui::Grid::new("keybind_grid").num_columns(1 + SLOTS).striped(true).show(ui, |ui| {
        for &action in Action::ALL {
            if !matches(action, &current, &needle) {
                continue;
            }
            if action.group() != group {
                group = action.group();
                ui.label(RichText::new(group).strong());
                ui.end_row();
            }
            let label = RichText::new(action.label());
            ui.label(if current.slots(action) == defaults.slots(action) { label } else { label.color(accent) });
            for (slot, binding) in current.slots(action).into_iter().enumerate() {
                let text = if capture == Some((action, slot)) {
                    RichText::new("Listening").strong()
                } else {
                    binding.map_or_else(|| RichText::new("Unbound").weak(), |b| RichText::new(format!("[{}]", b.label())))
                };
                let response = ui
                    .add(egui::Button::new(text).min_size(egui::vec2(104.0, 0.0)))
                    .on_hover_text("[RMB] Clear");
                if response.clicked() {
                    keybinds::begin_capture(action, slot);
                }
                if response.secondary_clicked() {
                    let mut next = current.clone();
                    next.assign(action, slot, None);
                    keybinds::apply(next);
                }
            }
            ui.end_row();
        }
    });
}
