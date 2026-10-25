//! The Tinker's Forge: a part/material/modifier workbench panel.
//!
//! The panel reads the adventure `View` and the authoritative survival
//! inventory, then emits ordinary factory actions. No UI state is stored in the
//! world, so opening it never mutates a save.
use super::*;
use crate::world::adventure::tinkering::{self, Material, Part};
use crate::world::factory::Action;
use crate::world::inventory::{Item, SurvivalInventory};

fn units_owned(inventory: &SurvivalInventory, resource: usize) -> u32 {
    inventory
        .count_item(Item::Resource(resource as u8))
        .saturating_mul(64)
        .saturating_add(inventory.resource_remainders[resource])
}

fn affordable(inventory: &SurvivalInventory, cost: &[(usize, u32)]) -> bool {
    cost.iter().all(|&(id, n)| {
        id < crate::world::factory::RESOURCE_COUNT && units_owned(inventory, id) >= n
    })
}

fn cost_text(cost: &[(usize, u32)]) -> String {
    cost.iter()
        .map(|&(id, n)| {
            let name = crate::world::expedition::NAMES
                .get(id)
                .copied()
                .unwrap_or("Material");
            format!("{} {:.2}", n as f32 / 64.0, name)
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn material_swatch(ui: &mut egui::Ui, material: Material, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let [r, g, b] = material.color();
    let color = Color32::from_rgb(r, g, b);
    let p = ui.painter();
    p.rect_filled(rect, 0, color);
    p.rect_stroke(
        rect,
        0,
        egui::Stroke::new(1.0, Color32::from_rgb(24, 20, 22)),
        egui::StrokeKind::Inside,
    );
    // Native-pixel highlight and shadow keep the swatch in the same language as
    // the item icons instead of reading as a flat web colour chip.
    let dpi = ui.ctx().pixels_per_point();
    let edge = (1.0 / dpi).max(1.0);
    p.rect_filled(
        egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), edge)),
        0,
        color.gamma_multiply(1.22),
    );
    p.rect_filled(
        egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.bottom() - edge),
            egui::vec2(rect.width(), edge),
        ),
        0,
        color.gamma_multiply(0.55),
    );
    response
}

/// Small vector glyphs for the three parts. They are drawn on the UI pixel grid,
/// so they stay crisp at every scale factor.
fn part_glyph(ui: &mut egui::Ui, part: Part, size: egui::Vec2, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter();
    let outline = egui::Stroke::new(2.0, Color32::from_rgb(26, 22, 24));
    let fill = egui::Stroke::new(2.0, color);
    let c = rect.center();
    match part {
        Part::Head => {
            let a = c + egui::vec2(-rect.width() * 0.30, rect.height() * 0.30);
            let b = c + egui::vec2(rect.width() * 0.28, -rect.height() * 0.32);
            p.line_segment([a, b], outline);
            p.line_segment([a, b], fill);
            let guard = c + egui::vec2(-rect.width() * 0.18, rect.height() * 0.06);
            p.line_segment(
                [
                    guard + egui::vec2(-rect.width() * 0.10, 0.0),
                    guard + egui::vec2(rect.width() * 0.06, 0.0),
                ],
                fill,
            );
            p.line_segment(
                [
                    guard,
                    guard + egui::vec2(rect.width() * 0.12, rect.height() * 0.12),
                ],
                fill,
            );
        }
        Part::Binding => {
            for (y, width) in [(-0.10, 0.30), (0.16, 0.24)] {
                let a = c + egui::vec2(-rect.width() * width, rect.height() * y);
                let b = c + egui::vec2(rect.width() * width, rect.height() * y);
                p.line_segment([a, b], outline);
                p.line_segment([a, b], fill);
            }
            p.line_segment(
                [
                    c + egui::vec2(-rect.width() * 0.08, -rect.height() * 0.28),
                    c + egui::vec2(rect.width() * 0.08, -rect.height() * 0.28),
                ],
                fill,
            );
        }
        Part::Grip => {
            let a = c + egui::vec2(-rect.width() * 0.26, rect.height() * 0.30);
            let b = c + egui::vec2(rect.width() * 0.26, -rect.height() * 0.30);
            p.line_segment([a, b], outline);
            p.line_segment([a, b], fill);
            for i in 0..3 {
                let t = 0.30 + i as f32 * 0.20;
                let mid = a.lerp(b, t);
                p.line_segment(
                    [
                        mid + egui::vec2(-rect.width() * 0.10, -rect.height() * 0.01),
                        mid + egui::vec2(rect.width() * 0.10, rect.height() * 0.01),
                    ],
                    fill,
                );
            }
        }
    }
}

