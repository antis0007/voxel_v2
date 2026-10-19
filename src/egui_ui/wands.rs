use super::{
    UiAction,
    icons::{self, Icon},
    presentation as style,
};
use crate::world::{
    adventure::{self, View, wands},
    factory::Action,
};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, vec2};

const SPELL_ORDER: &[u32] = &[0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15];
const MODIFIER_ORDER: &[u32] = &[16, 17, 18, 19, 20, 21, 22, 23];

#[derive(Clone, Copy, Debug)]
pub(super) enum WandDrag {
    DeckSlot(usize, u32),
    PaletteSpell(u32),
}

fn stat_row(ui: &mut egui::Ui, label: &str, value: String, value_color: Color32) {
    ui.horizontal(|ui| {
        ui.set_min_width(228.0);
        ui.label(
            RichText::new(label)
                .font(FontId::monospace(11.0))
                .color(style::MUTED),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .font(FontId::monospace(11.5))
                    .color(value_color),
            );
        });
    });
}

fn signed_sec(v: f32) -> String {
    if v.abs() < 0.005 {
        "0.00 s".to_string()
    } else if v > 0.0 {
        format!("+{v:.2} s")
    } else {
        format!("{v:.2} s")
    }
}

fn signed_deg(v: f32) -> String {
    if v.abs() < 0.05 {
        "0.0 DEG".to_string()
    } else if v > 0.0 {
        format!("+{v:.1} DEG")
    } else {
        format!("{v:.1} DEG")
    }
}

/// Draws visual Noita/Wand-Sim style Cast-Block brackets and Trigger-Payload tethers
/// underneath a horizontal row of wand slot rectangles (`slot_rects[0..cap]`).
pub(super) fn paint_cast_block_brackets(
    painter: &egui::Painter,
    slot_rects: &[Rect],
    eval: &wands::CastBlockEval,
) {
    if slot_rects.is_empty() || eval.actions.is_empty() {
        return;
    }

    // Collect all slot indices participating in the evaluated cast block.
    let mut block_slots: Vec<usize> = eval.modifier_slots.clone();
    for act in &eval.actions {
        if !block_slots.contains(&act.slot) {
            block_slots.push(act.slot);
        }
        if let Some(p_slot) = act.payload_slot {
            if !block_slots.contains(&p_slot) {
                block_slots.push(p_slot);
            }
        }
    }

    // 1. Primary Cast-Block Bracket when modifiers or multicasts bind 2+ slots together
    if block_slots.len() > 1 {
        let min_idx = *block_slots.iter().min().unwrap_or(&0);
        let max_idx = *block_slots.iter().max().unwrap_or(&0);
        if let (Some(r_start), Some(r_end)) = (slot_rects.get(min_idx), slot_rects.get(max_idx)) {
            let y = r_start.bottom() + 2.5;
            let x0 = r_start.left() + 3.0;
            let x1 = r_end.right() - 3.0;
            let stroke = Stroke::new(1.5, style::AMBER);
            painter.line_segment([Pos2::new(x0, y - 3.0), Pos2::new(x0, y)], stroke);
            painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], stroke);
            painter.line_segment([Pos2::new(x1, y), Pos2::new(x1, y - 3.0)], stroke);
            for &idx in &block_slots {
                if let Some(r) = slot_rects.get(idx) {
                    painter.circle_filled(Pos2::new(r.center().x, y), 1.8, style::AMBER);
                }
            }
        }
    }

    // 2. Secondary cyan Trigger -> Payload tether arc
    for act in &eval.actions {
        if let Some(p_slot) = act.payload_slot {
            if let (Some(r_carrier), Some(r_payload)) =
                (slot_rects.get(act.slot), slot_rects.get(p_slot))
            {
                let y = r_carrier.top() - 2.0;
                let c0 = Pos2::new(r_carrier.center().x, y);
                let c1 = Pos2::new(r_payload.center().x, y);
                let tether_col = Color32::from_rgb(255, 168, 72);
                let stroke = Stroke::new(1.4, tether_col);
                painter.line_segment([Pos2::new(c0.x, y + 3.0), c0], stroke);
                painter.line_segment([c0, c1], stroke);
                painter.line_segment([c1, Pos2::new(c1.x, y + 3.0)], stroke);
            }
        }
    }
}

