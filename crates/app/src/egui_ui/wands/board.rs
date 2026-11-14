//! Noita-style wand screen: a stat card with the wand's slot row, a spell grid
//! below it, and a compact bar of the same slots on the inventory page.
//!
//! Geometry is in units on the container grid (see `inventory::screen`), so
//! the wand page lines up with the inventory and every slot is 20 units.
use super::{
    PALETTE_COUNT, PALETTE_TABS, WandDrag, cast_program, paint_cast_block_brackets, show_spell_hover_card,
    show_wand_hover_card,
};
use crate::egui_ui::{
    UiAction,
    icons::{self, Icon},
    inventory::screen,
    presentation as style,
};
use crate::world::{
    adventure::{self, View, wand_items, wands},
    factory::Action,
};
use egui::{Align2, Color32, FontId, Painter, Rect, Sense, Stroke, vec2};

/// Points per unit on the wand page: 234 units span its 468 points.
const U: f32 = 2.0;
const W: f32 = 234.0;
const SLOT: f32 = 20.0;
const PITCH: f32 = 22.0;
const GRID_COLUMNS: usize = 8;
const LIBRARY_PITCH: f32 = 28.0;
const LIBRARY_SLOT: f32 = 24.0;
const GRID_ROWS_SHOWN: f32 = 3.0;
const MODES: [(&str, &str); 4] = [
    ("Direct", "Spells/Cast: 1 · Order: Left to Right"),
    ("Twin", "Spells/Cast: 2 · Spread"),
    ("Impact", "Release: On Impact"),
    ("Shuffle", "Order: Random · Recharge: Faster"),
];
/// One frame colour per library tab, in `PALETTE_TABS` order.
const TYPE_COLOURS: [Color32; PALETTE_COUNT] = [
    Color32::from_rgb(88, 150, 230),
    Color32::from_rgb(110, 200, 120),
    Color32::from_rgb(190, 120, 220),
    Color32::from_rgb(230, 170, 70),
    Color32::from_rgb(230, 96, 70),
    Color32::from_rgb(150, 130, 240),
    Color32::from_rgb(112, 211, 215),
];
const LOOK: &str = "wand_board_look";

/// Remembers the held wand's look bits for the card sprite this frame.
pub(in crate::egui_ui) fn publish_look(ctx: &egui::Context, look: Option<u16>) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(LOOK), look));
}

fn type_colour(spell: u32) -> Color32 {
    PALETTE_TABS
        .iter()
        .position(|(_, list)| list.contains(&spell))
        .map_or(style::EDGE, |i| TYPE_COLOURS[i])
}

/// A recessed slot tinted and edged with the spell's category colour.
fn spell_slot(p: &Painter, r: Rect, u: f32, spell: u32, selected: bool, dim: bool) {
    screen::well(p, r, u);
    if spell != wands::EMPTY {
        let c = type_colour(spell);
        let inner = r.shrink(u);
        p.rect_filled(inner, 0, c.gamma_multiply(0.18));
        p.rect_stroke(inner, 0, Stroke::new(u * 0.5, c.gamma_multiply(0.8)), egui::StrokeKind::Inside);
        icons::paint(p, r.shrink(u * 2.0), Icon::Spell(spell));
    }
    if selected {
        screen::selected(p, r, u);
    }
    if dim {
        p.rect_filled(r.shrink(u), 0, Color32::from_black_alpha(160));
    }
}

fn wand_sprite(ctx: &egui::Context, p: &Painter, r: Rect, tier: u32) {
    match ctx.data(|d| d.get_temp::<Option<u16>>(egui::Id::new(LOOK))).flatten() {
        Some(look) => crate::egui_ui::wand_icon::paint(p, r, look),
        None => icons::paint(p, r, Icon::Wand(tier.min(255) as u8)),
    }
}

fn text(p: &Painter, at: egui::Pos2, align: Align2, s: impl ToString, size: f32, c: Color32) {
    p.text(at, align, s.to_string(), FontId::proportional(size), c);
}