fn stat_bar(ui: &mut egui::Ui, label: &str, multiplier: f32, color: Color32) {
    let normal = ((multiplier - 0.90) / 0.42).clamp(0.0, 1.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(11.0).color(presentation::MUTED));
        ui.add(
            egui::ProgressBar::new(normal)
                .desired_width((ui.available_width() - 12.0).clamp(68.0, 180.0))
                .fill(color)
                .text(RichText::new(format!("×{multiplier:.2}")).size(11.0)),
        );
    });
}

fn part_card(
    ui: &mut egui::Ui,
    part: Part,
    current: u8,
    bench: bool,
    has_weapon: bool,
    inventory: &SurvivalInventory,
    action: &mut UiAction,
) {
    let material = Material::from_u8(current).unwrap_or(Material::Copper);
    let [r, g, b] = material.color();
    let accent = Color32::from_rgb(r, g, b);
    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(20, 18, 22, 232))
        .stroke(egui::Stroke::new(1.0, accent.linear_multiply(0.55)))
        .inner_margin(egui::Margin::same(9))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                part_glyph(ui, part, egui::vec2(30.0, 30.0), accent);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(part.label().to_uppercase())
                            .size(12.5)
                            .color(presentation::PAPER),
                    );
                    ui.label(
                        RichText::new(format!("{} · {}", material.name(), part.stat_label()))
                            .size(11.0)
                            .color(accent),
                    );
                });
            });
            ui.label(
                RichText::new(part.blurb())
                    .size(11.0)
                    .color(presentation::MUTED),
            );
            ui.add_space(3.0);
            let value = match part {
                Part::Head => tinkering::head_multiplier(current),
                Part::Binding => tinkering::binding_multiplier(current),
                Part::Grip => tinkering::grip_multiplier(current),
            };
            ui.label(
                RichText::new(format!("Currently ×{value:.2}"))
                    .size(11.0)
                    .color(presentation::PAPER),
            );
            ui.small(material.trait_detail());
            ui.add_space(5.0);
            ui.horizontal_wrapped(|ui| {
                for option in Material::ALL {
                    let selected = option.as_u8() == current;
                    let cost = tinkering::part_cost(part, option);
                    let can_buy = affordable(inventory, cost);
                    let enabled = bench && has_weapon && !selected && can_buy;
                    let mut text = RichText::new(option.name()).size(11.0);
                    if selected {
                        let [r, g, b] = option.color();
                        text = text.color(Color32::from_rgb(r, g, b));
                    }
                    let response = ui
                        .add_enabled(
                            enabled,
                            egui::Button::new(text).min_size(egui::vec2(58.0, 22.0)),
                        )
                        .on_hover_ui(|ui| {
                            ui.horizontal(|ui| {
                                material_swatch(ui, option, 14.0);
                                ui.label(RichText::new(option.name()).strong());
                            });
                            ui.small(option.trait_detail());
                            ui.small(format!(
                                "{}: ×{:.2}",
                                part.stat_label(),
                                match part {
                                    Part::Head => tinkering::head_multiplier(option.as_u8()),
                                    Part::Binding => {
                                        tinkering::binding_multiplier(option.as_u8())
                                    }
                                    Part::Grip => tinkering::grip_multiplier(option.as_u8()),
                                }
                            ));
                            if selected {
                                ui.small("Already fitted.");
                            } else if !bench {
                                ui.small("Requires a nearby workbench.");
                            } else if !has_weapon {
                                ui.small("Equip a melee weapon first.");
                            } else {
                                ui.small(format!("Cost: {}", cost_text(cost)));
                            }
                        });
                    if response.clicked() {
                        *action =
                            UiAction::Factory(Action::FitToolPart(part as u8, option.as_u8()));
                    }
                }
            });
        });
}