/// Renders a Noita-style Wand Hover Card displaying all 8 wand statistics,
/// Always-Cast passive, live next-cast-block evaluation, and the visual spell deck.
pub(super) fn show_wand_hover_card(ui: &mut egui::Ui, view: &View) {
    let stats = wands::stats(view.wand, view.loadout.mode, view.rank);
    let eval = wands::evaluate_cast_block(
        view.wand,
        view.rare_spells,
        view.discoveries,
        view.rank,
        &view.loadout,
        view.loadout.cursor,
    );

    ui.set_min_width(248.0);
    ui.set_max_width(268.0);
    ui.spacing_mut().item_spacing = vec2(4.0, 2.5);

    // Header: Wand Name + Chassis subtitle
    ui.horizontal(|ui| {
        let [r, g, b] = adventure::spell_color(view.spell);
        let gem_col = Color32::from_rgb(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
        );
        let (dot_rect, _) = ui.allocate_exact_size(vec2(10.0, 14.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(dot_rect.center(), 4.0, gem_col);
        ui.strong(
            RichText::new(stats.name)
                .size(14.0)
                .color(style::AMBER),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("Tier {}", view.wand))
                    .font(FontId::monospace(10.5))
                    .color(style::MANA),
            );
        });
    });
    ui.label(
        RichText::new(stats.chassis)
            .size(11.0)
            .color(style::MUTED),
    );

    ui.separator();

    // Noita 8-Stat Table
    stat_row(
        ui,
        "Shuffle",
        if stats.shuffle { "Yes" } else { "No" }.to_string(),
        if stats.shuffle { style::AMBER } else { style::JADE },
    );
    stat_row(
        ui,
        "Spells/Cast",
        format!("{}", stats.spells_per_cast),
        style::PAPER,
    );
    stat_row(
        ui,
        "Cast Delay",
        format!("{:.2} s  (next {:.2} s)", stats.cast_delay, eval.cast_delay),
        if eval.cast_delay < stats.cast_delay {
            style::JADE
        } else {
            style::PAPER
        },
    );
    stat_row(
        ui,
        "Recharge Time",
        format!("{:.2} s", stats.recharge_time),
        if stats.recharge_time <= 0.22 {
            style::JADE
        } else {
            style::PAPER
        },
    );
    stat_row(
        ui,
        "Mana Max",
        format!("{:.0}", stats.mana_max),
        style::MANA,
    );
    stat_row(
        ui,
        "Mana Charge Spd",
        format!("{:.0}/s", stats.mana_charge_speed),
        style::MANA,
    );
    stat_row(
        ui,
        "Capacity",
        format!("{}", stats.capacity),
        style::PAPER,
    );
    stat_row(
        ui,
        "Spread",
        format!("{:.1} DEG", stats.spread_deg),
        if stats.spread_deg <= 0.5 {
            style::JADE
        } else {
            style::AMBER
        },
    );

    if let Some(always) = stats.always_cast {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Always Casts")
                    .font(FontId::monospace(11.0))
                    .color(style::AMBER),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(adventure::spell_name(always))
                        .font(FontId::monospace(11.5))
                        .color(style::AMBER),
                );
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
                icons::paint(ui.painter(), r, Icon::Spell(always));
            });
        });
    }

    ui.separator();

    // Visual Deck Strip with Cast-Block Brackets
    let cap = wands::capacity(view.wand);
    let mut slot_rects = Vec::with_capacity(cap);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(3.0, 5.0);
        for idx in 0..cap {
            let s = view.loadout.slot(idx);
            let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), egui::Sense::hover());
            slot_rects.push(r);
            icons::frame(ui.painter(), r, idx == view.loadout.cursor);
            if s != wands::EMPTY {
                icons::paint(ui.painter(), r.shrink(2.5), Icon::Spell(s));
            } else {
                ui.painter().rect_filled(
                    r.shrink(3.0),
                    0,
                    Color32::from_black_alpha(175),
                );
            }
            ui.painter().text(
                r.left_top() + vec2(2.0, 1.0),
                Align2::LEFT_TOP,
                format!("{}", idx + 1),
                FontId::monospace(8.5),
                if idx == view.loadout.cursor {
                    style::AMBER
                } else {
                    style::MUTED
                },
            );
        }
    });
    paint_cast_block_brackets(ui.painter(), &slot_rects, &eval);
    ui.add_space(3.0);

    // Live Cast Block summary
    if !eval.actions.is_empty() {
        let mut chain = Vec::new();
        for act in &eval.actions {
            let mut tags = Vec::new();
            if act.mods.homing {
                tags.push("Homing");
            }
            if act.mods.orbit {
                tags.push("Orbit");
            }
            if act.mods.bore {
                tags.push("Bore");
            }
            if act.mods.trail {
                tags.push("Wake");
            }
            let prefix = if tags.is_empty() {
                String::new()
            } else {
                format!("[{}] ", tags.join("+"))
            };
            if let Some(p) = act.payload {
                chain.push(format!(
                    "{}{} → {}",
                    prefix,
                    adventure::spell_name(act.spell),
                    adventure::spell_name(p)
                ));
            } else {
                chain.push(format!("{}{}", prefix, adventure::spell_name(act.spell)));
            }
        }
        ui.label(
            RichText::new(format!(
                "Next Cast ({:.0} MP): {}",
                eval.mana_cost,
                chain.join(" + ")
            ))
            .font(FontId::monospace(10.5))
            .color(if eval.mana_cost > view.mana {
                style::DANGER
            } else {
                style::MANA
            }),
        );
    }
}

