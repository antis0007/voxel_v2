use super::{
    UiAction,
    icons::{self, Icon},
    presentation as style,
};
use crate::world::{
    adventure::{self, View, wands},
    factory::Action,
};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, vec2};

const BOLTS: &[u32] = &[0, 2, 3, 5, 6, 9, 10, 11, 12, 13, 14, 15, 28, 29];
const EFFECTS: &[u32] = &[1, 4, 7, 24, 25, 30, 31, 32, 34, 35, 39];
const SUMMONS: &[u32] = &[36];
const SHOT_MODIFIERS: &[u32] = &[
    16, 17, 18, 19, 20, 21, 22, 23, 26, 27, 37, 38, 45, 51, 52, 53, 54, 55, 64, 65, 66, 67, 68,
    69,
];
const PROGRAM_MODIFIERS: &[u32] = &[40, 41, 42, 43, 44, 46, 47, 48, 49, 50, 70];
const ELEMENTS: &[u32] = &[56, 57, 58, 59, 60, 61, 62, 63];

/// The workbench column and every hover card use this width. A card that grows
/// to fit the spell under the pointer resizes the panel around it.
const BOARD_W: f32 = 468.0;
const HOVER_W: f32 = 252.0;
const LIBRARY_COLUMNS: usize = 7;

/// Fixed library tabs. Keeping one category visible at a time makes the board
/// short enough to stay put while a spell is dragged from it.
const PALETTE_TABS: [(&str, &[u32], bool); 6] = [
    ("Bolts", BOLTS, false),
    ("Effects", EFFECTS, false),
    ("Summons", SUMMONS, false),
    ("Shot mods", SHOT_MODIFIERS, true),
    ("Elements", ELEMENTS, true),
    ("Program", PROGRAM_MODIFIERS, true),
];

#[derive(Clone, Copy, Debug)]
pub(super) enum WandDrag {
    DeckSlot(usize, u32),
    PaletteSpell(u32),
}

/// One painted row of fixed height. Text is clipped instead of wrapping, so a
/// longer value cannot make the card taller.
fn stat_row(ui: &mut egui::Ui, label: &str, value: String, value_color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(HOVER_W, 16.0), egui::Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);
    painter.text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        label,
        FontId::monospace(11.0),
        style::MUTED,
    );
    painter.text(
        rect.right_center(),
        Align2::RIGHT_CENTER,
        value,
        FontId::monospace(11.5),
        value_color,
    );
}

fn clipped_line(ui: &mut egui::Ui, text: &str, font: FontId, color: Color32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(HOVER_W, height), egui::Sense::hover());
    ui.painter().with_clip_rect(rect).text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        text,
        font,
        color,
    );
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

fn shot_tags(mods: &wands::ShotMods) -> Vec<&'static str> {
    let mut tags = Vec::new();
    if mods.homing && !mods.seek_ally {
        tags.push("Homing");
    }
    if mods.seek_ally {
        tags.push("Seek");
    }
    if mods.orbit {
        tags.push("Ricochet");
    }
    if mods.bore {
        tags.push("Bore");
    }
    if mods.trail {
        tags.push("Wake");
    }
    if mods.volatile {
        tags.push("Volatile");
    }
    if mods.siphon {
        tags.push("Siphon");
    }
    if mods.blood {
        tags.push("Blood");
    }
    if mods.unstable {
        tags.push("Unstable");
    }
    match mods.path {
        1 => tags.push("Curve"),
        2 => tags.push("Wiggle"),
        3 => tags.push("Arc"),
        _ => {}
    }
    if mods.radius > 1.05 {
        tags.push("Wide");
    }
    if mods.life_mul > 1.05 {
        tags.push("Long");
    }
    if mods.speed_mul > 1.05 {
        tags.push("Fast");
    }
    if mods.damage_mul > 1.05 {
        tags.push("Heavy");
    }
    if mods.bounces > 0 {
        tags.push("Pierce");
    }
    if let Some(element) = mods.element {
        tags.push(match element {
            crate::world::adventure::status::Element::Fire => "Fire",
            crate::world::adventure::status::Element::Frost => "Frost",
            crate::world::adventure::status::Element::Shock => "Shock",
            crate::world::adventure::status::Element::Toxic => "Toxin",
            crate::world::adventure::status::Element::Wet => "Wet",
        });
    }
    if mods.chain {
        tags.push("Chain");
    }
    if mods.shrapnel {
        tags.push("Shrapnel");
    }
    tags
}

