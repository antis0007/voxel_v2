//! Sign and lectern text pages, the chest grid with its storage panel, station pages, and sign text over the world.
use super::presentation as style;
use super::slot_grid::SlotGrid;
use crate::world::block_data::{self};
use crate::world::inventory::{ItemStack, PACK_SLOTS, SlotAddr};
use crate::world::{BlockId, World, scanner};
use engine_content::shaped;
use glam::{IVec3, Mat4, Vec3};

/// Sign text is drawn within this distance, in metres.
const LABEL_RANGE_M: f32 = 8.0;
const SLOT_PX: f32 = 40.0;

/// Draws in-world sign text and, while `open` names a block, its page.
pub fn draw(ctx: &egui::Context, open: &mut Option<IVec3>, world: &mut World, eye: Vec3, view_proj: Mat4, network: bool) {
    labels(ctx, world, eye, view_proj);
    super::storage_scan::highlight(ctx, world, view_proj);
    let Some(p) = *open else { return };
    let block = world.get(p);
    if (eye - p.as_vec3() - Vec3::splat(0.5)).length() * world.voxel_scale.meters() > 6.0 {
        if !network && world.inventory.cursor.is_some() { world.inventory.return_transient_to_pack(); world.sync_from_inventory(); }
        *open = None; return;
    }
    if network && block.chest_tier().is_none() { *open = None; return; }
    if let Some(limits) = shaped::text_limits(block) {
        text_page(ctx, open, world, p, block, limits);
    } else if block.chest_tier().is_some() {
        chest_page(ctx, open, world, p, eye * world.voxel_scale.meters(), network);
    } else if !super::station_page::draw(ctx, open, world, p, block) {
        world.inventory.return_transient_to_pack();
        world.progression.wear.reconcile(&world.inventory);
        world.sync_from_inventory();
        *open = None;
    }
}

fn text_page(ctx: &egui::Context, open: &mut Option<IVec3>, world: &mut World, p: IVec3, block: BlockId, (lines, chars): (usize, usize)) {
    let id = egui::Id::new("block_page_text");
    let mut draft: (IVec3, Vec<String>) = ctx
        .data_mut(|d| d.get_temp(id))
        .filter(|(at, _): &(IVec3, Vec<String>)| *at == p)
        .unwrap_or_else(|| {
            let text = world.block_data.text(p);
            // An unwritten dungeon lectern reads its quest summary.
            let quest = (block == BlockId::Lectern && text.iter().all(|l| l.is_empty()))
                .then(|| crate::world::dungeon_quest::lectern_lines(world, p))
                .flatten();
            (p, quest.unwrap_or_else(|| text.to_vec()))
        });
    draft.1.resize(lines, String::new());
    let mut done = false;
    egui::Window::new(block.display_name())
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            for line in &mut draft.1 {
                ui.add(egui::TextEdit::singleline(line).char_limit(chars).desired_width(chars as f32 * 9.0).font(egui::TextStyle::Monospace));
            }
            done = ui.button("Done").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
        });
    if done {
        block_data::set_text(world, p, &draft.1);
        super::map::inspect_record(world, p, world.block_data.text(p));
        ctx.data_mut(|d| d.remove::<(IVec3, Vec<String>)>(id));
        *open = None;
    } else {
        ctx.data_mut(|d| d.insert_temp(id, draft));
    }
}

pub(super) fn slot(ui: &mut egui::Ui, stack: Option<ItemStack>) -> egui::Response {
    slot_sized(ui, stack, SLOT_PX)
}

pub(super) fn slot_sized(ui: &mut egui::Ui, stack: Option<ItemStack>, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    paint_slot(ui, rect, response, stack)
}

/// Paints one item slot into a rect already allocated by the caller.
pub(super) fn paint_slot(ui: &egui::Ui, rect: egui::Rect, response: egui::Response, stack: Option<ItemStack>) -> egui::Response {
    let painter = ui.painter();
    super::icons::frame(painter, rect, response.hovered());
    if let Some(stack) = stack {
        super::inventory::draw_item_icon(painter, rect.shrink(3.0), stack.item);
        if stack.count > 1 {
            painter.text(rect.right_bottom() - egui::vec2(2.0, 1.0), egui::Align2::RIGHT_BOTTOM, stack.count.to_string(), egui::FontId::proportional(if rect.width() < 32.0 { 9.0 } else { 11.0 }), style::PAPER);
        }
        return response.on_hover_text(format!("{} × {}", stack.item.name(), stack.count));
    }
    response
}

