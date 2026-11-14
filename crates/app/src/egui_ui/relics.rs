//! Brief, square-framed combat rewards and the active ward's remaining charges.
use super::{
    icons::{self, Icon},
    presentation as style,
};
use crate::world::{adventure::View, expedition::NAMES};
use egui::{Align2, Color32, Rect, RichText, vec2};

pub(super) fn reward(ctx: &egui::Context, view: &View) {
    if view.boss.is_some() {
        return;
    }
    let Some(reward) = &view.reward else {
        return;
    };
    let width = (ctx.content_rect().width() - 48.0).clamp(100.0, 244.0);
    let reserved = ctx
        .data_mut(|d| d.get_temp::<Option<Rect>>(egui::Id::new("hud_top_reserved")))
        .flatten();
    let left = ctx.content_rect().center().x - width * 0.5 - 10.0;
    let slot = super::hud_layout::place(ctx, "reward");
    let top = reserved
        .filter(|r| r.right() + 8.0 > left)
        .map_or(slot.offset.y, |r| (r.bottom() + 8.0 - ctx.content_rect().top()).max(slot.offset.y));
    let shown = egui::Area::new(egui::Id::new("sanctum_reward"))
        .anchor(slot.align, [slot.offset.x, top])
        .interactable(false)
        .show(ctx, |ui| {
            style::hud_frame(ui.ctx()).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_width(width);
                    ui.label(RichText::new(reward.name).color(style::AMBER).size(16.0));
                    ui.label(
                        RichText::new(if reward.name == "The workshop lives again" {
                            "Expedition complete"
                        } else if reward.items.is_empty() {
                            "Relic claimed"
                        } else {
                            "Supplies recovered"
                        })
                        .color(style::MUTED)
                        .size(11.0),
                    );
                    if let Some(id) = reward.discovery {
                        let inscription =
                            &crate::world::adventure::inscriptions::INSCRIPTIONS[id as usize];
                        ui.label(
                            RichText::new(inscription.name)
                                .color(style::MANA)
                                .size(14.0),
                        );
                        ui.label(
                            RichText::new(inscription.benefit)
                                .color(style::PAPER)
                                .size(11.0),
                        );
                    }
                    let gap = ui.spacing().item_spacing.x;
                    let columns = ((width + gap) / (56.0 + gap)).floor().max(1.0) as usize;
                    for row in reward.items.chunks(columns) {
                        ui.horizontal(|ui| {
                            let occupied = row.len() as f32 * (56.0 + gap) - gap;
                            ui.add_space(((width - occupied) * 0.5).max(0.0));
                            for &(item, amount) in row {
                                icons::slot(
                                    ui,
                                    Icon::Resource(item),
                                    Some(amount),
                                    false,
                                    NAMES[item],
                                );
                            }
                        });
                    }
                    let (r, _) = ui.allocate_exact_size(vec2(width, 2.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 0, style::INK);
                    let remaining = Rect::from_min_size(
                        r.min,
                        vec2(
                            r.width()
                                * (reward.seconds
                                    / if reward.discovery.is_some() {
                                        10.0
                                    } else {
                                        7.0
                                    })
                                .clamp(0.0, 1.0),
                            r.height(),
                        ),
                    );
                    ui.painter()
                        .rect_filled(remaining, 0, Color32::from_rgb(164, 139, 84));
                });
            });
        });
    super::hud_layout::claim(ctx, "reward", shown.response.rect);
}

/// A compact collection of permanent magical discoveries, with readable histories.
pub(super) fn expedition_goal(ui: &mut egui::Ui, view: &View) {
    ui.collapsing("Workshop heart", |ui| {
    ui.label(
        RichText::new(if view.heart_restored {
            "The workshop lives again"
        } else {
            "Restore the workshop heart"
        })
        .color(style::AMBER)
        .size(16.0),
    );
    ui.horizontal_wrapped(|ui| {
        for (i, (label, spell)) in [
            ("Foundry · Cinderbrand", 3),
            ("Archive · Rift well", 6),
            ("Cathedral · Winterglass", 2),
        ]
        .into_iter()
        .enumerate()
        {
            let owned = view.sigils & (1 << i) != 0;
            let response = icons::slot(ui, Icon::Spell(spell), None, owned, label);
            if !owned {
                ui.painter().rect_filled(
                    response.rect.shrink(2.0),
                    0,
                    Color32::from_black_alpha(150),
                );
            }
        }
    });
    ui.label(RichText::new(if view.heart_restored { "Heart restored" }
        else if view.sigils==7 { "Offer the three relics at the workshop heart." }
        else { "Find the foundry, archive and cathedral relics." }).color(style::PAPER));
    if !view.route.is_empty() {
        ui.label(RichText::new(&view.route).color(style::MUTED).size(12.0));
    } else if !view.heart_restored {
        ui.label(RichText::new("The reliquary below the workshop reveals a guardian.").color(style::MUTED).size(12.0));
    }
    ui.add_space(8.0);
    });
}