fn cast_program(eval: &wands::CastBlockEval) -> String {
    if eval.actions.is_empty() {
        return "Empty wand — this cast does nothing.".to_string();
    }
    let mut chain = Vec::new();
    for act in &eval.actions {
        let tags = shot_tags(&act.mods);
        let prefix = if tags.is_empty() {
            String::new()
        } else {
            format!("[{}] ", tags.join("+"))
        };
        let mut step = format!("{}{}", prefix, adventure::spell_name(act.spell));
        if let Some(payload) = act.payload {
            let payload_tags = shot_tags(&act.payload_mods);
            step.push_str(" → ");
            if !payload_tags.is_empty() {
                step.push('[');
                step.push_str(&payload_tags.join("+"));
                step.push_str("] ");
            }
            step.push_str(adventure::spell_name(payload));
        }
        if let Some(follow) = act.follow {
            step.push_str(" → ");
            step.push_str(adventure::spell_name(follow));
        }
        chain.push(step);
    }
    format!(
        "Next cast ({:.0} mana): {}",
        eval.mana_cost,
        chain.join(" + ")
    )
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
/// Every row is a fixed height, including empty ones, so the card does not
/// change size as the pointer moves across the wand.
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

    ui.set_width(HOVER_W);
    ui.set_max_width(HOVER_W);
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

    let (header, _) = ui.allocate_exact_size(vec2(HOVER_W, 20.0), egui::Sense::hover());
    let [r, g, b] = adventure::spell_color(view.spell);
    let gem_col = Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
    ui.painter()
        .circle_filled(header.left_center() + vec2(6.0, 0.0), 4.0, gem_col);
    let header_text = ui.painter().with_clip_rect(header.shrink2(vec2(16.0, 0.0)));
    header_text.text(
        header.left_center() + vec2(16.0, 0.0),
        Align2::LEFT_CENTER,
        stats.name,
        FontId::proportional(14.0),
        style::AMBER,
    );
    header_text.text(
        header.right_center(),
        Align2::RIGHT_CENTER,
        format!("Tier {}", view.wand),
        FontId::monospace(10.5),
        style::MANA,
    );
    clipped_line(
        ui,
        stats.chassis,
        FontId::proportional(11.0),
        style::MUTED,
        16.0,
    );
    let (rule, _) = ui.allocate_exact_size(vec2(HOVER_W, 8.0), egui::Sense::hover());
    ui.painter().hline(
        rule.x_range(),
        rule.center().y,
        Stroke::new(1.0, style::EDGE),
    );

    // Noita 8-Stat Table
    stat_row(
        ui,
        "Shuffle",
        if stats.shuffle { "Yes" } else { "No" }.to_string(),
        if stats.shuffle {
            style::AMBER
        } else {
            style::JADE
        },
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
    stat_row(ui, "Capacity", format!("{}", stats.capacity), style::PAPER);
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

    let always_name = stats
        .always_cast
        .map(adventure::spell_name)
        .unwrap_or("None");
    let (always_rect, _) = ui.allocate_exact_size(vec2(HOVER_W, 18.0), egui::Sense::hover());
    let always_paint = ui.painter().with_clip_rect(always_rect);
    always_paint.text(
        always_rect.left_center(),
        Align2::LEFT_CENTER,
        "Always Casts",
        FontId::monospace(11.0),
        style::AMBER,
    );
    always_paint.text(
        always_rect.right_center() - vec2(18.0, 0.0),
        Align2::RIGHT_CENTER,
        always_name,
        FontId::monospace(11.0),
        style::AMBER,
    );
    if let Some(always) = stats.always_cast {
        let icon = Rect::from_center_size(
            always_rect.right_center() - vec2(8.0, 0.0),
            vec2(16.0, 16.0),
        );
        icons::paint(ui.painter(), icon, Icon::Spell(always));
    }

    let (rule, _) = ui.allocate_exact_size(vec2(HOVER_W, 8.0), egui::Sense::hover());
    ui.painter().hline(
        rule.x_range(),
        rule.center().y,
        Stroke::new(1.0, style::EDGE),
    );

    // One reserved row. Slots are painted inside it and never wrap onto a
    // second line, which used to grow the card under the pointer.
    let (deck, _) = ui.allocate_exact_size(vec2(HOVER_W, 28.0), egui::Sense::hover());
    let cap = wands::capacity(view.wand).min(8);
    let mut slot_rects = Vec::with_capacity(cap);
    for idx in 0..cap {
        let spell = view.loadout.slot(idx);
        let slot = Rect::from_min_size(
            deck.left_top() + vec2(idx as f32 * 27.0, 2.0),
            vec2(24.0, 24.0),
        );
        slot_rects.push(slot);
        icons::frame(ui.painter(), slot, idx == view.loadout.cursor);
        if spell != wands::EMPTY {
            icons::paint(ui.painter(), slot.shrink(2.5), Icon::Spell(spell));
        } else {
            ui.painter()
                .rect_filled(slot.shrink(3.0), 0, Color32::from_black_alpha(175));
        }
        ui.painter().text(
            slot.left_top() + vec2(2.0, 1.0),
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
    paint_cast_block_brackets(ui.painter(), &slot_rects, &eval);

    let program = cast_program(&eval);
    let program_color = if eval.actions.is_empty() {
        style::MUTED
    } else if eval.mana_cost > view.mana {
        style::DANGER
    } else {
        style::MANA
    };
    clipped_line(ui, &program, FontId::monospace(10.5), program_color, 16.0);
}

/// Renders a Noita-style Spell / Modifier Hover Card with type badge, mana drain,
/// damage, cast delay delta, recharge delta, spread delta, and world physics note.
/// The card is the same size for every spell. Missing stats stay as a dash so
/// moving across the board does not resize the popup or the panel under it.
pub(super) fn show_spell_hover_card(ui: &mut egui::Ui, spell: u32, view: &View) {
    let available = view.can_wield(spell);
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

    ui.set_width(HOVER_W);
    ui.set_max_width(HOVER_W);
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

    let (header, _) = ui.allocate_exact_size(vec2(HOVER_W, 36.0), egui::Sense::hover());
    let icon = Rect::from_min_size(header.left_top() + vec2(0.0, 4.0), vec2(28.0, 28.0));
    icons::frame(ui.painter(), icon, true);
    icons::paint(ui.painter(), icon.shrink(2.0), Icon::Spell(spell));
    let text_clip = Rect::from_min_max(icon.right_top() + vec2(6.0, 0.0), header.right_bottom());
    let text = ui.painter().with_clip_rect(text_clip);
    text.text(
        text_clip.left_top() + vec2(0.0, 4.0),
        Align2::LEFT_TOP,
        adventure::spell_name(spell),
        FontId::proportional(13.5),
        style::PAPER,
    );
    text.text(
        text_clip.left_bottom() + vec2(0.0, -2.0),
        Align2::LEFT_BOTTOM,
        adventure::spell_kind_tag(spell),
        FontId::monospace(10.0),
        if is_mod { style::AMBER } else { style::MANA },
    );

    clipped_line(
        ui,
        adventure::spell_hint(spell),
        FontId::proportional(11.0),
        style::MUTED,
        16.0,
    );
    clipped_line(
        ui,
        adventure::spell_flavor(spell),
        FontId::proportional(11.0),
        style::PAPER.gamma_multiply(0.72),
        16.0,
    );
    let (rule, _) = ui.allocate_exact_size(vec2(HOVER_W, 8.0), egui::Sense::hover());
    ui.painter().hline(
        rule.x_range(),
        rule.center().y,
        Stroke::new(1.0, style::EDGE),
    );

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
    stat_row(
        ui,
        "Base Damage",
        if dmg > 0.0 {
            format!("{dmg:.0}")
        } else {
            "—".to_string()
        },
        style::PAPER,
    );
    stat_row(
        ui,
        "Cast Delay",
        signed_sec(delay_mod),
        if delay_mod < -0.005 {
            style::JADE
        } else if delay_mod > 0.005 {
            style::AMBER
        } else {
            style::MUTED
        },
    );
    stat_row(
        ui,
        "Recharge Time",
        signed_sec(rech_mod),
        if rech_mod < -0.005 {
            style::JADE
        } else if rech_mod > 0.005 {
            style::AMBER
        } else {
            style::MUTED
        },
    );
    stat_row(
        ui,
        "Spread",
        signed_deg(spread_mod),
        if spread_mod < -0.05 {
            style::JADE
        } else if spread_mod > 0.05 {
            style::AMBER
        } else {
            style::MUTED
        },
    );
    clipped_line(
        ui,
        if available {
            "Ready"
        } else if adventure::spell_available(spell, view.wand, view.rare_spells) {
            "Undiscovered · defeat guardians and clear sites"
        } else {
            "Locked · Claim a sanctum reliquary or upgrade the wand"
        },
        FontId::proportional(10.5),
        if available { style::JADE } else { style::AMBER },
        16.0,
    );
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
    compact: bool,
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
    // A single row. `horizontal_wrapped` grew the inventory and the workbench
    // whenever the pointer, a drag, or a longer cast line changed the width.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
        for i in 0..cap {
            let spell = view.loadout.slot(i);
            let label = if spell != wands::EMPTY {
                adventure::spell_name(spell)
            } else {
                "Empty slot"
            };
            let (rect, response) = ui.allocate_exact_size(
                {
                    let dist = (i as i32 - *selected as i32).abs();
                    let (px, _) = super::wand_xmb::slot_metrics(dist);
                    vec2(px, px)
                },
                egui::Sense::click_and_drag(),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
            });
            slot_rects.push(rect);

            let is_hovered = response.hovered() || response.contains_pointer();
            if is_hovered {
                *hovered_deck_slot = Some(i);
            }
            deck_drag_hint(ui, rect, drag_state.is_some(), is_hovered);

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
            let (_, opacity) = super::wand_xmb::slot_metrics((i as i32 - *selected as i32).abs());
            if opacity < 0.99 && i != *selected {
                p.rect_filled(
                    rect,
                    0,
                    Color32::from_black_alpha(((1.0 - opacity) * 170.0) as u8),
                );
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
                } else {
                    show_empty_slot_card(ui);
                }
            });
        }
    });

    paint_cast_block_brackets(ui.painter(), &slot_rects, &eval);
    if !compact {
        ui.add_space(4.0);
        let program = cast_program(&eval);
        let color = if eval.actions.is_empty() {
            style::MUTED
        } else if eval.mana_cost > view.mana {
            style::DANGER
        } else {
            style::MANA
        };
        let width = ui.available_width().min(BOARD_W);
        let (rect, _) = ui.allocate_exact_size(vec2(width, 28.0), egui::Sense::hover());
        let galley = ui
            .painter()
            .layout(program, FontId::monospace(11.0), color, rect.width());
        ui.painter()
            .with_clip_rect(rect)
            .galley(rect.left_top(), galley, color);
    }
}

