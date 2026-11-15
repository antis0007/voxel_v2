//! Goal tracker under the minimap and the Goals page of the field book.
mod settlement;
pub(crate) mod review;
use super::grimoire;
use super::presentation as style;
use crate::world::goals::{Tier, View};
use egui::{Align2, Color32, FontId, Rect, RichText, vec2};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

const PREF_FILE: &str = "goal_track.pref";
const FLASH_SECONDS: f64 = 2.5;
const WIDTH: f32 = 248.0;
fn action_label(text:&str)->String {
    use crate::keybinds::{Action, hint};
    let mut text = text.to_owned();
    for (token, action) in [("[F]", Action::Interact), ("[Break]", Action::Break), ("[Inventory]", Action::Inventory), ("[Cast]", Action::Cast)] {
        text = text.replace(token, &format!("[{}]", hint(action)));
    }
    text
}
/// 0 unread, 1 shown, 2 hidden.
static PREF: AtomicU8 = AtomicU8::new(0);
static WARNED: AtomicBool = AtomicBool::new(false);

fn key() -> egui::Id {
    egui::Id::new("goal_track_view")
}

fn pref_path() -> std::path::PathBuf {
    crate::world::storage::save_directory().join(PREF_FILE)
}

/// Stores this frame's snapshot for the tracker and the Goals page.
pub fn publish(ctx: &egui::Context, view: Arc<View>) {
    ctx.data_mut(|d| d.insert_temp(key(), view));
}

pub(crate) fn publish_world(ctx: &egui::Context, world: &crate::world::World) {
    let scope = world.regions.view.root.clone();
    let old = world_scope(ctx);
    let now = ctx.input(|i| i.time);
    let prior_sample = ctx.data(|d| d.get_temp::<f64>(egui::Id::new("settlement_goal_sample")));
    let refresh_city = old.as_ref() != Some(&scope)
        || prior_sample.is_none_or(|last| now < last || now - last >= 0.25);
    let city = refresh_city.then(|| crate::world::farming::jobs::onboarding::view(world));
    ctx.data_mut(|d| {
        if old.as_ref() != Some(&scope) {
            d.remove::<(u32, f64)>(egui::Id::new("goal_track_flash"));
        }
        d.insert_temp(egui::Id::new("goal_world"), scope);
        if let Some(city) = city {
            d.insert_temp(egui::Id::new("settlement_goal_view"), city);
            d.insert_temp(egui::Id::new("settlement_goal_sample"), now);
        }
        d.insert_temp(egui::Id::new("survival_goal_visible"),
            world.rules.mode == crate::world::rules::GameMode::Survival
                && !crate::world::farming::jobs::onboarding::is_settlement(world));
    });
    publish(ctx, world.goals.view());
}

pub(crate) fn controls(ui: &mut egui::Ui) {
    let mut shown = enabled();
    if ui.checkbox(&mut shown, "Goal Tracker").changed() { set_enabled(shown); }
}

fn hide_text() -> String {
    format!("[{}] Hide", crate::keybinds::hint(crate::keybinds::Action::GoalTrack))
}

fn hide_hint(ui: &mut egui::Ui) {
    ui.label(RichText::new(hide_text()).size(10.0).color(style::MUTED));
}

/// Shortcuts for the survival tracker and its current recipe.
fn key_hints(ui: &mut egui::Ui, recipe: bool) {
    let text = if recipe { format!("{} · [Alt+R] Goal recipe", hide_text()) } else { hide_text() };
    ui.label(RichText::new(text).size(10.0).color(style::MUTED));
    ui.label(RichText::new("Inventory → Projects").size(10.0).color(style::MUTED));
}

fn view(ctx: &egui::Context) -> Option<Arc<View>> {
    ctx.data(|d| d.get_temp::<Arc<View>>(key()))
}

/// The world identity already published with the goal snapshot. UI progress
/// must not leak from a previous world sharing the same egui context.
pub(super) fn world_scope(ctx: &egui::Context) -> Option<Arc<std::path::PathBuf>> {
    ctx.data(|d| d.get_temp::<Arc<std::path::PathBuf>>(egui::Id::new("goal_world")))
}

/// Tier of the current goal: `None` before an evaluated, nonempty track,
/// `Some(None)` once that track is finished. A pending/failed track is not completion.
pub(super) fn current_tier(ctx: &egui::Context) -> Option<Option<Tier>> {
    view(ctx).filter(|v| !v.rows.is_empty()).map(|v| v.current.as_ref().and_then(|row| row.tier))
}

