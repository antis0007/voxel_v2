//! Inventory navigation, Minecraft-style survival inventory screen (E), 2×2 hand-crafting grid,
//! HWYLA/modded-style compact recipe & tooltip QOL, and separate creative block catalogue.
use super::*;
use crate::world::inventory::{
    ArmorTier, CRAFT_SLOTS, CraftingRecipe, EQUIP_SLOTS, EquipSlot, HAND_RECIPES, HOTBAR_SLOTS,
    Item, ItemStack, PACK_SLOTS, PointerRelease, SlotAddr, SurvivalInventory, pointer_release,
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

/// Everything the creative item catalogue can grant. Gear is single-stack;
/// materials and provisions use their normal limits.
fn creative_items() -> &'static [Item] {
    static ITEMS: std::sync::OnceLock<Vec<Item>> = std::sync::OnceLock::new();
    ITEMS.get_or_init(|| {
        let mut items = Vec::with_capacity(44);
        for resource in 0..crate::world::factory::RESOURCE_COUNT as u8 {
            items.push(Item::Resource(resource));
        }
        for tier in 1..=5 {
            items.push(Item::Weapon(tier));
        }
        for tier in [ArmorTier::Copper, ArmorTier::Steel] {
            for slot in EquipSlot::ALL {
                items.push(Item::Armor(slot, tier));
            }
        }
        items.push(Item::Armor(EquipSlot::Head, ArmorTier::Lumen));
        for tool in 1..=5 {
            items.push(Item::Tool(tool));
        }
        for food in [
            crate::world::inventory::FoodKind::FungalRation,
            crate::world::inventory::FoodKind::RoastedCap,
            crate::world::inventory::FoodKind::Trailcake,
        ] {
            items.push(Item::Food(food));
        }
        items
    })
}

fn creative_item_group(item: Item) -> &'static str {
    match item {
        Item::Resource(_) => "Materials",
        Item::Weapon(_) => "Weapons",
        Item::Armor(_, _) => "Armour",
        Item::Tool(_) => "Tools",
        Item::Food(_) => "Food",
        Item::Block(_) => "Blocks",
    }
}

/// Case-insensitive substring test without allocating a lowercase copy of every
/// catalogue entry on every frame. Non-ASCII probes fall back to the allocating
/// path so Unicode names still search correctly.
fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if !needle.is_ascii() {
        return haystack.to_lowercase().contains(needle);
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn short_name(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        return name.to_owned();
    }
    let mut out: String = name.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn draw_creative_item_card(ui: &mut egui::Ui, size: [f32; 2], item: Item) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(size[0], size[1]), egui::Sense::click());
    let p = ui.painter();
    icons::frame(p, rect, false);
    let icon = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + rect.height() * 0.40),
        egui::vec2(32.0, 32.0),
    );
    draw_item_icon(p, icon, item);
    p.text(
        egui::pos2(rect.center().x, rect.bottom() - 4.0),
        Align2::CENTER_BOTTOM,
        short_name(item.name(), 15),
        egui::FontId::proportional(10.0),
        presentation::PAPER,
    );
    p.text(
        rect.left_top() + egui::vec2(4.0, 3.0),
        Align2::LEFT_TOP,
        creative_item_group(item),
        egui::FontId::monospace(8.5),
        presentation::MUTED,
    );
    if response.hovered() {
        p.rect_stroke(
            rect,
            0,
            egui::Stroke::new(1.0, presentation::AMBER),
            egui::StrokeKind::Inside,
        );
    }
    response
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
        Item::Tool(t) => icons::paint(
            painter,
            rect,
            icons::Icon::Spell((t.saturating_sub(3)) as u32),
        ),
    }
}

