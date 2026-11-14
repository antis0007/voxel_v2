use super::UiAction;
use crate::world::factory::{View, overview};

pub(crate) const LOGISTICS: u32 = 11;
const BUILD: u32 = 0;

pub(super) enum Destination {
    Page(u32),
    Map,
}

pub(super) fn navigation(ui: &mut egui::Ui, page: Option<u32>) -> Option<Destination> {
    ui.horizontal(|ui| {
        let mut destination = None;
        if ui.selectable_label(page == Some(BUILD), "Build").clicked() && page != Some(BUILD) {
            destination = Some(Destination::Page(BUILD));
        }
        if ui.selectable_label(page.is_none(), "Map").clicked() && page.is_some() {
            destination = Some(Destination::Map);
        }
        if ui.selectable_label(page == Some(LOGISTICS), "Logistics").clicked() && page != Some(LOGISTICS) {
            destination = Some(Destination::Page(LOGISTICS));
        }
        destination
    }).inner
}

pub(super) fn logistics(ui: &mut egui::Ui, view: &View, page: &mut u32, action: &mut UiAction) {
    ui.horizontal(|ui| {
        ui.strong("Logistics");
        ui.weak(format!("{} Machines", view.count));
        if ui.small_button("Production").clicked() { *page = 7; }
    });
    if let Some((cell, node)) = &view.focused {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(node.kind.name());
                ui.weak(overview::classify(node).fix());
                if ui.small_button("Inspect").clicked() { *page = 3; }
                if ui.small_button("Locate").clicked() {
                    *action = UiAction::WorkshopMap(Some((*cell, node.kind.name())));
                }
            });
            ui.columns(2, |columns| {
                buffer(&mut columns[0], "Input", &node.input);
                buffer(&mut columns[1], "Output", &node.output);
            });
        });
    }
    if let Some((news, rest)) = view.workshop_alerts.split_first() {
        let mut alert = |ui: &mut egui::Ui, row: &crate::world::factory::WorkshopAlert| {
            let label = format!("{} · {}", row.health.fix(), row.kind.name());
            if ui.link(label).clicked() {
                *action = UiAction::WorkshopMap(Some((row.target, row.health.fix())));
            }
        };
        ui.separator();
        alert(ui, news);
        if !rest.is_empty() {
            ui.collapsing("Alerts", |ui| {
                for row in rest { alert(ui, row); }
            });
        }
    }
}

fn buffer(ui: &mut egui::Ui, title: &str, contents: &[u32]) {
    use crate::world::expedition::NAMES;
    ui.weak(title);
    if contents.iter().all(|&amount| amount == 0) {
        ui.weak("Empty");
        return;
    }
    egui::ScrollArea::vertical().id_salt(title).max_height(76.0).show(ui, |ui| {
        for (&name, &amount) in NAMES.iter().zip(contents).filter(|(_, amount)| **amount != 0) {
            ui.label(format!("{} · {}", super::icons::amount_text(amount as f32 / 64.0), name));
        }
    });
}
