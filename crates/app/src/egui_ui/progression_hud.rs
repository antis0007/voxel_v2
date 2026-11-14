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

pub(super) fn assembled_bar(painter:&egui::Painter,rect:Rect,tool:&engine_tools::ToolAssembly) {
    let Ok(stats)=tool.stats() else{return;};
    if tool.durability_left>=stats.durability {return;}
    let frac=tool.durability_left as f32/stats.durability as f32;
    let back=Rect::from_min_size(egui::pos2(rect.left()+2.0,rect.bottom()-3.0),vec2((rect.width()-4.0).max(1.0),3.0));
    painter.rect_filled(back,0,Color32::BLACK);
    let fill=Rect::from_min_size(back.min,vec2(back.width()*frac,2.0));
    painter.rect_filled(fill,0,Color32::from_rgb((230.0*(1.0-frac))as u8,(210.0*frac)as u8,52));
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
        Kind::Resistance => Color32::from_rgb(168, 121, 99),
        Kind::Absorption => Color32::from_rgb(235, 199, 70),
        Kind::Haste => Color32::from_rgb(218, 190, 86),
        Kind::JumpBoost => Color32::from_rgb(100, 199, 98),
    }
}

fn roman(level: u8) -> &'static str {
    ["", "", " II", " III", " IV", " V"].get(level as usize).copied().unwrap_or("")
}

const BADGE: f32 = 26.0;
/// Seconds left when a badge starts to blink.
const EXPIRING_S: f32 = 5.0;

/// Whether `kind` is active this frame.
pub(super) fn active(ctx: &egui::Context, kind: Kind) -> bool {
    view(ctx).is_some_and(|v| v.effects.iter().any(|a| a.kind == kind))
}

/// `45s`, `3m`: whole units, minutes once past ninety seconds.
fn remaining(seconds: f32) -> String {
    if seconds > 90.0 { format!("{:.0}m", (seconds / 60.0).round()) } else { format!("{:.0}s", seconds.ceil()) }
}

/// Effect badges in one row just above `vitals`: buffs first, then debuffs,
/// each with a sweep for time spent. `mana_poison` is the zone multiplier
/// while mana poisoning bites. Returns the row's rect when anything drew.
pub(super) fn effect_bar(ctx: &egui::Context, vitals: Rect, mana_poison: Option<f32>, interactable: bool) -> Option<Rect> {
    let view = view(ctx);
    let mut list: Vec<_> = view.iter().flat_map(|v| v.effects.iter().copied()).collect();
    list.sort_by_key(|a| (a.kind.harmful(), a.kind as u8));
    if list.is_empty() && mana_poison.is_none() {
        return None;
    }
    let time = ctx.input(|i| i.time as f32);
    let still = super::magic_fx::reduced(ctx);
    let label_h = 11.0;
    let top = vitals.top() - 6.0 - BADGE - label_h;
    let area = egui::Area::new(egui::Id::new("status_effects"))
        .fixed_pos(egui::pos2(vitals.left(), top))
        .interactable(interactable)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
            ui.horizontal(|ui| {
                for a in &list {
                    let total = a.total.max(a.seconds).max(0.001);
                    let blink = !still && a.seconds < EXPIRING_S && (time * 3.0).fract() < 0.5;
                    let id = a.kind as usize;
                    let rim = if a.kind.harmful() { style::DANGER } else { color(a.kind) };
                    let response = badge(ui, id, rim, Some(1.0 - a.seconds / total), &remaining(a.seconds), a.level, blink);
                    response.on_hover_text(format!("{}{} · {}", a.kind.name(), roman(a.level), remaining(a.seconds)));
                }
                if let Some(m) = mana_poison {
                    let id = super::icons::sprites16::MANA_POISONING;
                    let response = badge(ui, id, style::DANGER, None, &format!("{m:.1}x"), 1, false);
                    response.on_hover_text(format!("Mana poisoning · {m:.1}x"));
                }
            });
        });
    Some(area.response.rect)
}

/// One badge: chamfered plate, icon, time sweep, level and the label below.
fn badge(ui: &mut egui::Ui, id: usize, rim: Color32, spent: Option<f32>, label: &str, level: u8, blink: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(BADGE, BADGE + 11.0), egui::Sense::hover());
    let tile = Rect::from_min_size(rect.min, vec2(BADGE, BADGE));
    let p = ui.painter();
    p.add(super::hud_style::plate(ui.ctx(), tile, rim));
    super::icons::sprites16::paint_effect(p, tile.shrink(4.0), id);
    if let Some(spent) = spent {
        super::hud_style::sweep(p, tile.shrink(3.0), spent.clamp(0.0, 1.0));
    }
    if blink {
        p.rect_filled(tile.shrink(2.0), 0, Color32::from_black_alpha(110));
    }
    if level > 1 {
        let at = tile.right_top() + vec2(-2.0, 1.0);
        let text = roman(level).trim();
        p.text(at + vec2(1.0, 1.0), Align2::RIGHT_TOP, text, FontId::monospace(9.0), style::INK);
        p.text(at, Align2::RIGHT_TOP, text, FontId::monospace(9.0), style::PAPER);
    }
    let foot = egui::pos2(tile.center().x, rect.bottom());
    p.text(foot + vec2(1.0, 1.0), Align2::CENTER_BOTTOM, label, FontId::monospace(10.0), style::INK);
    p.text(foot, Align2::CENTER_BOTTOM, label, FontId::monospace(10.0), style::PAPER);
    response
}