fn modifier_card(
    ui: &mut egui::Ui,
    modifier: &tinkering::Modifier,
    installed: bool,
    bench: bool,
    has_weapon: bool,
    inventory: &SurvivalInventory,
    action: &mut UiAction,
) {
    let can_buy = affordable(inventory, modifier.cost);
    let enabled = bench && has_weapon && (installed || can_buy);
    let fill = if installed {
        presentation::JADE.linear_multiply(0.20)
    } else {
        Color32::from_rgba_premultiplied(24, 21, 25, 220)
    };
    let stroke = if installed {
        presentation::JADE
    } else {
        Color32::from_rgba_premultiplied(255, 255, 255, 28)
    };
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .inner_margin(egui::Margin::same(7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let response = ui.add_enabled(
                    enabled,
                    egui::Button::new(
                        RichText::new(if installed {
                            format!("{}  ✓", modifier.name)
                        } else {
                            modifier.name.to_owned()
                        })
                        .color(if installed {
                            presentation::JADE
                        } else {
                            presentation::PAPER
                        }),
                    )
                    .min_size(egui::vec2(78.0, 24.0)),
                );
                if response.clicked() {
                    *action = UiAction::Factory(Action::ToggleToolModifier(modifier.bit));
                }
                let hint = if installed {
                    "Installed. Click to remove (free)."
                } else if !bench {
                    "Requires a nearby workbench."
                } else if !has_weapon {
                    "Equip a melee weapon first."
                } else if !can_buy {
                    "Missing materials."
                } else {
                    "Install this modifier."
                };
                response.on_hover_ui(|ui| {
                    ui.label(RichText::new(modifier.detail).size(11.5));
                    ui.small(hint);
                    if !installed {
                        ui.small(format!("Cost: {}", cost_text(modifier.cost)));
                    }
                });
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(modifier.detail)
                            .size(11.0)
                            .color(presentation::MUTED),
                    );
                    if !installed {
                        ui.label(RichText::new(cost_text(modifier.cost)).size(11.0).color(
                            if can_buy {
                                presentation::MUTED
                            } else {
                                presentation::DANGER
                            },
                        ));
                    }
                });
            });
        });
}

