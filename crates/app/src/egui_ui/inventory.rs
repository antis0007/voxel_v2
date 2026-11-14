//! Inventory navigation, Minecraft-style survival inventory screen (E), 2×2 hand-crafting grid,
//! HWYLA/modded-style compact recipe & tooltip QOL, and separate creative block catalogue.
use super::*;
mod descriptions;
pub(super) mod screen;
pub(super) mod search;
mod tabs;
pub(super) mod unlocks;
mod catalogue;
use crate::world::inventory::{
    ArmorTier, CRAFT_SLOTS, CraftingRecipe, EQUIP_SLOTS, EquipSlot, HAND_GRID, HOTBAR_SLOTS,
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
pub(crate) fn creative_items() -> &'static [Item] {
    crate::creative::catalogue::items()
}

fn creative_item_group(item: Item) -> &'static str {
    match item {
        Item::Assembled { .. } | Item::Part { .. } | Item::Cast(_) => "Equipment",
        Item::Resource(_) => "Materials",
        Item::Weapon(_) => "Weapons",
        Item::Armor(_, _) => "Armour",
        Item::Tool(_) | Item::Bucket(_) => "Tools",
        Item::Wand { .. } => "Wands",
        Item::Food(_) => "Food",
        Item::Block(_) | Item::PowerKit(_) => "Blocks",
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

#[cfg(test)]
fn short_name(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        return name.to_owned();
    }
    let mut out: String = name.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

pub(crate) fn draw_item_icon(painter: &egui::Painter, rect: egui::Rect, item: Item) {
    match super::held_art::art(item) {
        super::held_art::Art::Block(block) => draw_voxel_icon(painter, rect, block),
        super::held_art::Art::Icon(icon) => icons::paint(painter, rect, icon),
        super::held_art::Art::Wand(look) => super::wand_icon::paint(painter, rect, look),
    }
    if let Item::Assembled{id,..}=item {
        if let Some(tool)=super::held_art::tool(id) {super::progression_hud::assembled_bar(painter,rect,&tool);}
    }
}

fn draw_equipment_silhouette(painter: &egui::Painter, rect: egui::Rect, slot: EquipSlot) {
    let icon = icons::Icon::Armor(slot as u8, 1);
    if icons::paint_silhouette(painter, rect.shrink(rect.width() * 0.08), icon, egui::Color32::from_rgba_unmultiplied(90, 84, 104, 90)) {
        return;
    }
    let r = rect.shrink(rect.width() * 0.22);
    icons::paint(painter, r, icon);
    // Dim overlay so empty equipment slots read as recessed silhouettes
    painter.rect_filled(r, 0, egui::Color32::from_rgba_unmultiplied(36, 30, 32, 165));
}

fn draw_player_preview(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    unit: f32,
    inv: &SurvivalInventory,
    equipment_view: &crate::world::adventure::View,
) {
    let resp = ui.interact(rect, egui::Id::new("survival_figure"), egui::Sense::hover());
    let p = ui.painter();
    screen::well(p, rect, unit);
    let inner = rect.shrink(unit);
    p.rect_filled(inner, 0, Color32::from_rgb(12, 10, 10));

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
            Some(Item::Armor(_, crate::world::inventory::ArmorTier::Kinetic)) => {
                Color32::from_rgb(232, 150, 64)
            }
            Some(Item::Armor(_, ArmorTier::Ballast)) => Color32::from_rgb(175, 122, 69),
            Some(Item::Armor(_, ArmorTier::Burner)) => Color32::from_rgb(210, 117, 58),
            Some(Item::Armor(_, ArmorTier::Induction)) => Color32::from_rgb(69, 188, 214),
            Some(Item::Armor(_, ArmorTier::Slime)) => Color32::from_rgb(117, 192, 73),
            Some(Item::Armor(_, ArmorTier::Glider)) => Color32::from_rgb(209, 184, 115),
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
    if inv.equipment[EquipSlot::Head as usize].is_some_and(|s| s.item == Item::Armor(EquipSlot::Head, ArmorTier::Survey)) {
        let visor = egui::Rect::from_center_size(head.center(), egui::Vec2::splat(head.width() * 1.25));
        icons::paint(p, visor, icons::Icon::Armor(EquipSlot::Head as u8, ArmorTier::Survey as u8));
    } else if inv.equipment[EquipSlot::Head as usize].is_some() {
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
            .size(13.0)
            .color(presentation::PAPER),
        );
        if equipment_view.wand > 0 {
            ui.separator();
            super::wands::show_wand_hover_card(ui, equipment_view);
        }
    });
}

pub(super) fn show_item_tooltip(resp: &egui::Response, stack: ItemStack, inv: &SurvivalInventory, addr: SlotAddr) {
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
        descriptions::details(ui, stack.item);
        super::progression_hud::durability_line(ui, addr);
        if let Some((before, after)) = inv.armor_comparison(stack.item) {
            let delta = (after - before) * 100.0;
            ui.label(RichText::new(format!("Equipped defence: {:.0}% → {:.0}% ({delta:+.0}%)", before*100.0, after*100.0))
                .color(if delta < 0.0 { presentation::DANGER } else { presentation::PAPER }));
        }
        let total = inv.count_item(stack.item);
        if total > stack.count {
            ui.small(format!("Owned: {total}"));
        }
        if descriptions::expanded(ui) && let Item::Wand { id, .. } = stack.item {
            super::wand_icon::card(ui, id);
        }
    });
}

