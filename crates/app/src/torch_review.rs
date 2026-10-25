//! Opt-in deterministic visual fixture. Runs only in an unsaved benchmark world.
use crate::{
    player::{FlyMode, Player},
    world::{BlockId, World},
};
use glam::{IVec3, Vec3};

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
    started: None,
    last_step: None,
    checked: 0,
    before_cancel: None,
});

pub fn drag_input(ctx: &egui::Context, input: &mut egui::RawInput) {
    let rect = |id| ctx.data_mut(|d| d.get_temp::<egui::Rect>(id));
    let (Some(card), Some(slot1), Some(slot4)) = (
        rect(egui::Id::new("review_torch_card")),
        rect(egui::Id::new(("review_hotbar", 1usize))),
        rect(egui::Id::new(("review_hotbar", 4usize))),
    ) else {
        return;
    };
    input.focused = true;
    let mut review = DRAG.lock().unwrap();
    let elapsed = review
        .started
        .get_or_insert_with(std::time::Instant::now)
        .elapsed()
        .as_secs_f32();
    let step = (elapsed / 0.35) as usize;
    if review.last_step == Some(step) {
        return;
    }
    review.last_step = Some(step);
    let mut pointer = |pos, pressed| {
        input.events.push(egui::Event::PointerMoved(pos));
        input.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    };
    match step {
        3 | 17 => pointer(card.center(), true),
        4 | 18 => input.events.push(egui::Event::PointerMoved(
            card.center() + egui::vec2(14.0, 0.0),
        )),
        5 => input.events.push(egui::Event::PointerMoved(slot4.center())),
        6 => pointer(slot4.center(), false),
        10 | 24 => pointer(slot4.center(), true),
        11 | 25 => input.events.push(egui::Event::PointerMoved(
            slot4.center() + egui::vec2(14.0, 0.0),
        )),
        12 => input.events.push(egui::Event::PointerMoved(slot1.center())),
        13 => pointer(slot1.center(), false),
        19 => input
            .events
            .push(egui::Event::PointerMoved(egui::pos2(1.0, 1.0))),
        20 => pointer(egui::pos2(1.0, 1.0), false),
        26 => {
            input.focused = false;
            input.events.push(egui::Event::WindowFocused(false));
        }
        27 => pointer(slot1.center(), false),
        _ => {}
    }
}

pub fn check_drag(creative: &crate::creative::CreativeState, dragging: bool) {
    let mut review = DRAG.lock().unwrap();
    let step = review.last_step.unwrap_or(0);
    let checkpoint = if step >= 28 {
        4
    } else if step >= 21 {
        3
    } else if step >= 14 {
        2
    } else if step >= 7 {
        1
    } else {
        0
    };
    if checkpoint <= review.checked {
        return;
    }
    let passed = match checkpoint {
        1 => creative.hotbar()[4] == BlockId::Torch && creative.selected_slot == 4 && !dragging,
        2 => {
            creative.hotbar()[1] == BlockId::Torch
                && creative.hotbar()[4] == BlockId::Grass
                && creative.selected_slot == 1
                && !dragging
        }
        _ => review.before_cancel.as_ref() == Some(creative.hotbar()) && !dragging,
    };
    if checkpoint == 2 {
        review.before_cancel = Some(*creative.hotbar());
    }
    let name = [
        "",
        "palette_to_hotbar",
        "hotbar_swap",
        "outside_cancel",
        "focus_loss_cancel",
    ][checkpoint];
    println!(
        "[torch-drag] {name} {} hotbar={:?} selected={} dragging={dragging}",
        if passed { "PASS" } else { "FAIL" },
        creative.hotbar(),
        creative.selected_slot
    );
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
    let z_dist = match mode() {
        "minerals" => 10.5,
        "survival" => 2.2,
        "factory" => 7.5,
        _ => 4.5,
    };
    player.pos = if mode() == "factory" {
        Vec3::new(-0.5, 101.0, 3.6) / scale
    } else {
        Vec3::new(0.5 * scale, 100.0, z_dist + 0.5 * scale) / scale
    };
    player.yaw = 0.0;
    player.pitch = if mode() == "factory" { -0.22 } else { -0.15 };
    player.vel = Vec3::ZERO;
    player.flying = true;
    player.fly_mode = FlyMode::Noclip;
    player.crouch_t = 0.0;
    player.selected_block = BlockId::Stone;
    if mode() == "factory"
        && let Some(offset) = factory_review_focus() {
            player.pos = (Vec3::new(offset.x as f32 + 0.5, 101.5 + offset.y as f32, offset.z as f32 + 3.5)) / scale;
        }
}

fn factory_review_focus() -> Option<IVec3> {
    Some(match std::env::var("VOXEL_REVIEW_FOCUS").ok()?.as_str() {
        "battery" => IVec3::new(-2, 0, 3),
        "matrix" => IVec3::new(4, 1, -2),
        "transformer" => IVec3::new(5, 0, 2),
        "assembler" => IVec3::new(-5, 0, -1),
        "cable" => IVec3::new(5, 0, 1),
        "pump" => IVec3::new(3, 0, 0),
        "pile" => IVec3::new(-4, 0, 2),
        _ => return None,
    })
}

