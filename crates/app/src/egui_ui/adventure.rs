use super::{
    icons::{self, Icon},
    presentation as style,
};
use crate::world::adventure::{QuestMarker, View, spell_name, wands};
use egui::{Align2, Color32, FontId, Rect, Stroke, vec2};

#[derive(Clone)]
struct HealthFeedback {
    previous: f32,
    trail: f32,
    hold: f32,
    maximum: f32,
}

#[derive(Clone)]
struct HungerFeedback {
    previous: f32,
    trail: f32,
    hold: f32,
}

// Ten health points per pixel heart, including partial fills, damage trail, and AppleSkin food-heal preview.
fn draw_hearts(
    ui: &mut egui::Ui,
    width: f32,
    value: f32,
    maximum: f32,
    trail: f32,
    preview_heal: f32,
    regen_active: bool,
    poison: f32,
    time: f32,
    align_left: bool,
) {
    let count = (maximum / 10.0).ceil() as usize;
    let icon_step = (width / 10.0).floor().clamp(8.0, 18.0);
    let columns = (width / icon_step).floor().clamp(5.0, 10.0) as usize;
    let rows = count.div_ceil(columns);
    let row_h = (icon_step + 2.0).max(10.0);
    let (rect, resp) =
        ui.allocate_exact_size(vec2(width, rows as f32 * row_h), egui::Sense::hover());
    let unit = (icon_step / 8.5).floor().max(1.0);
    let shape = [
        " ## ## ", "#######", "#######", " ##### ", "  ###  ", "   #   ",
    ];
    let mut mesh = egui::Mesh::default();
    let group_width = columns.min(count) as f32 * icon_step;
    let start_x = if align_left {
        rect.left()
    } else {
        rect.center().x - group_width * 0.5
    };
    let origin = egui::pos2(
        start_x.round(),
        (rect.top() + 2.0).round(),
    );
    let low_hp = value <= 25.0;
    let preview_top = (value + preview_heal).min(maximum);
    let pulse = ((time * 3.8).sin() * 0.5 + 0.5).clamp(0.20, 0.78);
    for heart in 0..count {
        let bounce = if low_hp {
            ((time * 11.0 + heart as f32 * 1.3).sin() * 1.2).round()
        } else if regen_active {
            let wave = ((time * 6.0 - heart as f32 * 0.55).sin()).max(0.0);
            -(wave * 1.4).round()
        } else {
            0.0
        };
        let offset = origin
            + vec2(
                (heart % columns) as f32 * icon_step,
                (heart / columns) as f32 * row_h + bounce,
            );
        let fill = ((value - heart as f32 * 10.0) / 10.0).clamp(0.0, 1.0);
        let old = ((trail - heart as f32 * 10.0) / 10.0).clamp(0.0, 1.0);
        let prev_fill = ((preview_top - heart as f32 * 10.0) / 10.0).clamp(0.0, 1.0);
        for (y, row) in shape.iter().enumerate() {
            for (x, ch) in row.bytes().enumerate() {
                if ch != b'#' {
                    continue;
                }
                let border = y == 0
                    || y == 5
                    || x == 0
                    || x == 6
                    || row.as_bytes().get(x.wrapping_sub(1)).copied() != Some(b'#')
                    || row.as_bytes().get(x + 1).copied() != Some(b'#');
                let frac = (x as f32 + 0.5) / 7.0;
                let active = frac <= fill;
                let color = if active {
                    let red = if y < 2 && x < 3 {
                        Color32::from_rgb(255, 148, 134)
                    } else if y >= 4 {
                        Color32::from_rgb(158, 42, 52)
                    } else {
                        Color32::from_rgb(228, 66, 74)
                    };
                    if poison > 0.0 {
                        // Mana poisoning: a violet glow travelling along the row.
                        let glow = (time * 2.2 - heart as f32 * 0.5).sin() * 0.5 + 0.5;
                        let violet = if y < 2 && x < 3 {
                            Color32::from_rgb(214, 190, 255)
                        } else if y >= 4 {
                            Color32::from_rgb(84, 46, 168)
                        } else {
                            Color32::from_rgb(150, 96, 248)
                        };
                        red.lerp_to_gamma(violet, poison * (0.65 + glow * 0.35))
                    } else {
                        red
                    }
                } else if poison > 0.0 && border {
                    Color32::from_rgb(86, 52, 58).lerp_to_gamma(Color32::from_rgb(120, 92, 220), poison * 0.6)
                } else if preview_heal > 0.0 && frac <= prev_fill {
                    Color32::from_rgba_unmultiplied(245, 142, 148, (pulse * 195.0) as u8)
                } else if frac <= old {
                    style::AMBER
                } else if border {
                    Color32::from_rgb(86, 52, 58)
                } else {
                    Color32::from_rgb(44, 26, 32)
                };
                let pixel =
                    Rect::from_min_size(offset + vec2(x as f32, y as f32) * unit, vec2(unit, unit));
                mesh.add_colored_rect(pixel.translate(vec2(unit, unit)), style::INK);
                mesh.add_colored_rect(pixel, color);
            }
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    if preview_heal > 0.0 && value < maximum {
        resp.on_hover_text(format!(
            "Health: {:.0} / {:.0} (+{:.0} from held food)",
            value.ceil(),
            maximum,
            preview_heal
        ));
    } else {
        resp.on_hover_text(format!("Health: {:.0} / {:.0}", value.ceil(), maximum));
    }
}

/// Ten hunger points per haunch icon, aligned on the right side opposite health.
/// Includes AppleSkin-style flashing restoration preview when holding food and 75% well-fed threshold tick.
fn draw_hunger(
    ui: &mut egui::Ui,
    width: f32,
    value: f32,
    trail: f32,
    preview_hunger: f32,
    eat_timer: f32,
    time: f32,
    align_right: bool,
) {
    const HAUNCH: [&str; 8] = [
        "  ####  ", " ###### ", "####### ", " ###### ", "  ####  ", "   ###  ", "    ### ",
        "     ## ",
    ];
    let icon_step = (width / 10.0).floor().clamp(8.0, 18.0);
    let row_h = (icon_step + 2.0).max(10.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, row_h), egui::Sense::hover());
    let unit = (icon_step / 8.8).floor().max(1.0);
    let group_width = 10.0 * icon_step;
    let start_x = if align_right {
        rect.right() - group_width
    } else {
        rect.center().x - group_width * 0.5
    };
    let origin = egui::pos2(
        start_x.round(),
        (rect.top() + 2.0).round(),
    );
    let starving = value <= 0.05;
    let low_hunger = value <= 25.0;
    let eating = eat_timer > 0.0;
    let preview_top = (value + preview_hunger).min(100.0);
    let pulse = ((time * 5.5).sin() * 0.5 + 0.5).clamp(0.25, 0.95);
    let mut mesh = egui::Mesh::default();

    for idx in 0..10 {
        let slot_index_from_right = 9 - idx;
        let threshold = slot_index_from_right as f32 * 10.0;
        let fill = ((value - threshold) / 10.0).clamp(0.0, 1.0);
        let old = ((trail - threshold) / 10.0).clamp(0.0, 1.0);
        let prev_fill = ((preview_top - threshold) / 10.0).clamp(0.0, 1.0);

        // Keep the HUD calm at normal hunger; starvation/eating use a tiny,
        // staggered lift instead of the old full-row shake.
        let bounce = if starving {
            ((time * 8.0 + idx as f32 * 1.1).sin() * 0.7).round()
        } else if eating {
            let wave = ((time * 10.0 - slot_index_from_right as f32 * 0.65).sin()).max(0.0);
            -(wave * 1.0).round()
        } else {
            0.0
        };

        let offset = origin + vec2(idx as f32 * icon_step, bounce);
        for (y, row) in HAUNCH.iter().enumerate() {
            for (x, ch) in row.bytes().enumerate() {
                if ch != b'#' {
                    continue;
                }
                let is_bone = y >= 5 && x >= 4;
                let border = y == 0
                    || y == 7
                    || x == 0
                    || x == 7
                    || row.as_bytes().get(x.wrapping_sub(1)).copied() != Some(b'#')
                    || row.as_bytes().get(x + 1).copied() != Some(b'#');
                // Fill from right to left within each haunch so half-icons split cleanly.
                let frac = (7.5 - x as f32) / 8.0;
                let active = frac <= fill;
                let trail_active = frac <= old;
                let preview_active = preview_hunger > 0.0 && frac <= prev_fill;

                let color = if active {
                    if is_bone {
                        style::PAPER
                    } else if eating && y < 2 {
                        Color32::from_rgb(252, 218, 134)
                    } else if y < 2 {
                        Color32::from_rgb(238, 182, 108)
                    } else if y >= 4 {
                        Color32::from_rgb(152, 78, 44)
                    } else {
                        Color32::from_rgb(198, 114, 62)
                    }
                } else if preview_active {
                    Color32::from_rgba_unmultiplied(244, 206, 122, (pulse * 195.0) as u8)
                } else if trail_active {
                    style::AMBER
                } else if starving && border {
                    Color32::from_rgb(176, 62, 58)
                } else if low_hunger && border {
                    Color32::from_rgb(148, 106, 56)
                } else if border {
                    Color32::from_rgb(82, 62, 52)
                } else {
                    Color32::from_rgb(42, 31, 28)
                };

                let pixel =
                    Rect::from_min_size(offset + vec2(x as f32, y as f32) * unit, vec2(unit, unit));
                mesh.add_colored_rect(pixel.translate(vec2(unit, unit)), style::INK);
                mesh.add_colored_rect(pixel, color);
            }
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));

    let state_label = if starving {
        "STARVING (-HP)"
    } else if low_hunger {
        "Low hunger (eat rations)"
    } else if value >= 75.0 {
        "Well-fed (regenerating health)"
    } else {
        "Satisfied"
    };
    if preview_hunger > 0.0 && value < 99.5 {
        resp.on_hover_text(format!(
            "Hunger: {:.0}% (+{:.0}% from held food) · {state_label}",
            value.ceil(),
            preview_hunger
        ));
    } else {
        resp.on_hover_text(format!("Hunger: {:.0}% · {state_label}", value.ceil()));
    }
}

fn draw_armor_row(ui: &mut egui::Ui, width: f32, armor_points: u32) {
    if armor_points == 0 {
        return;
    }
    const SHIELD: [&str; 6] = ["######", "######", "######", " #### ", "  ##  ", "  ##  "];
    let icon_step = (width / 10.0).floor().clamp(8.0, 18.0);
    let unit = (icon_step / 8.5).floor().max(1.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 9.0 * unit), egui::Sense::hover());
    let origin = egui::pos2(
        rect.left().round(),
        (rect.top() + 1.0).round(),
    );
    let mut mesh = egui::Mesh::default();
    let icons = (armor_points.div_ceil(2) as usize).min(10);
    for i in 0..icons {
        let full = armor_points >= (i as u32 + 1) * 2;
        let offset = origin + vec2(i as f32 * icon_step, 0.0);
        for (y, row) in SHIELD.iter().enumerate() {
            for (x, ch) in row.bytes().enumerate() {
                if ch != b'#' {
                    continue;
                }
                let active = full || x < 3;
                let color = if active {
                    if y == 0 {
                        Color32::from_rgb(214, 226, 236)
                    } else {
                        Color32::from_rgb(142, 164, 184)
                    }
                } else {
                    Color32::from_rgb(56, 62, 70)
                };
                let pixel =
                    Rect::from_min_size(offset + vec2(x as f32, y as f32) * unit, vec2(unit, unit));
                mesh.add_colored_rect(pixel.translate(vec2(unit, unit)), style::INK);
                mesh.add_colored_rect(pixel, color);
            }
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    resp.on_hover_text(format!("Armor protection: {} / 20", armor_points));
}

const POISON: Color32 = Color32::from_rgb(170, 120, 255);
const ABSORB: Color32 = Color32::from_rgb(235, 199, 70);

/// Plate behind the vitals, the health numeral and poisoning sparks.
fn vitals_plate(ui: &egui::Ui, backdrop: egui::layers::ShapeIdx, view: &View, maximum: f32, half_w: f32, time: f32) {
    let panel = ui.min_rect().expand2(vec2(6.0, 4.0));
    let low = view.health <= maximum * 0.25;
    let accent = if view.mana_poison > 0.0 {
        Color32::from_rgb(120, 84, 220)
    } else if low {
        style::DANGER
    } else {
        style::MANA
    };
    let mut shapes = Vec::with_capacity(2);
    if view.mana_poison > 0.0 {
        // Steady glow that follows the poison level and fades with it; a fast
        // pulse here competed with the hearts' own wave.
        let breath = 0.85 + (time * 1.5).sin() * 0.15;
        shapes.push(super::hud_style::glow(ui.ctx(), panel, POISON, 2, view.mana_poison * 0.55 * breath));
    } else if low {
        let pulse = 0.6 + (time * 4.0).sin() * 0.4;
        shapes.push(super::hud_style::glow(ui.ctx(), panel, style::DANGER, 3, pulse));
    }
    shapes.push(super::hud_style::plate(ui.ctx(), panel, accent));
    ui.painter().set(backdrop, egui::Shape::Vec(shapes));
    let numeral = if view.mana_poison > 0.0 { POISON } else if low { style::DANGER } else { style::PAPER };
    // The hearts already carry health; a bare number beside them read as an
    // unlabelled stat. It shows on request, or when the exact value matters.
    if low || view.mana_poison > 0.0 || ui.input(|i| i.modifiers.alt) {
        ui.painter().text(
            egui::pos2(panel.left() - 6.0, panel.bottom()),
            Align2::RIGHT_BOTTOM,
            format!("{:.0}", view.health.max(0.0).ceil()),
            FontId::monospace(22.0),
            numeral,
        );
    }
    if view.absorption > 0.0 {
        // Absorption reads as a gold shield segment under the hearts, sized
        // against full health, with its amount above the health numeral.
        let width = (half_w.max(60.0) * (view.absorption / maximum.max(1.0)).min(1.0)).round();
        let bar = Rect::from_min_size(egui::pos2(panel.left() + 6.0, panel.bottom() - 4.0), vec2(width, 2.0));
        ui.painter().rect_filled(bar.translate(vec2(1.0, 1.0)), 0, style::INK);
        ui.painter().rect_filled(bar, 0, ABSORB);
        ui.painter().text(
            egui::pos2(panel.left() - 6.0, panel.bottom() - 22.0),
            Align2::RIGHT_BOTTOM,
            format!("+{:.0}", view.absorption.ceil()),
            FontId::monospace(12.0),
            ABSORB,
        );
    }
    if view.mana_poison > 0.0 {
        // Sparks lift just off the heart row; taller emitters crossed the
        // affinity text above it.
        let hearts = Rect::from_min_max(
            egui::pos2(panel.left() + 6.0, panel.bottom() - 24.0),
            egui::pos2(panel.left() + 6.0 + half_w.max(60.0), panel.bottom() - 8.0),
        );
        super::hud_style::sparks(ui.painter(), hearts, POISON, time, (3.0 * view.mana_poison).round() as u32 + 1, 7);
    }
}

/// Mana at a glance: a thin bar across the vitals width with the recovery rate
/// at its end, like the experience bar it sits beside. The bar was only inside
/// the wand instrument (cooldown or Alt), so a player holding a sword saw a line
/// of text instead of a gauge, and that text wrapped at 1440p. The state word
/// appears only off neutral ground. Recovery and density are separate
/// quantities: lush ground is safe at 6x recovery.
fn mana_strip(ui: &mut egui::Ui, view: &View, max_mana: f32, width: f32) {
    use crate::world::adventure::affinity::{BLIGHT_BELOW, POISON_AT};
    let m = view.affinity;
    let (state, color) = if m >= POISON_AT {
        (Some("Poisoning"), POISON)
    } else if m < BLIGHT_BELOW {
        (Some("Blight"), Color32::from_rgb(150, 138, 120))
    } else if m >= 1.25 {
        (Some("Affinity"), style::MANA)
    } else {
        (None, style::MUTED)
    };
    let recovery = crate::world::ambience::mana_recovery(m);
    let health = crate::world::ambience::health_recovery(m);
    let label = match state {
        Some(word) => format!("{recovery:.1}× · {word}"),
        None => format!("{recovery:.1}×"),
    };
    let galley = ui.painter().layout_no_wrap(label, FontId::monospace(11.0), color);
    let (rect, response) = ui.allocate_exact_size(vec2(width, galley.size().y.max(9.0)), egui::Sense::hover());
    let bar_w = (width - galley.size().x - 6.0).max(24.0).floor();
    let bar = super::grimoire::snap(Rect::from_min_size(
        egui::pos2(rect.left(), (rect.center().y - 2.5).round()),
        vec2(bar_w, 5.0),
    ));
    let p = ui.painter();
    p.rect_filled(bar, 0, Color32::from_rgb(14, 12, 16));
    let mut fill = bar.shrink(1.0);
    fill.set_width((fill.width() * (view.mana / max_mana.max(1.0)).clamp(0.0, 1.0)).floor());
    p.rect_filled(fill, 0, if m >= POISON_AT { POISON } else { style::MANA });
    p.rect_stroke(bar, 0, Stroke::new(1.0, style::EDGE), egui::StrokeKind::Inside);
    p.galley(egui::pos2(bar.right() + 6.0, rect.top()), galley, color);
    response.on_hover_text(format!(
        "Mana: {:.0} / {:.0}\n{}\nMana density: {m:.2}x\nMana regen: {recovery:.1}x\nHealth regen: {health:.1}x · Well-fed",
        view.mana, max_mana, view.location.unwrap_or("Still air")));
}

/// One instrument: a fixed mana field, cast-cost marker and recharge rail.
fn mana_and_timers(
    ui: &mut egui::Ui,
    width: f32,
    view: &View,
    maximum: f32,
    eval: &wands::CastBlockEval,
) {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 24.0), egui::Sense::hover());
    let p = ui.painter();
    let rect = super::grimoire::snap(rect);
    icons::frame(p, rect, false);
    let gauge = rect.shrink(2.0);
    let mana_bar = Rect::from_min_size(gauge.min, vec2(gauge.width(), 15.0));
    let channel = view.concentration_upkeep;
    let low = view.mana < channel.map_or(eval.mana_cost, |upkeep| upkeep * 0.5)
        || view.mana_flash > 0.0;
    let poisoned = view.affinity >= crate::world::adventure::affinity::POISON_AT;
    let color = if low { style::DANGER } else if poisoned { POISON } else { style::MANA };
    if view.mana >= maximum - 0.5 {
        let time = ui.input(|i| i.time as f32);
        p.add(super::hud_style::glow(ui.ctx(), mana_bar, color, 2, 0.55 + (time * 3.0).sin() * 0.25));
    }
    let fill = (mana_bar.width() * (view.mana / maximum).clamp(0.0, 1.0)).floor();
    p.rect_filled(
        Rect::from_min_size(mana_bar.min, vec2(fill, 15.0)),
        0,
        color.gamma_multiply(0.28),
    );
    p.rect_filled(Rect::from_min_size(mana_bar.min, vec2(fill, 1.0)), 0, color);
    if eval.mana_cost > 0.0 && channel.is_none() {
        let x = mana_bar.left()
            + (mana_bar.width() * (eval.mana_cost / maximum).clamp(0.0, 1.0)).floor();
        p.rect_filled(
            Rect::from_min_size(egui::pos2(x, mana_bar.top()), vec2(1.0, 3.0)),
            0,
            if low {
                style::DANGER
            } else {
                super::grimoire::GOLD_LIGHT
            },
        );
    }
    let text = format!("Mana {:.0}/{maximum:.0}", view.mana.max(0.0));
    p.text(
        mana_bar.left_top() + vec2(3.0, 1.0),
        Align2::LEFT_TOP,
        text,
        FontId::proportional(11.0),
        style::PAPER,
    );
    let recharging = view.recharge_total > 0.01 && view.cooldown <= view.recharge_total + 0.01;
    let state = if let Some(upkeep) = channel {
        format!("Channel · {} mana/s", crate::world::adventure::spell_defs::card::amount(upkeep))
    } else if view.cooldown > 0.0 {
        format!("{:.1} s", view.cooldown)
    } else if eval.actions.is_empty() {
        "Empty".to_owned()
    } else {
        format!("{} / cast", crate::world::adventure::spell_defs::card::amount(eval.mana_cost))
    };
    p.text(
        mana_bar.right_top() - vec2(3.0, -1.0),
        Align2::RIGHT_TOP,
        state,
        FontId::proportional(11.0),
        if low { style::DANGER } else { style::PAPER },
    );
    let rail = Rect::from_min_size(gauge.min + vec2(0.0, 18.0), vec2(gauge.width(), 2.0));
    p.rect_filled(rail, 0, super::grimoire::SILVER_DARK);
    if channel.is_some() {
        let time = ui.input(|i| i.time as f32);
        p.rect_filled(rail, 0, color.gamma_multiply(0.74 + 0.26 * (time * 5.0).sin()));
    } else if view.cooldown > 0.0 {
        let total = if recharging {
            view.recharge_total
        } else {
            view.cast_total
        }
        .max(0.06);
        p.rect_filled(
            Rect::from_min_size(
                rail.min,
                vec2(
                    (rail.width() * (view.cooldown / total).clamp(0.0, 1.0)).floor(),
                    2.0,
                ),
            ),
            0,
            if recharging {
                style::AMBER
            } else {
                style::MANA
            },
        );
    }
    resp.on_hover_ui(|ui| {
        super::wands::show_wand_hover_card(ui, view);
    });
}