/// Same outer size as a spell card, so an empty slot does not collapse the popup.
fn show_empty_slot_card(ui: &mut egui::Ui) {
    ui.set_width(HOVER_W);
    ui.set_max_width(HOVER_W);
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let (header, _) = ui.allocate_exact_size(vec2(HOVER_W, 36.0), egui::Sense::hover());
    ui.painter().text(
        header.left_center(),
        Align2::LEFT_CENTER,
        "Empty slot",
        FontId::proportional(13.5),
        style::PAPER,
    );
    clipped_line(
        ui,
        "Drag a spell here, or click one in the library.",
        FontId::proportional(11.0),
        style::MUTED,
        16.0,
    );
    clipped_line(ui, " ", FontId::proportional(11.0), style::MUTED, 16.0);
    let (rule, _) = ui.allocate_exact_size(vec2(HOVER_W, 8.0), egui::Sense::hover());
    ui.painter().hline(
        rule.x_range(),
        rule.center().y,
        Stroke::new(1.0, style::EDGE),
    );
    for label in [
        "Mana Drain",
        "Base Damage",
        "Cast Delay",
        "Recharge Time",
        "Spread",
    ] {
        stat_row(ui, label, "—".to_string(), style::MUTED);
    }
    clipped_line(
        ui,
        "Right-click a filled slot to clear it",
        FontId::proportional(10.5),
        style::MUTED,
        16.0,
    );
}