fn draw_equipment_silhouette(painter: &egui::Painter, rect: egui::Rect, slot: EquipSlot) {
    let icon = icons::Icon::Armor(slot as u8, 1);
    let r = rect.shrink(rect.width() * 0.22);
    icons::paint(painter, r, icon);
    // Dim overlay so empty equipment slots read as recessed silhouettes
    painter.rect_filled(r, 0, egui::Color32::from_rgba_unmultiplied(36, 30, 32, 165));
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
        ui.separator();
        ui.small("LMB: Pick/Place · RMB: Split/Place 1 · Shift+Click: Transfer");
        ui.small("Ctrl+Click: Transfer All · Double-Click: Gather");
        ui.small("Drag outside this panel: Throw stack · Q / Ctrl+Q: Drop 1 / All");
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
    lmb_split_visited: &mut Vec<SlotAddr>,
    lmb_split_origin: &mut Option<ItemStack>,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(slot_px, slot_px), egui::Sense::click_and_drag());
    let stack = inv.slot(addr);
    let label = match stack {
        Some(s) => format!("{} × {}", s.item.name(), s.count),
        None => match addr {
            SlotAddr::Equipment(i) => EquipSlot::from_index(i)
                .map_or("Equipment", |e| e.label())
                .into(),
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

    // Highlight matching item stacks when hovering a slot or holding a cursor item
    let match_item = inv.cursor.map(|c| c.item).or_else(|| {
        ui.ctx()
            .data_mut(|d| d.get_temp::<Option<Item>>(egui::Id::new("surv_hovered_item")))
            .flatten()
    });
    if let (Some(st), Some(target_item)) = (stack, match_item) {
        if st.item == target_item && !is_hovered {
            painter.rect_stroke(
                rect.shrink(1.0),
                0,
                egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(218, 172, 105, 130)),
                egui::StrokeKind::Inside,
            );
        }
    }
    if lmb_split_visited.contains(&addr) {
        painter.rect_filled(
            rect.shrink(2.0),
            0,
            Color32::from_rgba_unmultiplied(211, 183, 125, 65),
        );
    }

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
            painter.text(
                pos,
                Align2::RIGHT_BOTTOM,
                count_text,
                font,
                presentation::PAPER,
            );
        }
        // Subtle stack fill bar along the bottom edge for multi-item stacks
        let max_st = inv.stack_limit(stack.item);
        if max_st > 1 && stack.count > 1 {
            let fill_ratio = (stack.count as f32 / max_st as f32).clamp(0.0, 1.0);
            if fill_ratio >= 0.999 {
                painter.line_segment(
                    [
                        egui::pos2(rect.left() + 2.0, rect.bottom() - 1.5),
                        egui::pos2(rect.right() - 2.0, rect.bottom() - 1.5),
                    ],
                    egui::Stroke::new(1.5, Color32::from_rgba_unmultiplied(218, 172, 105, 140)),
                );
            }
        }
        show_item_tooltip(&response, stack, inv);
    } else if let SlotAddr::Equipment(i) = addr {
        if let Some(eq) = EquipSlot::from_index(i) {
            draw_equipment_silhouette(painter, rect, eq);
            response.clone().on_hover_text(eq.hint());
        }
    }

    // Interactions, matching Minecraft's inventory pointer:
    // - Left press picks the stack up immediately. Releasing on that same slot
    //   leaves it on the cursor; releasing on one other slot places it all.
    // - Left press while already carrying starts a drag. One slot places the
    //   whole stack on release; several slots split it evenly.
    // - A second press on the same slot within 300ms gathers matching stacks.
    // - Shift/Ctrl click still quick-transfers. Right press splits or places one.
    let (shift, ctrl) = ui
        .ctx()
        .input(|i| (i.modifiers.shift, i.modifiers.ctrl || i.modifiers.command));
    let rmb_down = ui
        .ctx()
        .input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
    let lmb_down = ui
        .ctx()
        .input(|i| i.pointer.button_down(egui::PointerButton::Primary));
    let primary_pressed = ui
        .ctx()
        .input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
    let rdrag_id = egui::Id::new("surv_rmb_drag_last");
    let sdrag_id = egui::Id::new("surv_shift_drag_last");
    let over = response.contains_pointer();
    if over {
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("surv_pointer_slot"), Some(addr)));
    }

    if response.middle_clicked() {
        inv.middle_click_slot(addr);
    } else if response.clicked() && (ctrl || shift) {
        if ctrl {
            inv.ctrl_click_slot(addr);
        } else {
            inv.shift_click_slot(addr);
        }
    } else if response.secondary_clicked() {
        inv.right_click_slot(addr);
        ui.ctx().data_mut(|d| d.insert_temp(rdrag_id, Some(addr)));
    }

    if primary_pressed && over && !shift && !ctrl {
        let now = ui.ctx().input(|i| i.time);
        let last_id = egui::Id::new("surv_lmb_last_press");
        let last = ui
            .ctx()
            .data_mut(|d| d.get_temp::<(SlotAddr, f64)>(last_id));
        let double =
            last.is_some_and(|(slot, t)| slot == addr && now - t < 0.30) && inv.cursor.is_some();
        ui.ctx().data_mut(|d| d.insert_temp(last_id, (addr, now)));
        if double {
            inv.gather_to_cursor(addr);
            *drag_slot = None;
            *lmb_split_origin = None;
            lmb_split_visited.clear();
        } else if inv.cursor.is_none() {
            if inv.slot(addr).is_some() {
                inv.click_slot(addr);
                *drag_slot = Some(addr);
                *lmb_split_origin = None;
                lmb_split_visited.clear();
            }
        } else if let Some(cur) = inv.cursor {
            *drag_slot = Some(addr);
            *lmb_split_origin = Some(cur);
            lmb_split_visited.clear();
            if inv.slot_accepts(addr, cur.item) && inv.slot(addr).is_none_or(|s| s.item == cur.item)
            {
                lmb_split_visited.push(addr);
            }
        }
    }

    // Mouse Tweaks: Scroll wheel over slot pushes/pulls 1 item (Shift+Wheel transfers whole stack)
    if is_hovered {
        let scroll_y = ui.ctx().input(|i| i.smooth_scroll_delta.y);
        if scroll_y.abs() >= 8.0 {
            if shift {
                inv.shift_click_slot(addr);
            } else {
                inv.wheel_slot(addr, if scroll_y > 0.0 { 1 } else { -1 });
            }
        }
    }

    // Mouse Tweaks: Right-click drag across slots deposits 1 item per newly entered slot
    if rmb_down && inv.cursor.is_some() && response.contains_pointer() {
        let last = ui
            .ctx()
            .data_mut(|d| d.get_temp::<Option<SlotAddr>>(rdrag_id))
            .flatten();
        if last != Some(addr) {
            inv.right_click_slot(addr);
            ui.ctx().data_mut(|d| d.insert_temp(rdrag_id, Some(addr)));
        }
    } else if !rmb_down {
        ui.ctx()
            .data_mut(|d| d.insert_temp::<Option<SlotAddr>>(rdrag_id, None));
    }

    // Mouse Tweaks: Shift + Left-drag (or Ctrl+Shift+Left-drag) sweeps across slots to quick-transfer
    if shift && lmb_down && inv.cursor.is_none() && response.contains_pointer() {
        let last = ui
            .ctx()
            .data_mut(|d| d.get_temp::<Option<SlotAddr>>(sdrag_id))
            .flatten();
        if last != Some(addr) {
            if ctrl {
                inv.ctrl_click_slot(addr);
            } else {
                inv.shift_click_slot(addr);
            }
            ui.ctx().data_mut(|d| d.insert_temp(sdrag_id, Some(addr)));
        }
    } else if !lmb_down || !shift {
        ui.ctx()
            .data_mut(|d| d.insert_temp::<Option<SlotAddr>>(sdrag_id, None));
    }

    // While a carried stack is being dragged, remember each empty or matching slot.
    // Include the release frame so the slot under the cursor is part of the gesture.
    let lmb_release = ui
        .ctx()
        .input(|i| i.pointer.button_released(egui::PointerButton::Primary));
    if !shift && !ctrl && lmb_split_origin.is_some() && over && (lmb_down || lmb_release) {
        if !lmb_split_visited.contains(&addr) {
            if let Some(orig) = *lmb_split_origin {
                if inv.slot_accepts(addr, orig.item)
                    && inv.slot(addr).is_none_or(|s| s.item == orig.item)
                {
                    lmb_split_visited.push(addr);
                }
            }
        }
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
            ui.label(RichText::new(item.name()).size(11.5).color(if ok {
                presentation::PAPER
            } else {
                presentation::MUTED
            }));
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
    let query_id = egui::Id::new("survival_recipe_query");
    let split_vis_id = egui::Id::new("survival_lmb_split_visited");
    let split_orig_id = egui::Id::new("survival_lmb_split_origin");
    let pointer_slot_id = egui::Id::new("surv_pointer_slot");
    ui.ctx()
        .data_mut(|d| d.insert_temp(pointer_slot_id, None::<SlotAddr>));
    let mut drag_slot = ui
        .ctx()
        .data_mut(|d| d.get_temp::<Option<SlotAddr>>(drag_id))
        .flatten();
    let mut ghost_recipe = ui
        .ctx()
        .data_mut(|d| d.get_temp::<Option<usize>>(ghost_id))
        .flatten();
    let mut craftable_only = ui
        .ctx()
        .data_mut(|d| d.get_temp::<bool>(craft_only_id))
        .unwrap_or(false);
    let mut recipe_query = ui
        .ctx()
        .data_mut(|d| d.get_temp::<String>(query_id))
        .unwrap_or_default();
    let mut lmb_split_visited = ui
        .ctx()
        .data_mut(|d| d.get_temp::<Vec<SlotAddr>>(split_vis_id))
        .unwrap_or_default();
    let mut lmb_split_origin = ui
        .ctx()
        .data_mut(|d| d.get_temp::<Option<ItemStack>>(split_orig_id))
        .flatten();
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
                                &mut lmb_split_visited,
                                &mut lmb_split_origin,
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
                            &mut lmb_split_visited,
                            &mut lmb_split_origin,
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
                                    &mut lmb_split_visited,
                                    &mut lmb_split_origin,
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
                            ui.small(format!(
                                "{batches} ready · Left-click takes one · Right-click takes one · Shift-click crafts the stack into the pack"
                            ));
                        });
                        if out_resp.contains_pointer()
                            && ui.ctx().input(|i| {
                                i.pointer.button_pressed(egui::PointerButton::Primary)
                                    || i.pointer.button_pressed(egui::PointerButton::Secondary)
                            })
                        {
                            let bulk = ui.ctx().input(|i| {
                                i.pointer.button_pressed(egui::PointerButton::Primary)
                                    && (i.modifiers.shift
                                        || i.modifiers.ctrl
                                        || i.modifiers.command)
                            });
                            let _ = survival.craft_from_grid(bulk);
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
                        .on_hover_text("Show only recipes you can craft from the pack")
                        .clicked()
                    {
                        craftable_only = !craftable_only;
                    }
                });
                ui.add(
                    egui::TextEdit::singleline(&mut recipe_query)
                        .hint_text("Search")
                        .desired_width(rem_w.min(160.0)),
                );
                egui::ScrollArea::vertical()
                    .id_salt("quick_recipes_scroll")
                    .max_height(char_h - 16.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let query = recipe_query.to_lowercase();
                        egui::Grid::new("quick_recipe_grid")
                            .spacing([3.0, 3.0])
                            .show(ui, |ui| {
                                let mut col_count = 0usize;
                                for (idx, recipe) in HAND_RECIPES.iter().enumerate() {
                                    if !query.is_empty()
                                        && !recipe.name.to_lowercase().contains(&query)
                                    {
                                        continue;
                                    }
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
                                        ui.small("Left-click fills the grid. Right-click crafts one onto the cursor. Shift-click crafts every batch into the pack.");
                                    });
                                    if resp.contains_pointer()
                                        && ui.ctx().input(|i| {
                                            i.pointer.button_pressed(egui::PointerButton::Primary)
                                                || i.pointer
                                                    .button_pressed(egui::PointerButton::Secondary)
                                        })
                                    {
                                        if craftable {
                                            ghost_recipe = None;
                                            let bulk = ui.ctx().input(|i| {
                                                i.pointer.button_pressed(egui::PointerButton::Primary)
                                                    && (i.modifiers.shift
                                                        || i.modifiers.ctrl
                                                        || i.modifiers.command)
                                            });
                                            let single_now = ui.ctx().input(|i| {
                                                i.pointer
                                                    .button_pressed(egui::PointerButton::Secondary)
                                            });
                                            if survival.stage_recipe_into_grid(recipe) {
                                                if bulk {
                                                    let _ = survival.craft_from_grid(true);
                                                } else if single_now {
                                                    let _ = survival.craft_from_grid(false);
                                                }
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
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("Workbench  ·  F")
                                .size(11.0)
                                .color(presentation::MUTED),
                        );
                        let names = crate::world::expedition::NAMES;
                        for (tier, title) in [
                            (0u32, "Lumen Twig"),
                            (1, "Quarry Conduit"),
                            (2, "Sanctum Sovereign"),
                        ] {
                            if !query.is_empty() && !title.to_lowercase().contains(&query) {
                                continue;
                            }
                            let cost = crate::world::adventure::wand_recipe(tier);
                            let line = cost
                                .iter()
                                .map(|(id, amount)| {
                                    format!("{} ×{}", names[*id], amount / 64)
                                })
                                .collect::<Vec<_>>()
                                .join("  ");
                            ui.small(format!("{title}  {line}"))
                                .on_hover_text("Found on the crystal altar of the matching Holy Mountain landing, or assembled with F at a workbench. Not a 2×2 recipe.");
                        }
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
                    .on_hover_text("Compact partial stacks and sort backpack by category [R]")
                    .clicked()
                    || ui.ctx().input(|i| i.key_pressed(egui::Key::R))
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
                        &mut lmb_split_visited,
                        &mut lmb_split_origin,
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
                        &mut lmb_split_visited,
                        &mut lmb_split_origin,
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

    // Save currently hovered item for cross-slot matching highlights on the next frame
    let hovered_item = hovered_slot.and_then(|addr| survival.slot(addr).map(|s| s.item));
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("surv_hovered_item"), hovered_item));

    // Number keys 1–9 while hovering any slot swap that slot with hotbar[0..9],
    // and pressing Q (1 item) or Ctrl+Q (entire stack) while hovering a slot drops it immediately!
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
            let full_stack = ui.ctx().input(|i| i.modifiers.ctrl || i.modifiers.command);
            if let Some(st) = survival.take_for_drop(addr, full_stack) {
                survival.cursor = Some(st);
                *action = UiAction::DropInventory(creative.selected_slot);
            }
        }
    }

    if ui
        .ctx()
        .input(|i| i.pointer.button_released(egui::PointerButton::Primary))
    {
        let unmodified = ui
            .ctx()
            .input(|i| !i.modifiers.shift && !i.modifiers.ctrl && !i.modifiers.command);
        if unmodified {
            let pointer_slot = ui
                .ctx()
                .data_mut(|d| d.get_temp::<Option<SlotAddr>>(pointer_slot_id))
                .flatten();
            let panel_rect = ui
                .ctx()
                .data_mut(|d| d.get_temp::<egui::Rect>(egui::Id::new("inventory_window_rect")));
            let pointer_pos = ui.ctx().input(|i| i.pointer.interact_pos());
            match pointer_release(
                lmb_split_origin.is_some(),
                drag_slot,
                pointer_slot,
                lmb_split_visited.len(),
            ) {
                PointerRelease::Place(addr) => survival.click_slot(addr),
                PointerRelease::Distribute => {
                    survival.distribute_even_across(lmb_split_origin, &lmb_split_visited);
                }
                PointerRelease::Drop => {
                    // A release outside the panel is a deliberate throw. A
                    // release over the recipe book or tab strip keeps the stack
                    // on the cursor, so browsing the inventory never discards.
                    let outside = pointer_pos
                        .is_some_and(|pos| panel_rect.is_some_and(|rect| !rect.contains(pos)));
                    if outside && survival.cursor.is_some() {
                        *action = UiAction::DropInventory(creative.selected_slot);
                    }
                }
                PointerRelease::Keep => {}
            }
        }
        drag_slot = None;
        lmb_split_origin = None;
        lmb_split_visited.clear();
    }
    ui.ctx().data_mut(|d| {
        d.insert_temp(drag_id, drag_slot);
        d.insert_temp(ghost_id, ghost_recipe);
        d.insert_temp(craft_only_id, craftable_only);
        d.insert_temp(query_id, recipe_query);
        d.insert_temp(split_vis_id, lmb_split_visited);
        d.insert_temp(split_orig_id, lmb_split_origin);
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
        if let Some(pos) = ui.ctx().input(|i| i.pointer.latest_pos()) {
            let p = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("survival_cursor_item"),
            ));
            let r =
                egui::Rect::from_center_size(pos + egui::vec2(10.0, 10.0), egui::vec2(30.0, 30.0));
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
            let panel_rect = ui
                .ctx()
                .data_mut(|d| d.get_temp::<egui::Rect>(egui::Id::new("inventory_window_rect")));
            let outside = panel_rect.is_some_and(|rect| !rect.contains(pos));
            let dragging = ui
                .ctx()
                .input(|i| i.pointer.button_down(egui::PointerButton::Primary));
            if outside && dragging {
                let font = egui::FontId::monospace(10.5);
                let galley =
                    p.layout_no_wrap("RELEASE TO THROW".to_owned(), font, presentation::AMBER);
                let at = pos + egui::vec2(16.0, 40.0);
                let bg = egui::Rect::from_min_size(
                    at - egui::vec2(4.0, 2.0),
                    galley.size() + egui::vec2(8.0, 4.0),
                );
                p.rect_filled(bg, 2.0, Color32::from_rgba_unmultiplied(22, 18, 20, 225));
                p.galley(at, galley, presentation::AMBER);
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
    let mut group = ctx.data_mut(|d| {
        d.get_temp::<String>(category_id)
            .unwrap_or_else(|| "All".into())
    });
    let catalogue_mode_id = egui::Id::new("creative_catalogue_mode");
    let mut catalogue_mode = ctx.data_mut(|d| d.get_temp::<u8>(catalogue_mode_id).unwrap_or(0));
    let give_count_id = egui::Id::new("creative_give_count");
    let mut give_count = ctx.data_mut(|d| d.get_temp::<u32>(give_count_id).unwrap_or(8));
    let sort_name_id = egui::Id::new("creative_sort_name");
    let mut sort_by_name = ctx.data_mut(|d| d.get_temp::<bool>(sort_name_id).unwrap_or(true));
    let screen = ctx.content_rect();
    let compact = screen.height() < 560.0 || screen.width() < 680.0;

    let window_response = egui::Window::new("Inventory")
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
                    (egui::TextStyle::Heading, 20.0),
                    (egui::TextStyle::Body, 10.0),
                    (egui::TextStyle::Button, 10.0),
                    (egui::TextStyle::Small, 10.0),
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
                    (5, "Forge"),
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

            // Page 4: creative catalogue. Blocks keep the original drag/palette
            // path used by the torch review capture; items grant stacks through
            // UiAction::GiveItem so they never bypass inventory limits.
            if page == 4 {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("CATALOGUE")
                            .size(10.5)
                            .color(presentation::AMBER),
                    );
                    for (mode, label, hint) in [
                        (0, "Blocks", "Placeable building materials"),
                        (1, "Items", "Materials, gear, food and tools"),
                    ] {
                        if ui
                            .selectable_label(catalogue_mode == mode, label)
                            .on_hover_text(hint)
                            .clicked()
                        {
                            catalogue_mode = mode;
                            group = "All".into();
                            *drag = None;
                        }
                    }
                    if catalogue_mode == 1 {
                        ui.separator();
                        ui.label(RichText::new("Give").size(10.0).color(presentation::MUTED));
                        for amount in [1u32, 8, 64] {
                            if ui
                                .selectable_label(give_count == amount, format!("×{amount}"))
                                .clicked()
                            {
                                give_count = amount;
                            }
                        }
                        ui.label(
                            RichText::new("Shift-click gives a full stack")
                                .size(9.5)
                                .color(presentation::MUTED),
                        );
                    }
                });
                let categories: &[&str] = if catalogue_mode == 0 {
                    &["All", "Terrain", "Building", "Nature", "Lighting", "Machines"]
                } else {
                    &["All", "Materials", "Weapons", "Armour", "Tools", "Food"]
                };
                ui.horizontal_wrapped(|ui| {
                    for label in categories {
                        if ui.selectable_label(group == *label, *label).clicked() {
                            group = (*label).into();
                            *drag = None;
                        }
                    }
                    if ui
                        .small_button(if sort_by_name { "A–Z" } else { "ID" })
                        .on_hover_text("Toggle between name and catalogue-ID sorting.")
                        .clicked()
                    {
                        sort_by_name = !sort_by_name;
                    }
                    if catalogue_mode == 0
                        && ui
                            .small_button("Fill hotbar")
                            .on_hover_text(
                                "Assign the first nine matching blocks to the hotbar. The current slot stays selected.",
                            )
                            .clicked()
                    {
                        let query = creative.palette_search.trim().to_lowercase();
                        let mut list: Vec<_> = material_defs()
                            .iter()
                            .filter(|m| {
                                m.tags.selectable
                                    && contains_ignore_case(m.display_name, &query)
                                    && (group == "All" || category(m.id) == group)
                            })
                            .collect();
                        if sort_by_name {
                            list.sort_by_key(|m| m.display_name);
                        } else {
                            list.sort_by_key(|m| m.id.as_u32());
                        }
                        let keep = creative.selected_slot;
                        for (index, def) in list.into_iter().take(HOTBAR_SLOTS).enumerate() {
                            creative.assign_hotbar_slot(index, def.id);
                        }
                        creative.select_slot(keep);
                        creative.tool = ToolKind::Hand;
                        *drag = None;
                    }
                });
                ui.horizontal(|ui| {
                    let hint = if catalogue_mode == 0 {
                        "Search creative blocks…"
                    } else {
                        "Search items, gear and materials…"
                    };
                    ui.add(
                        egui::TextEdit::singleline(&mut creative.palette_search)
                            .hint_text(hint)
                            .desired_width((ui.available_width() - 52.0).max(80.0)),
                    );
                    if ui.button("Clear").clicked() {
                        creative.palette_search.clear();
                    }
                });
            }

            egui::ScrollArea::vertical()
                .id_salt((
                    "inventory_body",
                    page,
                    catalogue_mode,
                    &group,
                    &creative.palette_search,
                ))
                .max_height(
                    (screen.height()
                        - (ui.cursor().top() - ui.min_rect().top())
                        - if compact { 96.0 } else { 140.0 })
                    .clamp(48.0, 380.0),
                )
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if page == 5 {
                        super::tinkering::draw(ui, equipment, survival, machines.bench, action);
                        return;
                    }
                    if page != 4 {
                        if let Some(pack) = expedition {
                            factory::draw(ui, machines, pack, equipment, action, page);
                        } else {
                            ui.weak("No supplies loaded.");
                        }
                        return;
                    }
                    let query = creative.palette_search.trim().to_lowercase();
                    draw_catalogue(
                        ui,
                        creative,
                        player,
                        survival,
                        catalogue_mode,
                        &group,
                        &query,
                        sort_by_name,
                        give_count,
                        compact,
                        drag,
                        hovered,
                        action,
                    );
                });

            if page == 4 {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    draw_hotbar_row(ui, creative, player, None, drag, drop_slot);
                });
            }
        });
    ctx.data_mut(|d| {
        if let Some(inner) = &window_response {
            d.insert_temp(egui::Id::new("inventory_window_rect"), inner.response.rect);
        }
        d.insert_temp(page_id, page);
        d.insert_temp(category_id, group);
        d.insert_temp(catalogue_mode_id, catalogue_mode);
        d.insert_temp(give_count_id, give_count);
        d.insert_temp(sort_name_id, sort_by_name);
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_catalogue(
    ui: &mut egui::Ui,
    creative: &mut CreativeState,
    player: &mut Player,
    survival: &SurvivalInventory,
    mode: u8,
    group: &str,
    query: &str,
    sort_by_name: bool,
    give_count: u32,
    compact: bool,
    drag: &mut Option<DragSource>,
    hovered: &mut Option<BlockId>,
    action: &mut UiAction,
) {
    let columns = ((ui.available_width() + 8.0) / 104.0).floor().max(1.0) as usize;
    let width = ((ui.available_width() - 8.0 * (columns - 1) as f32) / columns as f32).min(120.0);
    let card = [width, if compact { 80.0 } else { 96.0 }];
    if mode == 0 {
        let mut matches: Vec<_> = material_defs()
            .iter()
            .filter(|m| {
                m.tags.selectable
                    && contains_ignore_case(m.display_name, query)
                    && (group == "All" || category(m.id) == group)
            })
            .collect();
        if sort_by_name {
            matches.sort_by_key(|m| m.display_name);
        } else {
            matches.sort_by_key(|m| m.id.as_u32());
        }
        egui::Grid::new("inventory_blocks")
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                for (index, def) in matches.iter().enumerate() {
                    let response = ui
                        .push_id(def.id.as_u32(), |ui| {
                            draw_palette_material_card(ui, card, def.id)
                        })
                        .inner;
                    if crate::torch_review::mode() == "drag" && def.id == BlockId::Torch {
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(egui::Id::new("review_torch_card"), response.rect)
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
        return;
    }

    let mut items: Vec<Item> = creative_items()
        .iter()
        .copied()
        .filter(|item| {
            contains_ignore_case(item.name(), query)
                && (group == "All" || creative_item_group(*item) == group)
        })
        .collect();
    if sort_by_name {
        items.sort_by_key(|item| item.name());
    } else {
        items.sort_by_key(|item| item.encode_u32());
    }
    if items.is_empty() {
        ui.weak("No items match that search.");
    }
    egui::Grid::new("inventory_items")
        .spacing([8.0, 8.0])
        .show(ui, |ui| {
            for (index, item) in items.iter().enumerate() {
                let response = ui
                    .push_id(("creative_item", index), |ui| {
                        draw_creative_item_card(ui, card, *item)
                    })
                    .inner;
                let full = ui.input(|i| i.modifiers.shift);
                let limit = survival.stack_limit(*item);
                let amount = if full { limit } else { give_count.min(limit) };
                response.clone().on_hover_ui(|ui| {
                    ui.set_max_width(220.0);
                    ui.strong(RichText::new(item.name()).size(13.0));
                    ui.label(
                        RichText::new(item.description())
                            .size(11.0)
                            .color(presentation::MUTED),
                    );
                    ui.small(if full {
                        format!("Click to add a full stack (×{amount}).")
                    } else {
                        format!("Click to add ×{amount}. Shift-click: full stack.")
                    });
                });
                if response.clicked() {
                    *action = UiAction::GiveItem(*item, amount);
                }
                if (index + 1) % columns == 0 {
                    ui.end_row();
                }
            }
        });
}

#[cfg(test)]
mod catalogue_tests {
    use super::*;

    #[test]
    fn creative_item_list_is_unique_and_covers_every_kind() {
        let items = creative_items();
        let mut seen = std::collections::HashSet::new();
        for item in items {
            assert!(seen.insert(*item), "duplicate creative item {item:?}");
        }
        assert!(items.iter().any(|i| matches!(i, Item::Resource(_))));
        assert!(items.iter().any(|i| matches!(i, Item::Weapon(_))));
        assert!(items.iter().any(|i| matches!(i, Item::Armor(_, _))));
        assert!(items.iter().any(|i| matches!(i, Item::Tool(_))));
        assert!(items.iter().any(|i| matches!(i, Item::Food(_))));
        assert_eq!(
            items
                .iter()
                .filter(|i| matches!(i, Item::Resource(_)))
                .count(),
            crate::world::factory::RESOURCE_COUNT
        );
    }

    #[test]
    fn catalogue_groups_cover_every_item() {
        for item in creative_items().iter().copied() {
            assert_ne!(creative_item_group(item), "Blocks");
        }
    }

    #[test]
    fn catalogue_search_is_case_insensitive() {
        assert!(contains_ignore_case("Steel Sword", "sword"));
        assert!(contains_ignore_case("Steel Sword", ""));
        assert!(!contains_ignore_case("Steel", "sword"));
        assert!(contains_ignore_case("Épée", "épée"));
    }

    #[test]
    fn short_names_are_bounded_without_breaking_utf8() {
        assert_eq!(short_name("Steel sword", 15), "Steel sword");
        assert_eq!(short_name("Lumen miner's helm", 10), "Lumen min…");
        assert_eq!(short_name("ééééé", 3), "éé…");
    }
}
