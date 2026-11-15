//! Compact, non-interactive tool readout. Detailed controls live in B / Q.
use super::presentation;
use crate::creative::{CreativeState, ToolKind};
use egui::{Align2, Color32, FontId, Stroke, pos2, vec2};

pub fn traversal(ctx: &egui::Context, player: &crate::player::Player, voxel_meters: f32) {
    let t = &player.traversal;
    let readout = if t.sling_charge > 0.0 {
        let snap = t.sling_charge >= 1.0 && t.sling_full <= crate::player::traversal::SLING_SNAP_WINDOW;
        Some((t.sling_charge, if snap { "Stretch: Snap".to_owned() }
            else { format!("Stretch: {:.0}%", t.sling_charge * 100.0) }))
    } else if t.thrust || t.hover {
        if t.jet == 1 { Some((t.fuel_seconds / crate::player::traversal::BURN_SECONDS,
            format!("Fuel: {:.1} s", t.fuel_seconds))) }
        else { Some((t.charge_fe / crate::player::traversal::CHARGE_CAPACITY,
            format!("Charge: {:.0} FE", t.charge_fe))) }
    } else if t.attached {
        let count = t.contacts.iter().flatten().count();
        Some((count as f32 / 8.0, format!("Grip: {count}/8")))
    } else if player.glide.deployed {
        let speed=player.vel.length()*voxel_meters;
        Some(((speed/45.0).clamp(0.0,1.0),format!("Glide: {speed:.1} m/s")))
    } else { None };
    if let Some((fraction, text)) = readout {
        egui::Area::new(egui::Id::new("traversal_meter"))
            .anchor(Align2::CENTER_BOTTOM, [0.0, -148.0]).interactable(false)
            .show(ctx, |ui| { ui.add(egui::ProgressBar::new(fraction).desired_width(160.0).text(text)); });
    }
}

#[derive(Default)]
pub struct Feedback {
    pub material_budget: Option<u32>,
    pub distance_m: Option<f32>,
    pub out_of_range: bool,
    pub air_placement: bool,
    pub total_cells: usize,
    pub visible_cells: usize,
    /// Survival break held on the aimed cell, 0..1.
    pub break_progress: Option<f32>,
    /// Tier the aimed cell needs when the held tool is below it.
    pub break_requires: Option<u8>,
}

impl Feedback {
    pub fn color(&self) -> Color32 {
        if self
            .material_budget
            .is_some_and(|n| (n as usize) < self.total_cells)
        {
            presentation::AMBER
        } else if self.air_placement {
            Color32::from_rgb(100, 215, 245)
        } else if self.out_of_range {
            presentation::AMBER
        } else if self.total_cells > 0 {
            Color32::from_rgb(150, 220, 180)
        } else {
            presentation::MUTED
        }
    }
}

