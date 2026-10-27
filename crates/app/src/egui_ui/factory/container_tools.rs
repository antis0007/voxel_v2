//! Storage container toolbar: sorted cell view and quick stack.
use super::super::{UiAction, icons};
use crate::world::{
    expedition::NAMES,
    factory::Action,
    inventory::{Item, sorting::sort_key},
};
use egui::{Align2, Color32, FontId, Sense, Stroke, vec2};
use glam::IVec3;

fn sorted_id(pos: IVec3) -> egui::Id {
    egui::Id::new(("container_sorted", pos))
}

/// Whether the container at `pos` shows its cells in sorted order.
pub(super) fn sorted(ctx: &egui::Context, pos: IVec3) -> bool {
    ctx.data(|d| d.get_temp::<bool>(sorted_id(pos)).unwrap_or(false))
}

/// Flips the sorted cell view of the container at `pos`.
pub(super) fn toggle_sorted(ctx: &egui::Context, pos: IVec3) {
    ctx.data_mut(|d| {
        let on = d.get_temp_mut_or_default::<bool>(sorted_id(pos));
        *on = !*on;
    });
}

/// Resource shown in each grid cell.
///
/// Compartments are fixed per resource, so sorting reorders the view only:
/// held resources first by category and id, then empty compartments by id.
pub(super) fn cell_order(input: &[u32], cells: usize, sorted: bool) -> Vec<Option<usize>> {
    let mut ids: Vec<usize> = (0..NAMES.len().min(input.len())).collect();
    if sorted {
        ids.sort_by_key(|&i| (input[i] == 0, sort_key(Item::Resource(i as u8))));
    }
    let mut order: Vec<Option<usize>> = ids.into_iter().take(cells).map(Some).collect();
    order.resize(cells, None);
    order
}

fn button(ui: &mut egui::Ui, label: &str, on: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(88.0, 24.0), Sense::click());
    let fill = if on {
        Color32::from_rgb(93, 73, 48)
    } else if response.hovered() {
        Color32::from_rgb(55, 46, 43)
    } else {
        Color32::from_rgb(29, 25, 29)
    };
    ui.painter().rect_filled(rect, 0, fill);
    let stroke = Stroke::new(1.0, if on { icons::GOLD } else { icons::TEAL });
    ui.painter().rect_stroke(rect, 0, stroke, egui::StrokeKind::Inside);
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), icons::PAPER);
    response
}

/// Sort and Stack buttons for the container at `pos`.
pub(super) fn toolbar(ui: &mut egui::Ui, pos: IVec3, action: &mut UiAction) {
    let on = sorted(ui.ctx(), pos);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
        if button(ui, "Sort", on).on_hover_text("[MMB]").clicked() {
            toggle_sorted(ui.ctx(), pos);
        }
        if button(ui, "Stack", false).clicked() {
            *action = UiAction::Factory(Action::QuickStack(pos));
        }
    });
}

/// Middle click anywhere on the panel toggles the sorted view.
pub(super) fn middle_click_sort(ui: &egui::Ui, pos: IVec3) {
    let clicked = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Middle));
    if clicked && ui.rect_contains_pointer(ui.min_rect()) {
        toggle_sorted(ui.ctx(), pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::factory::RESOURCE_COUNT;

    #[test]
    fn sorted_view_lists_held_resources_first_and_keeps_every_id() {
        let mut input = [0u32; RESOURCE_COUNT];
        input[12] = 64;
        input[1] = 128;
        input[21] = 64;
        let plain = cell_order(&input, 20, false);
        assert_eq!(plain[0], Some(0));
        assert!(!plain.contains(&Some(21)), "crate grid ends before id 21");

        let sorted = cell_order(&input, 20, true);
        assert_eq!(sorted[..3], [Some(1), Some(12), Some(21)]);
        let mut seen: Vec<usize> = cell_order(&input, NAMES.len(), true).into_iter().flatten().collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..NAMES.len()).collect::<Vec<_>>());
        assert_eq!(cell_order(&input, 27, true)[26], None);
    }
}