/// Compact counts share the same width budget in inventory and HUD slots.
pub(super) fn stack_count(painter: &egui::Painter, rect: egui::Rect, unit: f32, count: u32) {
    let text = count.to_string();
    let mut font = egui::FontId::monospace(rect.height() * 0.30);
    let available = (rect.width() - 3.0 * unit).max(1.0);
    // Reserve four digits even for small stacks so their type size stays stable.
    let sample = if text.len() < 4 { "9999" } else { &text };
    let width = painter.layout_no_wrap(sample.to_owned(), font.clone(), presentation::PAPER).size().x;
    if width > available {
        font.size *= available / width;
    }
    if crate::torch_review::mode() == "ui" && count >= 1000 {
        let key = egui::Id::new(("review_count_fit", count, rect.width().to_bits(), rect.height().to_bits()));
        if !painter.ctx().data_mut(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
            let measured = painter.layout_no_wrap(text.clone(), font.clone(), presentation::PAPER).size().x;
            eprintln!("[ui-count] count={count} slot={:.1} font={:.2} width={measured:.2} available={available:.2}", rect.width(), font.size);
            painter.ctx().data_mut(|d| d.insert_temp(key, true));
        }
    }
    let pos = rect.right_bottom() - egui::vec2(unit, unit);
    let prism = super::prism::active(painter.ctx());
    painter.text(pos + egui::vec2(unit, unit) * 0.5, Align2::RIGHT_BOTTOM, &text, font.clone(),
        if prism { Color32::from_rgb(3, 5, 11) } else { presentation::INK });
    painter.text(pos, Align2::RIGHT_BOTTOM, text, font,
        if prism { Color32::from_rgb(243, 247, 255) } else { presentation::PAPER });
}

fn draw_survival_slot(
    ui: &mut egui::Ui,
    inv: &mut SurvivalInventory,
    addr: SlotAddr,
    rect: egui::Rect,
    unit: f32,
    selected: bool,
    hovered_slot: &mut Option<SlotAddr>,
    drag_slot: &mut Option<SlotAddr>,
    lmb_split_visited: &mut Vec<SlotAddr>,
    lmb_split_origin: &mut Option<ItemStack>,
) -> egui::Response {
    let tag = match addr {
        SlotAddr::Hotbar(i) => ("surv_hotbar", i),
        SlotAddr::Offhand => ("surv_offhand", 0),
        SlotAddr::Container(i) => ("surv_container", i),
        SlotAddr::Pack(i) => ("surv_pack", i),
        SlotAddr::Equipment(i) => ("surv_equip", i),
        SlotAddr::CraftGrid(i) => ("surv_craft", i),
    };
    let response = ui.interact(rect, egui::Id::new(("survival_slot", tag)), egui::Sense::click_and_drag());
    let stack = inv.slot(addr);
    let label = match stack {
        Some(s) => format!("{} × {}", s.item.name(), s.count),
        None => match addr {
            SlotAddr::Equipment(i) => EquipSlot::from_index(i)
                .map_or("Equipment", |e| e.label())
                .into(),
            SlotAddr::Hotbar(i) => format!("Hotbar {}", i + 1),
            SlotAddr::Offhand => "Offhand".into(),
            SlotAddr::Pack(i) => format!("Pack {}", i + 1),
            SlotAddr::Container(i) => format!("Container {}", i + 1),
            SlotAddr::CraftGrid(i) => format!("Craft {}", i + 1),
        },
    };
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &label)
    });

    // Save rect for deterministic verification harness
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new(tag), rect));

    let is_hovered = response.hovered();
    if response.hovered() && !ui.ctx().egui_wants_keyboard_input()
        && ui.input(|i| i.key_pressed(egui::Key::L)) {
        inv.toggle_protected(addr);
    }
    if is_hovered {
        *hovered_slot = Some(addr);
    }

    let painter = ui.painter();
    screen::well(painter, rect, unit);
    if selected {
        screen::selected(painter, rect, unit);
    }

    // Highlight matching item stacks when hovering a slot or holding a cursor item
    let match_item = inv.cursor.map(|c| c.item).or_else(|| {
        ui.ctx()
            .data_mut(|d| d.get_temp::<Option<Item>>(egui::Id::new("surv_hovered_item")))
            .flatten()
    });
    if let (Some(st), Some(target_item)) = (stack, match_item)
        && st.item == target_item && !is_hovered {
            painter.rect_stroke(
                rect.shrink(unit),
                0,
                egui::Stroke::new(unit, Color32::from_rgba_unmultiplied(218, 172, 105, 130)),
                egui::StrokeKind::Inside,
            );
        }
    if lmb_split_visited.contains(&addr) {
        painter.rect_filled(
            rect.shrink(unit),
            0,
            Color32::from_rgba_unmultiplied(211, 183, 125, 65),
        );
    }

    if let Some(stack) = stack {
        draw_item_icon(painter, rect.shrink(unit), stack.item);
        if matches!(addr, SlotAddr::Pack(_) | SlotAddr::Hotbar(_) | SlotAddr::Container(_)) {
            search::paint(painter, rect, unit, Some(stack.item));
        }
        if stack.count > 1 {
            stack_count(painter, rect, unit, stack.count);
        }
        super::progression_hud::wear_bar(painter, rect.shrink(unit), addr);
        show_item_tooltip(&response, stack, inv, addr);
    } else if let SlotAddr::Equipment(i) = addr
        && let Some(eq) = EquipSlot::from_index(i) {
            draw_equipment_silhouette(painter, rect, eq);
            response.clone().on_hover_text(eq.hint());
        }
    if inv.is_protected(addr) {
        painter.text(rect.left_top() + egui::vec2(2.0, 1.0), Align2::LEFT_TOP,
            "L", egui::FontId::monospace(10.0), presentation::PAPER);
    }
    if is_hovered {
        screen::hover(painter, rect, unit);
    }

    // Interactions, matching Minecraft's inventory pointer:
    // - Left click on an empty hand picks the stack up on release over the same
    //   slot, so a press that is cancelled or dragged away does nothing. Releasing
    //   on one other slot later places it all.
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

    // Empty-hand pickup fires on release over the slot it was pressed on.
    if response.clicked() && !shift && !ctrl && inv.cursor.is_none() && inv.slot(addr).is_some() {
        inv.click_slot(addr);
        *drag_slot = Some(addr);
        *lmb_split_origin = None;
        lmb_split_visited.clear();
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
    if !shift && !ctrl && lmb_split_origin.is_some() && over && (lmb_down || lmb_release)
        && !lmb_split_visited.contains(&addr)
            && let Some(orig) = *lmb_split_origin
                && inv.slot_accepts(addr, orig.item)
                    && inv.slot(addr).is_none_or(|s| s.item == orig.item)
                {
                    lmb_split_visited.push(addr);
                }

    response
}