/// Magical-tech fixture: an extractor line, a lift that climbs a level, and a
/// powered line, so pipe traffic and machine models can be captured together.
fn install_factory(world: &mut World) {
    use crate::world::factory::{self, CAPACITY, Kind};
    let scale = world.voxel_scale.meters();
    let n = (1.0 / scale) as i32;
    let mut edits = Vec::new();
    for y in 99 * n..106 * n {
        for z in -8 * n..4 * n {
            for x in -6 * n..6 * n {
                edits.push((
                    IVec3::new(x, y, z),
                    if y < 100 * n {
                        BlockId::StoneBricks
                    } else {
                        BlockId::Air
                    },
                ));
            }
        }
    }
    world.set_many(&edits);
    let at = |x: i32, y: i32, z: i32| IVec3::new(x * n, (100 + y) * n, z * n);
    // Line A: crate -> extractor -> pipes -> furnace.
    if let Some(c) = factory::fixture_place(world, at(-4, 0, 0), Kind::Storage) {
        c.input[9] = CAPACITY;
        c.input[10] = CAPACITY;
    }
    if let Some(p) = factory::fixture_place(world, at(-3, 0, 0), Kind::Pipe) {
        p.mode = 1;
    }
    for x in -2..=1 {
        factory::fixture_place(world, at(x, 0, 0), Kind::Pipe);
    }
    factory::fixture_place(world, at(2, 0, 0), Kind::Furnace);
    // Line B: a crusher's output climbs two levels into a crate.
    if let Some(m) = factory::fixture_place(world, at(-4, 0, -2), Kind::Crusher) {
        m.output[9] = CAPACITY;
    }
    for (x, y) in [(-3, 0), (-2, 0), (-2, 1), (-2, 2), (-1, 2), (0, 2)] {
        factory::fixture_place(world, at(x, y, -2), Kind::Pipe);
    }
    factory::fixture_place(world, at(1, 2, -2), Kind::Storage);
    // Line C: generator and cable into a fabricator, plus a tank and pump.
    if let Some(g) = factory::fixture_place(world, at(-4, 0, 2), Kind::Generator) {
        g.mode = 1;
        g.input[17] = 256;
    }
    for x in -3..=0 {
        factory::fixture_place(world, at(x, 0, 2), Kind::Cable);
    }
    factory::fixture_place(world, at(1, 0, 2), Kind::Fabricator);
    factory::fixture_place(world, at(3, 0, 2), Kind::Tank);
    factory::fixture_place(world, at(3, 0, 0), Kind::Pump);
    factory::fixture_place(world, at(3, 0, 1), Kind::FluidPipe);
    // Line D: a battery bank on the generator's cable, in Mana font mode.
    if let Some(b) = factory::fixture_place(world, at(-2, 0, 3), Kind::Battery) {
        b.energy = factory::power::battery_capacity(0) * 3 / 5;
        b.mode = 3;
    }
    // Brick multiblocks behind the lines: a coke oven and a blast furnace,
    // their controllers facing the camera.
    use crate::world::factory::multiblock;
    for (kind, x, fuel) in [(Kind::CokeOven, -3, 3usize), (Kind::BlastFurnace, 1, factory::COKE)] {
        let p = at(x, 1, -5);
        for c in multiblock::shell(kind, p, IVec3::NEG_Z, n) {
            factory::fixture_place(world, c, multiblock::brick_kind(kind).unwrap());
        }
        if let Some(c) = factory::fixture_place(world, p, kind) {
            c.input[fuel] = 8 * 64;
            if kind == Kind::BlastFurnace {
                c.input[2] = 8 * 64;
            }
        }
    }
    // Line E: a 4 x 4 x 4 induction matrix with cells and a provider,
    // a transformer onto an HV line, and an assembler building generators.
    for y in 0..4 {
        for z in -5..=-2 {
            for x in 3..=6 {
                let wall = y == 0 || y == 3 || z == -5 || z == -2 || x == 3 || x == 6;
                let kind = if (x, y, z) == (4, 1, -2) {
                    Kind::InductionPort
                } else if wall {
                    Kind::InductionCasing
                } else if (x, y, z) == (4, 1, -4) {
                    Kind::InductionProvider
                } else {
                    Kind::InductionCell
                };
                factory::fixture_place(world, at(x, y, z), kind);
            }
        }
    }
    factory::fixture_place(world, at(5, 0, 2), Kind::Transformer);
    for z in [0, 1] {
        if let Some(c) = factory::fixture_place(world, at(5, 0, z), Kind::Cable) {
            c.upgrade = 1;
        }
    }
    if let Some(a) = factory::fixture_place(world, at(-5, 0, -1), Kind::Assembler) {
        let generator = crate::world::inventory::all_recipes()
            .iter()
            .position(|r| r.output.item == crate::world::inventory::Item::Block(Kind::Generator.block()))
            .unwrap();
        a.target = generator as u32 + 1;
        a.input[6] = 8 * 128;
        a.input[8] = 8 * 128;
        a.energy = 40;
    }
    world.factory.focus = factory_review_focus().map(|p| at(p.x, p.y, p.z));
    if std::env::var("VOXEL_REVIEW_FOCUS").is_ok_and(|f| f == "pump" || f == "pile") {
        factory::chips::review_fixture(world, at(3, 0, 0), at(3, 0, 2), at(-4, 0, 2));
    }
}