/// Columns of every player-facing item grid, matching the hotbar.
pub(super) const ROW: usize = 9;
/// Cell side of compact container grids, in points.
pub(super) const COMPACT_PX: f32 = 26.0;

pub(super) fn grid(ui: &mut egui::Ui, name: &str, stacks: &[Option<ItemStack>]) -> Option<usize> {
    let mut clicked = None;
    SlotGrid::new(ROW, SLOT_PX).show(ui, name, stacks.len(), |ui, i, rect, response| {
        if paint_slot(ui, rect, response, stacks[i]).clicked() {
            clicked = Some(i);
        }
    });
    clicked
}

fn chest_page(ctx: &egui::Context, open: &mut Option<IVec3>, world: &mut World, p: IVec3, eye_m: Vec3, network: bool) {
    let slots = world.block_data.chest(p);
    let pack: Vec<Option<ItemStack>> = world.inventory.pack.iter().chain(world.inventory.hotbar.iter()).copied().collect();
    let (mut take, mut store, mut done, mut bulk) = (None, None, false, None);
    egui::Window::new("Chest")
        .id(egui::Id::new("compact_storage_scanner"))
        .title_bar(false)
        .collapsible(false)
        .auto_sized()
        .default_width(344.0).min_width(344.0).max_width(344.0)
        .max_height(ctx.content_rect().height() - 24.0)
        .vscroll(true)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            let local = ui.style_mut();
            for (kind, size) in [(egui::TextStyle::Body, 11.0), (egui::TextStyle::Button, 11.0), (egui::TextStyle::Small, 10.0), (egui::TextStyle::Heading, 13.0), (egui::TextStyle::Monospace, 10.0)] {
                if let Some(font) = local.text_styles.get_mut(&kind) { font.size = size; }
            }
            local.spacing.button_padding = egui::vec2(5.0, 2.0);
            local.spacing.item_spacing = egui::vec2(4.0, 3.0);
            local.spacing.interact_size = egui::vec2(18.0, 18.0);
            local.spacing.combo_width = 92.0;
            ui.horizontal(|ui| {
                ui.strong(world.get(p).display_name());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    done = ui.button("×").on_hover_text("Close · [Esc]").clicked();
                });
            });
            {
                ui.weak(format!("{} / {} Slots", slots.iter().flatten().count(), slots.len()));
                take = compact_grid(ui, "chest_slots", &slots);
                ui.horizontal(|ui| {
                    if ui.button("Take All").clicked() {
                        bulk = Some(block_data::ChestAction::TakeAll);
                    }
                    if ui.button("Stack Matching").on_hover_text("Deposit matching items").clicked() {
                        bulk = Some(block_data::ChestAction::StackMatching);
                    }
                    if ui.button("Sort").clicked() {
                        bulk = Some(block_data::ChestAction::Sort);
                    }
                });
                ui.separator();
                ui.strong("Inventory");
                store = compact_grid(ui, "chest_pack", &pack[..PACK_SLOTS]);
                ui.add_space(3.0);
                if let Some((i, gesture)) = compact_grid(ui, "chest_hotbar", &pack[PACK_SLOTS..]) {
                    store = Some((PACK_SLOTS + i, gesture));
                }
                ui.separator();
                egui::CollapsingHeader::new("Slot Controls").show(ui, |ui| {
                    ui.small("[LMB] Stack · [RMB] Split / Place One");
                    ui.small("[Shift + LMB] Transfer · [Double Click] Gather · [1–9] Swap");
                });
                egui::CollapsingHeader::new("Storage Scanner").show(ui, |ui| {
                    super::storage_scan::panel(ui, world, p, eye_m, network);
                });
            }
            done |= ui.input(|i| i.key_pressed(egui::Key::Escape));
        });
    let request = take.map(|(i, g)| block_data::ChestAction::Chest(i, g))
        .or_else(|| store.map(|(i, g)| block_data::ChestAction::Player(
            if i < PACK_SLOTS { SlotAddr::Pack(i) } else { SlotAddr::Hotbar(i - PACK_SLOTS) }, g)))
        .or(bulk);
    if let Some(request) = request {
        if network {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("scanner_request"),
                (p, scanner::commands::Action::Chest(request))));
        } else {
            block_data::edit_chest(world, p, request);
        }
    }
    if done && !network && world.inventory.cursor.is_some() {
        world.inventory.return_transient_to_pack();
    }
    if !network && done {
        world.sync_from_inventory();
    }
    if done { *open = None; }
    else { paint_cursor(ctx, world); }
}

