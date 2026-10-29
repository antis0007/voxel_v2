//! XP bar above the hotbar, wear bars on slot icons and status effect badges.
use super::presentation as style;
use crate::world::inventory::SlotAddr;
use crate::world::progression::{Kind, Slot, View};
use egui::{Align2, Color32, FontId, Rect, vec2};
use std::sync::Arc;

const XP_BAR_HEIGHT: f32 = 12.0;

fn key() -> egui::Id {
    egui::Id::new("progression_view")
}

/// Stores this frame's snapshot for the HUD and slot painters.
pub fn publish(ctx: &egui::Context, view: View) {
    ctx.data_mut(|d| d.insert_temp(key(), Arc::new(view)));
}

fn view(ctx: &egui::Context) -> Option<Arc<View>> {
    ctx.data(|d| d.get_temp::<Arc<View>>(key()))
}

fn durability(ctx: &egui::Context, addr: SlotAddr) -> Option<(u32, u32)> {
    let slot = Slot::of_addr(addr)?;
    view(ctx)?.durability.iter().find(|(s, _, _)| *s == slot).map(|&(_, left, max)| (left, max))
}

/// Wear bar along the bottom of a slot icon, shown once the stack is worn.
pub(super) fn wear_bar(painter: &egui::Painter, rect: Rect, addr: SlotAddr) {
    let Some((left, max)) = durability(painter.ctx(), addr) else { return };
    if left >= max || max == 0 {
        return;
    }
    let frac = left as f32 / max as f32;
    let unit = (rect.width() / 18.0).round().max(1.0);
    let back = Rect::from_min_size(
        egui::pos2(rect.left() + 2.0 * unit, rect.bottom() - 3.0 * unit),
        vec2(rect.width() - 4.0 * unit, 2.0 * unit),
    );
    painter.rect_filled(back, 0, Color32::BLACK);
    let fill = Rect::from_min_size(back.min, vec2((back.width() * frac).round().max(unit), unit));
    let color = Color32::from_rgb((255.0 * (1.0 - frac)) as u8, (255.0 * frac) as u8, 0);
    painter.rect_filled(fill, 0, color);
}

/// `Durability: N / M` for the tooltip of a durable stack.
pub(super) fn durability_line(ui: &mut egui::Ui, addr: SlotAddr) {
    if let Some((left, max)) = durability(ui.ctx(), addr) {
        ui.label(egui::RichText::new(format!("Durability: {left} / {max}")).color(style::PAPER));
    }
}

/// Draws the XP bar directly above `hotbar` and returns the hotbar rect grown to include it.
pub(super) fn xp_bar(ctx: &egui::Context, hotbar: Rect) -> Rect {
    let Some(view) = view(ctx) else { return hotbar };
    let rect = Rect::from_min_size(
        egui::pos2(hotbar.left() + 4.0, hotbar.top() - XP_BAR_HEIGHT),
        vec2(hotbar.width() - 8.0, 5.0),
    );
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("xp_bar")));
    painter.rect_filled(rect.expand(1.0), 0, style::INK);
    painter.rect_filled(rect, 0, Color32::from_rgb(28, 40, 20));
    let fill = Rect::from_min_size(rect.min, vec2((rect.width() * view.progress.clamp(0.0, 1.0)).round(), rect.height()));
    painter.rect_filled(fill, 0, Color32::from_rgb(128, 224, 48));
    if view.level > 0 {
        let at = egui::pos2(rect.center().x, rect.center().y - 1.0);
        let font = FontId::monospace(11.0);
        let text = view.level.to_string();
        for d in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0)] {
            painter.text(at + d, Align2::CENTER_CENTER, &text, font.clone(), style::INK);
        }
        painter.text(at, Align2::CENTER_CENTER, &text, font, Color32::from_rgb(128, 255, 32));
    }
    hotbar.with_min_y(hotbar.top() - XP_BAR_HEIGHT - 2.0)
}

fn color(kind: Kind) -> Color32 {
    match kind {
        Kind::Regeneration => Color32::from_rgb(205, 92, 171),
        Kind::Speed => Color32::from_rgb(124, 175, 198),
        Kind::Slowness => Color32::from_rgb(90, 108, 129),
        Kind::Strength => Color32::from_rgb(147, 36, 35),
        Kind::Weakness => Color32::from_rgb(72, 77, 72),
        Kind::Poison => Color32::from_rgb(78, 147, 49),
        Kind::FireResistance => Color32::from_rgb(228, 154, 58),
    }
}

fn roman(level: u8) -> &'static str {
    ["", "", " II", " III", " IV", " V"].get(level as usize).copied().unwrap_or("")
}

/// Active effects as badges with seconds left, top right.
pub(super) fn effects(ctx: &egui::Context) {
    let Some(view) = view(ctx) else { return };
    if view.effects.is_empty() {
        return;
    }
    egui::Area::new("status_effects".into())
        .anchor(Align2::RIGHT_TOP, [-12.0, 12.0])
        .interactable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for a in &view.effects {
                    let (rect, response) = ui.allocate_exact_size(vec2(36.0, 30.0), egui::Sense::hover());
                    let painter = ui.painter();
                    let badge = Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 8.0), vec2(14.0, 14.0));
                    painter.rect_filled(badge.expand(1.0), 0, style::INK);
                    painter.rect_filled(badge, 0, color(a.kind));
                    if a.kind.harmful() {
                        painter.rect_filled(badge.shrink(4.0), 0, style::INK);
                    }
                    painter.text(
                        egui::pos2(rect.center().x, rect.bottom()),
                        Align2::CENTER_BOTTOM,
                        format!("{:.0}s", a.seconds.ceil()),
                        FontId::monospace(10.0),
                        style::PAPER,
                    );
                    response.on_hover_text(format!("{}{}", a.kind.name(), roman(a.level)));
                }
            });
        });
}
