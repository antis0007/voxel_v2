//! Inventory navigation, Minecraft-style survival inventory screen (E), 2×2 hand-crafting grid,
//! HWYLA/modded-style compact recipe & tooltip QOL, and separate creative block catalogue.
use super::*;
use crate::world::inventory::{
    CRAFT_SLOTS, CraftingRecipe, EQUIP_SLOTS, EquipSlot, HAND_RECIPES, HOTBAR_SLOTS, Item,
    ItemStack, PACK_SLOTS, SlotAddr, SurvivalInventory,
};

fn category(block: BlockId) -> &'static str {
    let name = block.display_name().to_lowercase();
    if block.is_torch() || name.contains("lumen") || name.contains("glow") {
        "Lighting"
    } else if crate::world::factory::Kind::from_block(block).is_some() {
        "Machines"
    } else if ["leaves", "log", "bush", "flower", "mushroom", "vine"]
        .iter()
        .any(|s| name.contains(s))
    {
        "Nature"
    } else if ["brick", "plank", "glass", "tile", "bench"]
        .iter()
        .any(|s| name.contains(s))
    {
        "Building"
    } else {
        "Terrain"
    }
}

pub(super) fn draw_item_icon(painter: &egui::Painter, rect: egui::Rect, item: Item) {
    match item {
        Item::Block(block) => draw_voxel_icon(painter, rect, block),
        Item::Resource(i) => icons::paint(painter, rect, icons::Icon::Resource(i as usize)),
        Item::Food(f) => icons::paint(painter, rect, icons::Icon::Food(f as u8)),
        Item::Weapon(tier) => icons::paint(painter, rect, icons::Icon::Sword(tier as u32)),
        Item::Armor(slot, tier) => {
            icons::paint(painter, rect, icons::Icon::Armor(slot as u8, tier as u8))
        }
        Item::Tool(1 | 2) => icons::paint(painter, rect, icons::Icon::Bore),
        Item::Tool(t) => icons::paint(painter, rect, icons::Icon::Spell((t.saturating_sub(3)) as u32)),
    }
}

fn draw_equipment_silhouette(painter: &egui::Painter, rect: egui::Rect, slot: EquipSlot) {
    let icon = icons::Icon::Armor(slot as u8, 1);
    let r = rect.shrink(rect.width() * 0.22);
    icons::paint(painter, r, icon);
    // Dim overlay so empty equipment slots read as recessed silhouettes
    painter.rect_filled(
        r,
        0,
        egui::Color32::from_rgba_unmultiplied(36, 30, 32, 165),
    );
}