pub fn install(world: &mut World) {
    if mode() == "minerals" {
        // Unsaved material comparison, left to right. Uses the normal terrain
        // mesher, skylight metadata and torch BRDF, not synthetic UI swatches.
        let materials = [BlockId::CoalOre, BlockId::IronOre, BlockId::CopperOre,
            BlockId::TinOre, BlockId::Crystal, BlockId::CopperBlock,
            BlockId::MithrilBlock, BlockId::StarGlass];
        let n = (1.0 / world.voxel_scale.meters()) as i32;
        let mut edits = Vec::new();
        for x in -9 * n..9 * n {
            for z in -3 * n..6 * n {
                for y in 99 * n..106 * n {
                    let block = if y < 100 * n { BlockId::StoneBricks }
                        else if (-8 * n..8 * n).contains(&x)
                            && (-n..n).contains(&z) && y < 103 * n {
                            materials[((x + 8 * n) / (2 * n)) as usize]
                        } else { BlockId::Air };
                    edits.push((IVec3::new(x, y, z), block));
                }
            }
        }
        for x in [-6, -2, 2, 6] {
            edits.push((IVec3::new(x * n, 100 * n, 3 * n), BlockId::Torch));
        }
        world.set_many(&edits);
        return;
    }
    if mode() == "factory" {
        install_factory(world);
        return;
    }
    let scale = world.voxel_scale.meters();
    let n = (1.0 / scale) as i32;
    let mut edits = Vec::new();
    for y in 99 * n..104 * n {
        for z in -2 * n..3 * n {
            for x in -3 * n..3 * n {
                let block =
                    if y < 100 * n || (x >= 0 && x < 2 * n && z >= -n && z < 0 && y < 103 * n) {
                        BlockId::StoneBricks
                    } else {
                        BlockId::Air
                    };
                edits.push((IVec3::new(x, y, z), block));
            }
        }
    }
    edits.push((IVec3::new(-n, 100 * n, 0), BlockId::Torch));
    edits.push((IVec3::new(n, 101 * n, 0), BlockId::TorchWallSouth));
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
            world
                .inventory
                .notify_overflow(crate::world::inventory::Item::Block(BlockId::Stone));
        }
        3 => {
            // Phase 3: E-key survival inventory screen open with 2x2 crafting grid & armor preview
            creative.show_palette = true;
            world.adventure.health = 90.0;
            world.adventure.hunger = 80.0;
            world.inventory.eat_timer = 0.0;
            world.inventory.starve_timer = 0.0;
            egui_ctx.data_mut(|d| d.insert_temp(egui::Id::new("inventory_page"), 2u32));
        }
        4 | 5 => {
            // Phases 4-5: the recipe browser. Coal coke's recipes, then the
            // crafting tree for a battery bank.
            use crate::world::inventory::Item;
            creative.show_palette = true;
            let (item, tab) = if phase == 4 {
                (Item::Resource(crate::world::factory::COKE as u8), 0u8)
            } else {
                (Item::Block(BlockId::BatteryBank), 2u8)
            };
            egui_ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("inventory_page"), 6u32);
                d.insert_temp(egui::Id::new("jei_selected"), Some(item));
                d.insert_temp(egui::Id::new("jei_tab"), tab);
            });
        }
        6 => {
            // Phase 6: the factory health tab, with one assembly of each kind of trouble.
            creative.show_palette = true;
            health_sample(world);
            egui_ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("inventory_page"), 7u32);
                d.insert_temp(egui::Id::new("production_mode"), true);
            });
        }
        _ => {
            creative.show_palette = false;
        }
    }
}

/// A row of assemblies on the survival room's floor, one per health state:
/// two starved crushers, a starved generator, a paused furnace, an unpowered
/// furnace, an unconfigured assembler and an idle battery.
fn health_sample(world: &mut World) {
    use crate::world::factory::{self, Kind};
    if !world.factory.nodes.is_empty() {
        return;
    }
    let at = |x: i32| IVec3::new(x, 100, -6);
    for x in [-6, -4] {
        factory::fixture_place(world, at(x), Kind::Crusher);
    }
    if let Some(n) = factory::fixture_place(world, at(-2), Kind::Furnace) {
        n.enabled = false;
    }
    factory::fixture_place(world, at(0), Kind::Assembler);
    factory::fixture_place(world, at(2), Kind::Generator);
    factory::fixture_place(world, at(4), Kind::Battery);
    if let Some(f) = factory::fixture_place(world, at(6), Kind::Furnace) {
        f.input[9] = 64;
    }
}