fn spell_deck(ui: &mut egui::Ui, view: &View, width: f32, maximum: f32) {
    let cap = wands::capacity(view.wand);
    let eval = wands::evaluate_cast_block(
        view.wand,
        view.rare_spells,
        view.discoveries,
        view.rank,
        &view.loadout,
        view.loadout.cursor,
    );
    let mut slots = Vec::with_capacity(cap);
    let mana_width = width - (cap as f32 * 20.0 - 2.0) - 4.0;
    // Both numbers must fit; narrow wands place the gauge underneath the deck.
    let inline_mana = mana_width >= 154.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(super::grimoire::GAP, 2.0);
        for index in 0..cap {
            let spell = view.loadout.slot(index);
            let (r, response) = ui.allocate_exact_size(
                vec2(super::grimoire::SLOT, super::grimoire::SLOT),
                egui::Sense::hover(),
            );
            slots.push(r);
            let r = super::grimoire::react(&response, r, index == view.loadout.cursor);
            icons::frame(ui.painter(), r, index == view.loadout.cursor);
            if spell != wands::EMPTY {
                icons::paint(ui.painter(), r.shrink(2.0), Icon::Spell(spell));
                if wands::is_modifier(spell) {
                    ui.painter().rect_filled(
                        Rect::from_min_size(r.left_top() + vec2(2.0, 2.0), vec2(2.0, 2.0)),
                        0,
                        style::MANA,
                    );
                }
                if index == view.loadout.cursor && view.cooldown > 0.0 {
                    super::hud_style::sweep(
                        ui.painter(),
                        r.shrink(2.0),
                        view.cooldown / view.cast_total.max(0.12),
                    );
                }
                if response.hovered() {
                    super::grimoire::focus(ui.painter(), r);
                }
                response.on_hover_ui(|ui| super::wands::show_spell_hover_card(ui, spell, view));
            }
        }
        if inline_mana {
            ui.add_space(2.0);
            mana_and_timers(ui, mana_width, view, maximum, &eval);
        }
    });
    super::wands::paint_cast_block_brackets(ui.painter(), &slots, &eval);
    if !inline_mana {
        mana_and_timers(ui, width, view, maximum, &eval);
    }
    let name = eval.actions.first().map_or("Empty wand", |action| spell_name(action.spell));
    let (r, response) = ui.allocate_exact_size(vec2(width, 16.0), egui::Sense::hover());
    // The name follows the evaluated next cast, including leading modifiers.
    // Clip its lane rather than painting over the controls on narrow displays.
    ui.painter()
        .rect_filled(r, 0, Color32::from_rgba_unmultiplied(29, 31, 34, 230));
    let hint = ui.painter().layout_no_wrap(
        crate::player::spell_binding_hint().to_owned(),
        FontId::proportional(10.0),
        style::MUTED,
    );
    let hint_pos = egui::pos2(r.right() - hint.size().x - 3.0, r.top() + 2.0);
    let name_rect = Rect::from_min_max(r.min, egui::pos2(hint_pos.x - 6.0, r.bottom()));
    ui.painter().with_clip_rect(name_rect.intersect(ui.clip_rect())).text(
        r.left_top() + vec2(3.0, 1.0),
        Align2::LEFT_TOP,
        name,
        FontId::proportional(12.0),
        style::PAPER,
    );
    ui.painter().galley(hint_pos, hint, style::MUTED);
    response.on_hover_text(name);
}