fn draw_player_preview(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    inv: &SurvivalInventory,
    equipment_view: &crate::world::adventure::View,
) {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter();
    icons::frame(p, rect, false);
    let inner = rect.shrink(3.0);
    p.rect_filled(inner, 0, Color32::from_rgb(24, 21, 26));

    // Subtle floor grid line
    let floor_y = inner.bottom() - inner.height() * 0.14;
    p.line_segment(
        [
            egui::pos2(inner.left() + 6.0, floor_y),
            egui::pos2(inner.right() - 6.0, floor_y),
        ],
        egui::Stroke::new(1.0, Color32::from_rgb(66, 56, 48)),
    );

    let cx = inner.center().x;
    let scale = (inner.height() / 96.0).clamp(0.55, 1.25);
    let head = egui::Rect::from_center_size(
        egui::pos2(cx, floor_y - 54.0 * scale),
        egui::vec2(18.0 * scale, 18.0 * scale),
    );
    let torso = egui::Rect::from_center_size(
        egui::pos2(cx, floor_y - 33.0 * scale),
        egui::vec2(20.0 * scale, 22.0 * scale),
    );
    let left_arm = egui::Rect::from_center_size(
        egui::pos2(cx - 14.0 * scale, floor_y - 33.0 * scale),
        egui::vec2(7.0 * scale, 20.0 * scale),
    );
    let right_arm = egui::Rect::from_center_size(
        egui::pos2(cx + 14.0 * scale, floor_y - 33.0 * scale),
        egui::vec2(7.0 * scale, 20.0 * scale),
    );
    let left_leg = egui::Rect::from_center_size(
        egui::pos2(cx - 5.0 * scale, floor_y - 12.0 * scale),
        egui::vec2(8.0 * scale, 20.0 * scale),
    );
    let right_leg = egui::Rect::from_center_size(
        egui::pos2(cx + 5.0 * scale, floor_y - 12.0 * scale),
        egui::vec2(8.0 * scale, 20.0 * scale),
    );

    let armor_color = |slot: EquipSlot, default: Color32| -> Color32 {
        match inv.equipment[slot as usize].map(|s| s.item) {
            Some(Item::Armor(_, crate::world::inventory::ArmorTier::Copper)) => {
                Color32::from_rgb(209, 128, 72)
            }
            Some(Item::Armor(_, crate::world::inventory::ArmorTier::Steel)) => {
                Color32::from_rgb(168, 189, 206)
            }
            Some(Item::Armor(_, crate::world::inventory::ArmorTier::Lumen)) => {
                Color32::from_rgb(112, 214, 186)
            }
            _ => default,
        }
    };

    // Legs & Boots
    let leg_col = armor_color(EquipSlot::Legs, Color32::from_rgb(76, 84, 102));
    p.rect_filled(left_leg, 0, leg_col);
    p.rect_filled(right_leg, 0, leg_col);
    if inv.equipment[EquipSlot::Feet as usize].is_some() {
        let boot_col = armor_color(EquipSlot::Feet, Color32::from_rgb(92, 68, 52));
        let h = 6.0 * scale;
        p.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(left_leg.left(), left_leg.bottom() - h),
                left_leg.right_bottom(),
            ),
            0,
            boot_col,
        );
        p.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(right_leg.left(), right_leg.bottom() - h),
                right_leg.right_bottom(),
            ),
            0,
            boot_col,
        );
    }

    // Torso & Arms
    let chest_col = armor_color(EquipSlot::Chest, Color32::from_rgb(98, 128, 136));
    p.rect_filled(left_arm, 0, chest_col.gamma_multiply(0.85));
    p.rect_filled(right_arm, 0, chest_col.gamma_multiply(0.85));
    p.rect_filled(torso, 0, chest_col);

    // Head & Helm
    p.rect_filled(head, 0, Color32::from_rgb(218, 182, 148));
    // Eyes
    let eye_w = (2.5 * scale).max(1.5);
    p.rect_filled(
        egui::Rect::from_center_size(
            egui::pos2(cx - 4.0 * scale, head.center().y),
            egui::vec2(eye_w, eye_w),
        ),
        0,
        presentation::INK,
    );
    p.rect_filled(
        egui::Rect::from_center_size(
            egui::pos2(cx + 4.0 * scale, head.center().y),
            egui::vec2(eye_w, eye_w),
        ),
        0,
        presentation::INK,
    );
    if inv.equipment[EquipSlot::Head as usize].is_some() {
        let helm_col = armor_color(EquipSlot::Head, Color32::from_rgb(209, 128, 72));
        let visor = egui::Rect::from_min_max(
            head.left_top() - egui::vec2(1.0, 1.0),
            egui::pos2(head.right() + 1.0, head.top() + head.height() * 0.48),
        );
        p.rect_filled(visor, 0, helm_col);
    }

    // Compact stat footer inside character box
    let def_pct = (inv.armor_reduction() * 100.0).round() as u32;
    p.text(
        inner.left_bottom() + egui::vec2(4.0, -2.0),
        Align2::LEFT_BOTTOM,
        format!("DEF {def_pct}%"),
        egui::FontId::monospace(10.5),
        if def_pct > 0 {
            presentation::JADE
        } else {
            presentation::MUTED
        },
    );
    resp.on_hover_ui(|ui| {
        ui.label(
            RichText::new(format!(
                "HP {:.0}  ·  Hunger {:.0}%  ·  Armor {}% reduction",
                equipment_view.health, equipment_view.hunger, def_pct
            ))
            .size(11.5)
            .color(presentation::PAPER),
        );
        if equipment_view.wand > 0 {
            ui.separator();
            super::wands::show_wand_hover_card(ui, equipment_view);
        }
    });
}

fn show_item_tooltip(resp: &egui::Response, stack: ItemStack, inv: &SurvivalInventory) {
    resp.clone().on_hover_ui(|ui| {
        ui.set_max_width(230.0);
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);
        ui.horizontal(|ui| {
            ui.strong(
                RichText::new(stack.item.name())
                    .color(presentation::PAPER)
                    .size(13.5),
            );
            if inv.stack_limit(stack.item) > 1 {
                ui.weak(format!("({}/{})", stack.count, inv.stack_limit(stack.item)));
            }
        });
        ui.label(
            RichText::new(stack.item.category())
                .size(11.0)
                .color(presentation::AMBER),
        );
        ui.label(
            RichText::new(stack.item.description())
                .size(11.5)
                .color(presentation::MUTED),
        );
        let total = inv.count_item(stack.item);
        if total > stack.count {
            ui.small(format!("Owned: {total}"));
        }
    });
}