/// Renders a Noita-style Spell / Modifier Hover Card with type badge, mana drain,
/// damage, cast delay delta, recharge delta, spread delta, and world physics note.
pub(super) fn show_spell_hover_card(ui: &mut egui::Ui, spell: u32, view: &View) {
    let available = adventure::spell_available(spell, view.wand, view.rare_spells);
    let cost = if spell == 22 {
        -20.0
    } else {
        adventure::inscriptions::cost(spell, view.discoveries)
    };
    let dmg = adventure::spell_base_damage(spell);
    let delay_mod = adventure::spell_cast_delay_delta(spell);
    let rech_mod = adventure::spell_recharge_delta(spell);
    let spread_mod = adventure::spell_spread_delta(spell);
    let is_mod = wands::is_modifier(spell);

    ui.set_min_width(226.0);
    ui.set_max_width(250.0);
    ui.spacing_mut().item_spacing = vec2(4.0, 2.0);

    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), egui::Sense::hover());
        icons::frame(ui.painter(), r, true);
        icons::paint(ui.painter(), r.shrink(2.0), Icon::Spell(spell));
        ui.vertical(|ui| {
            ui.strong(
                RichText::new(adventure::spell_name(spell))
                    .size(13.5)
                    .color(style::PAPER),
            );
            ui.label(
                RichText::new(adventure::spell_kind_tag(spell))
                    .font(FontId::monospace(10.0))
                    .color(if is_mod { style::AMBER } else { style::MANA }),
            );
        });
    });

    ui.label(
        RichText::new(adventure::spell_hint(spell))
            .size(11.5)
            .color(style::MUTED),
    );

    ui.separator();

    stat_row(
        ui,
        "Mana Drain",
        if cost < 0.0 {
            format!("{cost:.0} (Add Mana)")
        } else {
            format!("{cost:.0}")
        },
        if cost < 0.0 { style::JADE } else { style::MANA },
    );
    if dmg > 0.0 {
        stat_row(ui, "Base Damage", format!("{dmg:.0}"), style::PAPER);
    }
    if delay_mod.abs() >= 0.005 {
        stat_row(
            ui,
            "Cast Delay",
            signed_sec(delay_mod),
            if delay_mod < 0.0 {
                style::JADE
            } else {
                style::AMBER
            },
        );
    }
    if rech_mod.abs() >= 0.005 {
        stat_row(
            ui,
            "Recharge Time",
            signed_sec(rech_mod),
            if rech_mod < 0.0 {
                style::JADE
            } else {
                style::AMBER
            },
        );
    }
    if spread_mod.abs() >= 0.05 {
        stat_row(
            ui,
            "Spread",
            signed_deg(spread_mod),
            if spread_mod < 0.0 {
                style::JADE
            } else {
                style::AMBER
            },
        );
    }
    if !available {
        ui.label(
            RichText::new("Locked · Claim guardian sanctum reliquary or upgrade wand")
                .size(10.5)
                .color(style::AMBER),
        );
    }
}