/// Positions health on the left and hunger on the right above the centered hotbar (Minecraft-style),
/// adapting cleanly to available window width and UI scale.
pub(super) fn hud(
    ctx: &egui::Context,
    view: &View,
    hotbar_rect: Rect,
    yaw: f32,
    eat_timer: f32,
    armor_points: u32,
    held_food: Option<crate::world::inventory::FoodKind>,
    cursor_free: bool,
    creative: bool,
) -> Rect {
    let maximum = view.max_health.max(1.0);
    let max_mana = view.mana_max.max(1.0);
    let dt = ctx.input(|i| i.predicted_dt).min(0.1);
    let time = ctx.input(|i| i.time as f32);

    let (preview_hunger, preview_heal) =
        held_food.map_or((0.0, 0.0), |f| (f.hunger_restore(), f.health_restore()));

    let hp_id = egui::Id::new("health_feedback");
    let mut hp_fb = ctx
        .data_mut(|d| d.get_temp::<HealthFeedback>(hp_id))
        .unwrap_or(HealthFeedback {
            previous: view.health,
            trail: view.health,
            hold: 0.0,
            maximum,
        });
    if maximum != hp_fb.maximum {
        hp_fb.trail = view.health;
        hp_fb.maximum = maximum;
    }
    if view.health < hp_fb.previous {
        hp_fb.trail = hp_fb.trail.max(hp_fb.previous);
        hp_fb.hold = 0.35;
    }
    hp_fb.hold = (hp_fb.hold - dt).max(0.0);
    if hp_fb.hold <= 0.0 {
        hp_fb.trail += (view.health - hp_fb.trail) * (1.0 - (-dt * 7.0).exp());
    }
    hp_fb.trail = hp_fb.trail.max(view.health);
    hp_fb.previous = view.health;
    ctx.data_mut(|d| d.insert_temp(hp_id, hp_fb.clone()));

    let hg_id = egui::Id::new("hunger_feedback");
    let mut hg_fb = ctx
        .data_mut(|d| d.get_temp::<HungerFeedback>(hg_id))
        .unwrap_or(HungerFeedback {
            previous: view.hunger,
            trail: view.hunger,
            hold: 0.0,
        });
    if view.hunger < hg_fb.previous {
        hg_fb.trail = hg_fb.trail.max(hg_fb.previous);
        hg_fb.hold = 0.35;
    }
    hg_fb.hold = (hg_fb.hold - dt).max(0.0);
    if hg_fb.hold <= 0.0 {
        hg_fb.trail += (view.hunger - hg_fb.trail) * (1.0 - (-dt * 7.0).exp());
    }
    hg_fb.trail = hg_fb.trail.max(view.hunger);
    hg_fb.previous = view.hunger;
    ctx.data_mut(|d| d.insert_temp(hg_id, hg_fb.clone()));

    let screen = ctx.content_rect();
    let available = (screen.width() - 24.0).max(160.0);
    // Match the hotbar width when it fits, or clamp to available screen width.
    let bar_width = hotbar_rect.width().min(available).floor();
    let bottom = hotbar_rect.top() - 4.0;
    let regen_active = view.health < maximum
        && ((view.hunger >= 75.0 && view.hurt <= 0.0)
            || super::progression_hud::active(ctx, crate::world::progression::Kind::Regeneration));

    let result = egui::Area::new(egui::Id::new("adventurer_vitals"))
        .anchor(Align2::CENTER_BOTTOM, [0.0, bottom - screen.bottom()])
        .interactable(cursor_free)
        .show(ctx, |ui| {
            if creative { return; }
            let backdrop = ui.painter().add(egui::Shape::Noop);
            ui.set_width(bar_width);
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            ui.spacing_mut().interact_size.y = 9.0;

            if view.resonance.active && ui.input(|i| i.modifiers.alt) {
                let run = &view.resonance;
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("Pressure {} / 5  ·  {}x", run.pressure(), run.salvage))
                        .monospace().size(12.0).color(style::AMBER));
                    if run.pending > 0 {
                        let glow = 0.80 + (time * 3.0).sin() * 0.20;
                        ui.label(egui::RichText::new("Upgrade Available")
                            .size(12.0).color(style::MANA.gamma_multiply(glow)));
                    }
                });
            }
            if view.ward > 0.0 && view.ward_hits > 0 && ui.input(|i| i.modifiers.alt) {
                ui.label(
                    egui::RichText::new(format!(
                        "WARD {}  {:.0}s",
                        view.ward_hits,
                        view.ward.ceil()
                    ))
                    .size(12.0)
                    .color(style::MANA),
                );
            }

            if super::inventory::unlocks::mana_line_open(
                ui.ctx(),
                view,
                view.affinity >= crate::world::adventure::affinity::POISON_AT,
            ) {
                mana_strip(ui, view, max_mana, bar_width);
            }

            if armor_points > 0 || (view.respawn_point.is_some() && ui.input(|i| i.modifiers.alt)) {
                ui.horizontal(|ui| {
                    if armor_points > 0 {
                        draw_armor_row(ui, (bar_width * 0.46).floor(), armor_points);
                    }
                    if let Some(sp) = view.respawn_point.filter(|_| ui.input(|i| i.modifiers.alt)) {
                        let lbl = ui.add(
                            egui::Label::new(
                                egui::RichText::new("Respawn set")
                                    .size(11.0)
                                    .color(style::JADE),
                            )
                            .sense(egui::Sense::hover()),
                        );
                        lbl.on_hover_text(format!(
                            "Respawn point: {:.0}, {:.0}, {:.0}",
                            sp.x, sp.y, sp.z
                        ));
                    }
                });
            }

            // Minecraft-style split row: Health on the left, Hunger on the right above the hotbar.
            let half_w = ((bar_width - 8.0) * 0.5).floor();
            if bar_width >= 160.0 {
                ui.horizontal(|ui| {
                    ui.set_width(bar_width);
                    ui.spacing_mut().item_spacing.x = 0.0;
                    draw_hearts(
                        ui,
                        half_w,
                        view.health,
                        maximum,
                        hp_fb.trail,
                        preview_heal,
                        regen_active,
                        view.mana_poison,
                        time,
                        true,
                    );
                    ui.add_space((bar_width - half_w * 2.0).max(4.0));
                    draw_hunger(
                        ui,
                        half_w,
                        view.hunger,
                        hg_fb.trail,
                        preview_hunger,
                        eat_timer,
                        time,
                        true,
                    );
                });
            } else {
                draw_hearts(
                    ui,
                    bar_width,
                    view.health,
                    maximum,
                    hp_fb.trail,
                    preview_heal,
                    regen_active,
                    view.mana_poison,
                    time,
                    false,
                );
                draw_hunger(
                    ui,
                    bar_width,
                    view.hunger,
                    hg_fb.trail,
                    preview_hunger,
                    eat_timer,
                    time,
                    false,
                );
            }
            vitals_plate(ui, backdrop, view, maximum, half_w, time);
        });
    let mut bounds = if creative { hotbar_rect } else { result.response.rect };
    if !creative {
        let poisoning = (view.affinity >= crate::world::adventure::affinity::POISON_AT).then_some(view.affinity);
        if let Some(row) = super::progression_hud::effect_bar(ctx, bounds, poisoning, cursor_free) {
            bounds = bounds.union(row);
        }
    }
    if view.wand > 0 && (view.cooldown > 0.0 || ctx.input(|i| i.modifiers.alt)) {
        let slots_width = wands::capacity(view.wand) as f32 * 20.0 - 2.0;
        let width = bar_width.max(slots_width + 84.0).min(available).floor();
        let wand = egui::Area::new(egui::Id::new("wand_instrument"))
            .anchor(
                Align2::CENTER_BOTTOM,
                // Clears the vitals plate (4 pt), its glow rings (6 pt) and this plate's own rim.
                [0.0, bounds.top() - 14.0 - screen.bottom()],
            )
            .interactable(cursor_free)
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().interact_size.y = 9.0;
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                let backdrop = ui.painter().add(egui::Shape::Noop);
                spell_deck(ui, view, width, max_mana);
                let plate = super::hud_style::plate(ui.ctx(), ui.min_rect().expand(4.0), style::MANA);
                ui.painter().set(backdrop, plate);
            });
        bounds = bounds.union(wand.response.rect);
    }
    super::relics::reward(ctx, view);
    super::relics::encounter(ctx, view);
    let alt = ctx.input(|i| i.modifiers.alt);
    if alt {
        super::relics::site_modifier(ctx, view);
    }
    // Only explicit quest tracking adds a pointer; Alt shows the bare heading.
    if alt || (view.quest.is_some() && view.boss.is_none()) {
        draw_compass(ctx, yaw, view.quest.as_ref());
    }
    combat_feedback(ctx, view, yaw);
    bounds
}