fn draw_survival_slot(
    ui: &mut egui::Ui,
    inv: &mut SurvivalInventory,
    addr: SlotAddr,
    slot_px: f32,
    selected: bool,
    hovered_slot: &mut Option<SlotAddr>,
    drag_slot: &mut Option<SlotAddr>,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(slot_px, slot_px), egui::Sense::click_and_drag());
    let stack = inv.slot(addr);
    let label = match stack {
        Some(s) => format!("{} × {}", s.item.name(), s.count),
        None => match addr {
            SlotAddr::Equipment(i) => EquipSlot::from_index(i).map_or("Equipment", |e| e.label()).into(),
            SlotAddr::Hotbar(i) => format!("Hotbar {}", i + 1),
            SlotAddr::Pack(i) => format!("Pack {}", i + 1),
            SlotAddr::CraftGrid(i) => format!("Craft {}", i + 1),
        },
    };
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &label)
    });

    // Save rect for deterministic verification harness
    let tag = match addr {
        SlotAddr::Hotbar(i) => ("surv_hotbar", i),
        SlotAddr::Pack(i) => ("surv_pack", i),
        SlotAddr::Equipment(i) => ("surv_equip", i),
        SlotAddr::CraftGrid(i) => ("surv_craft", i),
    };
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new(tag), rect));

    let is_hovered = response.hovered();
    if is_hovered {
        *hovered_slot = Some(addr);
    }

    let painter = ui.painter();
    icons::frame(painter, rect, selected || is_hovered);

    // Hotbar number badge
    if let SlotAddr::Hotbar(slot) = addr {
        painter.text(
            rect.min + egui::vec2(3.0, 1.5),
            Align2::LEFT_TOP,
            hotbar_key_label(slot),
            egui::FontId::monospace((slot_px * 0.26).clamp(9.0, 11.5)),
            if selected {
                presentation::PAPER
            } else {
                presentation::MUTED
            },
        );
    }

    if let Some(stack) = stack {
        let pad = (slot_px * 0.14).clamp(3.0, 6.0);
        draw_item_icon(painter, rect.shrink(pad), stack.item);
        if stack.count > 1 {
            let count_text = format!("{}", stack.count);
            let font = egui::FontId::monospace((slot_px * 0.30).clamp(9.5, 12.0));
            let pos = rect.right_bottom() - egui::vec2(2.5, 1.5);
            painter.text(
                pos + egui::vec2(1.0, 1.0),
                Align2::RIGHT_BOTTOM,
                &count_text,
                font.clone(),
                presentation::INK,
            );
            painter.text(pos, Align2::RIGHT_BOTTOM, count_text, font, presentation::PAPER);
        }
        show_item_tooltip(&response, stack, inv);
    } else if let SlotAddr::Equipment(i) = addr {
        if let Some(eq) = EquipSlot::from_index(i) {
            draw_equipment_silhouette(painter, rect, eq);
            response.clone().on_hover_text(eq.hint());
        }
    }

    // Interactions:
    // - Shift+Click quick transfer
    // - Double-click gather loose partial stacks onto cursor
    // - Left-click pickup/place/merge/swap
    // - Right-click split/place-one
    // - Mouse Tweaks: Shift+Left-Drag sweep transfer, Right-Drag 1-per-slot paint, Scroll wheel push/pull
    let shift = ui.ctx().input(|i| i.modifiers.shift);
    let rmb_down = ui.ctx().input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
    let lmb_down = ui.ctx().input(|i| i.pointer.button_down(egui::PointerButton::Primary));
    let rdrag_id = egui::Id::new("surv_rmb_drag_last");
    let sdrag_id = egui::Id::new("surv_shift_drag_last");

    if response.double_clicked() && !shift {
        inv.gather_to_cursor(addr);
    } else if response.clicked() {
        if shift {
            inv.shift_click_slot(addr);
        } else {
            inv.click_slot(addr);
        }
    } else if response.secondary_clicked() {
        inv.right_click_slot(addr);
        ui.ctx().data_mut(|d| d.insert_temp(rdrag_id, Some(addr)));
    }

    // Mouse Tweaks: Scroll wheel over slot pushes/pulls 1 item
    if is_hovered {
        let scroll_y = ui.ctx().input(|i| i.smooth_scroll_delta.y);
        if scroll_y.abs() >= 8.0 {
            inv.wheel_slot(addr, if scroll_y > 0.0 { 1 } else { -1 });
        }
    }

    // Mouse Tweaks: Right-click drag across slots deposits 1 item per newly entered slot
    if rmb_down && inv.cursor.is_some() && response.contains_pointer() {
        let last = ui.ctx().data_mut(|d| d.get_temp::<Option<SlotAddr>>(rdrag_id)).flatten();
        if last != Some(addr) {
            inv.right_click_slot(addr);
            ui.ctx().data_mut(|d| d.insert_temp(rdrag_id, Some(addr)));
        }
    } else if !rmb_down {
        ui.ctx().data_mut(|d| d.insert_temp::<Option<SlotAddr>>(rdrag_id, None));
    }

    // Mouse Tweaks: Shift + Left-drag sweeps across slots to quick-transfer
    if shift && lmb_down && inv.cursor.is_none() && response.contains_pointer() {
        let last = ui.ctx().data_mut(|d| d.get_temp::<Option<SlotAddr>>(sdrag_id)).flatten();
        if last != Some(addr) {
            inv.shift_click_slot(addr);
            ui.ctx().data_mut(|d| d.insert_temp(sdrag_id, Some(addr)));
        }
    } else if !lmb_down || !shift {
        ui.ctx().data_mut(|d| d.insert_temp::<Option<SlotAddr>>(sdrag_id, None));
    }

    if response.drag_started() && !shift && inv.cursor.is_none() && inv.slot(addr).is_some() {
        *drag_slot = Some(addr);
        inv.click_slot(addr);
    }
    if *drag_slot != Some(addr)
        && inv.cursor.is_some()
        && response.contains_pointer()
        && ui.ctx().input(|i| i.pointer.button_released(egui::PointerButton::Primary))
    {
        inv.click_slot(addr);
        *drag_slot = None;
    }

    response
}