/// `top` clears the compass label and, when shown, the minimap above.
pub fn draw(ctx: &egui::Context, tool: &CreativeState, feedback: &Feedback, top: f32) {
    egui::Area::new(egui::Id::new("tool_telemetry"))
        .anchor(Align2::RIGHT_TOP, [-12.0, top])
        .interactable(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(220.0, 80.0), egui::Sense::hover());
            let p = ui.painter();
            p.rect_filled(rect, 0.0, super::grimoire::LEATHER);
            super::grimoire::binding(p, rect, false);
            let left = rect.left() + 10.0;
            let right = rect.right() - 10.0;
            let top = rect.top();
            let color = feedback.color();
            p.text(
                pos2(left, top + 9.0),
                Align2::LEFT_TOP,
                tool.tool.label(),
                FontId::proportional(12.0),
                presentation::PAPER,
            );
            p.text(
                pos2(right, top + 10.0),
                Align2::RIGHT_TOP,
                "B settings",
                FontId::proportional(10.0),
                presentation::MUTED,
            );
            let size = match tool.tool {
                ToolKind::Hand => format!(
                    "{:.2} m wide",
                    if tool.size_in_metres {
                        tool.hand_size_m
                    } else {
                        tool.active_voxel_m
                    }
                ),
                ToolKind::BuildersWand | ToolKind::DestructorWand => format!(
                    "{} {} max",
                    tool.wand_max_blocks,
                    if tool.size_in_metres { "m²" } else { "faces" }
                ),
                ToolKind::ExplosiveWand => format!(
                    "{} {} radius",
                    tool.explosion_radius,
                    if tool.size_in_metres { "m" } else { "cell" }
                ),
                _ => format!("{} cells", feedback.total_cells),
            };
            p.text(
                pos2(left, top + 27.0),
                Align2::LEFT_TOP,
                size,
                FontId::proportional(10.0),
                presentation::MUTED,
            );
            p.text(
                pos2(right, top + 27.0),
                Align2::RIGHT_TOP,
                format!("Reach {:.1} m", tool.placement_distance),
                FontId::proportional(11.0),
                presentation::PAPER,
            );
            let y = top + 46.0;
            p.line_segment(
                [pos2(left, y), pos2(right, y)],
                Stroke::new(3.0, presentation::EDGE),
            );
            p.line_segment(
                [
                    pos2(left, y),
                    pos2(left + (right - left) * tool.placement_distance / 24.0, y),
                ],
                Stroke::new(3.0, color),
            );
            if let Some(distance) = feedback.distance_m {
                let x = left + (right - left) * (distance / 24.0).clamp(0.0, 1.0);
                p.line_segment(
                    [pos2(x, y - 3.0), pos2(x, y + 3.0)],
                    Stroke::new(1.5, presentation::PAPER),
                );
            }
            let status = if tool.pending_cells() > 0 {
                format!("Applying {} cells", tool.pending_cells())
            } else if let Some(n) = feedback.material_budget {
                format!("{} / {} cells available", n, feedback.total_cells)
            } else if feedback.air_placement {
                "Air placement".into()
            } else if feedback.out_of_range {
                format!(
                    "Beyond reach · {:.1} m",
                    feedback.distance_m.unwrap_or_default()
                )
            } else if feedback.total_cells > 0 {
                let count = format!(
                    "{} {}",
                    feedback.total_cells,
                    if feedback.total_cells == 1 {
                        "cell"
                    } else {
                        "cells"
                    }
                );
                if feedback.visible_cells < feedback.total_cells {
                    format!("{count} · sampled")
                } else if let Some(distance) = feedback.distance_m {
                    format!("{count} · {distance:.1} m")
                } else {
                    count
                }
            } else {
                "Aim at a surface".into()
            };
            p.text(
                pos2(left, top + 55.0),
                Align2::LEFT_TOP,
                status,
                FontId::proportional(10.0),
                color,
            );
        });
}

/// Break progress bar under the crosshair, shown only while a survival break is held.
pub fn draw_break_progress(ctx: &egui::Context, feedback: &Feedback) {
    let Some(progress) = feedback.break_progress else { return };
    egui::Area::new(egui::Id::new("break_progress"))
        .anchor(Align2::CENTER_CENTER, [0.0, 26.0])
        .interactable(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(64.0, 18.0), egui::Sense::hover());
            let p = ui.painter();
            let bar = egui::Rect::from_min_size(rect.left_top(), vec2(rect.width(), 4.0));
            let color = if feedback.break_requires.is_some() {
                presentation::AMBER
            } else {
                presentation::PAPER
            };
            p.rect_filled(bar, 1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 140));
            let filled = egui::Rect::from_min_size(bar.min, vec2(bar.width() * progress, bar.height()));
            p.rect_filled(filled, 1.0, color);
            if let Some(tier) = feedback.break_requires {
                p.text(
                    pos2(rect.center().x, bar.bottom() + 2.0),
                    Align2::CENTER_TOP,
                    format!("Requires: Tier {tier}"),
                    FontId::proportional(10.0),
                    color,
                );
            }
        });
}

/// Seconds the crosshair hit marker stays up after a connecting melee hit.
const HIT_MARKER_SECONDS: f64 = 0.18;

/// Four short diagonal ticks around the crosshair when `landed` changes.
pub(super) fn hit_marker(ui: &egui::Ui, center: egui::Pos2, landed: u32) {
    let id = egui::Id::new("melee_hit_marker");
    let now = ui.input(|i| i.time);
    let (seen, since) = ui.ctx().data(|d| d.get_temp::<(u32, f64)>(id)).unwrap_or((landed, f64::NEG_INFINITY));
    let since = if seen != landed { now } else { since };
    ui.ctx().data_mut(|d| d.insert_temp(id, (landed, since)));
    let age = now - since;
    if !(0.0..HIT_MARKER_SECONDS).contains(&age) { return; }
    ui.ctx().request_repaint();
    let fade = (1.0 - age / HIT_MARKER_SECONDS) as f32;
    let stroke = Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 236, 214, (230.0 * fade) as u8));
    let painter = ui.painter();
    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let d = vec2(x, y);
        painter.line_segment([center + d * 5.0, center + d * 10.0], stroke);
    }
}