/// A fixed-geometry spell library. The old board stacked every category and
/// reflowed its wrapped rows as the pointer changed; this keeps one category in
/// a reserved-height grid, with tabs and a search field that never resize.
fn paint_spell_library(
    ui: &mut egui::Ui,
    view: &View,
    action: &mut UiAction,
    selected: &mut usize,
    drag_state: &mut Option<WandDrag>,
    tab: &mut usize,
    query: &mut String,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
        for (index, (title, spells, _)) in PALETTE_TABS.iter().enumerate() {
            let label = format!("{title} {}", spells.len());
            if ui
                .selectable_label(*tab == index, RichText::new(label).size(10.5))
                .clicked()
            {
                *tab = index;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(query)
                    .hint_text("Search…")
                    .desired_width(120.0),
            );
        });
    });

    let (title, active, modifier_stroke) = PALETTE_TABS[*tab];
    let query = query.trim().to_lowercase();
    let mut spells: Vec<u32> = if query.is_empty() {
        active.to_vec()
    } else {
        PALETTE_TABS
            .iter()
            .flat_map(|(_, list, _)| list.iter().copied())
            .filter(|spell| {
                adventure::spell_name(*spell)
                    .to_lowercase()
                    .contains(&query)
            })
            .collect()
    };
    spells.sort_unstable();
    spells.dedup();

    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(18, 18, 24, 210))
        .stroke(Stroke::new(1.0, style::EDGE))
        .inner_margin(egui::Margin::same(7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(title.to_uppercase())
                        .font(FontId::monospace(10.0))
                        .color(if modifier_stroke {
                            style::AMBER
                        } else {
                            style::MANA
                        }),
                );
                ui.label(
                    RichText::new(format!(
                        "{}/{} found",
                        found_count(&spells, &view.spell_log),
                        spells.len()
                    ))
                        .font(FontId::monospace(9.5))
                        .color(style::MUTED),
                );
                if modifier_stroke {
                    ui.label(
                        RichText::new("place before a spell")
                            .font(FontId::monospace(9.5))
                            .color(style::AMBER),
                    );
                }
            });
            spell_library_grid(
                ui,
                view,
                action,
                selected,
                drag_state,
                *tab,
                &spells,
                modifier_stroke,
            );
        });
}

