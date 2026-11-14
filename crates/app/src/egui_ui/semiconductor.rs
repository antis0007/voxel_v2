//! The lab inventory and authored production batches on a placed fabricator,
//! laid out like the recipe browser: stat tags, input cells, output cells.
use super::presentation::{AMBER, DANGER, EDGE, MUTED, PAPER};
use super::{UiAction, chip_art, icons};
use crate::world::factory::{
    View,
    chips::{
        Action,
        production::{OPERATIONS, Operation, Order, Production, SAND_CAP, STOCK_CAP, Supply},
    },
};
use crate::world::inventory::Item;
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, vec2};
use glam::IVec3;

const CELL: f32 = 36.0;
const CARD: Color32 = Color32::from_rgb(33, 28, 32);
const WELL: Color32 = Color32::from_rgb(24, 21, 25);

/// What a cell shows: a lab supply or a raw item from the pipe buffer.
#[derive(Clone, Copy)]
enum Feed {
    Supply(Supply),
    Item(Item),
}

use super::icons::amount_text;

/// One framed cell: sprite, amount at the lower right, `hover` on hover.
fn cell(ui: &mut egui::Ui, feed: Feed, amount: Option<f32>, edge: Option<Color32>, dim: bool, hover: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(CELL, CELL), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0, WELL);
    p.rect_stroke(rect, 0, Stroke::new(1.0, edge.unwrap_or(EDGE)), egui::StrokeKind::Inside);
    let art = rect.shrink(CELL * 0.14);
    match feed {
        Feed::Supply(s) => {
            chip_art::supply(p, art, s);
        }
        Feed::Item(item) => super::inventory::draw_item_icon(p, art, item),
    }
    if dim {
        p.rect_filled(rect.shrink(1.0), 0, Color32::from_rgba_unmultiplied(29, 31, 34, 150));
    }
    if let Some(n) = amount {
        let at = rect.right_bottom() - vec2(2.0, 1.0);
        let count = amount_text(n);
        p.text(at + vec2(1.0, 1.0), Align2::RIGHT_BOTTOM, &count, FontId::monospace(13.0), Color32::BLACK);
        p.text(at, Align2::RIGHT_BOTTOM, count, FontId::monospace(13.0), PAPER);
    }
    resp.on_hover_text(hover)
}

/// A small bordered stat, as the recipe browser shows time and power.
fn stat(ui: &mut egui::Ui, text: &str, colour: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 8))
        .stroke(Stroke::new(1.0, EDGE))
        .inner_margin(egui::Margin::symmetric(4, 0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(14.0).color(colour));
        });
}

fn card<R>(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, EDGE))
        .inner_margin(egui::Margin::same(5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(4.0, 3.0);
            body(ui)
        })
        .inner
}

/// Inputs → outputs of an operation as cells; short lab stock is edged red.
fn feed_row(ui: &mut egui::Ui, op: &Operation, p: &Production) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
        for &(item, n) in op.raw {
            cell(ui, Feed::Item(item), Some(n as f32), None, false, item.name());
        }
        for &(s, n) in op.inputs {
            let stock = p.stock[s as usize];
            cell(ui, Feed::Supply(s), Some(n as f32), (stock < n).then_some(DANGER), false, &format!("{} · Stock {stock}", s.name()));
        }
        ui.label(RichText::new("→").size(15.0).color(MUTED));
        for &(s, n) in op.outputs {
            cell(ui, Feed::Supply(s), Some(n as f32), None, false, s.name());
        }
    });
}

fn operation_card(ui: &mut egui::Ui, index: usize, op: &Operation, view: &View, action: &mut UiAction) {
    let p = &view.fabrication;
    let site = view.fabrication_site;
    let busy = site.is_some_and(|s| p.owns((s.x, s.y, s.z)));
    let free = if site.is_none() {
        Some("Fabricator Required")
    } else if busy {
        Some("Fabricator Busy")
    } else {
        None
    };
    card(ui, |ui| {
        ui.label(RichText::new(op.name).size(16.0).color(PAPER));
        ui.horizontal_wrapped(|ui| {
            stat(ui, &format!("{} s", op.seconds), PAPER);
            stat(ui, &format!("{} kW", op.power), AMBER);
        });
        ui.add_space(6.0);
        feed_row(ui, op, p);
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
                let ready = free.is_none() && p.available(op.inputs);
                let batch = ui
                    .add_enabled(ready, egui::Button::new("Batch"))
                    .on_hover_text("Batch")
                    .on_disabled_hover_text(free.unwrap_or("Missing Lab Stock"));
                if batch.clicked() && let Some(site) = site {
                    *action = UiAction::Chip(Action::Manufacture { site, recipe: index as u8 });
                }
                let (out, per) = op.outputs[0];
                let target = (per * 4).min(STOCK_CAP);
                let order = ui
                    .add_enabled(free.is_none(), egui::Button::new("Order"))
                    .on_hover_text(format!("Standing Order · {target} × {}", out.name()))
                    .on_disabled_hover_text(free.unwrap_or_default());
                if order.clicked() && let Some(site) = site {
                    *action = UiAction::Chip(Action::SetOrder { site, recipe: index as u8, target, enabled: true });
                }
        });
        if let Some(reason) = free {
            ui.label(RichText::new(reason).color(AMBER));
        }
        for &(s, need) in op.inputs {
            let have = p.stock[s as usize];
            ui.label(RichText::new(format!("{}: {have} / {need}", s.name()))
                .color(if have < need { DANGER } else { MUTED }));
        }
        for &(item, need) in op.raw {
            ui.label(RichText::new(format!("{}: {need}", item.name())).color(MUTED));
        }
    });
}