pub fn enabled() -> bool {
    match PREF.load(Ordering::Relaxed) {
        0 => {
            let shown = std::fs::read(pref_path()).map_or(true, |b| b.first() != Some(&b'0'));
            PREF.store(if shown { 1 } else { 2 }, Ordering::Relaxed);
            shown
        }
        v => v == 1,
    }
}

pub fn set_enabled(shown: bool) {
    PREF.store(if shown { 1 } else { 2 }, Ordering::Relaxed);
    let written = std::fs::create_dir_all(crate::world::storage::save_directory())
        .and_then(|()| std::fs::write(pref_path(), if shown { b"1" } else { b"0" }));
    if let Err(e) = written
        && !WARNED.swap(true, Ordering::Relaxed)
    {
        eprintln!("[goals] Cannot write tracker preference: {e}");
    }
}

pub fn toggle() {
    set_enabled(!enabled());
}

/// Draws the tracker below the compass strip. Hidden once every goal is met.
pub(crate) fn tracker(ctx: &egui::Context) {
    if !enabled() {
        return;
    }
    if let Some(Some(city)) = ctx.data(|d| d.get_temp::<Option<crate::world::farming::jobs::onboarding::View>>(egui::Id::new("settlement_goal_view"))) {
        settlement::draw(ctx, &city);
        return;
    }
    if !ctx.data(|d| d.get_temp::<bool>(egui::Id::new("survival_goal_visible"))).unwrap_or(false) { return; }
    let Some(view) = view(ctx) else { return };
    let now = ctx.input(|i| i.time);
    let flash_id = egui::Id::new("goal_track_flash");
    let previous = ctx.data(|d| d.get_temp::<(u32, f64)>(flash_id));
    let (seen, since) = previous.unwrap_or((view.met_count, f64::MIN));
    if previous.is_none() { ctx.data_mut(|d| d.insert_temp(flash_id, (seen, since))); }
    let since = if seen != view.met_count {
        ctx.data_mut(|d| d.insert_temp(flash_id, (view.met_count, now)));
        now
    } else {
        since
    };
    let flash = ((FLASH_SECONDS - (now - since)) / FLASH_SECONDS).clamp(0.0, 1.0) as f32;
    if flash > 0.0 {
        ctx.request_repaint();
    }
    if view.current.is_none() && flash <= 0.0 {
        return;
    }
    // Pinned under the minimap like a modpack quest pin: the top centre belongs to
    // the look-at panel and compass, and a goal is read at a glance, not aimed at.
    let slot = super::hud_layout::place(ctx, "goal_tracker");
    let alt = ctx.input(|i| i.modifiers.alt);
    // Full strength while it changes or on Alt; otherwise it recedes a little.
    let opacity = if alt || flash > 0.0 || now - since < SETTLE_SECONDS { 1.0 } else { RESTING_OPACITY };
    let shown = egui::Area::new(egui::Id::new("goal_tracker"))
        .anchor(slot.align, slot.offset)
        .interactable(false)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_opacity(opacity);
            let plate = tracker_frame().show(ui, |ui| {
                ui.set_width(slot.width.unwrap_or(WIDTH));
                ui.spacing_mut().item_spacing = vec2(4.0, 3.0);
                if flash > 0.0
                    && let Some(title) = &view.last_met
                {
                    let a = (flash * 255.0) as u8;
                    let c = style::JADE;
                    ui.label(RichText::new(format!("{title} · Done")).size(11.0)
                        .color(Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)));
                }
                let Some(row) = &view.current else { return };
                let tier = row.tier.map_or("Goal", Tier::label);
                ui.label(RichText::new(tier)
                    .size(10.0).color(style::MUTED));
                ui.label(RichText::new(&row.title).size(12.5).color(style::AMBER));
                if !row.action.is_empty() {
                    ui.label(RichText::new(action_label(&row.action)).size(11.0).color(style::PAPER));
                }
                // The goal's own count first; the recipe inputs after it are capped at
                // what the goal needs, so a met input reads `2 / 2`, never `3 / 2`.
                if let Some(target) = &view.target {
                    ingredient(ui, target.item, target.held, target.needed);
                }
                let inputs: Vec<_> = view.ingredients.iter()
                    .filter(|i| view.target.as_ref().is_none_or(|t| t.item != i.item))
                    .collect();
                if !inputs.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        if view.target.is_some() {
                            ui.label(RichText::new("From").size(10.0).color(style::MUTED));
                        }
                        for ing in inputs {
                            ingredient(ui, ing.item, ing.held, ing.needed);
                        }
                    });
                }
                // Keys are only shown on request, never at rest.
                if alt { key_hints(ui, view.recipe_item.is_some()); }
            });
            spine(ui.painter(), plate.response.rect);
        });
    super::hud_layout::claim(ctx, "goal_tracker", shown.response.rect);
}