/// Slot clicks and drags shared by the card and the inventory bar.
fn slot_input(
    response: &egui::Response,
    i: usize,
    spell: u32,
    selected: &mut usize,
    drag: &mut Option<WandDrag>,
    action: &mut UiAction,
) {
    if response.clicked() {
        *selected = i;
    }
    if response.secondary_clicked() && spell != wands::EMPTY {
        *action = UiAction::Factory(Action::SetWandSlot(i as u32, wands::EMPTY));
    }
    if response.drag_started() && spell != wands::EMPTY {
        *drag = Some(WandDrag::DeckSlot(i, spell));
        *selected = i;
    }
}

/// The wand card: sprite, name, mode, eight stats, mana, slots, next cast.
fn card(
    ui: &mut egui::Ui,
    view: &View,
    action: &mut UiAction,
    selected: &mut usize,
    drag: &mut Option<WandDrag>,
    hovered_slot: &mut Option<usize>,
) {
    let stats = wand_items::stats_for(view.wand, &view.loadout, view.rank);
    let eval = super::preview_cast(view);
    let (rect, _) = ui.allocate_exact_size(vec2(W, 112.0) * U, Sense::hover());
    let at = |x: f32, y: f32| rect.min + vec2(x, y) * U;
    let p = ui.painter().clone();
    screen::panel(&p, rect, U);

    let sprite = Rect::from_min_size(at(7.0, 7.0), vec2(36.0, 36.0) * U);
    screen::well(&p, sprite, U);
    wand_sprite(ui.ctx(), &p, sprite.shrink(3.0 * U), view.wand);
    ui.interact(sprite, egui::Id::new("wand_board_sprite"), Sense::hover())
        .on_hover_ui(|ui| show_wand_hover_card(ui, view));

    text(&p, at(49.0, 7.0), Align2::LEFT_TOP, stats.name, 7.0 * U, style::AMBER);
    text(&p, at(227.0, 8.0), Align2::RIGHT_TOP, format!("Tier {}", view.wand), 5.5 * U, style::MUTED);

    if let Some(binding) = crate::world::adventure::enchanting::binding(view.loadout.mode) {
        text(&p, at(49.0, 20.0), Align2::LEFT_TOP, format!("{} · {}", binding.name, binding.effect), 5.0 * U, style::PAPER);
    }
    for (k, (name, tip)) in MODES.iter().enumerate()
        .filter(|_| !crate::world::adventure::wands::is_grimoire(view.loadout.mode))
    {
        let r = Rect::from_min_size(at(49.0 + k as f32 * 45.0, 17.0), vec2(42.0, 14.0) * U);
        let on = view.loadout.mode == k as u32;
        screen::tab(&p, r, U, on);
        text(&p, r.center(), Align2::CENTER_CENTER, name, 6.0 * U, if on { style::PAPER } else { style::MUTED });
        let response = ui.interact(r, egui::Id::new(("wand_board_mode", k)), Sense::click());
        if response.hovered() && !on {
            screen::hover(&p, r, U);
        }
        if response.on_hover_text(*tip).clicked() && !on {
            *action = UiAction::Factory(Action::SetWandMode(k as u32));
        }
    }

    let quicker = |now: f32, base: f32| if now < base { style::JADE } else { style::PAPER };
    let rows: [[(&str, String, Color32); 2]; 2] = [
        [
            ("Shuffle", if stats.shuffle { "Yes" } else { "No" }.into(), style::PAPER),
            ("Cast delay", format!("{:.2} s", eval.cast_delay), quicker(eval.cast_delay, stats.cast_delay)),
        ],
        [
            ("Spells/Cast", stats.spells_per_cast.to_string(), style::PAPER),
            ("Recharge", format!("{:.2} s", stats.recharge_time), style::PAPER),
        ],
    ];
    for (row, pair) in rows.iter().enumerate() {
        let y = 34.0 + row as f32 * 11.0;
        for (col, (label, value, colour)) in pair.iter().enumerate() {
            let x0 = 49.0 + col as f32 * 92.0;
            text(&p, at(x0, y), Align2::LEFT_TOP, label, 5.5 * U, style::MUTED);
            text(&p, at(x0 + 86.0, y), Align2::RIGHT_TOP, value, 5.5 * U, *colour);
        }
    }

    // Always-cast spell, then mana as a bar across the card.
    text(&p, at(7.0, 58.0), Align2::LEFT_TOP, "Always casts", 5.5 * U, style::MUTED);
    match stats.always_cast {
        Some(spell) => {
            let r = Rect::from_min_size(at(219.0, 56.0), vec2(9.0, 9.0) * U);
            icons::paint(&p, r, Icon::Spell(spell));
            text(&p, at(216.0, 58.0), Align2::RIGHT_TOP, adventure::spell_name(spell), 5.5 * U, style::AMBER);
        }
        None => text(&p, at(227.0, 58.0), Align2::RIGHT_TOP, "None", 5.5 * U, style::MUTED),
    }
    let bar = Rect::from_min_size(at(7.0, 70.0), vec2(220.0, 8.0) * U);
    screen::well(&p, bar, U);
    let fill = (view.mana / stats.mana_max.max(1.0)).clamp(0.0, 1.0);
    let inner = bar.shrink(U);
    p.rect_filled(Rect::from_min_size(inner.min, vec2(inner.width() * fill, inner.height())), 0, style::MANA.gamma_multiply(0.8));
    let cost = if eval.mana_cost > view.mana { style::DANGER } else { style::PAPER };
    text(&p, bar.center(), Align2::CENTER_CENTER, format!("{:.0} / {:.0}", view.mana, stats.mana_max), 4.5 * U, cost);

    let cap = wands::capacity(view.wand);
    let mut rects = Vec::with_capacity(cap);
    for i in 0..cap {
        let r = Rect::from_min_size(at(7.0 + i as f32 * PITCH, 82.0), vec2(SLOT, SLOT) * U);
        rects.push(r);
        let spell = view.loadout.slot(i);
        let response = ui.interact(r, egui::Id::new(("wand_board_slot", i)), Sense::click_and_drag());
        if response.contains_pointer() {
            *hovered_slot = Some(i);
        }
        spell_slot(&p, r, U, spell, i == *selected, !wands::fits_instrument(spell, view.loadout.mode));
        // The slot the next cast starts from: a notch on its top edge.
        if i == view.loadout.cursor {
            p.rect_filled(Rect::from_min_size(r.center_top() + vec2(-2.0, 1.0) * U, vec2(4.0, 1.0) * U), 0, style::AMBER);
        }
        if response.hovered() || (drag.is_some() && response.contains_pointer()) {
            screen::hover(&p, r, U);
        }
        let ready = response.contains_pointer() && drag.is_some();
        let highlight = ui.ctx().animate_bool_with_time(response.id.with("drop"), ready, 0.12);
        if highlight > 0.0 {
            p.rect_stroke(r.shrink(U), 0, Stroke::new(U, style::MANA.gamma_multiply(highlight)),
                egui::StrokeKind::Inside);
        }
        slot_input(&response, i, spell, selected, drag, action);
        response.on_hover_ui(|ui| {
            if spell != wands::EMPTY {
                show_spell_hover_card(ui, spell, view);
            } else {
                ui.label(format!("Slot {} · Empty", i + 1));
            }
        });
    }
    paint_cast_block_brackets(&p, &rects, &eval);
    let program = cast_program(&eval);
    let colour = if eval.actions.is_empty() {
        style::MUTED
    } else if eval.mana_cost > view.mana {
        style::DANGER
    } else {
        style::MANA
    };
    let line = Rect::from_min_size(at(7.0, 105.0), vec2(220.0, 7.0) * U);
    p.with_clip_rect(line).text(line.left_top(), Align2::LEFT_TOP, &program, FontId::proportional(5.5 * U), colour);
    ui.interact(line, egui::Id::new("wand_next_cast"), Sense::hover()).on_hover_text(program);
}