pub(super) fn codex(ui: &mut egui::Ui, view: &View) {
    use crate::world::adventure::inscriptions::INSCRIPTIONS;
    expedition_goal(ui, view);
    ui.collapsing(
        format!("Inscriptions  {} / {}", view.discoveries.count_ones(), INSCRIPTIONS.len()),
        |ui| {
            let id = ui.make_persistent_id("inscription_page");
            let mut selected = ui
                .ctx()
                .data_mut(|d| d.get_temp::<usize>(id))
                .unwrap_or_else(|| (view.discoveries.trailing_zeros() as usize).min(INSCRIPTIONS.len() - 1));
            ui.horizontal_wrapped(|ui| {
                for (i, lore) in INSCRIPTIONS.iter().enumerate() {
                    let owned = view.discoveries & (1 << i) != 0;
                    let response = icons::slot(
                        ui,
                        Icon::Spell(lore.spell),
                        None,
                        selected == i,
                        if owned { lore.name } else { lore.site },
                    );
                    if !owned {
                        ui.painter().rect_filled(
                            response.rect.shrink(2.0),
                            0,
                            Color32::from_black_alpha(160),
                        );
                    }
                    if response.clicked() {
                        selected = i;
                    }
                }
            });
            ui.ctx().data_mut(|d| d.insert_temp(id, selected));
            let lore = &INSCRIPTIONS[selected];
            if view.discoveries & (1 << selected) != 0 {
                ui.label(RichText::new(lore.name).color(style::AMBER).size(16.0));
                ui.label(RichText::new(lore.benefit).color(style::MANA));
                if let Some(record) = crate::world::adventure::chronicle::history(selected) {
                    ui.label(record);
                }
            } else if let Some(hint) = crate::world::adventure::chronicle::hint(selected) {
                ui.label(hint);
            } else {
                ui.label(format!(
                    "Find a {} and claim the inscription from its reliquary.",
                    lore.site.to_lowercase()
                ));
            }
        },
    );
}

pub(super) fn encounter(ctx: &egui::Context, view: &View) {
    if let Some(boss) = &view.boss {
        egui::Area::new(egui::Id::new("guardian_health"))
            .anchor(Align2::LEFT_TOP, [12.0, 82.0])
            .interactable(false)
            .show(ctx, |ui| {
                style::hud_frame(ui.ctx()).show(ui, |ui| {
                    ui.set_width((ctx.content_rect().width() - 48.0).clamp(100.0, 220.0));
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(boss.name).color(style::AMBER).size(12.0));
                        ui.label(
                            RichText::new(boss.action.unwrap_or(if boss.health < boss.maximum * 0.5 {
                                "Enraged"
                            } else {
                                "Active"
                            }))
                            .color(style::MUTED)
                            .size(11.0),
                        );
                        let (r, _) = ui.allocate_exact_size(
                            vec2(ui.available_width(), 5.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(r, 0, style::INK);
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                r.min,
                                vec2(
                                    r.width() * (boss.health / boss.maximum).clamp(0.0, 1.0),
                                    r.height(),
                                ),
                            ),
                            0,
                            style::DANGER,
                        );
                    });
                });
            });
    }
    if let Some(biome) = view.hell_biome {
        egui::Area::new(egui::Id::new("hell_biome"))
            .anchor(Align2::LEFT_TOP, [16.0, 52.0])
            .interactable(false)
            .show(ctx, |ui| { ui.small(biome); });
    }
    if let Some(focus) = view.relic_focus.as_ref().or(view.undercrown_focus.as_ref()) {
        egui::Area::new(egui::Id::new("world_interaction_prompt"))
            .anchor(Align2::CENTER_TOP, [0.0, ctx.content_rect().height() * 0.5 + 24.0])
            .interactable(false)
            .show(ctx, |ui| {
                style::hud_frame(ui.ctx()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let action = focus.action.strip_prefix("[F]").or_else(|| focus.action.strip_prefix("F "));
                        if let Some(verb) = action {
                            let key = crate::keybinds::hint(crate::keybinds::Action::Interact);
                            ui.label(RichText::new(format!("[{key}]")).color(style::AMBER).strong().size(12.0));
                            let verb = verb.trim();
                            if verb == "Talk" {
                                ui.label(RichText::new(verb).color(style::PAPER).size(12.0));
                                ui.label(RichText::new(focus.title).color(style::JADE).strong().size(12.0));
                            } else if matches!(verb, "Use" | "Toggle") {
                                ui.label(RichText::new(format!("{verb} {}", focus.title)).color(style::PAPER).size(12.0));
                            } else {
                                ui.label(RichText::new(verb).color(style::PAPER).size(12.0));
                            }
                        } else {
                            ui.label(RichText::new(focus.action).color(style::PAPER).size(12.0));
                        }
                    });
                });
            });
    }
}

/// The active site modifier, shown like a Noita perk: name and one line.
pub(super) fn site_modifier(ctx: &egui::Context, view: &View) {
    let Some((name, line)) = view.site_modifier else {
        return;
    };
    let top = if view.boss.is_some() { 150.0 } else { 82.0 };
    egui::Area::new(egui::Id::new("site_modifier"))
        .anchor(Align2::LEFT_TOP, [12.0, top])
        .interactable(false)
        .show(ctx, |ui| {
            style::hud_frame(ui.ctx()).show(ui, |ui| {
                ui.set_width((ctx.content_rect().width() - 48.0).clamp(100.0, 220.0));
                ui.label(RichText::new(name).color(style::AMBER).size(12.0));
                ui.label(RichText::new(line).color(style::MUTED).size(11.0));
            });
        });
}
