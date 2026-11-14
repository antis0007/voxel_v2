//! Death screen and the breath row above the vitals.
use super::{UiAction, presentation as style};
use crate::world::adventure::death::Death;
use egui::{Align2, Color32, RichText, Stroke, vec2};

const BUBBLES: usize = 10;
const BUBBLE_STEP: f32 = 10.0;
const BUBBLE_RADIUS: f32 = 3.5;

/// A small segmented charge strip below the hotbar, hidden at full charge.
pub(super) fn levitation(ctx: &egui::Context, hotbar: egui::Rect, seconds: f32) {
    if seconds >= 1.5 { return; }
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("levitation_charge")));
    let left = hotbar.center().x - 45.0;
    let y = hotbar.bottom() + 3.0;
    for i in 0..10 {
        let rect = egui::Rect::from_min_size(egui::pos2(left + i as f32 * 9.0, y), vec2(7.0, 3.0));
        painter.rect_filled(rect, 0.0, style::INK);
        let fill = (seconds / 1.5 * 10.0 - i as f32).clamp(0.0, 1.0);
        if fill > 0.0 {
            painter.rect_filled(egui::Rect::from_min_size(rect.min, vec2(rect.width() * fill, rect.height())),
                0.0, Color32::from_rgb(132, 196, 232));
        }
    }
}

/// Draws breath bubbles over the hunger side while breath is below full.
pub(super) fn breath(ctx: &egui::Context, vitals: egui::Rect, breath: f32) {
    if breath >= 1.0 {
        return;
    }
    let id = egui::Id::new("breath_row");
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, id));
    let y = vitals.top() - BUBBLE_RADIUS - 3.0;
    let units = breath.clamp(0.0, 1.0) * BUBBLES as f32;
    // Bubbles burst from the left, mirroring how hunger empties toward health.
    for k in 0..BUBBLES {
        let fill = (units - k as f32).clamp(0.0, 1.0);
        if fill <= 0.0 {
            continue;
        }
        let center = egui::pos2(
            (vitals.right() - (k as f32 + 0.5) * BUBBLE_STEP).round(),
            y.round(),
        );
        let radius = BUBBLE_RADIUS * (0.45 + 0.55 * fill);
        painter.circle(
            center,
            radius,
            Color32::from_rgb(132, 196, 232),
            Stroke::new(1.0, style::INK),
        );
        painter.circle_filled(
            center + vec2(-radius * 0.35, -radius * 0.35),
            (radius * 0.3).max(0.8),
            style::PAPER,
        );
    }
}

/// Sets the map's death marker once per death. `feet` is in voxels; the body
/// stays where it fell until the respawn, so this is the death cell.
pub(super) fn mark_death(ctx: &egui::Context, dead: bool, feet: glam::Vec3) {
    let id = egui::Id::new("death_waypoint_set");
    let marked = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if dead && !marked {
        super::map::set_death_waypoint(feet);
        let now = ctx.input(|i| i.time);
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("death_started"), now));
    }
    if !dead && marked {
        let now = ctx.input(|i| i.time);
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("respawn_started"), now));
    }
    if dead != marked {
        ctx.data_mut(|d| d.insert_temp(id, dead));
    }
    if !dead && !super::magic_fx::reduced(ctx) {
        let elapsed = ctx.input(|i| i.time) - ctx.data(|d| d.get_temp::<f64>(egui::Id::new("respawn_started"))).unwrap_or(-10.0);
        if elapsed < 0.7 {
            let alpha = (180.0 * (1.0 - elapsed / 0.7).clamp(0.0, 1.0)) as u8;
            ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("respawn_fade")))
                .rect_filled(ctx.content_rect(), 0, Color32::from_black_alpha(alpha));
            ctx.request_repaint();
        }
    }
}

/// Full-screen death overlay. Returns `Respawn` when the button is pressed.
pub(super) fn death_screen(ctx: &egui::Context, death: &Death, keep_inventory: bool, checkpoint: bool) -> UiAction {
    let screen = ctx.content_rect();
    let elapsed = (ctx.input(|i| i.time) - ctx.data(|d| d.get_temp::<f64>(egui::Id::new("death_started"))).unwrap_or(0.0)) as f32;
    let fade = if super::magic_fx::reduced(ctx) { 1.0 } else { (elapsed / 0.65).clamp(0.0, 1.0) };
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("death_shade")));
    painter.rect_filled(
        screen,
        0,
        Color32::from_rgba_unmultiplied(24, 12, 17, (155.0 * fade) as u8),
    );
    // Quiet edge darkening without flashes or repeated pulses.
    for i in 0..12 {
        let inset = i as f32 * screen.height() * 0.012;
        painter.rect_stroke(screen.shrink(inset), 0,
            Stroke::new(screen.height() * 0.025, Color32::from_black_alpha((8.0 * fade) as u8)), egui::StrokeKind::Inside);
    }
    if elapsed < 0.8 { ctx.request_repaint(); }
    let mut action = UiAction::None;
    egui::Area::new(egui::Id::new("death_screen"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            style::hud_frame(ui.ctx()).show(ui, |ui| {
                ui.set_min_width(220.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("You died").size(30.0).color(style::PAPER));
                    ui.label(
                        RichText::new(death.cause.label())
                            .size(16.0)
                            .strong()
                            .color(style::DANGER),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(if keep_inventory { "Inventory kept".to_owned() }
                            else { format!("Dropped stacks: {}", death.dropped) })
                            .color(style::PAPER),
                    );
                    if death.dropped > 0 {
                        let map = crate::keybinds::hint(crate::keybinds::Action::Map);
                        ui.weak(format!("Map [{map}] → Death waypoint"));
                        ui.weak("Walk over dropped gear to recover it");
                    }
                    ui.label(
                        RichText::new(format!(
                            "Location: {:.0}, {:.0}, {:.0} m",
                            death.pos.x, death.pos.y, death.pos.z
                        ))
                        .monospace()
                        .color(style::MUTED),
                    );
                    ui.add_space(10.0);
                    ui.label(RichText::new(if checkpoint { "Respawn: checkpoint" } else { "Respawn: world spawn" }).color(style::MUTED));
                    if ui.add_enabled(elapsed >= 0.65, egui::Button::new("Respawn").min_size(vec2(180.0, 32.0))).clicked() {
                        action = UiAction::Respawn;
                    }
                    if ui.button("World list").clicked() { action = UiAction::BackToWorldSelect; }
                });
            });
        });
    action
}
