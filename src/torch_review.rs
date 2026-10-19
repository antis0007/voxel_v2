//! Opt-in deterministic visual fixture. Runs only in an unsaved benchmark world.
use crate::{player::{Player, FlyMode}, world::{World, BlockId}};
use glam::{Vec3, IVec3};

// Opt-in renderer integration replay: feeds the same egui events as native
// mouse input, using rectangles recorded by the real inventory layout.
#[derive(Default)]
struct DragReview {
    started: Option<std::time::Instant>,
    last_step: Option<usize>,
    checked: usize,
    before_cancel: Option<[BlockId; 9]>,
}
static DRAG: std::sync::Mutex<DragReview> = std::sync::Mutex::new(DragReview {
    started: None, last_step: None, checked: 0, before_cancel: None,
});

pub fn drag_input(ctx: &egui::Context, input: &mut egui::RawInput) {
    let rect = |id| ctx.data_mut(|d| d.get_temp::<egui::Rect>(id));
    let (Some(card), Some(slot1), Some(slot4)) = (
        rect(egui::Id::new("review_torch_card")),
        rect(egui::Id::new(("review_hotbar", 1usize))),
        rect(egui::Id::new(("review_hotbar", 4usize))),
    ) else { return; };
    input.focused = true;
    let mut review = DRAG.lock().unwrap();
    let elapsed = review.started.get_or_insert_with(std::time::Instant::now).elapsed().as_secs_f32();
    let step = (elapsed / 0.35) as usize;
    if review.last_step == Some(step) { return; }
    review.last_step = Some(step);
    let mut pointer = |pos, pressed| {
        input.events.push(egui::Event::PointerMoved(pos));
        input.events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary,
            pressed, modifiers: egui::Modifiers::NONE });
    };
    match step {
        3 | 17 => pointer(card.center(), true),
        4 | 18 => input.events.push(egui::Event::PointerMoved(card.center() + egui::vec2(14.0, 0.0))),
        5 => input.events.push(egui::Event::PointerMoved(slot4.center())),
        6 => pointer(slot4.center(), false),
        10 | 24 => pointer(slot4.center(), true),
        11 | 25 => input.events.push(egui::Event::PointerMoved(slot4.center() + egui::vec2(14.0, 0.0))),
        12 => input.events.push(egui::Event::PointerMoved(slot1.center())),
        13 => pointer(slot1.center(), false),
        19 => input.events.push(egui::Event::PointerMoved(egui::pos2(1.0, 1.0))),
        20 => pointer(egui::pos2(1.0, 1.0), false),
        26 => { input.focused = false; input.events.push(egui::Event::WindowFocused(false)); },
        27 => pointer(slot1.center(), false),
        _ => {}
    }
}

pub fn check_drag(creative: &crate::creative::CreativeState, dragging: bool) {
    let mut review = DRAG.lock().unwrap();
    let step = review.last_step.unwrap_or(0);
    let checkpoint = if step >= 28 { 4 } else if step >= 21 { 3 } else if step >= 14 { 2 } else if step >= 7 { 1 } else { 0 };
    if checkpoint <= review.checked { return; }
    let passed = match checkpoint {
        1 => creative.hotbar()[4] == BlockId::Torch && creative.selected_slot == 4 && !dragging,
        2 => creative.hotbar()[1] == BlockId::Torch && creative.hotbar()[4] == BlockId::Grass && creative.selected_slot == 1 && !dragging,
        _ => review.before_cancel.as_ref() == Some(creative.hotbar()) && !dragging,
    };
    if checkpoint == 2 { review.before_cancel = Some(*creative.hotbar()); }
    let name = ["", "palette_to_hotbar", "hotbar_swap", "outside_cancel", "focus_loss_cancel"][checkpoint];
    println!("[torch-drag] {name} {} hotbar={:?} selected={} dragging={dragging}", if passed { "PASS" } else { "FAIL" }, creative.hotbar(), creative.selected_slot);
    review.checked = checkpoint;
}

pub fn mode() -> &'static str {
    static MODE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    MODE.get_or_init(|| std::env::var("VOXEL_TORCH_REVIEW_MODE").unwrap_or_default())
}

pub fn motion_camera(player: &mut Player, scale: f32, elapsed: f32) {
    camera(player, scale);
    if mode() == "motion" {
        // Retreat across the detail bands, then return through the same boundaries.
        let t = ((elapsed - 8.0) / 72.0).clamp(0.0, 1.0);
        player.pos.z += (56.0 * (std::f32::consts::PI * t).sin()) / scale;
        player.pitch = -0.04;
    }
}

