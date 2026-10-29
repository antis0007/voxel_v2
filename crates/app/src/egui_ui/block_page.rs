//! Sign and lectern text pages, the chest grid, station pages, and sign text over the world.
use super::presentation as style;
use crate::world::block_data::{self, CHEST_SLOTS};
use crate::world::inventory::{HOTBAR_SLOTS, ItemStack, PACK_SLOTS, SlotAddr};
use crate::world::{BlockId, World};
use engine_content::shaped;
use glam::{IVec3, Mat4, Vec3};

/// Sign text is drawn within this distance, in metres.
const LABEL_RANGE_M: f32 = 8.0;
const SLOT_PX: f32 = 40.0;

/// Draws in-world sign text and, while `open` names a block, its page.
pub fn draw(ctx: &egui::Context, open: &mut Option<IVec3>, world: &mut World, eye: Vec3, view_proj: Mat4) {
    labels(ctx, world, eye, view_proj);
    let Some(p) = *open else { return };
    let block = world.get(p);
    if let Some(limits) = shaped::text_limits(block) {
        text_page(ctx, open, world, p, block, limits);
    } else if block == BlockId::Chest {
        chest_page(ctx, open, world, p);
    } else if !super::station_page::draw(ctx, open, world, p, block) {
        *open = None;
    }
}

fn text_page(ctx: &egui::Context, open: &mut Option<IVec3>, world: &mut World, p: IVec3, block: BlockId, (lines, chars): (usize, usize)) {
    let id = egui::Id::new("block_page_text");
    let mut draft: (IVec3, Vec<String>) = ctx
        .data_mut(|d| d.get_temp(id))
        .filter(|(at, _): &(IVec3, Vec<String>)| *at == p)
        .unwrap_or_else(|| (p, world.block_data.text(p).to_vec()));
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
        ctx.data_mut(|d| d.remove::<(IVec3, Vec<String>)>(id));
        *open = None;
    } else {
        ctx.data_mut(|d| d.insert_temp(id, draft));
    }
}

pub(super) fn slot(ui: &mut egui::Ui, stack: Option<ItemStack>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(SLOT_PX, SLOT_PX), egui::Sense::click());
    let painter = ui.painter();
    super::icons::frame(painter, rect, response.hovered());
    if let Some(stack) = stack {
        super::inventory::draw_item_icon(painter, rect.shrink(3.0), stack.item);
        if stack.count > 1 {
            painter.text(rect.right_bottom() - egui::vec2(3.0, 2.0), egui::Align2::RIGHT_BOTTOM, stack.count.to_string(), egui::FontId::proportional(11.0), style::PAPER);
        }
        return response.on_hover_text(format!("{} × {}", stack.item.name(), stack.count));
    }
    response
}

pub(super) fn grid(ui: &mut egui::Ui, name: &str, stacks: &[Option<ItemStack>]) -> Option<usize> {
    let mut clicked = None;
    egui::Grid::new(name).spacing(egui::vec2(2.0, 2.0)).show(ui, |ui| {
        for (i, stack) in stacks.iter().enumerate() {
            if slot(ui, *stack).clicked() {
                clicked = Some(i);
            }
            if i % 9 == 8 {
                ui.end_row();
            }
        }
    });
    clicked
}

fn chest_page(ctx: &egui::Context, open: &mut Option<IVec3>, world: &mut World, p: IVec3) {
    let slots = world.block_data.chest(p);
    let pack: Vec<Option<ItemStack>> = world.inventory.pack.iter().chain(world.inventory.hotbar.iter()).copied().collect();
    let (mut take, mut store, mut done) = (None, None, false);
    egui::Window::new("Chest")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            take = grid(ui, "chest_slots", &slots[..CHEST_SLOTS]);
            ui.separator();
            ui.label(egui::RichText::new("Pack").color(style::MUTED));
            store = grid(ui, "chest_pack", &pack);
            done = ui.button("Done").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
        });
    if let Some(i) = take {
        block_data::take(world, p, i);
    }
    if let Some(i) = store {
        let addr = if i < PACK_SLOTS { SlotAddr::Pack(i) } else { SlotAddr::Hotbar((i - PACK_SLOTS).min(HOTBAR_SLOTS - 1)) };
        block_data::store(world, p, addr);
    }
    if take.is_some() || store.is_some() {
        world.sync_from_inventory();
    }
    if done {
        *open = None;
    }
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