pub(super) fn draw(ui: &mut egui::Ui, view: &View, action: &mut UiAction) {
    ui.scope(|ui| {
    ui.set_width(BOARD_W);
    ui.set_max_width(BOARD_W);
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
    let tab_id = ui.make_persistent_id("wand_library_tab");
    let query_id = ui.make_persistent_id("wand_library_query");
    let mut library_tab = ui
        .ctx()
        .data_mut(|d| d.get_temp::<usize>(tab_id).unwrap_or(0))
        .min(PALETTE_TABS.len() - 1);
    let mut library_query = ui
        .ctx()
        .data_mut(|d| d.get_temp::<String>(query_id).unwrap_or_default());
    let mut hovered_deck_slot: Option<usize> = None;

    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(20, 19, 24, 225))
        .stroke(Stroke::new(1.0, style::EDGE))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            // Wand Deck Strip (supports drag-swap, right-click clear, and cast-block brackets)
            draw_wand_deck_strip(
                ui,
                view,
                action,
                &mut selected,
                &mut drag_state,
                &mut hovered_deck_slot,
                false,
            );
            let slot_spell = view.loadout.slot(selected);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Slot {}", selected + 1))
                        .font(FontId::monospace(10.0))
                        .color(style::AMBER),
                );
                ui.label(
                    RichText::new(if slot_spell == wands::EMPTY {
                        "empty — choose a spell below".to_owned()
                    } else {
                        adventure::spell_name(slot_spell).to_owned()
                    })
                    .font(FontId::monospace(10.0))
                    .color(style::MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("Clear slot")
                        .on_hover_text("Clear selected slot (or right-click any slot)")
                        .clicked()
                    {
                        *action = UiAction::Factory(Action::SetWandSlot(
                            selected as u32,
                            wands::EMPTY,
                        ));
                    }
                });
            });
        });

    ui.small(
        "Modifiers in front of a spell apply to it. The amber index is where the next cast starts. Click writes the selected slot; shift-click fills the first empty slot. Right-click or drag off the deck to clear. An empty wand casts nothing.",
    );

    paint_spell_library(
        ui,
        view,
        action,
        &mut selected,
        &mut drag_state,
        &mut library_tab,
        &mut library_query,
    );

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
        d.insert_temp(tab_id, library_tab);
        d.insert_temp(query_id, library_query);
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
    let blurb = match view.loadout.mode {
        1 => "Prism Chassis: Draws 2 spells per cast with slight spread.",
        2 => "Trigger Chassis: First projectile carries the next projectile spell and releases it on impact.",
        3 => "Chaotic Chassis: Shuffles deck order with -28% Cast Delay and -30% Recharge Time.",
        _ => "Conduit Chassis: Evaluates modifier chains left-to-right in strict slot order.",
    };
    let width = ui.available_width().min(BOARD_W);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 28.0), egui::Sense::hover());
    let galley = ui
        .painter()
        .layout(blurb.to_owned(), FontId::proportional(10.0), style::MUTED, rect.width());
    ui.painter()
        .with_clip_rect(rect)
        .galley(rect.left_top(), galley, style::MUTED);
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
                    true,
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