/// Interactive Noita-style Wand Deck strip with:
/// - Drag-and-drop slot swapping (`SwapWandSlots`)
/// - Dragging spells from the palette directly onto any slot
/// - Right-click slot to clear (`SetWandSlot(slot, EMPTY)`)
/// - Visual Cast-Block brackets and Trigger-Payload tethers
pub(super) fn draw_wand_deck_strip(
    ui: &mut egui::Ui,
    view: &View,
    action: &mut UiAction,
    selected: &mut usize,
    drag_state: &mut Option<WandDrag>,
    hovered_deck_slot: &mut Option<usize>,
) {
    let cap = wands::capacity(view.wand);
    let eval = wands::evaluate_cast_block(
        view.wand,
        view.rare_spells,
        view.discoveries,
        view.rank,
        &view.loadout,
        view.loadout.cursor,
    );

    let mut slot_rects = Vec::with_capacity(cap);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 6.0);
        for i in 0..cap {
            let spell = view.loadout.slot(i);
            let label = if spell != wands::EMPTY {
                adventure::spell_name(spell)
            } else {
                "Empty slot"
            };
            let (rect, response) =
                ui.allocate_exact_size(vec2(38.0, 38.0), egui::Sense::click_and_drag());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
            });
            slot_rects.push(rect);

            let is_hovered = response.hovered() || response.contains_pointer();
            if is_hovered {
                *hovered_deck_slot = Some(i);
            }

            let p = ui.painter();
            icons::frame(
                p,
                rect,
                i == *selected || i == view.loadout.cursor || is_hovered,
            );
            if spell != wands::EMPTY {
                icons::paint(p, rect.shrink(4.0), Icon::Spell(spell));
                if wands::is_modifier(spell) {
                    p.rect_stroke(
                        rect.shrink(1.5),
                        0,
                        Stroke::new(1.0, style::AMBER),
                        egui::epaint::StrokeKind::Inside,
                    );
                }
            } else {
                p.rect_filled(rect.shrink(3.0), 0, Color32::from_black_alpha(180));
            }

            // Slot index badge
            p.text(
                rect.left_top() + vec2(3.0, 2.0),
                Align2::LEFT_TOP,
                format!("{}", i + 1),
                FontId::monospace(9.5),
                if i == view.loadout.cursor {
                    style::AMBER
                } else {
                    style::MUTED
                },
            );

            if response.clicked() {
                *selected = i;
            }
            if response.secondary_clicked() && spell != wands::EMPTY {
                *action = UiAction::Factory(Action::SetWandSlot(i as u32, wands::EMPTY));
            }
            if response.drag_started() && spell != wands::EMPTY {
                *drag_state = Some(WandDrag::DeckSlot(i, spell));
                *selected = i;
            }
            response.on_hover_ui(|ui| {
                if spell != wands::EMPTY {
                    show_spell_hover_card(ui, spell, view);
                    ui.small("Drag to swap slots · Right-click to unequip");
                    ui.separator();
                }
                show_wand_hover_card(ui, view);
            });
        }
        if ui
            .small_button("Empty")
            .on_hover_text("Clear selected slot (or Right-click any slot)")
            .clicked()
        {
            *action = UiAction::Factory(Action::SetWandSlot(*selected as u32, wands::EMPTY));
        }
    });

    paint_cast_block_brackets(ui.painter(), &slot_rects, &eval);
    ui.add_space(4.0);
}