/// Seconds a changed goal stays at full strength before it recedes.
const SETTLE_SECONDS: f64 = 8.0;
const RESTING_OPACITY: f32 = 0.82;

/// Translucent, borderless plate: a persistent element must weigh less than the
/// on-demand look-at panel, so it gets no frame line, only a gold spine.
fn tracker_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(14, 16, 20, 196))
        .inner_margin(egui::Margin { left: 10, right: 8, top: 6, bottom: 7 })
}

/// Two-point gold spine on the left edge of a tracker plate.
fn spine(p: &egui::Painter, rect: Rect) {
    p.rect_filled(Rect::from_min_max(rect.left_top(), egui::pos2(rect.left() + 2.0, rect.bottom())), 0, grimoire::GOLD_DARK);
}

fn ingredient(ui: &mut egui::Ui, item: crate::world::inventory::Item, held: u32, needed: u32) {
    let text = format!("{} / {needed}", held.min(needed));
    let font = FontId::proportional(10.0);
    let text_w = ui.painter().layout_no_wrap(text.clone(), font.clone(), Color32::WHITE).size().x;
    let (rect, response) = ui.allocate_exact_size(vec2(18.0 + 3.0 + text_w + 6.0, 18.0), egui::Sense::hover());
    let icon = Rect::from_min_size(rect.min, vec2(18.0, 18.0));
    let p = ui.painter();
    super::icons::frame(p, icon, false);
    super::inventory::draw_item_icon(p, icon.shrink(2.0), item);
    let color = if held >= needed { style::JADE } else { style::PAPER };
    p.text(egui::pos2(icon.right() + 3.0, rect.center().y), Align2::LEFT_CENTER, text, font, color);
    response.on_hover_text(item.name());
}

pub(super) fn next(ui: &mut egui::Ui) {
    let Some(view) = view(ui.ctx()) else { return };
    if let Some(row) = &view.current {
        ui.label(RichText::new(&row.title).strong().color(style::AMBER));
        ui.label(action_label(&row.action));
        ui.horizontal_wrapped(|ui| {
            if let Some(item) = row.recipe_item {
                recipe_link(ui, item);
            }
            for input in view.ingredients.iter().filter(|i| Some(i.item) != row.recipe_item && i.held < i.needed) {
                recipe_link(ui, input.item);
            }
        });
    }
}

/// Completed and available goals.
pub(super) fn page(ui: &mut egui::Ui) {
    let Some(view) = view(ui.ctx()) else { return };
    controls(ui);
    let done = view.rows.iter().filter(|r| r.done).count();
    ui.label(RichText::new(format!("Met: {done} / {}", view.rows.len())).color(style::MUTED));
    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
        for tier in Tier::ALL {
            let rows: Vec<_> = view.rows.iter().filter(|r| r.tier == Some(tier)).collect();
            let met = rows.iter().filter(|r| r.done).count();
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{} · {met} / {}", tier.label(), rows.len())).strong().color(style::AMBER));
            for row in rows {
                let current = view.current.as_ref().is_some_and(|c| c.title == row.title && c.action == row.action);
                let (mark, color) = match (row.done, current) {
                    (true, _) => ("Done", style::JADE),
                    (false, true) => ("Next", style::AMBER),
                    (false, false) if row.optional => ("Extra", style::MUTED),
                    (false, false) => ("Open", style::MUTED),
                };
                ui.horizontal_wrapped(|ui| {
                    ui.add_sized([34.0, 14.0], egui::Label::new(RichText::new(mark).size(10.0).color(color)));
                    if !row.done && let Some(item) = row.recipe_item {
                        if ui.link(&row.title).on_hover_text(item.name()).clicked() {
                            super::recipe_book::request(ui.ctx(), item, false);
                        }
                    } else {
                        ui.label(RichText::new(&row.title).color(if row.done { style::MUTED } else { style::PAPER }));
                    }
                    ui.label(RichText::new(action_label(&row.action)).size(10.0).color(style::MUTED));
                });
            }
        }
    });
}

fn recipe_link(ui: &mut egui::Ui, item: crate::world::inventory::Item) {
    if ui.link(item.name()).on_hover_text("Recipes and sources").clicked() {
        super::recipe_book::request(ui.ctx(), item, false);
    }
}
