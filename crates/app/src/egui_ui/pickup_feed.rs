//! Pickup feed, bottom right above the subtitles: one row per item picked up in
//! the last few seconds, counts merged, newest at the bottom. It replaces a
//! single badge beside the hotbar that was taller than a slot and read as a
//! tenth hotbar slot (docs/design/HUD_LAYOUT.md).
//!
//! The inventory keeps only the latest notice (and already merges repeats of
//! one item), so the feed folds each new notice into its own short history.
use super::presentation as style;
use crate::world::inventory::{Item, PickupNotice};
use egui::{Color32, FontId, vec2};

const SHOWN_S: f64 = 3.0;
const FADE_S: f64 = 0.6;
const MAX_ROWS: usize = 5;

#[derive(Clone, Default)]
struct Feed {
    rows: Vec<Row>,
    /// The notice folded in last, so a held notice is not counted twice.
    last: Option<(Item, u32, bool)>,
}

#[derive(Clone)]
struct Row {
    item: Item,
    count: u32,
    overflow: bool,
    at: f64,
}

fn id() -> egui::Id {
    egui::Id::new("pickup_feed_rows")
}

/// Amount a notice adds over the one folded in before it: a notice that grew
/// for the same item adds the difference, anything else is a fresh pickup.
fn added(last: Option<(Item, u32, bool)>, notice: (Item, u32, bool)) -> u32 {
    match last {
        Some((item, count, overflow)) if item == notice.0 && overflow == notice.2 && notice.1 >= count => {
            notice.1 - count
        }
        _ => notice.1,
    }
}

/// Folds this frame's notice into the feed and draws it. `notice` is `None`
/// when there is no notice or its item is not unlocked for toasts yet.
pub(super) fn draw(ctx: &egui::Context, notice: Option<&PickupNotice>) {
    let now = ctx.input(|i| i.time);
    let mut feed = ctx.data(|d| d.get_temp::<Feed>(id())).unwrap_or_default();
    match notice {
        Some(n) => {
            let key = (n.item, n.count, n.overflow);
            if feed.last != Some(key) {
                let amount = added(feed.last, key);
                feed.last = Some(key);
                if let Some(row) = feed.rows.iter_mut().find(|r| r.item == n.item && r.overflow == n.overflow) {
                    row.count = row.count.saturating_add(amount);
                    row.at = now;
                } else {
                    feed.rows.push(Row { item: n.item, count: amount, overflow: n.overflow, at: now });
                }
                // The refreshed row moves to the bottom, where the eye last looked.
                feed.rows.sort_by(|a, b| a.at.total_cmp(&b.at));
            }
        }
        None => feed.last = None,
    }
    feed.rows.retain(|r| now - r.at < SHOWN_S + FADE_S);
    let excess = feed.rows.len().saturating_sub(MAX_ROWS);
    feed.rows.drain(..excess);
    if feed.rows.is_empty() {
        ctx.data_mut(|d| d.insert_temp(id(), feed));
        return;
    }
    ctx.request_repaint();
    let slot = super::hud_layout::place(ctx, "pickup_feed");
    let shown = egui::Area::new(egui::Id::new("pickup_feed"))
        .anchor(slot.align, slot.offset)
        .interactable(false)
        .show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
                for row in &feed.rows {
                    let fade = (((SHOWN_S + FADE_S) - (now - row.at)) / FADE_S).clamp(0.0, 1.0) as f32;
                    feed_row(ui, row, fade);
                }
            });
        });
    super::hud_layout::claim(ctx, "pickup_feed", shown.response.rect);
    ctx.data_mut(|d| d.insert_temp(id(), feed));
}

/// `+3 Log [icon]` on a dark strip, right-aligned like the subtitles below it.
fn feed_row(ui: &mut egui::Ui, row: &Row, fade: f32) {
    ui.scope(|ui| {
        ui.set_opacity(fade);
        let (text, colour) = if row.overflow {
            ("Pack Full".to_owned(), style::AMBER)
        } else {
            (format!("+{} {}", row.count, row.item.name()), style::PAPER)
        };
        let galley = ui.painter().layout_no_wrap(text, FontId::proportional(11.0), colour);
        let icon = 16.0;
        let (rect, _) = ui.allocate_exact_size(vec2(galley.size().x + icon + 14.0, icon + 4.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 0, Color32::from_black_alpha(150));
        let icon_rect = egui::Rect::from_min_size(rect.right_top() + vec2(-icon - 2.0, 2.0), vec2(icon, icon));
        super::inventory::draw_item_icon(p, icon_rect, row.item);
        p.galley(egui::pos2(rect.left() + 5.0, rect.center().y - galley.size().y * 0.5), galley, colour);
    });
}