/// Category tabs, a search field and the spell grid.
fn library(
    ui: &mut egui::Ui,
    view: &View,
    action: &mut UiAction,
    selected: &mut usize,
    drag: &mut Option<WandDrag>,
    tab: &mut usize,
    query: &mut String,
) {
    let channel_tab = PALETTE_TABS.len() - 1;
    let channel_width = 51.0;
    let pitch = (W - channel_width) / channel_tab as f32;
    let (bar, _) = ui.allocate_exact_size(vec2(W, 16.0) * U, Sense::hover());
    let p = ui.painter().clone();
    for (i, (title, list)) in PALETTE_TABS.iter().enumerate() {
        let width = if i == channel_tab { channel_width } else { pitch };
        let r = Rect::from_min_size(bar.min + vec2(i as f32 * pitch, 0.0) * U, vec2(width - 1.0, 16.0) * U);
        let on = *tab == i;
        screen::tab(&p, r, U, on);
        p.rect_filled(
            Rect::from_min_size(r.left_bottom() + vec2(2.0, -3.0) * U, vec2(width - 5.0, 1.0) * U),
            0,
            TYPE_COLOURS[i].gamma_multiply(if on { 1.0 } else { 0.5 }),
        );
        text(&p, r.center() - vec2(0.0, U), Align2::CENTER_CENTER, title, 5.5 * U, if on { style::PAPER } else { style::MUTED });
        let found = super::found_count(list, &view.spell_log);
        let response = ui.interact(r, egui::Id::new(("wand_board_tab", i)), Sense::click());
        if response.hovered() && !on {
            screen::hover(&p, r, U);
        }
        if response.on_hover_text(format!("Found: {found} / {}", list.len())).clicked() {
            *tab = i;
            query.clear();
        }
    }
    let channel = *tab == channel_tab;
    ui.horizontal(|ui| {
        ui.add_sized([W * U - 90.0, 30.0], egui::TextEdit::singleline(query)
            .hint_text(if channel { "Search concentration" } else { "Search spells" }));
        if ui.add_enabled(!query.is_empty(), egui::Button::new("Clear search").min_size(vec2(76.0, 30.0))).clicked() {
            query.clear();
        }
    });

    let q = query.trim().to_lowercase();
    let mut spells: Vec<u32> = if q.is_empty() {
        PALETTE_TABS[*tab].1.to_vec()
    } else if channel {
        PALETTE_TABS[*tab].1.iter().copied()
            .filter(|&s| adventure::spell_name(s).to_lowercase().contains(&q))
            .collect()
    } else {
        PALETTE_TABS[..channel_tab].iter()
            .flat_map(|(_, list)| list.iter().copied())
            .filter(|&s| adventure::spell_name(s).to_lowercase().contains(&q))
            .collect()
    };
    spells.retain(|&spell| spell != adventure::spells::STORM_SURGE || view.spell_log.grants(spell));
    spells.sort_unstable();
    spells.dedup();

    let usable = spells.iter().filter(|&&spell| view.can_wield(spell)).count();
    let status = if channel && !wands::is_grimoire(view.loadout.mode) {
        "Bind a tier III wand at an Enchanting Table to use concentration.".to_owned()
    } else if channel {
        format!("{usable} available · Hold Cast to sustain a field")
    } else {
        format!("{usable} available · Shift-click fills an empty slot")
    };
    ui.label(egui::RichText::new(status).size(12.0)
        .color(if channel { style::MANA } else { style::MUTED }));
    if spells.is_empty() {
        ui.add_space(16.0);
        ui.label(egui::RichText::new("No Matching Spells").color(style::MUTED));
        ui.add_space(16.0);
        return;
    }
    let rows = spells.len().div_ceil(GRID_COLUMNS) as f32;
    let cap = wands::capacity(view.wand);
    egui::ScrollArea::vertical()
        .id_salt("wand_board_grid")
        .max_height((GRID_ROWS_SHOWN * LIBRARY_PITCH + 8.0) * U)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(W, rows * LIBRARY_PITCH + 8.0) * U, Sense::hover());
            let p = ui.painter().clone();
            screen::panel(&p, rect, U);
            for (k, &spell) in spells.iter().enumerate() {
                let (row, col) = ((k / GRID_COLUMNS) as f32, (k % GRID_COLUMNS) as f32);
                let r = Rect::from_min_size(rect.min + vec2(7.0 + col * LIBRARY_PITCH, 4.0 + row * LIBRARY_PITCH) * U, vec2(LIBRARY_SLOT, LIBRARY_SLOT) * U);
                let available = view.can_wield(spell);
                let response = ui.interact(r, egui::Id::new(("wand_board_spell", spell)), Sense::click_and_drag());
                spell_slot(&p, r, U, spell, false, !available);
                if response.hovered() && available {
                    screen::hover(&p, r, U);
                }
                if response.drag_started() && available {
                    *drag = Some(WandDrag::PaletteSpell(spell));
                }
                let response = response.on_hover_ui(|ui| show_spell_hover_card(ui, spell, view));
                if (response.clicked() || response.secondary_clicked()) && available {
                    let first_empty = (0..cap).find(|&i| view.loadout.slot(i) == wands::EMPTY);
                    let to_first_empty = response.secondary_clicked()
                        || ui.input(|i| i.modifiers.shift || i.modifiers.ctrl || i.modifiers.command);
                    let target = if to_first_empty {
                        let Some(empty) = first_empty else { continue };
                        empty
                    } else {
                        *selected
                    };
                    *action = UiAction::Factory(Action::SetWandSlot(target as u32, spell));
                    *selected = target;
                }
            }
        });
}