/// Fixed-size spell cards in a reserved-height scroll area. Cards never change
/// size on hover or drag; only their outline and dim overlay change.
fn spell_library_grid(
    ui: &mut egui::Ui,
    view: &View,
    action: &mut UiAction,
    selected: &mut usize,
    drag_state: &mut Option<WandDrag>,
    tab: usize,
    spells: &[u32],
    modifier_stroke: bool,
) {
    let cap = wands::capacity(view.wand);
    let body_height = 150.0;
    ui.set_min_height(body_height);
    egui::ScrollArea::vertical()
        .id_salt(("spell_library", tab))
        .max_height(body_height)
        .min_scrolled_height(body_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let columns = LIBRARY_COLUMNS;
            egui::Grid::new(("spell_grid", tab))
                .spacing([3.0, 4.0])
                .show(ui, |ui| {
                    for (index, spell) in spells.iter().copied().enumerate() {
                        let available = view.can_wield(spell);
                        let (r, response) =
                            ui.allocate_exact_size(vec2(54.0, 58.0), Sense::click_and_drag());
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                available,
                                adventure::spell_name(spell),
                            )
                        });
                        let p = ui.painter();
                        icons::frame(p, r, response.hovered());
                        let icon = Rect::from_center_size(
                            egui::pos2(r.center().x, r.top() + 24.0),
                            vec2(34.0, 34.0),
                        );
                        icons::paint(p, icon, Icon::Spell(spell));
                        if modifier_stroke {
                            p.rect_stroke(
                                r.shrink(1.5),
                                0,
                                Stroke::new(1.0, style::AMBER),
                                egui::epaint::StrokeKind::Inside,
                            );
                            p.line_segment(
                                [
                                    r.left_top() + vec2(2.0, 5.0),
                                    r.left_bottom() + vec2(2.0, -5.0),
                                ],
                                Stroke::new(2.0, style::AMBER),
                            );
                        }
                        let name = adventure::spell_name(spell);
                        let short: String = if name.chars().count() > 12 {
                            name.chars().take(11).chain("…".chars()).collect()
                        } else {
                            name.to_owned()
                        };
                        p.text(
                            egui::pos2(r.center().x, r.bottom() - 3.0),
                            Align2::CENTER_BOTTOM,
                            short,
                            FontId::proportional(9.0),
                            if available {
                                style::PAPER
                            } else {
                                style::MUTED
                            },
                        );
                        if !available {
                            p.rect_filled(r, 0, Color32::from_rgba_unmultiplied(12, 10, 14, 155));
                        }
                        if response.hovered() {
                            p.rect_stroke(
                                r,
                                0,
                                Stroke::new(1.0, style::AMBER),
                                egui::epaint::StrokeKind::Inside,
                            );
                        }
                        if response.drag_started() && available {
                            *drag_state = Some(WandDrag::PaletteSpell(spell));
                        }
                        let hovered =
                            response.on_hover_ui(|ui| show_spell_hover_card(ui, spell, view));
                        if (hovered.clicked() || hovered.secondary_clicked()) && available {
                            let first_empty =
                                (0..cap).find(|&idx| view.loadout.slot(idx) == wands::EMPTY);
                            let target_slot = if hovered.secondary_clicked()
                                || ui.ctx().input(|i| {
                                    i.modifiers.shift || i.modifiers.ctrl || i.modifiers.command
                                }) {
                                first_empty.unwrap_or(*selected)
                            } else {
                                *selected
                            };
                            *action =
                                UiAction::Factory(Action::SetWandSlot(target_slot as u32, spell));
                            *selected = (target_slot + 1) % cap;
                        }
                        if (index + 1) % columns == 0 {
                            ui.end_row();
                        }
                    }
                });
        });
}