/// The standing order on the focused fabricator: its target, status and feed levels.
fn order_card(ui: &mut egui::Ui, view: &View, site: IVec3, order: &Order, action: &mut UiAction) {
    let p = &view.fabrication;
    let op = &OPERATIONS[order.recipe as usize];
    let (out, _) = op.outputs[0];
    let stock = p.stock[out as usize];
    card(ui, |ui| {
        ui.horizontal(|ui| {
            cell(ui, Feed::Supply(out), Some(stock as f32), None, false, &format!("{} · {stock} / {}", out.name(), order.target));
            ui.vertical(|ui| {
                ui.label(RichText::new(op.name).size(14.0).color(icons::GOLD));
                let status = view.fabrication_node.as_ref().map_or("Idle", |n| super::factory::status_label(n.status));
                ui.label(RichText::new(status).size(14.0).color(MUTED));
            });
        });
        ui.horizontal_wrapped(|ui| {
                if ui.button("Clear Order").clicked() {
                    *action = UiAction::Chip(Action::ClearOrder(site));
                }
                if !op.raw.is_empty() && ui.button("Load Feed").clicked() {
                    *action = UiAction::Chip(Action::FeedOrder(site));
                }
        });
        let mut enabled = order.enabled;
        let mut target = order.target;
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            changed |= ui.checkbox(&mut enabled, "Repeat").changed();
            ui.label(RichText::new("Target").size(14.0).color(MUTED));
            changed |= ui.add(egui::DragValue::new(&mut target).range(1..=STOCK_CAP)).changed();
        });
        icons::gauge(ui, stock as f32 / order.target.max(1) as f32, icons::TEAL, &format!("{stock} / {}", order.target));
        if changed {
            *action = UiAction::Chip(Action::SetOrder { site, recipe: order.recipe, target, enabled });
        }
        if let Some(node) = &view.fabrication_node
            && !op.raw.is_empty()
        {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                for &(item, need) in op.raw {
                    let (have, cap) = match item {
                        Item::Resource(r) => (node.input[r as usize] as f32 / 64.0, 64),
                        _ => (order.sand as f32, SAND_CAP),
                    };
                    let hover = format!("{} · {} / {cap}", item.name(), amount_text(have));
                    cell(ui, Feed::Item(item), Some(have), (have < need as f32).then_some(DANGER), false, &hover);
                }
            });
        }
    });
}

fn jobs(ui: &mut egui::Ui, view: &View, site: Option<IVec3>, action: &mut UiAction) {
    let p = &view.fabrication;
    for job in &p.jobs {
        let op = &OPERATIONS[job.recipe as usize];
        let progress = 1.0 - job.remaining as f32 / (op.seconds * 120) as f32;
        ui.horizontal(|ui| {
            cell(ui, Feed::Supply(op.outputs[0].0), None, None, false, op.outputs[0].0.name());
            ui.vertical(|ui| {
                ui.label(RichText::new(op.name).size(14.0).color(PAPER));
                icons::gauge(ui, progress, icons::TEAL, &format!("{:.0}%", progress * 100.0));
            });
            if let Some(site) = site
                && (site.x, site.y, site.z) != job.site
                && !p.owns((site.x, site.y, site.z))
                && ui.small_button("Reassign Batch").clicked()
            {
                *action = UiAction::Chip(Action::MoveBatch { from: IVec3::new(job.site.0, job.site.1, job.site.2), site });
            }
        });
    }
}

fn lots(ui: &mut egui::Ui, view: &View, site: Option<IVec3>, action: &mut UiAction) {
    let Some(lib) = &view.chips else { return };
    let p = &view.fabrication;
    let open = lib.lots.iter().filter(|l| l.records.last().is_none_or(|r| r.kind != engine_chip::fab::StepKind::Package));
    for lot in open {
        let binding = p.lots.iter().find(|l| l.id == lot.id);
        let elsewhere = site.is_some_and(|s| binding.is_some_and(|b| b.site != (s.x, s.y, s.z)));
        if binding.is_some() && !elsewhere {
            continue;
        }
        ui.horizontal(|ui| {
            ui.label(format!("Lot {} · {}", lot.id, if binding.is_none() { "Unassigned" } else { "Assigned" }));
            let resumable = site.is_some_and(|s| !p.owns((s.x, s.y, s.z)));
            if ui.add_enabled(resumable, egui::Button::new("Resume")).clicked()
                && let Some(site) = site
            {
                *action = UiAction::Chip(Action::ResumeLot { site, lot: lot.id });
            }
        });
    }
}