/// Ends a slot or library drag on release. A drop outside a slot cancels.
fn finish_drag(ui: &egui::Ui, drag: &mut Option<WandDrag>, hovered_slot: Option<usize>, selected: &mut usize, action: &mut UiAction, u: f32) {
    let Some(current) = *drag else { return };
    let spell = match current {
        WandDrag::DeckSlot(_, s) | WandDrag::PaletteSpell(s) => s,
    };
    if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
        let p = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("wand_board_ghost")));
        let r = Rect::from_center_size(pos + vec2(4.0, 4.0) * u, vec2(SLOT, SLOT) * u);
        spell_slot(&p, r, u, spell, false, false);
    }
    if !ui.ctx().input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
        return;
    }
    match (current, hovered_slot) {
        (WandDrag::DeckSlot(from, _), Some(to)) if from != to => {
            *action = UiAction::Factory(Action::SwapWandSlots(from as u32, to as u32));
            *selected = to;
        }
        (WandDrag::PaletteSpell(s), Some(to)) => {
            *action = UiAction::Factory(Action::SetWandSlot(to as u32, s));
            *selected = to;
        }
        _ => {}
    }
    *drag = None;
}

/// The wand page: the card, then the spell grid.
pub(super) fn draw(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    if view.wand == 0 {
        ui.label(egui::RichText::new("Equip a wand or grimoire.").color(style::MUTED));
        return;
    }
    let cap = wands::capacity(view.wand);
    let ids = ["wand_edit_slot", "wand_drag_state", "wand_library_tab", "wand_library_query"].map(egui::Id::new);
    let (mut selected, mut drag, mut tab, mut query) = ui.ctx().data_mut(|d| {
        (
            d.get_temp::<usize>(ids[0]).unwrap_or(0).min(cap - 1),
            d.get_temp::<Option<WandDrag>>(ids[1]).flatten(),
            d.get_temp::<usize>(ids[2]).unwrap_or(0).min(PALETTE_TABS.len() - 1),
            d.get_temp::<String>(ids[3]).unwrap_or_default(),
        )
    });
    let mut hovered_slot = None;
    ui.scope(|ui| {
        ui.set_width(W * U);
        ui.spacing_mut().item_spacing = vec2(8.0, 5.0 * U);
        card(ui, view, action, &mut selected, &mut drag, &mut hovered_slot);
        ui.horizontal(|ui| {
            let spell = view.loadout.slot(selected);
            ui.label(egui::RichText::new(format!("Slot {}  ·  {}", selected + 1, adventure::spell_name(spell)))
                .size(13.0).color(style::PAPER));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(spell != wands::EMPTY,
                    egui::Button::new("Remove spell").min_size(vec2(96.0, 30.0))).clicked()
                {
                    *action = UiAction::Factory(Action::SetWandSlot(selected as u32, wands::EMPTY));
                }
            });
        });
        super::rune_desk::draw(ui, view, action, W * U);
        library(ui, view, action, &mut selected, &mut drag, &mut tab, &mut query);
    });
    finish_drag(ui, &mut drag, hovered_slot, &mut selected, action, U);
    ui.ctx().data_mut(|d| {
        d.insert_temp(ids[0], selected);
        d.insert_temp(ids[1], drag);
        d.insert_temp(ids[2], tab);
        d.insert_temp(ids[3], query);
    });
}