/// The wand board's 468 points plus the body scroll bar.
const WAND_PAGE_W: f32 = 484.0;

/// Points per GUI unit on the survival page: the largest step whose container
/// fits beside the item list and under the tab row.
fn survival_unit(screen: egui::Rect, pixels_per_point: f32) -> f32 {
    let side = if screen.width() < 720.0 { 0.0 } else { 24.0 + super::recipe_book::SIDEBAR_WIDTH };
    screen::unit_for(egui::vec2(screen.width() - 48.0 - side, screen.height() - 120.0), pixels_per_point)
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
    descriptions::details(ui, recipe.output.item);
    ui.separator();
    for &(item, need) in recipe.ingredients {
        let have = inv.count_item(item);
        let ok = have >= need;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
            draw_item_icon(ui.painter(), r, item);
            ui.label(RichText::new(item.name()).size(13.0).color(if ok {
                presentation::PAPER
            } else {
                presentation::MUTED
            }));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.monospace(
                    RichText::new(format!("{have}/{need}"))
                        .size(13.0)
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
    unit: f32,
    open_wand_tab: &mut bool,
    bench: bool,
    size: i32,
) {
    super::wand_icon::publish(ui.ctx(), equipment_view);
    let recipes = crate::world::inventory::all_recipes();
    // Leaving the bench folds the 3 × 3 grid back to the 2 × 2 corner.
    if !bench {
        for i in 0..CRAFT_SLOTS {
            if !HAND_GRID.contains(&i) && survival.craft_grid[i].is_some() {
                let _ = survival.shift_click_slot(SlotAddr::CraftGrid(i));
            }
        }
    }
    let grid_slots: &[usize] = if bench {
        &[0, 1, 2, 3, 4, 5, 6, 7, 8]
    } else {
        &HAND_GRID
    };
    let ghost_missing = Color32::from_rgba_unmultiplied(150, 36, 28, 130);
    let ghost_held = Color32::from_rgba_unmultiplied(12, 10, 10, 140);

    let mut hovered_slot: Option<SlotAddr> = None;
    let drag_id = egui::Id::new("survival_drag_origin");
    let ghost_id = egui::Id::new("survival_ghost_recipe");
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

    // Short screens scroll instead of clipping the top tabs and the hotbar.
    let scroll_h = (ui.ctx().content_rect().height() - 128.0).max(200.0);
    let panel_size = egui::vec2(screen::WIDTH, screen::HEIGHT) * unit;
    egui::ScrollArea::vertical()
        .id_salt("survival_page")
        .max_height(scroll_h)
        // The auto-sized window offers only last frame's height, so without a
        // floor the scroll area and window never grow past their first size.
        .min_scrolled_height((panel_size.y + 8.0).min(scroll_h))
        .auto_shrink([false, true])
        .show(ui, |ui| ui.with_layout(
            if ui.ctx().content_rect().width() < 720.0 {
                egui::Layout::top_down(egui::Align::Min)
            } else { egui::Layout::left_to_right(egui::Align::Min) }, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(24.0, 12.0);

        // Left: the vanilla container, every element on the 18-unit slot grid.
        ui.vertical(|ui| {
            ui.set_width(panel_size.x);
            let t = &player.traversal;
            if t.jet != 0 {
                let (fraction, text) = if t.jet == 1 {
                    (t.fuel_seconds / crate::player::traversal::BURN_SECONDS, format!("Fuel: {:.1} s", t.fuel_seconds))
                } else {
                    (t.charge_fe / crate::player::traversal::CHARGE_CAPACITY, format!("Charge: {:.0} FE", t.charge_fe))
                };
                ui.add(egui::ProgressBar::new(fraction).text(text));
            }
            if equipment_view.wand > 0 {
                super::wands::board::inline_bar(ui, equipment_view, action, open_wand_tab, unit);
            }
            let layout = screen::Layout::new(bench);
            let (panel_rect, _) = ui.allocate_exact_size(panel_size, egui::Sense::hover());
            let frame = screen::Frame { origin: panel_rect.min, unit };
            screen::panel(ui.painter(), panel_rect, unit);
            let label = |ui: &egui::Ui, at: egui::Pos2, text: &str| {
                ui.painter().text(
                    frame.pos(at),
                    Align2::LEFT_TOP,
                    text,
                    egui::FontId::proportional((6.0 * unit).max(12.0)),
                    screen::LABEL,
                );
            };
            let mut slot = |ui: &mut egui::Ui, survival: &mut SurvivalInventory, addr, rect, selected| {
                draw_survival_slot(
                    ui,
                    survival,
                    addr,
                    rect,
                    unit,
                    selected,
                    &mut hovered_slot,
                    &mut drag_slot,
                    &mut lmb_split_visited,
                    &mut lmb_split_origin,
                )
            };

            if survival.container.is_some() {
                label(ui, egui::pos2(8.0, 5.0), "Chest");
                for i in 0..crate::world::containers::SLOTS {
                    slot(ui, survival, SlotAddr::Container(i), frame.slot(egui::pos2(7.0, 16.0), 9, i), false);
                }
            } else {
            if let (Some(at), Some(figure)) = (layout.armour, layout.figure) {
                for i in 0..EQUIP_SLOTS {
                    slot(ui, survival, SlotAddr::Equipment(i), frame.slot(at, 1, i), false);
                }
                draw_player_preview(ui, frame.rect(figure), unit, survival, equipment_view);
            }

            let offhand = if bench { egui::pos2(151.0, 48.0) } else { egui::pos2(77.0, 53.0) };
            slot(ui, survival, SlotAddr::Offhand, frame.slot(offhand, 1, 0), false).on_hover_text("Offhand");
            label(ui, layout.crafting_label, "Crafting");
            let mut craft_rects = [egui::Rect::NOTHING; CRAFT_SLOTS];
            for (k, &i) in grid_slots.iter().enumerate() {
                craft_rects[i] = frame.slot(layout.grid, layout.grid_cols, k);
                slot(ui, survival, SlotAddr::CraftGrid(i), craft_rects[i], false);
            }

            // Ghost of a recipe picked in the book without the ingredients:
            // dimmed where carried, red where missing.
            if let Some(rec) = ghost_recipe.and_then(|idx| recipes.get(idx)) {
                for (i, item, _) in rec.cells(bench, 1).unwrap_or_default() {
                    let need = rec.ingredients.iter().find(|(it, _)| *it == item).map_or(1, |&(_, n)| n);
                    let r = craft_rects[i].shrink(unit);
                    draw_item_icon(ui.painter(), r, item);
                    let missing = survival.count_item(item) < need;
                    ui.painter().rect_filled(r, 0, if missing { ghost_missing } else { ghost_held });
                }
            }

            let matched = survival.matching_grid_recipe(bench);
            screen::arrow(ui.painter(), frame.rect(layout.arrow), unit, matched.is_some());
            let out_rect = frame.rect(layout.output);
            let out_resp = ui.interact(out_rect, egui::Id::new("survival_output"), egui::Sense::click());
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("surv_craft_output"), out_rect));
            screen::well(ui.painter(), out_rect, unit);
            let item_rect = egui::Rect::from_center_size(out_rect.center(), egui::Vec2::splat(16.0 * unit));
            if let Some((recipe, batches)) = matched {
                draw_item_icon(ui.painter(), item_rect, recipe.output.item);
                if recipe.output.count > 1 {
                    stack_count(ui.painter(), item_rect.expand(unit), unit, recipe.output.count);
                }
                if out_resp.hovered() {
                    screen::hover(ui.painter(), item_rect.expand(unit), unit);
                }
                out_resp.clone().on_hover_ui(|ui| {
                    draw_recipe_tooltip(ui, recipe, survival);
                    ui.label(RichText::new(format!("Ready: {}", batches.saturating_mul(recipe.output.count))).size(13.0).color(presentation::PAPER));
                    if let Err(reason) = survival.grid_craft_batches(false, bench) {
                        ui.colored_label(presentation::DANGER, reason.message());
                    }
                    match survival.grid_craft_batches(true, bench) {
                        Ok(fit) => {
                            ui.label(RichText::new(format!("[Shift] → Inventory · {}", fit.saturating_mul(recipe.output.count))).size(13.0).color(presentation::MUTED));
                        }
                        Err(crate::world::inventory::CraftBlocker::OutputSpace) => {
                            ui.colored_label(presentation::DANGER, "Inventory Full");
                        }
                        _ => {}
                    }
                });
                if out_resp.clicked() || out_resp.secondary_clicked() {
                    let bulk = out_resp.clicked()
                        && ui.ctx().input(|i| {
                            i.modifiers.shift || i.modifiers.ctrl || i.modifiers.command
                        });
                    let _ = survival.craft_from_grid(bulk, bench);
                }
            } else if let Some(rec) = ghost_recipe.and_then(|idx| recipes.get(idx)) {
                draw_item_icon(ui.painter(), item_rect, rec.output.item);
                ui.painter().rect_filled(item_rect, 0, ghost_held);
                if out_resp.on_hover_ui(|ui| draw_recipe_tooltip(ui, rec, survival)).clicked() {
                    ghost_recipe = None;
                }
            }

            // Vanilla labels the pack only on the bench, where no figure sits above it.
            if bench {
                label(ui, layout.inventory_label, "Inventory");
            }
            }
            // Small icon buttons on the label row, right-aligned with the pack.
            for (k, tip) in [(0, "Sort · [L] locks hovered slot"), (1, "Stack Transfer"), (2, "Recover overflow")] {
                if k == 2 && survival.overflow.is_empty() { continue; }
                let at = egui::pos2(160.0 - k as f32 * 11.0, 71.0);
                let r = frame.rect(egui::Rect::from_min_size(at, egui::vec2(9.0, 9.0)));
                let resp = ui.interact(r, egui::Id::new(("survival_pack_button", k)), egui::Sense::click());
                screen::well(ui.painter(), r, unit);
                if k == 0 {
                    screen::sort_glyph(ui.painter(), r.min, unit);
                } else {
                    screen::transfer_glyph(ui.painter(), r.min, unit);
                }
                if resp.hovered() {
                    screen::hover(ui.painter(), r, unit);
                }
                if resp.on_hover_text(tip).clicked() {
                    if k == 0 {
                        survival.sort_pack();
                    } else if k == 2 {
                        survival.recover_overflow();
                    } else {
                        survival.quick_stack_to_hotbar();
                    }
                }
            }
            for idx in 0..PACK_SLOTS {
                slot(ui, survival, SlotAddr::Pack(idx), frame.slot(layout.pack, 9, idx), false);
            }
            for s in 0..HOTBAR_SLOTS {
                let selected = s == creative.selected_slot;
                let resp = slot(ui, survival, SlotAddr::Hotbar(s), frame.slot(layout.hotbar, 9, s), selected);
                if resp.double_clicked()
                    && let Some(ItemStack { item: Item::Food(_), .. }) = survival.hotbar[s]
                {
                    creative.select_slot(s);
                    *action = UiAction::Factory(crate::world::factory::Action::EatRation);
                }
            }
        });

        // Right: the shared item list, as tall as the container beside it.
        ui.vertical(|ui| super::recipe_book::sidebar(ui, survival, bench, panel_size.y, size));
    }));

    // Save currently hovered item for cross-slot matching highlights on the next frame
    let hovered_item = hovered_slot.and_then(|addr| survival.slot(addr).map(|s| s.item));
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("surv_hovered_item"), hovered_item));

    // Number keys 1–9 while hovering any slot swap that slot with hotbar[0..9],
    // and pressing Q (1 item) or Ctrl+Q (entire stack) while hovering a slot drops it immediately!
    if let Some(addr) = hovered_slot && !ui.ctx().egui_wants_keyboard_input() {
        if matches!(addr, SlotAddr::Pack(_))
            && survival.cursor.is_none()
            && ui.ctx().input(|i| i.pointer.button_clicked(egui::PointerButton::Middle)) {
            survival.sort_pack();
        }
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
        if let Some(stack) = survival.slot(addr) {
            let (r, u) = ui
                .ctx()
                .input(|i| (i.key_pressed(egui::Key::R), i.key_pressed(egui::Key::U)));
            if r || u {
                super::recipe_book::request(ui.ctx(), stack.item, u);
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
                    let detail_rect = ui.ctx().data_mut(|d| d.get_temp::<egui::Rect>(egui::Id::new("jei_detail_rect")));
                    let outside = pointer_pos.is_some_and(|pos|
                        panel_rect.is_some_and(|rect| !rect.contains(pos))
                            && detail_rect.is_none_or(|rect| !rect.contains(pos)));
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
        d.insert_temp(split_vis_id, lmb_split_visited);
        d.insert_temp(split_orig_id, lmb_split_origin);
    });

    // Sync selected hotbar block/weapon when player clicks or rearranges hotbar
    if let Some(stack) = survival.hotbar[creative.selected_slot] {
        match stack.item {
            Item::PowerKit(k) => player.selected_block = k.kind.block(),
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
    if let Some(cursor) = survival.cursor
        && let Some(pos) = ui.ctx().input(|i| i.pointer.latest_pos()) {
            let p = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("survival_cursor_item"),
            ));
            let r =
                egui::Rect::from_center_size(pos + egui::vec2(10.0, 10.0), egui::vec2(30.0, 30.0));
            draw_item_icon(&p, r, cursor.item);
            if cursor.count > 1 {
                stack_count(&p, r, r.width() / screen::SLOT, cursor.count);
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
    orbital: &crate::world::orbital::View,
    action: &mut UiAction,
) {
    *hovered = None;
    // The wand page draws the held wand's own sprite; the bank only knows its tier.
    let look = equipment.active_wand.and_then(|id| {
        survival.hotbar.iter().chain(survival.pack.iter()).flatten().find_map(|s| match s.item {
            Item::Wand { id: held, look } if held == id => Some(look),
            _ => None,
        })
    });
    super::wands::board::publish_look(ctx, look);
    let page_id = egui::Id::new("inventory_page");
    let mut page = ctx.data_mut(|d| d.get_temp::<u32>(page_id).unwrap_or(2));
    let project_return_id = egui::Id::new("recipe_return_to_projects");
    let mut project_return = ctx.data(|d| d.get_temp::<bool>(project_return_id).unwrap_or(false));
    let guide_return_id = egui::Id::new("recipe_return_to_magic_guide");
    let mut guide_return = ctx.data(|d| d.get_temp::<bool>(guide_return_id).unwrap_or(false));
    let reading_guide = page == 6 && super::recipe_book::guide_open(ctx);
    if super::recipe_book::take_request(ctx) {
        project_return |= page == 9;
        guide_return |= reading_guide;
        page = 2;
    }
    let category_id = egui::Id::new("inventory_category");
    let mut group = ctx.data_mut(|d| {
        d.get_temp::<String>(category_id)
            .unwrap_or_else(|| "All".into())
    });
    let catalogue_mode_id = egui::Id::new("creative_catalogue_mode");
    let mut catalogue_mode = ctx.data_mut(|d| d.get_temp::<u8>(catalogue_mode_id).unwrap_or(0));
    let give_count_id = egui::Id::new("creative_give_count");
    let mut give_count = ctx.data_mut(|d| d.get_temp::<u32>(give_count_id).unwrap_or(1));
    let sort_name_id = egui::Id::new("creative_sort_name");
    let mut sort_by_name = ctx.data_mut(|d| d.get_temp::<bool>(sort_name_id).unwrap_or(true));
    let screen = ctx.content_rect();
    let compact = screen.height() < 400.0 || screen.width() < 680.0;
    // An inspected storage node (F) sits beside the pack on the survival page.
    let chest_focus = machines
        .focused
        .as_ref()
        .filter(|(_, node)| node.kind == crate::world::factory::Kind::Storage);
    // Container panel is 360 wide, plus the 12 pt gap that separates it from the pack.
    // Hidden rather than clipped when pack + chest + window margins do not fit.
    let unit = survival_unit(screen, ctx.pixels_per_point());
    let panel_w = screen::WIDTH * unit;
    let item_list_w = super::recipe_book::SIDEBAR_WIDTH;
    let pack_w = if screen.width() < 720.0 { panel_w.max(item_list_w) } else { panel_w + 24.0 + item_list_w };
    let book_w = (screen.width() - 48.0).clamp(220.0, 1040.0);
    let side_w = if survival.container.is_none() && chest_focus.is_some() && pack_w + 372.0 + 48.0 <= screen.width() {
        372.0
    } else {
        0.0
    };

    let window = egui::Window::new("Inventory")
        .frame(egui::Frame::new().fill(super::themes::current(ctx).panel)
            .stroke(egui::Stroke::new(1.0, super::themes::current(ctx).edge))
            .inner_margin(egui::Margin::symmetric(14, 10)))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(if page == 2 {
            // Pack width + gap + recipe column + window margins.
            (pack_w + 28.0_f32 + side_w).min(screen.width() - 20.0)
        } else {
            (screen.width() - 60.0).clamp(240.0, 520.0)
        })
        .min_width(220.0)
        .max_width(match page {
            6 => book_w + 28.0,
            1 => WAND_PAGE_W + 28.0,
            _ => (screen.width() - 20.0).max(220.0),
        })
        .max_height((screen.height() - 16.0).max(200.0));
    let window_response = window
        .show(ctx, |ui| {
            if page == 2 {
                // The survival page has a fixed width: pack and recipe columns.
                // Without it the tab strip claims the whole screen.
                let inv_w = pack_w + side_w;
                ui.set_width(inv_w);
                ui.set_max_width(inv_w);
                // Match catalogue controls to the container's labels instead
                // of inheriting the much larger menu typography.
                for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                    ui.style_mut().text_styles.insert(style, egui::FontId::proportional(15.0));
                }
                ui.spacing_mut().interact_size.y = 26.0;
            } else if page == 6 {
                // Fixed, so the detail and item columns divide a known width
                // instead of growing the auto-sized window past the screen.
                ui.set_width(book_w);
                ui.set_max_width(book_w);
            } else if page == 4 {
                let catalogue_w = (screen.width() - 80.0).clamp(280.0, 720.0);
                ui.set_width(catalogue_w);
                ui.set_max_width(catalogue_w);
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                ui.spacing_mut().button_padding = egui::vec2(10.0, 6.0);
                ui.spacing_mut().interact_size.y = 28.0;
            } else if page == 1 {
                ui.set_width(WAND_PAGE_W);
                ui.set_max_width(WAND_PAGE_W);
            }
            if compact {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);
                ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
                ui.spacing_mut().interact_size.y = 20.0;
                for (kind, size) in [
                    (egui::TextStyle::Heading, 20.0),
                    (egui::TextStyle::Body, 14.0),
                    (egui::TextStyle::Button, 14.0),
                    (egui::TextStyle::Small, 12.0),
                ] {
                    ui.style_mut()
                        .text_styles
                        .insert(kind, egui::FontId::proportional(size));
                }
            }

            if page == 2
                && !ui.ctx().egui_wants_keyboard_input()
                && ui.ctx().input(|i| i.key_pressed(egui::Key::Q))
            {
                *action = UiAction::DropInventory(creative.selected_slot);
            }
            if page == 4 && !creative.catalogue_open()
                || !crate::world::systems::Systems::active().tab_visible(page)
            {
                page = 2;
            }
            // Pages open as the player reaches them; the page on screen always stays reachable.
            let unlocks = unlocks::derive(ctx, creative.catalogue_open(), survival, machines, equipment, expedition.is_some());
            ui.horizontal(|ui| {
                if (unlocks.projects_open() || page == 9)
                    && ui.selectable_label(page == 9, if page == 2 && project_return { "‹ Projects" } else { "Projects" }).clicked()
                {
                    page = 9;
                    *drag = None;
                }
                if let Some(next) = tabs::strip(ui, page, unit, creative.catalogue_open(), unlocks) {
                    if next == tabs::MAPS {
                        super::map_hub::open_from_menu(creative, None);
                    } else {
                        // A tab opens the page itself, not a guide left open earlier.
                        if next == 6 && page != 6 {
                            super::recipe_book::close_magic_guide(ctx);
                        }
                        page = next;
                    }
                    *drag = None;
                }
                if (page == 1 || page == 2 && guide_return)
                    && ui.small_button(if guide_return { "‹ Guide" } else { "Guide" })
                        .on_hover_text("Field Guide to Magic")
                        .clicked()
                {
                    super::recipe_book::open_magic_guide(ctx);
                    page = 6;
                    *drag = None;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("×").on_hover_text("Close [E] [Esc]").clicked() {
                        survival.return_transient_to_pack();
                        creative.show_palette = false;
                        *drag = None;
                    }
                    if page == 4 && ui.button("Materials").clicked() {
                        creative.show_material_editor = true;
                        creative.show_palette = false;
                        *drag = None;
                    }
                });
            });
            ui.separator();

            if machines.workshop.is_some() || machines.count > 0 {
                match super::workshop::navigation(ui, Some(page)) {
                    Some(super::workshop::Destination::Page(next)) => {
                        page = next;
                        *drag = None;
                    }
                    Some(super::workshop::Destination::Map) => {
                        survival.return_transient_to_pack();
                        *drag = None;
                        *action = UiAction::WorkshopMap(None);
                        return;
                    }
                    None => {}
                }
                ui.separator();
            }
            if page == super::workshop::LOGISTICS {
                super::workshop::logistics(ui, machines, &mut page, action);
                return;
            }

            // Page 2: Minecraft-style Survival Inventory (Armor + Character + 2×2 Craft + 27 Pack + 9 Hotbar)
            if page == 2 {
                let mut open_wand_tab = false;
                ui.horizontal_top(|ui| {
                    if side_w > 0.0 && let Some((pos, node)) = chest_focus {
                        ui.vertical(|ui| super::factory::container_panel(ui, *pos, node, action));
                        ui.add_space(12.0);
                    }
                    ui.vertical(|ui| {
                        draw_survival_pack_view(
                            ui,
                            creative,
                            player,
                            survival,
                            equipment,
                            action,
                            unit,
                            &mut open_wand_tab,
                            machines.bench,
                            machines.cells_per_metre,
                        );
                    });
                });
                if open_wand_tab {
                    page = 1;
                }
                return;
            }

            if page == 8 {
                super::orbital::draw(ui, orbital, action);
                return;
            }

            if page == 10 { super::records::page(ui); return; }
            if page == 9 {
                super::projects::draw(ui, equipment, survival, machines, unlocks, &mut page);
                return;
            }

            // Page 7: Factorio-style production statistics.
            if page == 7 {
                super::production::draw(ui, machines, action);
                return;
            }

            // Page 6: JEI-style recipe browser and crafting-tree planner.
            if page == 6 {
                let staged = egui::ScrollArea::vertical().id_salt("recipe_page")
                    .max_height((screen.height() - 140.0).max(80.0))
                    .auto_shrink([false, false]).show(ui, |ui| {
                        let selected = ctx.data(|d| d.get_temp::<Option<Item>>(egui::Id::new("jei_selected"))).flatten();
                        if !super::recipe_book::guide_open(ctx) && super::semiconductor::relevant(selected) {
                            super::semiconductor::section(ui, machines, action);
                        }
                        super::recipe_book::draw(ui, survival, machines.bench, machines.cells_per_metre)
                    }).inner;
                if staged {
                    page = 2;
                }
                return;
            }

            // Page 4: creative catalogue. Blocks keep the original drag/palette
            // path used by the torch review capture; equipment selection uses
            // the shared host/local transaction, preserving displaced items.
            if page == 4 {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new("Catalogue")
                            .size(13.0)
                            .color(presentation::AMBER),
                    );
                    if catalogue_mode == 1 {
                        ui.separator();
                        ui.label(RichText::new("Quantity").size(13.0).color(presentation::MUTED));
                        for amount in [1u32, 8, 64] {
                            if ui
                                .selectable_label(give_count == amount, format!("×{amount}"))
                                .clicked()
                            {
                                give_count = amount;
                            }
                        }
                        ui.label(
                            RichText::new("[Shift] Full Stack")
                                .size(13.0)
                                .color(presentation::MUTED),
                        );
                    }
                });
                if catalogue::tabs(ui, &mut catalogue_mode, &mut group) { *drag = None; }
                ui.horizontal_wrapped(|ui| {
                    ui.label(if group == "All" { if catalogue_mode == 0 { "Blocks" } else { "Items" } } else { &group });
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
                                    && catalogue::matches(Item::Block(m.id), &query)
                                    && (group == "All" || category(m.id) == group)
                            })
                            .collect();
                        if sort_by_name {
                            list.sort_by_key(|m| m.display_name);
                        } else {
                            list.sort_by_key(|m| m.id.as_u32());
                        }
                        let items: Vec<_> = list.into_iter().take(HOTBAR_SLOTS)
                            .map(|def| Item::Block(def.id)).collect();
                        if !items.is_empty() { *action = UiAction::SelectCreativeRow(items); }
                        creative.tool = ToolKind::Hand;
                        creative.show_panel = false;
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
                        - if page == 4 { 64.0 } else { 48.0 })
                    .clamp(48.0, (screen.height() - 128.0).max(48.0)),
                )
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if page == 5 {
                        super::tool_station::page(ui, equipment, survival, machines.bench, action);
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
                    draw_hotbar_row(ui, creative, player, Some(survival), drag, drop_slot);
                });
            }
        });
    if let Some(inner) = &window_response {
        super::prism::edge(&ctx.layer_painter(inner.response.layer_id), inner.response.rect, 0.75, false);
        grimoire::binding(&ctx.layer_painter(inner.response.layer_id), inner.response.rect, true);
    }
    if creative.show_palette && page == 2 {
        super::recipe_book::detail_window(ctx, survival, machines.bench, machines.cells_per_metre);
    }
    ctx.data_mut(|d| {
        if let Some(inner) = &window_response {
            d.insert_temp(egui::Id::new("inventory_window_rect"), inner.response.rect);
        }
        d.insert_temp(page_id, page);
        d.insert_temp(project_return_id, page == 2 && creative.show_palette && project_return);
        d.insert_temp(guide_return_id, page == 2 && creative.show_palette && guide_return);
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
    let edge = if compact { 40.0 } else { 48.0 };
    let columns = ((ui.available_width() + 4.0) / (edge + 4.0)).floor().max(1.0) as usize;
    let card = [edge, edge];
    if mode == 0 {
        let mut matches: Vec<_> = material_defs()
            .iter()
            .filter(|m| {
                m.tags.selectable
                    && catalogue::matches(Item::Block(m.id), query)
                    && (group == "All" || category(m.id) == group)
            })
            .collect();
        if sort_by_name {
            matches.sort_by_key(|m| m.display_name);
        } else {
            matches.sort_by_key(|m| m.id.as_u32());
        }
        if matches.is_empty() {
            ui.add_space(16.0);
            ui.weak("No Matching Blocks");
            return;
        }
        egui::Grid::new("inventory_blocks")
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                for (index, def) in matches.iter().enumerate() {
                    let response = ui
                        .push_id(def.id.as_u32(), |ui| {
                            catalogue::slot(ui, card, Item::Block(def.id)).on_hover_text(def.display_name)
                        })
                        .inner;
                    if crate::torch_review::mode() == "drag" && def.id == BlockId::Torch {
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(egui::Id::new("review_torch_card"), response.rect)
                        });
                    }
                    if survival.hotbar[creative.selected_slot]
                        .is_some_and(|stack| stack.item == Item::Block(def.id)) {
                        ui.painter().rect_stroke(
                            response.rect,
                            0,
                            egui::Stroke::new(2.0, presentation::JADE),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let shortcut_slot = if response.hovered() && !ui.ctx().egui_wants_keyboard_input() {
                        [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3,
                         egui::Key::Num4, egui::Key::Num5, egui::Key::Num6,
                         egui::Key::Num7, egui::Key::Num8, egui::Key::Num9]
                            .into_iter().position(|key| ui.input_mut(|input|
                                input.consume_key(egui::Modifiers::NONE, key)))
                    } else { None };
                    if let Some(slot) = shortcut_slot.or_else(||
                        (response.clicked() || response.clicked_by(egui::PointerButton::Middle)
                            || response.clicked_by(egui::PointerButton::Secondary))
                            .then_some(creative.selected_slot)) {
                        creative.assign_hotbar_slot(slot, def.id);
                        creative.tool = ToolKind::Hand;
                        creative.show_panel = false;
                        let item = Item::Block(def.id);
                        let count = if response.clicked_by(egui::PointerButton::Secondary) { 1 }
                            else { survival.stack_limit(item) };
                        *action = UiAction::SelectCreativeItem(item, count, slot);
                        if slot == creative.selected_slot { player.selected_block = def.id; }
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
            catalogue::matches(*item, query)
                && (group == "All" || creative_item_group(*item) == group)
        })
        .collect();
    if sort_by_name {
        items.sort_by_key(|item| item.name());
    } else {
        items.sort_by_key(|item| item.encode_u32());
    }
    if items.is_empty() {
        ui.add_space(16.0);
        ui.weak("No Matching Items");
    }
    egui::Grid::new("inventory_items")
        .spacing([4.0, 4.0])
        .show(ui, |ui| {
            for (index, item) in items.iter().enumerate() {
                let response = ui
                    .push_id(("creative_item", item.encode_u32()), |ui| {
                        catalogue::slot(ui, card, *item)
                    })
                    .inner;
                if survival.hotbar[creative.selected_slot].is_some_and(|stack| stack.item == *item) {
                    screen::selected(ui.painter(), response.rect, 1.0);
                }
                let full = ui.input(|i| i.modifiers.shift);
                let limit = survival.stack_limit(*item);
                let amount = if full { limit } else { give_count.min(limit) };
                response.clone().on_hover_ui(|ui| {
                    ui.set_max_width(220.0);
                    ui.strong(RichText::new(item.name()).size(13.0));
                    descriptions::details(ui, *item);
                    ui.small(format!("[LMB] ×{amount} · [RMB] ×1 · [1–9] Hotbar"));
                });
                let shortcut = catalogue::shortcut(ui, &response);
                if response.clicked() || response.clicked_by(egui::PointerButton::Secondary)
                    || response.clicked_by(egui::PointerButton::Middle) || shortcut.is_some() {
                    let count = if response.clicked_by(egui::PointerButton::Secondary) { 1 }
                        else if response.clicked_by(egui::PointerButton::Middle) { limit } else { amount };
                    creative.tool = ToolKind::Hand;
                    creative.show_panel = false;
                    *action = UiAction::SelectCreativeItem(*item, count, shortcut.unwrap_or(creative.selected_slot));
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