pub fn camera(player: &mut Player, scale: f32) {
    let z_dist = if mode() == "survival" { 2.2 } else { 4.5 };
    player.pos = Vec3::new(0.5 * scale, 100.0, z_dist + 0.5 * scale) / scale;
    player.yaw = 0.0;
    player.pitch = -0.15;
    player.vel = Vec3::ZERO;
    player.flying = true;
    player.fly_mode = FlyMode::Noclip;
    player.crouch_t = 0.0;
    player.selected_block = BlockId::Stone;
}

pub fn install(world: &mut World) {
    let scale = world.voxel_scale.meters();
    let n = (1.0 / scale) as i32;
    let mut edits = Vec::new();
    for y in 99*n..104*n { for z in -2*n..3*n { for x in -3*n..3*n {
        let block = if y < 100*n || (x >= 0 && x < 2*n && z >= -n && z < 0 && y < 103*n) {
            BlockId::StoneBricks
        } else { BlockId::Air };
        edits.push((IVec3::new(x,y,z),block));
    } } }
    edits.push((IVec3::new(-n,100*n,0),BlockId::Torch));
    edits.push((IVec3::new(n,101*n,0),BlockId::TorchWallSouth));
    world.set_many(&edits);
    // Representative collected/refined stacks for the inventory capture.
    for (i, count) in [(0, 1536), (3, 320), (4, 160), (6, 640), (12, 64), (18, 96)] {
        world.expedition.resources[i] = count;
    }
    world.reconcile_legacy_to_inventory();
    use crate::world::inventory::{ArmorTier, EquipSlot, FoodKind, Item, ItemStack};
    world.inventory.equipment[0] = Some(ItemStack::new(
        Item::Armor(EquipSlot::Head, ArmorTier::Copper),
        1,
    ));
    world.inventory.equipment[1] = Some(ItemStack::new(
        Item::Armor(EquipSlot::Chest, ArmorTier::Steel),
        1,
    ));
    world.inventory.hotbar[0] = Some(ItemStack::new(Item::Weapon(2), 1));
    world.inventory.hotbar[1] = Some(ItemStack::new(Item::Food(FoodKind::RoastedCap), 6));
    world.inventory.hotbar[2] = Some(ItemStack::new(Item::Block(BlockId::Torch), 24));
    world.inventory.hotbar[3] = Some(ItemStack::new(Item::Block(BlockId::StoneBricks), 48));
    world.inventory.craft_grid[0] = Some(ItemStack::new(Item::Block(BlockId::Log), 3));
    world.sync_from_inventory();
}

pub fn apply_survival_phase(
    world: &mut World,
    player: &mut Player,
    creative: &mut crate::creative::CreativeState,
    egui_ctx: &egui::Context,
    phase: usize,
) {
    player.flying = false;
    creative.show_panel = false;
    creative.tool = crate::creative::ToolKind::Hand;
    match phase {
        0 => {
            // Phase 0: HUD gameplay - full health & hunger, aimed at StoneBricks block (HWYLA top pill + hotbar + hearts + haunches)
            creative.show_palette = false;
            creative.selected_slot = 0;
            world.adventure.health = 100.0;
            world.adventure.hunger = 100.0;
            world.inventory.eat_timer = 0.0;
            world.inventory.starve_timer = 0.0;
        }
        1 => {
            // Phase 1: HUD gameplay - partial hunger + eating flash + pickup toast
            creative.show_palette = false;
            creative.selected_slot = 1;
            world.adventure.health = 85.0;
            world.adventure.hunger = 65.0;
            world.inventory.eat_timer = 0.75;
            world.inventory.notify_pickup(
                crate::world::inventory::Item::Block(BlockId::StoneBricks),
                4,
            );
        }
        2 => {
            // Phase 2: HUD gameplay - low hunger / starvation state + full pack overflow notice
            creative.show_palette = false;
            creative.selected_slot = 2;
            world.adventure.health = 45.0;
            world.adventure.hunger = 5.0;
            world.inventory.eat_timer = 0.0;
            world.inventory.starve_timer = 0.85;
            world.inventory.notify_overflow(crate::world::inventory::Item::Block(BlockId::Stone));
        }
        _ => {
            // Phase 3: E-key survival inventory screen open with 2x2 crafting grid & armor preview
            creative.show_palette = true;
            world.adventure.health = 90.0;
            world.adventure.hunger = 80.0;
            world.inventory.eat_timer = 0.0;
            world.inventory.starve_timer = 0.0;
            egui_ctx.data_mut(|d| d.insert_temp(egui::Id::new("inventory_page"), 2u32));
        }
    }
}