/// Renders the complete forge page. `bench` comes from the factory view and is
/// the same proximity check used by every other workbench recipe.
pub(super) fn draw(
    ui: &mut egui::Ui,
    view: &crate::world::adventure::View,
    inventory: &SurvivalInventory,
    bench: bool,
    action: &mut UiAction,
) {
    let has_weapon = view.weapon != 0;
    let tool_name = crate::world::adventure::tinkering::display_name(
        crate::world::adventure::sword_name(view.weapon),
        view.tool_head,
        view.tool_bind,
        view.tool_grip,
    );
    let tool_summary = tinkering::stat_summary(view.tool_head, view.tool_bind, view.tool_grip);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("The Tinker's Forge")
                .family(egui::FontFamily::Name("PixelAccent".into()))
                .size(20.0)
                .color(icons::GOLD)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (text, color) = if bench {
                ("Workbench in reach", presentation::JADE)
            } else {
                ("Find a workbench", presentation::AMBER)
            };
            ui.label(RichText::new(text).size(11.0).color(color));
        });
    });
    ui.label(
        RichText::new(
        "Choose a head, binding, grip and modifiers.",
        )
        .size(11.0)
        .color(presentation::MUTED),
    );

    egui::Frame::new()
        .fill(Color32::from_rgba_premultiplied(18, 16, 20, 235))
        .stroke(egui::Stroke::new(1.0, presentation::EDGE))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(58.0, 58.0), egui::Sense::hover());
                icons::frame(ui.painter(), rect, false);
                icons::paint(
                    ui.painter(),
                    rect.shrink(6.0),
                    icons::Icon::Sword(view.weapon.max(1)),
                );
                ui.vertical(|ui| {
                    let name = if has_weapon {
                        tool_name.as_str()
                    } else {
                        "No blade equipped"
                    };
                    ui.label(RichText::new(name).size(16.0).color(presentation::PAPER));
                    if has_weapon {
                        ui.label(
                            RichText::new(&tool_summary)
                                .size(11.0)
                                .color(presentation::MUTED),
                        );
                    } else {
                        ui.label(
                            RichText::new("Recover or craft a blade, then equip it in Gear.")
                                .size(11.0)
                                .color(presentation::AMBER),
                        );
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (name, detail) in
                            tinkering::traits(view.tool_head, view.tool_bind, view.tool_grip)
                        {
                            egui::Frame::new()
                                .fill(Color32::from_rgba_premultiplied(44, 38, 36, 230))
                                .stroke(egui::Stroke::new(1.0, presentation::EDGE))
                                .inner_margin(egui::Margin::symmetric(5, 1))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(name).size(11.0).color(icons::GOLD))
                                        .on_hover_text(detail);
                                });
                        }
                    });
                });
                if ui.available_width() > 120.0 {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                        stat_bar(
                            ui,
                            "Damage",
                            tinkering::head_multiplier(view.tool_head),
                            icons::GOLD,
                        );
                        stat_bar(
                            ui,
                            "Sweep",
                            tinkering::binding_multiplier(view.tool_bind),
                            presentation::JADE,
                        );
                        stat_bar(
                            ui,
                            "Swing",
                            tinkering::grip_multiplier(view.tool_grip),
                            presentation::MANA,
                        );
                    });
                }
            });
        });

    ui.add_space(6.0);
    ui.label(
        RichText::new("FITTINGS")
            .size(11.0)
            .color(presentation::AMBER),
    );
    if ui.available_width() >= 560.0 {
        ui.columns(3, |columns| {
            for (index, part) in Part::ALL.into_iter().enumerate() {
                let current = match part {
                    Part::Head => view.tool_head,
                    Part::Binding => view.tool_bind,
                    Part::Grip => view.tool_grip,
                };
                part_card(
                    &mut columns[index],
                    part,
                    current,
                    bench,
                    has_weapon,
                    inventory,
                    action,
                );
            }
        });
    } else {
        for part in Part::ALL {
            let current = match part {
                Part::Head => view.tool_head,
                Part::Binding => view.tool_bind,
                Part::Grip => view.tool_grip,
            };
            part_card(ui, part, current, bench, has_weapon, inventory, action);
            ui.add_space(4.0);
        }
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("MODIFIERS & ENCHANTMENTS")
                .size(11.0)
                .color(presentation::AMBER),
        );
        if view.tool_mods != 0 {
            ui.label(
                RichText::new(format!("{} installed", view.tool_mods.count_ones()))
                    .size(11.0)
                    .color(presentation::JADE),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    bench && has_weapon,
                    egui::Button::new(RichText::new("Strip fittings").size(11.0)),
                )
                .on_hover_text("Return all three parts to copper. Modifiers are kept.")
                .clicked()
            {
                *action = UiAction::Factory(Action::ClearToolParts);
            }
        });
    });
    if ui.available_width() >= 430.0 {
        ui.columns(2, |columns| {
            for (index, modifier) in tinkering::MODIFIERS.iter().enumerate() {
                modifier_card(
                    &mut columns[index % 2],
                    modifier,
                    view.tool_mods & modifier.bit != 0,
                    bench,
                    has_weapon,
                    inventory,
                    action,
                );
                if index % 2 == 1 {
                    columns[0].add_space(4.0);
                    columns[1].add_space(4.0);
                }
            }
        });
    } else {
        for modifier in tinkering::MODIFIERS.iter() {
            modifier_card(
                ui,
                modifier,
                view.tool_mods & modifier.bit != 0,
                bench,
                has_weapon,
                inventory,
                action,
            );
            ui.add_space(4.0);
        }
    }
}
