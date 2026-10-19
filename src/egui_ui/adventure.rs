use super::{icons::{self, Icon}, presentation as style};
use crate::world::adventure::{View, spell_name, wands};
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
    time: f32,
    align_left: bool,
) {
    let count = (maximum / 10.0).ceil() as usize;
    let icon_step = (width / 10.0).floor().clamp(11.0, 17.0);
    let columns = (width / icon_step).floor().clamp(5.0, 10.0) as usize;
    let rows = count.div_ceil(columns);
    let row_h = (icon_step + 1.0).max(13.0);
    let (rect, resp) =
        ui.allocate_exact_size(vec2(width, rows as f32 * row_h), egui::Sense::hover());
    let dpi = ui.ctx().pixels_per_point();
    let unit = ((icon_step / 8.5) * dpi).floor().max(1.0) / dpi;
    let shape = [
        " ## ## ",
        "#######",
        "#######",
        " ##### ",
        "  ###  ",
        "   #   ",
    ];
    let mut mesh = egui::Mesh::default();
    let group_width = columns.min(count) as f32 * icon_step;
    let start_x = if align_left {
        rect.left()
    } else {
        rect.center().x - group_width * 0.5
    };
    let origin = egui::pos2(
        (start_x * dpi).round() / dpi,
        ((rect.top() + 2.0) * dpi).round() / dpi,
    );
    let low_hp = value <= 25.0;
    let preview_top = (value + preview_heal).min(maximum);
    let pulse = ((time * 5.5).sin() * 0.5 + 0.5).clamp(0.25, 0.95);
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
                    if y < 2 && x < 3 {
                        Color32::from_rgb(255, 148, 134)
                    } else if y >= 4 {
                        Color32::from_rgb(158, 42, 52)
                    } else {
                        Color32::from_rgb(228, 66, 74)
                    }
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
        "  ####  ",
        " ###### ",
        "####### ",
        " ###### ",
        "  ####  ",
        "   ###  ",
        "    ### ",
        "     ## ",
    ];
    let icon_step = (width / 10.0).floor().clamp(11.0, 17.0);
    let row_h = (icon_step + 1.0).max(13.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, row_h), egui::Sense::hover());
    let dpi = ui.ctx().pixels_per_point();
    let unit = ((icon_step / 8.8) * dpi).floor().max(1.0) / dpi;
    let group_width = 10.0 * icon_step;
    let start_x = if align_right {
        rect.right() - group_width
    } else {
        rect.center().x - group_width * 0.5
    };
    let origin = egui::pos2(
        (start_x * dpi).round() / dpi,
        ((rect.top() + 1.5) * dpi).round() / dpi,
    );
    let starving = value <= 0.05;
    let low_hunger = value <= 25.0;
    let eating = eat_timer > 0.0;
    let preview_top = (value + preview_hunger).min(100.0);
    let pulse = ((time * 5.5).sin() * 0.5 + 0.5).clamp(0.25, 0.95);
    let mut mesh = egui::Mesh::default();

    for idx in 0..10 {
        let slot_Index_from_right = 9 - idx;
        let threshold = slot_Index_from_right as f32 * 10.0;
        let fill = ((value - threshold) / 10.0).clamp(0.0, 1.0);
        let old = ((trail - threshold) / 10.0).clamp(0.0, 1.0);
        let prev_fill = ((preview_top - threshold) / 10.0).clamp(0.0, 1.0);

        let bounce = if starving {
            ((time * 13.0 + idx as f32 * 1.7).sin() * 1.4).round()
        } else if low_hunger {
            ((time * 8.5 + idx as f32 * 1.4).sin() * 1.0).round()
        } else if eating {
            let wave = ((time * 14.0 - slot_Index_from_right as f32 * 0.6).sin()).max(0.0);
            -(wave * 1.5).round()
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

    // Subtle 75% well-fed regen threshold tick mark (2.5 haunches from left of the right-aligned group)
    let tick_x = origin.x + 2.5 * icon_step - 1.0;
    ui.painter().line_segment(
        [
            egui::pos2(tick_x, rect.top() + 1.0),
            egui::pos2(tick_x, rect.bottom() - 2.0),
        ],
        Stroke::new(
            1.0,
            if value >= 75.0 {
                style::JADE
            } else {
                Color32::from_rgba_unmultiplied(173, 157, 139, 95)
            },
        ),
    );

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
    const SHIELD: [&str; 6] = [
        "######",
        "######",
        "######",
        " #### ",
        "  ##  ",
        "  ##  ",
    ];
    let icon_step = (width / 10.0).floor().clamp(11.0, 17.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 13.0), egui::Sense::hover());
    let dpi = ui.ctx().pixels_per_point();
    let unit = ((icon_step / 8.8) * dpi).floor().max(1.0) / dpi;
    let origin = egui::pos2(
        (rect.left() * dpi).round() / dpi,
        ((rect.top() + 1.0) * dpi).round() / dpi,
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

/// Noita-style Mana + Cast Delay / Recharge dual gauge bar with low-mana flash warning.
fn mana_and_timers(ui: &mut egui::Ui, width: f32, view: &View, maximum: f32, time: f32) {
    let eval = wands::evaluate_cast_block(
        view.wand,
        view.rare_spells,
        view.discoveries,
        view.rank,
        &view.loadout,
        view.loadout.cursor,
    );
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 18.0), egui::Sense::hover());
    let p = ui.painter();

    let bar_w = (width - 88.0).max(90.0);
    let mana_bar = Rect::from_min_size(rect.min + vec2(0.0, 2.0), vec2(bar_w, 7.0));
    let timer_bar = Rect::from_min_size(rect.min + vec2(0.0, 11.0), vec2(bar_w, 4.0));

    let flashing = view.mana_flash > 0.0 && ((time * 18.0).sin() > -0.2);
    p.rect_filled(
        mana_bar.expand(1.5),
        0,
        if flashing { style::DANGER } else { style::INK },
    );
    p.rect_filled(mana_bar, 0, Color32::from_rgb(38, 35, 62));

    let fill_frac = (view.mana / maximum).clamp(0.0, 1.0);
    let filled = Rect::from_min_size(mana_bar.min, vec2(fill_frac * mana_bar.width(), mana_bar.height()));
    p.rect_filled(
        filled,
        0,
        if flashing {
            style::DANGER
        } else {
            Color32::from_rgb(91, 128, 218)
        },
    );
    p.rect_filled(
        Rect::from_min_size(filled.min, vec2(filled.width(), 2.0)),
        0,
        Color32::from_rgb(164, 198, 248),
    );

    // Preview next cast mana cost tick on the mana bar
    if eval.mana_cost > 0.0 {
        let cost_frac = (eval.mana_cost / maximum).clamp(0.0, 1.0);
        let cost_x = mana_bar.left() + cost_frac * mana_bar.width();
        p.line_segment(
            [
                egui::pos2(cost_x, mana_bar.top()),
                egui::pos2(cost_x, mana_bar.bottom()),
            ],
            Stroke::new(
                1.2,
                if view.mana < eval.mana_cost {
                    style::DANGER
                } else {
                    style::AMBER
                },
            ),
        );
    }

    // Cast Delay (Cyan) & Recharge Time (Amber) sub-bar
    p.rect_filled(timer_bar.expand(1.0), 0, style::INK);
    p.rect_filled(timer_bar, 0, Color32::from_rgb(28, 26, 34));
    if view.cooldown > 0.0 {
        let total = view.cast_total.max(0.06);
        let frac = (view.cooldown / total).clamp(0.0, 1.0);
        let recharging = view.recharge_total > 0.01 && view.cooldown <= view.recharge_total + 0.01;
        let timer_col = if recharging {
            style::AMBER
        } else {
            Color32::from_rgb(112, 224, 246)
        };
        p.rect_filled(
            Rect::from_min_size(timer_bar.min, vec2(frac * timer_bar.width(), timer_bar.height())),
            0,
            timer_col,
        );
    }

    let text = if view.mana_flash > 0.0 {
        "LOW MP!".to_string()
    } else if view.cooldown > 0.0 && view.recharge_total > 0.01 && view.cooldown <= view.recharge_total + 0.01 {
        format!("RCH {:.2}s", view.cooldown)
    } else {
        format!("{:.0} MP", view.mana.max(0.0).floor())
    };
    let text_col = if view.mana_flash > 0.0 {
        style::DANGER
    } else if view.cooldown > 0.0 && view.recharge_total > 0.01 {
        style::AMBER
    } else {
        style::PAPER
    };
    p.text(
        rect.right_top() + vec2(1.0, 1.0),
        Align2::RIGHT_TOP,
        &text,
        FontId::monospace(13.0),
        style::INK,
    );
    p.text(
        rect.right_top(),
        Align2::RIGHT_TOP,
        text,
        FontId::monospace(13.0),
        text_col,
    );

    resp.on_hover_ui(|ui| {
        super::wands::show_wand_hover_card(ui, view);
    });
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
) -> Rect {
    let maximum = 100.0 + view.rank as f32 * 20.0;
    let max_mana = 100.0 + view.rank as f32 * 15.0;
    let dt = ctx.input(|i| i.predicted_dt).min(0.1);
    let time = ctx.input(|i| i.time as f32);

    let (preview_hunger, preview_heal) = held_food.map_or((0.0, 0.0), |f| {
        (f.hunger_restore(), f.health_restore())
    });

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
    let bar_width = hotbar_rect.width().max(260.0).min(available).min(380.0);
    let compact = screen.height() < 480.0 || bar_width < 240.0;
    let bottom = hotbar_rect.top() - 4.0;
    let regen_active = view.hunger >= 75.0 && view.health < maximum && view.hurt <= 0.0;

    let result = egui::Area::new("adventurer_vitals".into())
        .anchor(Align2::CENTER_BOTTOM, [0.0, bottom - screen.bottom()])
        .interactable(cursor_free)
        .show(ctx, |ui| {
            ui.set_width(bar_width);
            ui.spacing_mut().item_spacing = vec2(4.0, 2.5);

            if view.wand > 0 {
                mana_and_timers(ui, bar_width, view, max_mana, time);
                if !compact {
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
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(3.0, 2.0);
                        for slot in 0..cap {
                            let spell = view.loadout.slot(slot);
                            let (r, slot_resp) =
                                ui.allocate_exact_size(vec2(22.0, 22.0), egui::Sense::hover());
                            slot_rects.push(r);
                            if spell == wands::EMPTY {
                                ui.painter().rect_filled(
                                    r.shrink(2.0),
                                    0,
                                    Color32::from_black_alpha(120),
                                );
                                ui.painter().rect_stroke(
                                    r.shrink(1.0),
                                    0,
                                    Stroke::new(1.0, Color32::from_rgb(62, 52, 48)),
                                    egui::epaint::StrokeKind::Inside,
                                );
                                continue;
                            }
                            icons::frame(ui.painter(), r, slot == view.loadout.cursor);
                            icons::paint(ui.painter(), r.shrink(2.0), Icon::Spell(spell));
                            if wands::is_modifier(spell) {
                                ui.painter().rect_stroke(
                                    r.shrink(1.0),
                                    0,
                                    Stroke::new(1.0, style::AMBER),
                                    egui::epaint::StrokeKind::Inside,
                                );
                            }
                            if slot == view.loadout.cursor && view.cooldown > 0.0 {
                                let progress = (view.cooldown / view.cast_total.max(0.12))
                                    .clamp(0.0, 1.0);
                                ui.painter().rect_filled(
                                    Rect::from_min_size(
                                        r.left_bottom() - vec2(0.0, 2.0),
                                        vec2(r.width() * progress, 2.0),
                                    ),
                                    0,
                                    style::MANA,
                                );
                            }
                            slot_resp.on_hover_ui(|ui| {
                                super::wands::show_spell_hover_card(ui, spell, view);
                                ui.separator();
                                super::wands::show_wand_hover_card(ui, view);
                            });
                        }
                        ui.add_space(2.0);
                        style::keycap(ui, "R");
                        let wand_lbl = ui.add(
                            egui::Label::new(
                                egui::RichText::new(spell_name(view.spell))
                                    .size(11.5)
                                    .color(style::PAPER),
                            )
                            .sense(egui::Sense::hover()),
                        );
                        style::keycap(ui, "T");
                        wand_lbl.on_hover_ui(|ui| {
                            super::wands::show_wand_hover_card(ui, view);
                        });
                    });
                    super::wands::paint_cast_block_brackets(ui.painter(), &slot_rects, &eval);
                    ui.add_space(2.0);
                }
            }
            if view.ward > 0.0 && view.ward_hits > 0 {
                ui.label(
                    egui::RichText::new(format!("WARD {}  {:.0}s", view.ward_hits, view.ward.ceil()))
                        .size(12.0)
                        .color(style::MANA),
                );
            }

            if armor_points > 0 {
                draw_armor_row(ui, (bar_width * 0.46).max(110.0), armor_points);
            }

            // Minecraft-style split row: Health on the left, Hunger on the right above the hotbar.
            let half_w = ((bar_width - 10.0) * 0.5).max(110.0);
            if bar_width >= 230.0 {
                ui.horizontal(|ui| {
                    ui.set_width(bar_width);
                    draw_hearts(
                        ui,
                        half_w,
                        view.health,
                        maximum,
                        hp_fb.trail,
                        preview_heal,
                        regen_active,
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
        });
    super::relics::reward(ctx, view);
    super::relics::encounter(ctx, view);
    combat_feedback(ctx, view, yaw);
    result.response.rect
}

fn combat_feedback(ctx: &egui::Context, view: &View, yaw: f32) {
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        "combat_feedback".into(),
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