pub(super) fn draw(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    let stats = wands::stats(view.wand, view.loadout.mode, view.rank);

    ui.horizontal(|ui| {
        let title_resp = ui.add(
            egui::Label::new(
                RichText::new(if view.wand == 0 { "Wand" } else { stats.name })
                    .size(18.0)
                    .color(style::AMBER),
            )
            .sense(egui::Sense::hover()),
        );
        if view.wand > 0 {
            title_resp.on_hover_ui(|ui| show_wand_hover_card(ui, view));
            let badge_resp = ui.add(
                egui::Label::new(
                    RichText::new(format!(
                        " [Hover Stats · Dly {:.2}s · Rch {:.2}s · {:.0}/{:.0} MP] ",
                        stats.cast_delay, stats.recharge_time, view.mana, stats.mana_max
                    ))
                    .font(FontId::monospace(11.0))
                    .color(style::MANA),
                )
                .sense(egui::Sense::hover()),
            );
            badge_resp.on_hover_ui(|ui| show_wand_hover_card(ui, view));
        }
    });

    if view.wand == 0 {
        ui.label("Craft one wand to start experimenting with spells and Noita-style modifiers.");
        return;
    }

    let cap = wands::capacity(view.wand);
    let sel_id = ui.make_persistent_id("wand_edit_slot");
    let drag_id = ui.make_persistent_id("wand_drag_state");
    let mut selected = ui
        .ctx()
        .data_mut(|d| d.get_temp::<usize>(sel_id))
        .unwrap_or(0)
        .min(cap - 1);
    let mut drag_state = ui
        .ctx()
        .data_mut(|d| d.get_temp::<Option<WandDrag>>(drag_id))
        .flatten();
    let mut hovered_deck_slot: Option<usize> = None;

    // Wand Deck Strip (supports drag-swap, right-click clear, and cast-block brackets)
    draw_wand_deck_strip(
        ui,
        view,
        action,
        &mut selected,
        &mut drag_state,
        &mut hovered_deck_slot,
    );

    ui.small(
        "Drag spells into slots or drag between slots to swap · Right-click a slot to clear · Bracketed slots cast together.",
    );

    // Spells & World-Interactive Magic
    ui.label(
        RichText::new("Spells & World Magic")
            .font(FontId::monospace(11.0))
            .color(style::MANA),
    );
    ui.horizontal_wrapped(|ui| {
        for &spell in SPELL_ORDER {
            let available = adventure::spell_available(spell, view.wand, view.rare_spells);
            let response = ui
                .add_enabled_ui(available, |ui| {
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(38.0, 38.0), egui::Sense::click_and_drag());
                    resp.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            ui.is_enabled(),
                            adventure::spell_name(spell),
                        )
                    });
                    icons::frame(
                        ui.painter(),
                        r,
                        view.loadout.slot(selected) == spell || resp.hovered(),
                    );
                    icons::paint(ui.painter(), r.shrink(4.0), Icon::Spell(spell));
                    resp
                })
                .inner;
            if response.drag_started() && available {
                drag_state = Some(WandDrag::PaletteSpell(spell));
            }
            if response
                .on_hover_ui(|ui| show_spell_hover_card(ui, spell, view))
                .clicked()
                && available
            {
                *action = UiAction::Factory(Action::SetWandSlot(selected as u32, spell));
                selected = (selected + 1) % cap;
            }
        }
    });

    // Combinable Noita-Style Modifiers
    ui.label(
        RichText::new("Combinable Modifiers (apply to next spell)")
            .font(FontId::monospace(11.0))
            .color(style::AMBER),
    );
    ui.horizontal_wrapped(|ui| {
        for &spell in MODIFIER_ORDER {
            let available = adventure::spell_available(spell, view.wand, view.rare_spells);
            let response = ui
                .add_enabled_ui(available, |ui| {
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(38.0, 38.0), egui::Sense::click_and_drag());
                    resp.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            ui.is_enabled(),
                            adventure::spell_name(spell),
                        )
                    });
                    icons::frame(
                        ui.painter(),
                        r,
                        view.loadout.slot(selected) == spell || resp.hovered(),
                    );
                    icons::paint(ui.painter(), r.shrink(4.0), Icon::Spell(spell));
                    ui.painter().rect_stroke(
                        r.shrink(1.5),
                        0,
                        Stroke::new(1.0, style::AMBER),
                        egui::epaint::StrokeKind::Inside,
                    );
                    resp
                })
                .inner;
            if response.drag_started() && available {
                drag_state = Some(WandDrag::PaletteSpell(spell));
            }
            if response
                .on_hover_ui(|ui| show_spell_hover_card(ui, spell, view))
                .clicked()
                && available
            {
                *action = UiAction::Factory(Action::SetWandSlot(selected as u32, spell));
                selected = (selected + 1) % cap;
            }
        }
    });

    // Resolve drag release or draw floating spell card on cursor
    if let Some(drag) = drag_state {
        let drag_spell = match drag {
            WandDrag::DeckSlot(_, s) | WandDrag::PaletteSpell(s) => s,
        };
        if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
            let p = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("wand_drag_ghost"),
            ));
            let r = Rect::from_center_size(pos + vec2(8.0, 8.0), vec2(32.0, 32.0));
            icons::frame(&p, r, true);
            icons::paint(&p, r.shrink(3.0), Icon::Spell(drag_spell));
        }
        if ui
            .ctx()
            .input(|i| i.pointer.button_released(egui::PointerButton::Primary))
        {
            match (drag, hovered_deck_slot) {
                (WandDrag::DeckSlot(from, _), Some(to)) if from != to => {
                    *action = UiAction::Factory(Action::SwapWandSlots(from as u32, to as u32));
                    selected = to;
                }
                (WandDrag::DeckSlot(from, _), None) => {
                    *action = UiAction::Factory(Action::SetWandSlot(from as u32, wands::EMPTY));
                }
                (WandDrag::PaletteSpell(s), Some(to)) => {
                    *action = UiAction::Factory(Action::SetWandSlot(to as u32, s));
                    selected = to;
                }
                _ => {}
            }
            drag_state = None;
        }
    }

    ui.ctx().data_mut(|d| {
        d.insert_temp(sel_id, selected);
        d.insert_temp(drag_id, drag_state);
    });

    // Chassis Selector
    ui.horizontal_wrapped(|ui| {
        for (mode, label) in ["Direct", "Twin", "Impact", "Shuffle"]
            .into_iter()
            .enumerate()
        {
            let resp = ui.selectable_label(view.loadout.mode == mode as u32, label);
            if resp.clicked() {
                *action = UiAction::Factory(Action::SetWandMode(mode as u32));
            }
            resp.on_hover_ui(|ui| show_wand_hover_card(ui, view));
        }
    });
    ui.small(match view.loadout.mode {
        1 => "Prism Chassis: Draws 2 spells per cast with slight spread.",
        2 => "Trigger Chassis: First projectile carries the next projectile spell and releases it on impact.",
        3 => "Chaotic Chassis: Shuffles deck order with -28% Cast Delay and -30% Recharge Time.",
        _ => "Conduit Chassis: Evaluates modifier chains left-to-right in strict slot order.",
    });
}