/// Pixel-art heading strip. The selected quest target is clamped to the edge with a chevron
/// when it is outside the visible arc.
fn draw_compass(ctx: &egui::Context, yaw: f32, target: Option<&QuestMarker>) {
    let content = ctx.content_rect();
    let width = (content.width() - 96.0).clamp(200.0, 300.0).floor();
    // 34 px tall so the labels (at +15) clear the bottom frame line; the top edge
    // stays where it was at 28 px.
    let slot = super::hud_layout::place(ctx, "compass");
    let bar = Rect::from_min_size(
        egui::pos2((content.center().x + slot.offset.x - width * 0.5).floor(), slot.top(ctx).floor()),
        vec2(width, 34.0),
    );
    super::hud_layout::claim(ctx, "compass", bar);
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("compass_strip"),
    ));
    p.rect_filled(bar, 0, Color32::from_rgba_unmultiplied(18, 21, 25, 180));
    super::grimoire::binding(&p, bar, false);

    // The heading readout owns a separate inset compartment. Rotating labels
    // and destination markers never paint over it or outside the binding.
    let readout = Rect::from_min_max(
        bar.right_top() + vec2(-36.0, 4.0),
        bar.right_bottom() + vec2(-4.0, -4.0),
    );
    let ribbon = Rect::from_min_max(
        bar.left_top() + vec2(4.0, 3.0),
        egui::pos2(readout.left() - 4.0, bar.bottom() - 3.0),
    );
    let ticks = p.with_clip_rect(ribbon);
    p.rect_filled(readout, 0, super::grimoire::RECESS);

    let heading = yaw.to_degrees().rem_euclid(360.0);
    let span = 104.0;
    let half = ribbon.width() * 0.5 - 12.0;
    let x_for = |relative: f32| ribbon.center().x + relative / span * half;
    let wrap = |angle: f32| {
        let mut a = angle.rem_euclid(360.0);
        if a > 180.0 {
            a -= 360.0;
        }
        a
    };

    for degree in (0..360).step_by(15) {
        let relative = wrap(degree as f32 - heading);
        if relative.abs() > span {
            continue;
        }
        let x = x_for(relative);
        let major = degree % 45 == 0;
        ticks.line_segment(
            [
                egui::pos2(x, bar.top() + if major { 5.0 } else { 8.0 }),
                egui::pos2(x, bar.top() + if major { 13.0 } else { 11.0 }),
            ],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(214, 208, 184, 120)),
        );
        if major {
            let label = match degree {
                0 => "N",
                45 => "NE",
                90 => "E",
                135 => "SE",
                180 => "S",
                225 => "SW",
                270 => "W",
                _ => "NW",
            };
            ticks.text(
                egui::pos2(x, bar.top() + 15.0),
                Align2::CENTER_TOP,
                label,
                FontId::proportional(if degree % 90 == 0 { 12.0 } else { 10.0 }),
                if degree == 0 {
                    style::AMBER
                } else {
                    style::PAPER
                },
            );
        }
    }

    p.text(
        readout.center(),
        Align2::CENTER_CENTER,
        format!("{:03}°", heading.round() as i32 % 360),
        FontId::monospace(10.0),
        style::PAPER,
    );

    // Fixed heading caret. The world rotates under it; the HUD stays legible.
    let center = egui::pos2(ribbon.center().x, bar.top() + 2.0);
    p.add(egui::Shape::convex_polygon(
        vec![center, center + vec2(-4.0, -6.0), center + vec2(4.0, -6.0)],
        style::AMBER,
        Stroke::new(1.0, style::INK),
    ));

    if let Some(target) = target {
        let relative = wrap(target.bearing_deg - heading);
        let clamped = relative.clamp(-span, span);
        let x = x_for(clamped);
        let color = match target.kind {
            3 => style::AMBER,
            5 => style::MANA,
            _ => style::JADE,
        };
        let marker = egui::pos2(x, bar.center().y - 3.0);
        ticks.circle_filled(
            marker,
            8.0,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 55),
        );
        ticks.add(egui::Shape::convex_polygon(
            vec![
                marker + vec2(0.0, -6.0),
                marker + vec2(5.0, 0.0),
                marker + vec2(0.0, 6.0),
                marker + vec2(-5.0, 0.0),
            ],
            color,
            Stroke::new(1.0, style::INK),
        ));
        if relative.abs() > span {
            let tip = if relative > 0.0 {
                marker + vec2(9.0, 0.0)
            } else {
                marker + vec2(-9.0, 0.0)
            };
            let wing = if relative > 0.0 { -1.0 } else { 1.0 };
            ticks.add(egui::Shape::convex_polygon(
                vec![
                    tip,
                    tip + vec2(6.0 * wing, -5.0),
                    tip + vec2(6.0 * wing, 5.0),
                ],
                color,
                Stroke::new(1.0, style::INK),
            ));
        }
        let height = if target.vertical_m > 1.5 {
            format!("  ↑ {:.0} m", target.vertical_m)
        } else if target.vertical_m < -1.5 {
            format!("  ↓ {:.0} m", -target.vertical_m)
        } else {
            String::new()
        };
        let caption = p.layout_no_wrap(
            format!("{}  {:.0} m{}", target.label, target.distance_m, height),
            FontId::proportional(11.0),
            style::PAPER,
        );
        // Keep the destination stable while turning, on a readable fieldbook
        // strip rather than unbacked text drifting across bright terrain.
        let ribbon = Rect::from_center_size(
            egui::pos2(bar.center().x, bar.bottom() + 12.0),
            vec2((caption.size().x + 16.0).min(content.width() - 16.0), 18.0),
        );
        p.rect_filled(ribbon, 0, Color32::from_rgba_unmultiplied(29, 31, 34, 230));
        p.rect_filled(Rect::from_min_size(ribbon.min, vec2(2.0, ribbon.height())), 0, color);
        p.with_clip_rect(ribbon.shrink2(vec2(6.0, 0.0))).galley(
            ribbon.center() - caption.size() * 0.5,
            caption,
            style::PAPER,
        );
        // The destination strip hangs below the bar; panels stacked under the
        // compass must clear it too.
        super::hud_layout::claim(ctx, "compass", bar.union(ribbon));
    }
}