/// The held wand above the inventory: sprite and slots on the container's
/// 176-unit width. The sprite opens the wand page.
pub(in crate::egui_ui) fn inline_bar(
    ui: &mut egui::Ui,
    view: &View,
    action: &mut UiAction,
    open_wand_tab: &mut bool,
    u: f32,
) {
    if view.wand == 0 {
        return;
    }
    let ids = ["inline_wand_slot", "inline_wand_drag"].map(egui::Id::new);
    let cap = wands::capacity(view.wand);
    let (mut selected, mut drag) = ui.ctx().data_mut(|d| {
        (
            d.get_temp::<usize>(ids[0]).unwrap_or(view.loadout.cursor).min(cap - 1),
            d.get_temp::<Option<WandDrag>>(ids[1]).flatten(),
        )
    });
    let (rect, _) = ui.allocate_exact_size(vec2(screen::WIDTH, 24.0) * u, Sense::hover());
    let p = ui.painter().clone();
    screen::panel(&p, rect, u);
    let sprite = Rect::from_min_size(rect.min + vec2(4.0, 4.0) * u, vec2(16.0, 16.0) * u);
    screen::well(&p, sprite, u);
    wand_sprite(ui.ctx(), &p, sprite.shrink(2.0 * u), view.wand);
    let response = ui.interact(sprite, egui::Id::new("inline_wand_sprite"), Sense::click());
    if response.hovered() {
        screen::hover(&p, sprite, u);
    }
    if response.on_hover_ui(|ui| show_wand_hover_card(ui, view)).clicked() {
        *open_wand_tab = true;
    }
    let mut hovered_slot = None;
    for i in 0..cap {
        let r = Rect::from_min_size(rect.min + vec2(24.0 + i as f32 * 17.0, 4.0) * u, vec2(16.0, 16.0) * u);
        let spell = view.loadout.slot(i);
        let response = ui.interact(r, egui::Id::new(("inline_wand_slot", i)), Sense::click_and_drag());
        if response.contains_pointer() {
            hovered_slot = Some(i);
        }
        spell_slot(&p, r, u, spell, i == view.loadout.cursor, false);
        if response.hovered() {
            screen::hover(&p, r, u);
        }
        slot_input(&response, i, spell, &mut selected, &mut drag, action);
        response.on_hover_ui(|ui| {
            if spell != wands::EMPTY {
                show_spell_hover_card(ui, spell, view);
            }
        });
    }
    // A library drag cannot start here, so only slot swaps resolve.
    finish_drag(ui, &mut drag, hovered_slot, &mut selected, action, u);
    ui.ctx().data_mut(|d| {
        d.insert_temp(ids[0], selected);
        d.insert_temp(ids[1], drag);
    });
}