fn reassign(ui: &mut egui::Ui, view: &View, site: IVec3, action: &mut UiAction) {
    let p = &view.fabrication;
    if p.owns((site.x, site.y, site.z)) || p.orders.is_empty() {
        return;
    }
    ui.collapsing("Reassign Order", |ui| {
        for order in &p.orders {
            ui.horizontal(|ui| {
                let (x, y, z) = order.site;
                ui.small(format!("{} · {x}, {y}, {z}", OPERATIONS[order.recipe as usize].name));
                if ui.small_button("Reassign").clicked() {
                    *action = UiAction::Chip(Action::MoveOrder { from: IVec3::new(x, y, z), site });
                }
            });
        }
    });
}

/// Review captures: VOXEL_REVIEW_FABRICATOR=1 keeps the section open.
fn review_open() -> Option<bool> {
    static OPEN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    OPEN.get_or_init(|| std::env::var("VOXEL_REVIEW_FABRICATOR").is_ok()).then_some(true)
}

const OPEN: &str = "semiconductor_open";

/// True for items some fabrication step consumes as raw input.
pub(super) fn relevant(item: Option<Item>) -> bool {
    item.is_some_and(|item| OPERATIONS.iter().any(|op| op.raw.iter().any(|&(raw, _)| raw == item)))
}

pub(super) fn section(ui: &mut egui::Ui, _view: &View, _action: &mut UiAction) {
    if ui.button("Semiconductors").clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(OPEN), true));
    }
}

pub(super) fn window(ctx: &egui::Context, view: &View, action: &mut UiAction) {
    let id = egui::Id::new(OPEN);
    let mut open = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(review_open().unwrap_or(false));
    if !open { return; }
    let screen = ctx.content_rect();
    let tab_id = egui::Id::new("semiconductor_tab");
    let recipe_id = egui::Id::new("semiconductor_recipe");
    let mut tab = ctx.data(|d| d.get_temp::<u8>(tab_id)).unwrap_or(0);
    let mut selected = ctx.data(|d| d.get_temp::<usize>(recipe_id)).unwrap_or(0).min(OPERATIONS.len() - 1);
    egui::Window::new("Semiconductors")
        .open(&mut open)
        .default_size([560.0, 380.0])
        .max_width((screen.width() - 32.0).max(160.0))
        .max_height((screen.height() - 48.0).max(120.0))
        .resizable(true)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
            ui.horizontal_wrapped(|ui| {
                for (value, label) in [(0, "Recipes"), (1, "Stock"), (2, "Queue")] {
                    ui.selectable_value(&mut tab, value, label);
                }
                if ui.button("Designer").clicked() { super::chip_editor::open(ctx); }
            });
            ui.separator();
            let p = &view.fabrication;
            let site = view.fabrication_site;
            if let Some(site) = site {
                ui.label(RichText::new(format!("Fabricator · {}, {}, {}", site.x, site.y, site.z)).color(MUTED));
            } else {
                ui.label(RichText::new("Fabricator Required").color(AMBER));
            }
            egui::ScrollArea::vertical().id_salt("semiconductor_body")
                .auto_shrink([false, false]).max_height(ui.available_height()).show(ui, |ui| {
                    match tab {
                        0 => {
                            egui::ComboBox::from_id_salt("semiconductor_recipe_picker")
                                .selected_text(OPERATIONS[selected].name)
                                .show_ui(ui, |ui| {
                                    for (i, op) in OPERATIONS.iter().enumerate() {
                                        ui.selectable_value(&mut selected, i, op.name);
                                    }
                                });
                            operation_card(ui, selected, &OPERATIONS[selected], view, action);
                        }
                        1 => {
                            for supply in Supply::ALL {
                                ui.horizontal(|ui| {
                                    let stock = p.stock[supply as usize];
                                    cell(ui, Feed::Supply(supply), None, None, stock == 0, supply.name());
                                    ui.vertical(|ui| {
                                        ui.label(supply.name());
                                        ui.label(RichText::new(format!("{stock} / {STOCK_CAP}")).color(MUTED));
                                    });
                                });
                            }
                        }
                        _ => {
                            if let Some(site) = site {
                                if let Some(order) = p.orders.iter().find(|o| o.site == (site.x, site.y, site.z)) {
                                    order_card(ui, view, site, order, action);
                                }
                                reassign(ui, view, site, action);
                            }
                            if p.jobs.is_empty() && p.orders.is_empty() && p.lots.is_empty() {
                                ui.label(RichText::new("Queue Empty").color(MUTED));
                            }
                            jobs(ui, view, site, action);
                            lots(ui, view, site, action);
                        }
                    }
                });
        });
    ctx.data_mut(|d| {
        d.insert_temp(id, open);
        d.insert_temp(tab_id, tab);
        d.insert_temp(recipe_id, selected);
    });
}