fn draw_recipe_tooltip(ui: &mut egui::Ui, recipe: &CraftingRecipe, inv: &SurvivalInventory) {
    ui.set_max_width(240.0);
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);
    ui.horizontal(|ui| {
        ui.strong(
            RichText::new(recipe.name)
                .color(presentation::PAPER)
                .size(13.5),
        );
        if recipe.output.count > 1 {
            ui.label(
                RichText::new(format!("×{}", recipe.output.count))
                    .color(presentation::AMBER)
                    .monospace(),
            );
        }
    });
    ui.label(
        RichText::new(recipe.output.item.description())
            .size(11.5)
            .color(presentation::MUTED),
    );
    ui.separator();
    for &(item, need) in recipe.ingredients {
        let have = inv.count_item(item);
        let ok = have >= need;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
            draw_item_icon(ui.painter(), r, item);
            ui.label(
                RichText::new(item.name())
                    .size(11.5)
                    .color(if ok { presentation::PAPER } else { presentation::MUTED }),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.monospace(
                    RichText::new(format!("{have}/{need}"))
                        .size(11.5)
                        .color(if ok {
                            presentation::JADE
                        } else {
                            presentation::AMBER
                        }),
                );
            });
        });
    }
}

fn draw_survival_pack_view(
    ui: &mut egui::Ui,
    creative: &mut CreativeState,
    player: &mut Player,
    survival: &mut SurvivalInventory,
    equipment_view: &crate::world::adventure::View,
    action: &mut UiAction,
    compact: bool,
    open_wand_tab: &mut bool,
) {
    let avail_w = ui.available_width();
    let screen_h = ui.ctx().content_rect().height();
    let spacing = if compact { 3.0 } else { 4.0 };
    let max_slot_w = ((avail_w - spacing * 8.0 - 8.0) / 9.0).floor();
    // 2 rows in compact top section + 3 pack rows + 1 hotbar row = 6 slot rows plus header/margins
    let max_slot_h = ((screen_h - 126.0) / 6.4).floor().max(20.0);
    let slot_px = max_slot_w
        .min(max_slot_h)
        .clamp(20.0, if compact { 30.0 } else { 40.0 });
    let grid_w = slot_px * 9.0 + spacing * 8.0;

    let mut hovered_slot: Option<SlotAddr> = None;
    let drag_id = egui::Id::new("survival_drag_origin");
    let ghost_id = egui::Id::new("survival_ghost_recipe");
    let craft_only_id = egui::Id::new("survival_craft_only_filter");
    let mut drag_slot = ui.ctx().data_mut(|d| d.get_temp::<Option<SlotAddr>>(drag_id)).flatten();
    let mut ghost_recipe = ui.ctx().data_mut(|d| d.get_temp::<Option<usize>>(ghost_id)).flatten();
    let mut craftable_only = ui.ctx().data_mut(|d| d.get_temp::<bool>(craft_only_id)).unwrap_or(false);
    if survival.craft_grid.iter().any(|s| s.is_some()) {
        ghost_recipe = None;
    }

    ui.vertical_centered(|ui| {
        ui.set_width(grid_w);
        ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);

        // Noita-style inline Wand Deck bar right at the top of the Survival Inventory
        if equipment_view.wand > 0 {
            super::wands::draw_inline_wand_bar(ui, equipment_view, action, open_wand_tab);
        }

        // Top section: Armor + Player preview + 2×2 Crafting Grid + Output + Quick Recipe Book
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);

            // 4 Armor slots (2×2 when vertical height is tight, or 1×4 column)
            if compact {
                egui::Grid::new("equip_grid_compact")
                    .spacing([spacing, spacing])
                    .show(ui, |ui| {
                        for i in 0..EQUIP_SLOTS {
                            draw_survival_slot(
                                ui,
                                survival,
                                SlotAddr::Equipment(i),
                                slot_px,
                                false,
                                &mut hovered_slot,
                                &mut drag_slot,
                            );
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
            } else {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
                    for i in 0..EQUIP_SLOTS {
                        draw_survival_slot(
                            ui,
                            survival,
                            SlotAddr::Equipment(i),
                            slot_px * 0.86,
                            false,
                            &mut hovered_slot,
                            &mut drag_slot,
                        );
                    }
                });
            }

            // Player character preview
            let char_h = if compact {
                slot_px * 2.0 + spacing
            } else {
                slot_px * 0.86 * 4.0 + spacing * 3.0
            };
            let char_w = (char_h * 0.68).clamp(52.0, 96.0);
            draw_player_preview(
                ui,
                egui::vec2(char_w, char_h),
                survival,
                equipment_view,
            );

            ui.add_space(spacing);

            // 2×2 Hand-Crafting Grid + Output Slot
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Craft")
                            .size(11.5)
                            .color(presentation::MUTED),
                    );
                    if (survival.craft_grid.iter().any(|s| s.is_some()) || ghost_recipe.is_some())
                        && ui
                            .small_button("×")
                            .on_hover_text("Return grid items to pack / clear ghost preview")
                            .clicked()
                    {
                        ghost_recipe = None;
                        for i in 0..CRAFT_SLOTS {
                            let _ = survival.shift_click_slot(SlotAddr::CraftGrid(i));
                        }
                    }
                });

                let mut craft_rects = [egui::Rect::NOTHING; CRAFT_SLOTS];
                ui.horizontal_top(|ui| {
                    egui::Grid::new("hand_craft_2x2")
                        .spacing([spacing, spacing])
                        .show(ui, |ui| {
                            for i in 0..CRAFT_SLOTS {
                                let resp = draw_survival_slot(
                                    ui,
                                    survival,
                                    SlotAddr::CraftGrid(i),
                                    slot_px,
                                    false,
                                    &mut hovered_slot,
                                    &mut drag_slot,
                                );
                                craft_rects[i] = resp.rect;
                                if i % 2 == 1 {
                                    ui.end_row();
                                }
                            }
                        });

                    // JEI/EMI Ghost Recipe Overlay when an uncraftable recipe was clicked
                    if let Some(rec) = ghost_recipe.and_then(|idx| HAND_RECIPES.get(idx)) {
                        for (slot_i, &(req_item, req_count)) in
                            rec.ingredients.iter().take(CRAFT_SLOTS).enumerate()
                        {
                            let r = craft_rects[slot_i];
                            let have = survival.count_item(req_item);
                            let missing = have < req_count;
                            ui.painter().rect_filled(
                                r.shrink(2.0),
                                0,
                                if missing {
                                    Color32::from_rgba_unmultiplied(185, 42, 46, 95)
                                } else {
                                    Color32::from_rgba_unmultiplied(82, 168, 112, 65)
                                },
                            );
                            draw_item_icon(ui.painter(), r.shrink(5.0), req_item);
                            ui.painter().text(
                                r.right_bottom() - egui::vec2(2.0, 1.5),
                                Align2::RIGHT_BOTTOM,
                                format!("{have}/{req_count}"),
                                egui::FontId::monospace(9.0),
                                if missing {
                                    presentation::DANGER
                                } else {
                                    presentation::JADE
                                },
                            );
                        }
                    }

                    // Arrow + Output Slot
                    ui.vertical(|ui| {
                        ui.add_space(slot_px * 0.55);
                        ui.label(
                            RichText::new("→")
                                .size(16.0)
                                .color(presentation::MUTED),
                        );
                    });

                    let matched = survival.matching_grid_recipe();
                    let out_px = (slot_px * 1.12).round();
                    let (out_rect, out_resp) = ui.allocate_exact_size(
                        egui::vec2(out_px, out_px),
                        egui::Sense::click(),
                    );
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(egui::Id::new("surv_craft_output"), out_rect);
                    });
                    icons::frame(
                        ui.painter(),
                        out_rect,
                        matched.is_some() || out_resp.hovered(),
                    );
                    if let Some((recipe, batches)) = matched {
                        let preview = ItemStack {
                            item: recipe.output.item,
                            count: recipe.output.count,
                        };
                        draw_item_icon(ui.painter(), out_rect.shrink(5.0), preview.item);
                        if preview.count > 1 {
                            ui.painter().text(
                                out_rect.right_bottom() - egui::vec2(3.0, 2.0),
                                Align2::RIGHT_BOTTOM,
                                format!("{}", preview.count),
                                egui::FontId::monospace(11.5),
                                presentation::PAPER,
                            );
                        }
                        out_resp.clone().on_hover_ui(|ui| {
                            draw_recipe_tooltip(ui, recipe, survival);
                            ui.small(format!("Batches ready: {batches}  ·  Shift-click crafts all"));
                        });
                        if out_resp.clicked() {
                            let shift = ui.ctx().input(|i| i.modifiers.shift);
                            let _ = survival.craft_from_grid(shift);
                        }
                    } else if let Some(rec) = ghost_recipe.and_then(|idx| HAND_RECIPES.get(idx)) {
                        draw_item_icon(ui.painter(), out_rect.shrink(5.0), rec.output.item);
                        ui.painter().rect_filled(
                            out_rect.shrink(2.0),
                            0,
                            Color32::from_rgba_unmultiplied(185, 42, 46, 80),
                        );
                        out_resp.on_hover_ui(|ui| {
                            draw_recipe_tooltip(ui, rec, survival);
                            ui.small("Missing ingredients highlighted in red");
                        });
                    } else {
                        out_resp.on_hover_text("2×2 Crafting Output · Click any recipe on the right to stage or preview");
                    }
                });
            });

            ui.add_space(spacing);

            // Modded/JEI-style compact recipe grid: icons with craftable/missing border + ghost staging
            let rem_w = ui.available_width().max(slot_px * 2.0);
            let rec_px = (slot_px * 0.84).clamp(22.0, 30.0);
            let rec_cols = ((rem_w + 3.0) / (rec_px + 3.0)).floor().max(2.0) as usize;
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Recipes")
                            .size(11.5)
                            .color(presentation::MUTED),
                    );
                    if ui
                        .selectable_label(craftable_only, "Ready")
                        .on_hover_text("Toggle showing only craftable recipes")
                        .clicked()
                    {
                        craftable_only = !craftable_only;
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt("quick_recipes_scroll")
                    .max_height(char_h - 16.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("quick_recipe_grid")
                            .spacing([3.0, 3.0])
                            .show(ui, |ui| {
                                let mut col_count = 0usize;
                                for (idx, recipe) in HAND_RECIPES.iter().enumerate() {
                                    let craftable = recipe
                                        .ingredients
                                        .iter()
                                        .all(|&(it, n)| survival.count_item(it) >= n);
                                    if craftable_only && !craftable {
                                        continue;
                                    }
                                    let (r, resp) = ui.allocate_exact_size(
                                        egui::vec2(rec_px, rec_px),
                                        egui::Sense::click(),
                                    );
                                    let p = ui.painter();
                                    p.rect_filled(
                                        r,
                                        0,
                                        if craftable {
                                            Color32::from_rgb(44, 52, 42)
                                        } else {
                                            Color32::from_rgb(36, 30, 32)
                                        },
                                    );
                                    p.rect_stroke(
                                        r,
                                        0,
                                        egui::Stroke::new(
                                            1.0,
                                            if resp.hovered() || ghost_recipe == Some(idx) {
                                                presentation::AMBER
                                            } else if craftable {
                                                presentation::JADE
                                            } else {
                                                Color32::from_rgb(76, 58, 56)
                                            },
                                        ),
                                        egui::StrokeKind::Inside,
                                    );
                                    draw_item_icon(p, r.shrink(3.0), recipe.output.item);
                                    if recipe.output.count > 1 {
                                        p.text(
                                            r.right_bottom() - egui::vec2(1.5, 1.0),
                                            Align2::RIGHT_BOTTOM,
                                            format!("{}", recipe.output.count),
                                            egui::FontId::monospace(9.0),
                                            if craftable {
                                                presentation::PAPER
                                            } else {
                                                presentation::MUTED
                                            },
                                        );
                                    }
                                    resp.clone().on_hover_ui(|ui| {
                                        draw_recipe_tooltip(ui, recipe, survival);
                                    });
                                    if resp.clicked() {
                                        if craftable {
                                            ghost_recipe = None;
                                            let shift = ui.ctx().input(|i| i.modifiers.shift);
                                            if survival.stage_recipe_into_grid(recipe) && shift {
                                                let _ = survival.craft_from_grid(true);
                                            }
                                        } else {
                                            // Stage JEI-style ghost recipe overlay
                                            for i in 0..CRAFT_SLOTS {
                                                let _ = survival.shift_click_slot(SlotAddr::CraftGrid(i));
                                            }
                                            ghost_recipe = if ghost_recipe == Some(idx) {
                                                None
                                            } else {
                                                Some(idx)
                                            };
                                        }
                                    }
                                    col_count += 1;
                                    if col_count % rec_cols == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
            });
        });

        // Inventory Profiles Next QOL strip: Pack label + Sort + Quick-Stack
        ui.horizontal(|ui| {
            ui.set_width(grid_w);
            ui.label(
                RichText::new("Backpack")
                    .size(11.0)
                    .color(presentation::MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Stack ▾")
                    .on_hover_text("Merge matching backpack items into hotbar stacks")
                    .clicked()
                {
                    survival.quick_stack_to_hotbar();
                }
                if ui
                    .small_button("Sort [R]")
                    .on_hover_text("Compact partial stacks and sort backpack by category [Middle-click / R]")
                    .clicked()
                    || ui.ctx().input(|i| i.key_pressed(egui::Key::R) || i.pointer.button_clicked(egui::PointerButton::Middle))
                {
                    survival.sort_pack();
                }
            });
        });

        // 27-Slot Main Pack (3 rows × 9 columns)
        egui::Grid::new("survival_pack_27")
            .spacing([spacing, spacing])
            .show(ui, |ui| {
                for idx in 0..PACK_SLOTS {
                    draw_survival_slot(
                        ui,
                        survival,
                        SlotAddr::Pack(idx),
                        slot_px,
                        false,
                        &mut hovered_slot,
                        &mut drag_slot,
                    );
                    if (idx + 1) % 9 == 0 {
                        ui.end_row();
                    }
                }
            });

        ui.add_space(if compact { 3.0 } else { 6.0 });

        // 9-Slot Survival Hotbar
        egui::Grid::new("survival_hotbar_9")
            .spacing([spacing, spacing])
            .show(ui, |ui| {
                for slot in 0..HOTBAR_SLOTS {
                    let selected = slot == creative.selected_slot;
                    let resp = draw_survival_slot(
                        ui,
                        survival,
                        SlotAddr::Hotbar(slot),
                        slot_px,
                        selected,
                        &mut hovered_slot,
                        &mut drag_slot,
                    );
                    if resp.double_clicked() {
                        if let Some(ItemStack {
                            item: Item::Food(_),
                            ..
                        }) = survival.hotbar[slot]
                        {
                            creative.select_slot(slot);
                            *action = UiAction::Factory(crate::world::factory::Action::EatRation);
                        }
                    }
                }
                ui.end_row();
            });
    });

    // Number keys 1–9 while hovering any slot swap that slot with hotbar[0..9],
    // and pressing Q while hovering a slot stages that slot to cursor and drops it immediately!
    if let Some(addr) = hovered_slot {
        let keys = [
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
            egui::Key::Num4,
            egui::Key::Num5,
            egui::Key::Num6,
            egui::Key::Num7,
            egui::Key::Num8,
            egui::Key::Num9,
        ];
        for (idx, key) in keys.into_iter().enumerate() {
            if ui.ctx().input(|i| i.key_pressed(key)) {
                survival.swap_slot_with_hotbar(addr, idx);
            }
        }
        if ui.ctx().input(|i| i.key_pressed(egui::Key::Q)) && survival.cursor.is_none() {
            if let Some(target) = survival.slot_mut(addr) {
                if let Some(st) = target.take() {
                    survival.cursor = Some(st);
                    *action = UiAction::DropInventory(creative.selected_slot);
                }
            }
        }
    }

    if ui.ctx().input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
        drag_slot = None;
    }
    ui.ctx().data_mut(|d| {
        d.insert_temp(drag_id, drag_slot);
        d.insert_temp(ghost_id, ghost_recipe);
        d.insert_temp(craft_only_id, craftable_only);
    });

    // Sync selected hotbar block/weapon when player clicks or rearranges hotbar
    if let Some(stack) = survival.hotbar[creative.selected_slot] {
        match stack.item {
            Item::Block(b) => player.selected_block = b,
            Item::Weapon(tier) => {
                if equipment_view.weapon != tier as u32 {
                    *action =
                        UiAction::Factory(crate::world::factory::Action::SelectSword(tier as u32));
                }
            }
            _ => {}
        }
    }

    // Floating cursor item preview when holding a stack in survival inventory
    if let Some(cursor) = survival.cursor {
        if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
            let p = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("survival_cursor_item"),
            ));
            let r = egui::Rect::from_center_size(pos + egui::vec2(10.0, 10.0), egui::vec2(30.0, 30.0));
            draw_item_icon(&p, r, cursor.item);
            if cursor.count > 1 {
                p.text(
                    r.right_bottom(),
                    Align2::RIGHT_BOTTOM,
                    format!("{}", cursor.count),
                    egui::FontId::monospace(11.5),
                    presentation::PAPER,
                );
            }
        }
    }
}

