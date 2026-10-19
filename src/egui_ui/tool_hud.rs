//! Compact, non-interactive tool readout. Detailed controls live in B / Q.
use super::presentation;
use crate::creative::{CreativeState, ToolKind};
use egui::{Align2, Color32, FontId, Stroke, pos2, vec2};

#[derive(Default)]
pub struct Feedback {
    pub material_budget: Option<u32>,
    pub distance_m: Option<f32>,
    pub out_of_range: bool,
    pub air_placement: bool,
    pub total_cells: usize,
    pub visible_cells: usize,
}

impl Feedback {
    pub fn color(&self) -> Color32 {
        if self.material_budget.is_some_and(|n| (n as usize) < self.total_cells) { presentation::AMBER }
        else if self.air_placement { Color32::from_rgb(100, 215, 245) }
        else if self.out_of_range { presentation::AMBER }
        else if self.total_cells > 0 { Color32::from_rgb(150, 220, 180) }
        else { presentation::MUTED }
    }
}

pub fn draw(ctx: &egui::Context, tool: &CreativeState, feedback: &Feedback) {
    egui::Area::new("tool_telemetry".into())
        .anchor(Align2::RIGHT_TOP, [-12.0, 12.0])
        .interactable(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(220.0, 80.0), egui::Sense::hover());
            let p = ui.painter();
            p.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(24, 23, 28, 220));
            let left = rect.left() + 10.0;
            let right = rect.right() - 10.0;
            let top = rect.top();
            let color = feedback.color();
            p.text(pos2(left, top + 9.0), Align2::LEFT_TOP, tool.tool.label(), FontId::proportional(12.0), presentation::PAPER);
            p.text(pos2(right, top + 10.0), Align2::RIGHT_TOP, "B settings", FontId::proportional(10.0), presentation::MUTED);
            let size = match tool.tool {
                ToolKind::Hand => format!("{:.2} m wide", if tool.size_in_metres { tool.hand_size_m } else { tool.active_voxel_m }),
                ToolKind::BuildersWand | ToolKind::DestructorWand => format!("{} {} max", tool.wand_max_blocks, if tool.size_in_metres { "m²" } else { "faces" }),
                ToolKind::ExplosiveWand => format!("{} {} radius",tool.explosion_radius,if tool.size_in_metres {"m"}else{"cell"}),
                _ => format!("{} cells", feedback.total_cells),
            };
            p.text(pos2(left, top + 27.0), Align2::LEFT_TOP, size, FontId::proportional(10.0), presentation::MUTED);
            p.text(pos2(right, top + 27.0), Align2::RIGHT_TOP, format!("Reach {:.1} m", tool.placement_distance), FontId::proportional(11.0), presentation::PAPER);
            let y = top + 46.0;
            p.line_segment([pos2(left,y),pos2(right,y)], Stroke::new(3.0, presentation::EDGE));
            p.line_segment([pos2(left,y),pos2(left+(right-left)*tool.placement_distance/24.0,y)], Stroke::new(3.0,color));
            if let Some(distance) = feedback.distance_m {
                let x = left + (right-left)*(distance/24.0).clamp(0.0,1.0);
                p.line_segment([pos2(x,y-3.0),pos2(x,y+3.0)],Stroke::new(1.5,presentation::PAPER));
            }
            let status = if tool.pending_cells()>0 { format!("Applying {} cells",tool.pending_cells()) }
                else if let Some(n) = feedback.material_budget { format!("{} / {} cells available", n, feedback.total_cells) }
                else if feedback.air_placement { "Air placement".into() }
                else if feedback.out_of_range { format!("Beyond reach · {:.1} m",feedback.distance_m.unwrap_or_default()) }
                else if feedback.total_cells>0 {
                    let count = format!("{} {}", feedback.total_cells, if feedback.total_cells == 1 { "cell" } else { "cells" });
                    if feedback.visible_cells < feedback.total_cells { format!("{count} · sampled") }
                    else if let Some(distance) = feedback.distance_m { format!("{count} · {distance:.1} m") }
                    else { count }
                }
                else { "Aim at a surface".into() };
            p.text(pos2(left,top+55.0),Align2::LEFT_TOP,status,FontId::proportional(10.0),color);
        });
}