/// Compact Noita-style Wand Tinkering tray embedded directly at the top of the Survival Inventory (`E`)
/// so players can drag-swap wand slots, right-click clear, or open full Spell Tinkering without hunting tabs.
pub(super) fn draw_inline_wand_bar(
    ui: &mut egui::Ui,
    view: &View,
    action: &mut UiAction,
    open_wand_tab: &mut bool,
) {
    if view.wand == 0 {
        return;
    }
    let stats = wands::stats(view.wand, view.loadout.mode, view.rank);
    let sel_id = ui.make_persistent_id("inline_wand_slot");
    let drag_id = ui.make_persistent_id("inline_wand_drag");
    let cap = wands::capacity(view.wand);
    let mut selected = ui
        .ctx()
        .data_mut(|d| d.get_temp::<usize>(sel_id))
        .unwrap_or(view.loadout.cursor)
        .min(cap - 1);
    let mut drag_state = ui
        .ctx()
        .data_mut(|d| d.get_temp::<Option<WandDrag>>(drag_id))
        .flatten();
    let mut hovered_slot: Option<usize> = None;

    egui::Frame::new()
        .fill(Color32::from_rgb(24, 21, 26))
        .stroke(Stroke::new(1.0, style::EDGE))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let lbl = ui.add(
                    egui::Label::new(
                        RichText::new(format!("{} (T{})", stats.name, view.wand))
                            .font(FontId::monospace(11.5))
                            .color(style::AMBER),
                    )
                    .sense(egui::Sense::hover()),
                );
                lbl.on_hover_ui(|ui| show_wand_hover_card(ui, view));

                draw_wand_deck_strip(
                    ui,
                    view,
                    action,
                    &mut selected,
                    &mut drag_state,
                    &mut hovered_slot,
                );

                if ui
                    .small_button("Spells ▸")
                    .on_hover_text("Open full Spell & Modifier Board")
                    .clicked()
                {
                    *open_wand_tab = true;
                }
            });
        });

    if let Some(WandDrag::DeckSlot(from, s)) = drag_state {
        if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
            let p = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("inline_wand_drag_ghost"),
            ));
            let r = Rect::from_center_size(pos + vec2(8.0, 8.0), vec2(28.0, 28.0));
            icons::frame(&p, r, true);
            icons::paint(&p, r.shrink(3.0), Icon::Spell(s));
        }
        if ui
            .ctx()
            .input(|i| i.pointer.button_released(egui::PointerButton::Primary))
        {
            if let Some(to) = hovered_slot {
                if from != to {
                    *action = UiAction::Factory(Action::SwapWandSlots(from as u32, to as u32));
                }
            } else {
                *action = UiAction::Factory(Action::SetWandSlot(from as u32, wands::EMPTY));
            }
            drag_state = None;
        }
    }

    ui.ctx().data_mut(|d| {
        d.insert_temp(sel_id, selected);
        d.insert_temp(drag_id, drag_state);
    });
}