fn combat_feedback(ctx: &egui::Context, view: &View, yaw: f32) {
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("combat_feedback"),
    ));
    let center = ctx.content_rect().center();
    if view.hit > 0.0 {
        for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            p.line_segment(
                [center + vec2(x, y) * 7.0, center + vec2(x, y) * 11.0],
                Stroke::new(1.5, style::AMBER),
            );
        }
    }
    // Name the creature behind the last hit, with its health, so a bite from
    // underground or off screen is never anonymous.
    if let Some((kind, fraction)) = view.damage_source {
        let name = crate::world::adventure::enemy_name(kind);
        let galley = p.layout_no_wrap(name.to_owned(), FontId::proportional(13.0), style::PAPER);
        let width = galley.size().x.max(120.0) + 16.0;
        let plate = Rect::from_center_size(center + vec2(0.0, 74.0), vec2(width, 34.0));
        p.rect_filled(plate, 0, Color32::from_rgba_unmultiplied(29, 31, 34, 225));
        p.rect_stroke(plate, 0, Stroke::new(1.0, style::INK), egui::StrokeKind::Inside);
        p.galley(egui::pos2(plate.center().x - galley.size().x * 0.5, plate.top() + 3.0), galley, style::PAPER);
        let track = Rect::from_min_max(
            egui::pos2(plate.left() + 8.0, plate.bottom() - 11.0),
            egui::pos2(plate.right() - 8.0, plate.bottom() - 5.0),
        );
        p.rect_filled(track, 0, Color32::from_rgb(14, 12, 16));
        let mut fill = track;
        fill.set_width(track.width() * fraction.clamp(0.0, 1.0));
        p.rect_filled(fill, 0, Color32::from_rgb(196, 62, 54));
    }
    if view.hurt <= 0.0 {
        return;
    }
    let intensity =
        (view.hurt / 0.7).clamp(0.0, 1.0) * (view.damage_amount / 18.0).clamp(0.45, 1.0);
    let r = ctx.content_rect();
    let edge = Color32::from_rgba_unmultiplied(168, 28, 36, (intensity * 105.0) as u8);
    for rect in [
        Rect::from_min_max(r.min, egui::pos2(r.max.x, r.min.y + 10.0)),
        Rect::from_min_max(egui::pos2(r.min.x, r.max.y - 10.0), r.max),
        Rect::from_min_max(r.min, egui::pos2(r.min.x + 10.0, r.max.y)),
        Rect::from_min_max(egui::pos2(r.max.x - 10.0, r.min.y), r.max),
    ] {
        p.rect_filled(rect, 0, edge);
    }
    if view.damage_direction.length_squared() <= 0.01 {
        return;
    }
    let forward = glam::Vec2::new(-yaw.sin(), -yaw.cos());
    let right = glam::Vec2::new(yaw.cos(), -yaw.sin());
    let source = glam::Vec2::new(view.damage_direction.x, view.damage_direction.z);
    let screen = vec2(source.dot(right), -source.dot(forward)).normalized();
    let tip = center + screen * 44.0;
    let wing = vec2(-screen.y, screen.x) * 8.0;
    p.add(egui::Shape::convex_polygon(
        vec![tip, tip - screen * 12.0 + wing, tip - screen * 12.0 - wing],
        Color32::from_rgba_unmultiplied(238, 86, 68, (intensity * 215.0) as u8),
        Stroke::new(1.0, style::INK),
    ));
}