/// Highlight the whole deck while a drag is active so the stable drop targets
/// are obvious before the pointer reaches one of them.
fn deck_drag_hint(ui: &egui::Ui, rect: Rect, dragging: bool, hovered: bool) {
    if !dragging {
        return;
    }
    let color = if hovered { style::AMBER } else { style::JADE };
    ui.painter().rect_stroke(
        rect,
        0,
        Stroke::new(if hovered { 2.0 } else { 1.0 }, color),
        egui::epaint::StrokeKind::Inside,
    );
}

/// How many of these spells the player has found. Spells held back by rare
/// magic (6 and 7) count as found, matching what the log lets through.
fn found_count(spells: &[u32], log: &crate::world::adventure::discovery::SpellLog) -> usize {
    spells.iter().filter(|&&id| log.grants(id)).count()
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    /// A spell that exists but is on no tab can never be placed on a wand, which
    /// is exactly how a new spell silently ships unusable.
    #[test]
    fn every_defined_spell_is_reachable_from_the_palette() {
        let listed: std::collections::HashSet<u32> = PALETTE_TABS
            .iter()
            .flat_map(|(_, list, _)| list.iter().copied())
            .collect();
        for id in 0..=adventure::MAX_SPELL_ID {
            if id == 8 || adventure::spell_name(id) == "Unknown spell" {
                continue;
            }
            assert!(listed.contains(&id), "{} ({id}) is on no palette tab", adventure::spell_name(id));
        }
        for id in &listed {
            assert_ne!(adventure::spell_name(*id), "Unknown spell", "tab lists undefined spell {id}");
        }
    }

    #[test]
    fn found_count_reflects_the_log() {
        use crate::world::adventure::discovery::SpellLog;
        let start = SpellLog::starter();
        assert_eq!(found_count(BOLTS, &SpellLog::all()), BOLTS.len());
        // Starter: Spark and Frost lance are bolts; Spore bloom is not found,
        // while rift well (6) is never blocked by the log.
        let bolts = found_count(BOLTS, &start);
        assert_eq!(bolts, 3, "spark, frost lance and rift well");
        assert_eq!(found_count(&[3, 5, 29], &start), 0);
    }
}
