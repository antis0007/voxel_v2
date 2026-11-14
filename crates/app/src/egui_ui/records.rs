//! Statistics and advancements panel, unlock toast and held-clock readout.
use super::presentation as style;
use crate::world::stats::advancements::{self, Trigger};
use crate::world::stats::{Stat, State, View};
use egui::{Align2, Color32, FontId, RichText, vec2};
use std::sync::Arc;

fn key() -> egui::Id {
    egui::Id::new("records_view")
}

/// Stores this frame's snapshot for the panel and HUD.
pub fn publish(ctx: &egui::Context, view: View) {
    ctx.data_mut(|d| d.insert_temp(key(), Arc::new(view)));
}

fn view(ctx: &egui::Context) -> Option<Arc<View>> {
    ctx.data(|d| d.get_temp::<Arc<View>>(key()))
}

/// `0.5` as `12:00`.
pub fn clock_text(time_of_day: f32) -> String {
    let minutes = (time_of_day.rem_euclid(1.0) * 1440.0) as u32 % 1440;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn outlined(painter: &egui::Painter, at: egui::Pos2, text: &str, size: f32, color: Color32) {
    let font = FontId::monospace(size);
    for d in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0)] {
        painter.text(at + d, Align2::CENTER_TOP, text, font.clone(), style::INK);
    }
    painter.text(at, Align2::CENTER_TOP, text, font, color);
}

/// Clock readout while one is held and the newest unlock toast.
pub fn hud(ctx: &egui::Context) {
    let Some(view) = view(ctx) else { return };
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("records_hud")));
    if let Some(time) = view.clock {
        let at = egui::pos2(screen.center().x, screen.top() + 12.0);
        outlined(&painter, at, &format!("Time: {}", clock_text(time)), 14.0, style::PAPER);
    }
    if let Some(name) = view.toast {
        let rect = egui::Rect::from_min_size(egui::pos2(screen.right() - 236.0, screen.top() + 16.0), vec2(220.0, 44.0));
        painter.rect_filled(rect, 0, style::INK);
        painter.rect_stroke(rect, 0, egui::Stroke::new(2.0, style::AMBER), egui::StrokeKind::Inside);
        let left = egui::pos2(rect.left() + 10.0, rect.top() + 6.0);
        painter.text(left, Align2::LEFT_TOP, "Advancement", FontId::monospace(11.0), style::AMBER);
        painter.text(left + vec2(0.0, 16.0), Align2::LEFT_TOP, name, FontId::monospace(14.0), style::PAPER);
    }
}

fn statistics(ui: &mut egui::Ui, view: &View) {
    egui::Grid::new("records_stats").num_columns(2).striped(true).show(ui, |ui| {
        for stat in Stat::ALL {
            ui.label(RichText::new(stat.label()).color(style::MUTED));
            ui.label(RichText::new(stat.format(crate::world::stats::State::from_view(view).get(stat))).color(style::PAPER).monospace());
            ui.end_row();
        }
    });
}

fn advancement_rows(ui: &mut egui::Ui, view: &View) {
    let state = State::from_view(view);
    let done = advancements::table().iter().enumerate().filter(|&(i, _)| state.is_unlocked(i)).count();
    ui.label(RichText::new(format!("Unlocked: {done} / {}", advancements::table().len())).color(style::MUTED));
    egui::Grid::new("records_advancements").num_columns(3).striped(true).show(ui, |ui| {
        for (i, row) in advancements::table().iter().enumerate() {
            let unlocked = state.is_unlocked(i);
            let indent = "  ".repeat(advancements::depth(i));
            let color = if unlocked { style::AMBER } else { style::MUTED };
            ui.label(RichText::new(format!("{indent}{}", row.name)).color(color));
            ui.label(RichText::new(row.trigger.requirement()).color(style::MUTED));
            ui.label(RichText::new(status(row.trigger, &state, view.level, unlocked)).color(style::PAPER).monospace());
            ui.end_row();
        }
    });
}

fn status(trigger: Trigger, state: &State, level: u32, unlocked: bool) -> String {
    if unlocked {
        return "Done".to_owned();
    }
    let (now, target) = trigger.progress(state, level);
    match trigger {
        Trigger::Stat(stat, _) => format!("{} / {}", stat.format(now), stat.format(target)),
        Trigger::Level(_) => format!("{now} / {target}"),
    }
}

pub(super) fn thresholds(ui: &mut egui::Ui) {
    use crate::world::progression::thresholds::Milestone;
    let lore = ui.ctx().data(|d| d.get_temp::<u32>(egui::Id::new("threshold_research"))).unwrap_or_default();
    for milestone in Milestone::ALL {
        let (name, task) = milestone.entry();
        ui.horizontal_wrapped(|ui| {
            let status = if milestone.recorded(lore) { "Done" } else { "Open" };
            ui.label(RichText::new(status).color(style::MUTED).monospace());
            ui.label(name);
            ui.label(RichText::new(task).color(style::MUTED));
        });
    }
}

/// The Statistics window with both tables.
pub fn draw(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    let Some(view) = view(ctx) else { return };
    egui::Window::new("Statistics")
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            statistics(ui, &view);
            ui.separator();
            ui.label(RichText::new("Advancements").color(style::AMBER));
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                advancement_rows(ui, &view);
                ui.collapsing("Portal achievements", thresholds);
            });
            if ui.button("Done").clicked() {
                *open = false;
            }
        });
}


pub(super) fn page(ui: &mut egui::Ui) {
    if let Some(view) = view(ui.ctx()) {
        statistics(ui, &view);
        ui.separator();
        advancement_rows(ui, &view);
        ui.collapsing("Portal achievements", thresholds);
    }
}