pub(super) fn draw(
    ctx: &egui::Context,
    creative: &mut CreativeState,
    player: &mut Player,
    survival: &mut SurvivalInventory,
    drag: &mut Option<DragSource>,
    drop_slot: &mut Option<usize>,
    hovered: &mut Option<BlockId>,
    expedition: Option<&crate::world::expedition::Expedition>,
    machines: &crate::world::factory::View,
    equipment: &crate::world::adventure::View,
    action: &mut UiAction,
) {
    *hovered = None;
    let page_id = egui::Id::new("inventory_page");
    let mut page = ctx.data_mut(|d| d.get_temp::<u32>(page_id).unwrap_or(2));
    let category_id = egui::Id::new("inventory_category");
    let mut group = ctx
        .data_mut(|d| d.get_temp::<String>(category_id).unwrap_or_else(|| "All".into()));
    let screen = ctx.content_rect();
    let compact = screen.height() < 560.0 || screen.width() < 680.0;

    egui::Window::new("Inventory")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width((screen.width() - 32.0).clamp(240.0, 620.0))
        .min_width(220.0)
        .max_width((screen.width() - 20.0).max(220.0))
        .max_height((screen.height() - 16.0).max(200.0))
        .show(ctx, |ui| {
            if compact {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);
                ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
                ui.spacing_mut().interact_size.y = 20.0;
                for (kind, size) in [
                    (egui::TextStyle::Heading, 16.0),
                    (egui::TextStyle::Body, 12.0),
                    (egui::TextStyle::Button, 12.0),
                    (egui::TextStyle::Small, 10.5),
                ] {
                    ui.style_mut()
                        .text_styles
                        .insert(kind, egui::FontId::proportional(size));
                }
            }

            // Minimal header bar: Mode tabs on the left, Close on the right
            ui.horizontal_wrapped(|ui| {
                for (id, label) in [
                    (2, "Inventory"),
                    (0, "Workbench"),
                    (1, "Relics"),
                    (3, "Machines"),
                    (4, "Creative"),
                ] {
                    let response = if compact {
                        ui.selectable_label(page == id, label)
                    } else {
                        front_end::tab(ui, label, page == id)
                    };
                    if response.clicked() {
                        page = id;
                        *drag = None;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("×").on_hover_text("Close [E / Esc]").clicked() {
                        survival.return_transient_to_pack();
                        creative.show_palette = false;
                        *drag = None;
                    }
                    if page == 2 {
                        if ui.ctx().input(|i| i.key_pressed(egui::Key::Q)) {
                            *action = UiAction::DropInventory(creative.selected_slot);
                        }
                        let held = survival.cursor.or(survival.hotbar[creative.selected_slot]);
                        if ui
                            .add_enabled(held.is_some(), egui::Button::new("Drop"))
                            .on_hover_text("[Q] Drop held stack · [F] Pick up nearby drops")
                            .clicked()
                        {
                            *action = UiAction::DropInventory(creative.selected_slot);
                        }
                    }
                    if page == 4 && ui.button("Materials").clicked() {
                        creative.show_material_editor = true;
                        creative.show_palette = false;
                        *drag = None;
                    }
                });
            });
            ui.separator();

            // Page 2: Minecraft-style Survival Inventory (Armor + Character + 2×2 Craft + 27 Pack + 9 Hotbar)
            if page == 2 {
                let mut open_wand_tab = false;
                draw_survival_pack_view(
                    ui,
                    creative,
                    player,
                    survival,
                    equipment,
                    action,
                    compact,
                    &mut open_wand_tab,
                );
                if open_wand_tab {
                    page = 0;
                }
                return;
            }

            // Page 4: Separate Creative Searchable Block Catalogue (preserves creative hotbar & torch_review)
            if page == 4 {
                ui.horizontal_wrapped(|ui| {
                    for label in ["All", "Terrain", "Building", "Nature", "Lighting", "Machines"] {
                        if ui.selectable_label(group == label, label).clicked() {
                            group = label.into();
                            *drag = None;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut creative.palette_search)
                            .hint_text("Search creative blocks…")
                            .desired_width((ui.available_width() - 56.0).max(80.0)),
                    );
                    if ui.button("Clear").clicked() {
                        creative.palette_search.clear();
                    }
                });
            }

            egui::ScrollArea::vertical()
                .id_salt(("inventory_body", page, &group, &creative.palette_search))
                .max_height(
                    (screen.height()
                        - (ui.cursor().top() - ui.min_rect().top())
                        - if compact { 96.0 } else { 140.0 })
                    .clamp(48.0, 380.0),
                )
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if page != 4 {
                        if let Some(pack) = expedition {
                            factory::draw(ui, machines, pack, equipment, action, page);
                        } else {
                            ui.weak("No supplies loaded.");
                        }
                        return;
                    }
                    let query = creative.palette_search.trim().to_lowercase();
                    let matches: Vec<_> = material_defs()
                        .iter()
                        .filter(|m| {
                            m.tags.selectable
                                && m.display_name.to_lowercase().contains(&query)
                                && (group == "All" || category(m.id) == group)
                        })
                        .collect();
                    let columns = ((ui.available_width() + 8.0) / 104.0).floor().max(1.0) as usize;
                    let width = ((ui.available_width() - 8.0 * (columns - 1) as f32)
                        / columns as f32)
                        .min(120.0);
                    egui::Grid::new("inventory_blocks")
                        .spacing([8.0, 8.0])
                        .show(ui, |ui| {
                            for (index, def) in matches.iter().enumerate() {
                                let response = ui
                                    .push_id(def.id.as_u32(), |ui| {
                                        draw_palette_material_card(
                                            ui,
                                            [width, if compact { 80.0 } else { 96.0 }],
                                            def.id,
                                        )
                                    })
                                    .inner;
                                if crate::torch_review::mode() == "drag" && def.id == BlockId::Torch
                                {
                                    ui.ctx().data_mut(|d| {
                                        d.insert_temp(
                                            egui::Id::new("review_torch_card"),
                                            response.rect,
                                        )
                                    });
                                }
                                if creative.selected_block() == def.id {
                                    ui.painter().rect_stroke(
                                        response.rect,
                                        0,
                                        egui::Stroke::new(2.0, presentation::JADE),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                if response.clicked() {
                                    creative.assign_hotbar_slot(creative.selected_slot, def.id);
                                    creative.tool = ToolKind::Hand;
                                    player.selected_block = def.id;
                                }
                                if response.hovered() {
                                    *hovered = Some(def.id);
                                }
                                if response.drag_started() {
                                    *drag = Some(DragSource::Palette(def.id));
                                }
                                if (index + 1) % columns == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                });

            if page == 4 {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    draw_hotbar_row(ui, creative, player, None, drag, drop_slot);
                });
            }
        });
    ctx.data_mut(|d| d.insert_temp(page_id, page));
    ctx.data_mut(|d| d.insert_temp(category_id, group));
}
