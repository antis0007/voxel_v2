use super::{style, WIDTH};
use crate::world::farming::jobs::onboarding::View;
use egui::{RichText, vec2};

pub(super) fn draw(ctx: &egui::Context, view: &View) {
    // The column under the minimap fits every window wider than a phone-sized one.
    if ctx.content_rect().width() < 720.0 {
        compact(ctx, view);
        return;
    }
    let slot = super::super::hud_layout::place(ctx, "goal_tracker");
    let shown = egui::Area::new(egui::Id::new("settlement_goal"))
        .anchor(slot.align, slot.offset)
        .interactable(false).order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let plate = super::tracker_frame().show(ui, |ui| {
                ui.set_width(slot.width.unwrap_or(WIDTH));
                ui.spacing_mut().item_spacing = vec2(4.0, 3.0);
                let (title, color) = if view.housing.is_some() {
                    if view.complete() { ("Housing Ready", style::JADE) }
                    else { ("Settlement", style::AMBER) }
                } else if view.complete() {
                    ("Settlement Established", style::JADE)
                } else { ("Settlement Start", style::AMBER) };
                ui.label(RichText::new(title).size(13.0).color(color));
                if let Some(housing) = &view.housing {
                    ui.label(RichText::new(format!("Housed: {} / {} residents", housing.housed, housing.residents))
                        .size(11.0).color(style::PAPER));
                    ui.label(RichText::new(format!("Housing Demand: {} residents", housing.demand))
                        .size(11.0).color(style::PAPER));
                    if view.unloaded == 0 {
                        ui.label(RichText::new(format!("Projects: {} / {} complete", housing.completed, housing.projects))
                            .size(11.0).color(style::PAPER));
                    }
                }
                ui.label(RichText::new(format!("First Meal: {} / 1", u8::from(view.meal_served)))
                    .size(11.0).color(if view.meal_served { style::JADE } else { style::PAPER }));
                if view.unloaded > 0 {
                    ui.label(RichText::new("Site Unloaded").size(11.0).color(style::MUTED));
                } else {
                    ui.label(RichText::new(format!("{}: {} / {} blocks", view.project, view.built, view.total))
                        .size(11.0).color(style::PAPER));
                    ui.add(egui::ProgressBar::new(view.built as f32 / view.total.max(1) as f32)
                        .desired_height(5.0));
                }
                if view.paused && !view.complete() {
                    ui.label(RichText::new("Work Paused").size(11.0).color(style::AMBER));
                }
                if view.housing.is_some() && !view.status.is_empty() {
                    ui.label(RichText::new(&view.status).size(10.0).color(style::MUTED));
                }
                if ui.input(|i| i.modifiers.alt) { super::hide_hint(ui); }
            });
            super::spine(ui.painter(), plate.response.rect);
        });
    super::super::hud_layout::claim(ctx, "goal_tracker", shown.response.rect);
}

fn compact(ctx: &egui::Context, view: &View) {
    // Leave the side panels and minimap clear on smaller windows.
    egui::Area::new(egui::Id::new("settlement_goal_compact"))
        .anchor(egui::Align2::LEFT_TOP, [16.0, 8.0])
        .interactable(false).order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let mut frame = style::hud_frame(ctx);
            frame.0.inner_margin = egui::Margin::symmetric(10, 4);
            frame.show(ui, |ui| {
                ui.set_width((ctx.content_rect().width() - 176.0).max(250.0));
                ui.spacing_mut().item_spacing = vec2(4.0, 1.0);
                ui.spacing_mut().interact_size.y = 14.0;
                let title = if view.complete() {
                    if view.housing.is_some() { "Housing Ready" } else { "Settlement Established" }
                } else if view.paused { "Work Paused" }
                else { "Settlement Start" };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).size(12.0)
                        .color(if view.complete() { style::JADE } else { style::AMBER }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), super::hide_hint);
                });
                if let Some(housing) = &view.housing {
                    let projects = if view.unloaded > 0 { "Site Unloaded".into() }
                        else { format!("Projects: {} / {}", housing.completed, housing.projects) };
                    ui.label(RichText::new(format!("Housed: {} / {} · Demand: {} · {projects}",
                        housing.housed, housing.residents, housing.demand)).size(11.0).color(style::PAPER));
                }
                let building = if view.unloaded > 0 { "Site Unloaded".into() }
                    else { format!("{}: {} / {} blocks", view.project, view.built, view.total) };
                ui.label(RichText::new(format!("First Meal: {} / 1 · {building}", u8::from(view.meal_served)))
                    .size(11.0).color(style::PAPER));
                ui.add(egui::ProgressBar::new(view.built as f32 / view.total.max(1) as f32)
                    .desired_height(4.0));
            });
        });
}
