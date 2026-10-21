//! "Here's what you're looking at" panel (HWYLA / Jade style) for the block
//! under the crosshair. It is non-interactive and sits at the top centre.
//!
//! Layout, top to bottom:
//! * icon tile, block name, and a temperature chip when the cell is hot;
//! * the item category in the accent colour, where Minecraft shows the mod name;
//! * chips: state of matter, hardness pips, passability, distance;
//! * for a machine, its status with energy and water readings;
//! * controls as keycaps (`F  Inspect · E  Inventory`).
//!
//! All data is passed in; this module reads no world state.
use super::presentation::{self, AMBER, DANGER, EDGE, JADE, MANA, MUTED, PAPER};
use crate::material::{BlockId, CollisionKind, MatterState};
use crate::world::factory::Node;
use egui::{Align2, Color32, RichText, Sense, Stroke, vec2};

pub struct LookAt<'a> {
    pub block: BlockId,
    /// Shown only for cells noticeably above ambient.
    pub temperature_c: Option<f32>,
    pub distance_m: Option<f32>,
    /// Control hint from `expedition::hint`, e.g. `F  Inspect   ·   E  Inventory`.
    pub hint: &'a str,
    /// The machine at the crosshair, when the block is one.
    pub machine: Option<&'a Node>,
}

/// Seconds the panel takes to settle when the target changes.
const SETTLE: f64 = 0.14;

pub fn draw(ctx: &egui::Context, look: &LookAt) {
    // Restart the fade-in whenever the target block changes.
    let state_id = egui::Id::new("look_at_state");
    let now = ctx.input(|i| i.time);
    let (last, since) = ctx
        .data_mut(|d| d.get_temp::<(u32, f64)>(state_id))
        .unwrap_or((u32::MAX, now));
    let since = if last == look.block.as_u32() { since } else { now };
    ctx.data_mut(|d| d.insert_temp(state_id, (look.block.as_u32(), since)));
    let appear = (((now - since) / SETTLE) as f32).clamp(0.0, 1.0);

    egui::Area::new("target_name".into())
        .anchor(Align2::CENTER_TOP, [0.0, 74.0 - (1.0 - appear) * 6.0])
        .interactable(false)
        .show(ctx, |ui| {
            ui.set_opacity(appear);
            presentation::hud_frame().show(ui, |ui| {
                ui.set_min_width(210.0);
                ui.set_max_width(290.0);
                // Rows are as tall as their text, not the default 18-point widget.
                ui.spacing_mut().interact_size.y = 12.0;
                ui.horizontal(|ui| {
                    icon_tile(ui, look.block);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        title_row(ui, look);
                        ui.label(
                            RichText::new(crate::world::inventory::Item::Block(look.block).category())
                                .size(10.0)
                                .italics()
                                .color(JADE),
                        );
                        chip_row(ui, look);
                    });
                });
                if let Some(node) = look.machine {
                    ui.add_space(3.0);
                    machine_rows(ui, node);
                }
                if !look.hint.is_empty() {
                    ui.add_space(3.0);
                    ui.separator();
                    keycap_row(ui, look.hint);
                }
            });
        });
}

fn icon_tile(ui: &mut egui::Ui, block: BlockId) {
    let (rect, _) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 18, 21));
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
    super::draw_voxel_icon(painter, rect.shrink(5.0), block);
}

fn title_row(ui: &mut egui::Ui, look: &LookAt) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(look.block.display_name())
                .size(13.5)
                .strong()
                .color(PAPER),
        );
        if let Some(temp_c) = look.temperature_c {
            // Scorching cells read red; merely warm ones amber.
            let colour = if temp_c > 300.0 { DANGER } else { Color32::from_rgb(245, 134, 62) };
            chip(ui, &format!("{temp_c:.0}°C"), colour);
        }
    });
}

fn chip_row(ui: &mut egui::Ui, look: &LookAt) {
    let material = look.block.material();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let (state, state_colour) = match material.physics.matter_state {
            MatterState::Solid => ("Solid", PAPER),
            MatterState::Powder => ("Powder", AMBER),
            MatterState::Liquid => ("Liquid", MANA),
            MatterState::Gas => ("Gas", MUTED),
            MatterState::Plasma => ("Plasma", DANGER),
            MatterState::Vacuum => ("Void", MUTED),
        };
        chip(ui, state, state_colour);
        if material.physics.collision == CollisionKind::Passable
            && material.physics.matter_state == MatterState::Solid
        {
            chip(ui, "Passable", MUTED);
        }
        // Hardness pips only mean something for things you can break.
        if material.physics.matter_state == MatterState::Solid && material.physics.hardness > 0.0 {
            hardness_pips(ui, material.physics.hardness);
        }
        if let Some(distance) = look.distance_m {
            chip(ui, &format!("{distance:.1} m"), MUTED);
        }
    });
}

/// Five pips; hardness 0..1.1 maps onto them so soft dirt shows one and
/// bedrock-grade material shows five.
fn hardness_pips(ui: &mut egui::Ui, hardness: f32) {
    let filled = ((hardness / 1.1) * 5.0).ceil().clamp(1.0, 5.0) as usize;
    let (rect, _) = ui.allocate_exact_size(vec2(5.0 * 7.0 + 2.0, 14.0), Sense::hover());
    let painter = ui.painter();
    for i in 0..5 {
        let pip = egui::Rect::from_min_size(
            rect.min + vec2(1.0 + i as f32 * 7.0, 4.0),
            vec2(5.0, 7.0),
        );
        painter.rect_filled(pip, 0.0, if i < filled { JADE } else { EDGE });
    }
}

fn chip(ui: &mut egui::Ui, text: &str, colour: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 10))
        .stroke(Stroke::new(1.0, EDGE))
        .inner_margin(egui::Margin::symmetric(4, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(9.5).color(colour));
        });
}

fn machine_rows(ui: &mut egui::Ui, node: &Node) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.label(RichText::new(node.status).size(10.5).strong().color(JADE));
        if !node.enabled {
            chip(ui, "Paused", AMBER);
        }
        chip(ui, &format!("E {}", node.energy), MANA);
        if node.water > 0 {
            chip(ui, &format!("H₂O {:.1} m³", node.water as f32 / 512.0), MANA);
        }
    });
}

/// `F  Inspect   ·   E  Inventory` becomes `[F] Inspect  [E] Inventory`. A
/// segment without a double-space is plain explanatory text.
fn keycap_row(ui: &mut egui::Ui, hint: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        for segment in hint.split('·').map(str::trim).filter(|s| !s.is_empty()) {
            match segment.split_once("  ") {
                Some((key, action)) => {
                    keycap(ui, key.trim());
                    ui.label(RichText::new(action.trim()).size(10.5).color(PAPER));
                }
                None => {
                    ui.label(RichText::new(segment).size(10.0).color(MUTED));
                }
            }
        }
    });
}

fn keycap(ui: &mut egui::Ui, key: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgb(52, 46, 50))
        .stroke(Stroke::new(1.0, EDGE))
        .corner_radius(2)
        .inner_margin(egui::Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(key).size(10.0).strong().color(PAPER));
        });
}