pub(super) fn paint_cursor(ctx: &egui::Context, world: &World) {
    if let Some(stack) = world.inventory.cursor
        && let Some(pos) = ctx.pointer_hover_pos() {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("chest_cursor")));
        let rect = egui::Rect::from_min_size(pos + egui::vec2(4.0, 4.0), egui::Vec2::splat(24.0));
        super::inventory::draw_item_icon(&painter, rect, stack.item);
        if stack.count > 1 { super::inventory::stack_count(&painter, rect, 1.0, stack.count); }
    }
}

pub(super) use block_data::SlotGesture as Gesture;

pub(super) fn slot_gesture(ui: &egui::Ui, response: egui::Response) -> Option<Gesture> {
    if response.secondary_clicked() { Some(Gesture::Split) }
    else if response.clicked() { Some(if ui.input(|i| i.modifiers.shift) { Gesture::Transfer } else { Gesture::Stack }) }
    else { None }
}

pub(super) fn compact_grid(ui: &mut egui::Ui, id: &str, stacks: &[Option<ItemStack>]) -> Option<(usize, Gesture)> {
    let mut clicked = None;
    SlotGrid::new(ROW, COMPACT_PX).show(ui, id, stacks.len(), |ui, i, rect, response| {
        {
            let response = paint_slot(ui, rect, response, stacks[i]);
            let hotbar = if response.hovered() && !ui.ctx().egui_wants_keyboard_input() {
                [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4,
                    egui::Key::Num5, egui::Key::Num6, egui::Key::Num7, egui::Key::Num8, egui::Key::Num9]
                    .iter().position(|&key| ui.input(|input| input.key_pressed(key)))
            } else { None };
            if let Some(index) = hotbar { clicked = Some((i, Gesture::Hotbar(index as u8))); }
            else if response.double_clicked() && !ui.input(|input| input.modifiers.shift) {
                clicked = Some((i, Gesture::Gather));
            } else if let Some(gesture) = slot_gesture(ui, response) { clicked = Some((i, gesture)); }
        }
    });
    clicked
}

/// Interim: sign text is painted by egui at the projected sign face, not in the world mesh.
fn labels(ctx: &egui::Context, world: &World, eye: Vec3, view_proj: Mat4) {
    let metres = world.voxel_scale.meters();
    let range = LABEL_RANGE_M / metres;
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let near = world.block_data.entries.range(key_bounds(eye, range, -1)..=key_bounds(eye, range, 1));
    for (&(x, y, z), entry) in near {
        let p = IVec3::new(x, y, z);
        let block_data::Entry::Text(lines) = entry else { continue };
        let Some((centre, normal, right)) = shaped::text_face(world.get(p), p) else { continue };
        let to_eye = eye - centre;
        if to_eye.length() > range || to_eye.dot(normal) <= 0.0 {
            continue;
        }
        // Without a depth test, a wall between the eye and the sign hides its text here.
        if crate::world::raycast_block(world, eye, -to_eye.normalize_or_zero(), to_eye.length() + 0.5).is_some_and(|(hit, _)| hit != p) {
            continue;
        }
        let project = |v: Vec3| {
            let c = view_proj * v.extend(1.0);
            (c.w > 1e-4).then(|| egui::pos2(screen.left() + (c.x / c.w * 0.5 + 0.5) * screen.width(), screen.top() + (0.5 - c.y / c.w * 0.5) * screen.height()))
        };
        let (Some(mid), Some(edge)) = (project(centre), project(centre + right * 0.45)) else { continue };
        let half = (edge - mid).length();
        if half < 6.0 || !screen.contains(mid) {
            continue;
        }
        // Board height is half a cell; four lines fill it.
        let size = (half * 0.2).clamp(6.0, 40.0);
        let top = mid.y - size * 1.1 * lines.len() as f32 * 0.5;
        for (i, line) in lines.iter().enumerate() {
            let at = egui::pos2(mid.x, top + size * 1.1 * (i as f32 + 0.5));
            painter.text(at, egui::Align2::CENTER_CENTER, line, egui::FontId::monospace(size), egui::Color32::from_rgb(40, 28, 18));
        }
    }
}

/// Lower or upper x bound of the record range near `eye`; y and z are filtered later.
fn key_bounds(eye: Vec3, range: f32, side: i32) -> block_data::Key {
    let x = (eye.x + range * side as f32).floor() as i32;
    let fill = if side < 0 { i32::MIN } else { i32::MAX };
    (x, fill, fill)
}

