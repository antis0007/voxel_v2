//! Anvil, grindstone, stonecutter and brewing stand pages.
use super::block_page::{Gesture, compact_grid, paint_cursor, slot, slot_gesture};
use super::brew_view;
use super::presentation as style;
use crate::world::inventory::{HOTBAR_SLOTS, ItemStack, PACK_SLOTS, SlotAddr};
use crate::world::stations::{self, brewing, table::STATIONS};
use crate::world::{BlockId, World};
use glam::IVec3;

const FIRST: SlotAddr = SlotAddr::CraftGrid(0);
const SECOND: SlotAddr = SlotAddr::CraftGrid(1);

fn window(title: &str) -> egui::Window<'_> {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
}

fn pack(world: &World) -> Vec<Option<ItemStack>> {
    world.inventory.pack.iter().chain(world.inventory.hotbar.iter()).copied().collect()
}

fn pack_addr(i: usize) -> SlotAddr {
    if i < PACK_SLOTS { SlotAddr::Pack(i) } else { SlotAddr::Hotbar((i - PACK_SLOTS).min(HOTBAR_SLOTS - 1)) }
}

/// Painted, since the UI font lacks arrow glyphs.
fn arrow(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 40.0), egui::Sense::hover());
    let stroke = egui::Stroke::new(2.5, style::MUTED);
    ui.painter().arrow(rect.left_center() + egui::vec2(4.0, 0.0), egui::vec2(20.0, 0.0), stroke);
}

fn done(ui: &egui::Ui) -> bool {
    ui.input(|i| i.key_pressed(egui::Key::Escape))
}

/// Draws the page for station `block` at `p`; false when `block` is not a station.
pub(super) fn draw(ctx: &egui::Context, open: &mut Option<IVec3>, world: &mut World, p: IVec3, block: BlockId) -> bool {
    let close = match block {
        BlockId::Anvil | BlockId::Grindstone => two_input(ctx, world, p, block),
        BlockId::Stonecutter => stonecutter(ctx, world, p),
        BlockId::BrewingStand => brewing_stand(ctx, world, p),
        BlockId::EnchantingTable => enchanting(ctx, world, p),
        _ => return false,
    };
    if close {
        world.inventory.return_transient_to_pack();
        sync(world);
        *open = None;
    } else { paint_cursor(ctx, world); }
    true
}

fn enchanting(ctx: &egui::Context, world: &mut World, p: IVec3) -> bool {
    use crate::world::adventure::enchanting as table;
    let rows = &*table::TABLE;

    let (mut close, mut choice) = (false, None);
    let held = table::held(world);
    let can_bind = crate::world::progression::study::available(world) && world.adventure.health > 0.0;
    let xp = world.progression.xp;
    let crystals = world.inventory.count_item(rows.crystal());
    window("Enchanting Table").show(ctx, |ui| {
        if let Some((selected, _, _)) = held {
            ui.horizontal(|ui| {
                slot(ui, world.inventory.hotbar[selected]);
                ui.label("Tier III");
            });
        } else {
            ui.label("Held Tier III Wand Required");
        }
        ui.label(format!("{xp}/{} XP · {crystals}/{} {}", rows.xp, rows.crystals, rows.crystal().name()));
        for binding in &rows.bindings {
            let mode = binding.mode;
            let active = held.is_some_and(|(_, _, record)| record.loadout.mode == mode);
            let ready = can_bind && held.is_some() && !active && xp >= rows.xp && crystals >= rows.crystals;
            if ui.add_enabled(ready, egui::Button::new(format!("{} · {}", binding.name, binding.effect))).clicked() {
                choice = Some(mode);
            }
            if active { ui.weak("Bound"); }
        }
        close = ui.button("Close").clicked() || done(ui);
    });
    if let Some(mode) = choice {
        table::bind(world, p, mode);
    }
    close
}

fn sync(world: &mut World) {
    world.progression.wear.reconcile(&world.inventory);
    world.sync_from_inventory();
}

fn gesture(world: &mut World, addr: SlotAddr, action: Gesture, target: Option<SlotAddr>) {
    let inv = &mut world.inventory;
    match (action, target) {
        (Gesture::Stack, _) => inv.click_slot(addr),
        (Gesture::Split, _) => inv.right_click_slot(addr),
        (Gesture::Gather, _) => inv.gather_to_cursor(addr),
        (Gesture::Hotbar(i), _) => { inv.swap_slot_with_hotbar(addr, usize::from(i)); }
        (Gesture::Transfer, Some(dst)) if inv.cursor.is_none()
            && inv.slot(dst).is_none_or(|slot| inv.slot(addr).is_some_and(|src| src.item == slot.item)) => {
            inv.click_slot(addr);
            inv.click_slot(dst);
            if inv.cursor.is_some() { inv.click_slot(addr); }
        }
        (Gesture::Transfer, None) => { inv.shift_click_slot(addr); }
        _ => {}
    }
    sync(world);
}

fn two_input(ctx: &egui::Context, world: &mut World, _p: IVec3, block: BlockId) -> bool {
    let anvil = block == BlockId::Anvil;
    let outcome = if anvil { stations::anvil_preview(world, FIRST, SECOND) }
        else { stations::grindstone_preview(world, FIRST, SECOND) };
    let salvage = !anvil && world.inventory.slot(SECOND).is_none();
    let result = outcome.and_then(|_| world.inventory.slot(FIRST)).or_else(|| {
        if !salvage { return None; }
        let (item, left, max) = stations::durable(world, FIRST)?;
        stations::salvage(item, left, max)
    });
    let level = world.progression.level();
    let cost = outcome.map_or(0, |o| o.levels);
    let allowed = result.is_some() && (cost <= level || world.rules.mode.creative_tools());
    let (mut input, mut picked, mut take, mut close) = (None, None, None, false);
    window(block.display_name()).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for addr in [FIRST, SECOND] {
                let response = slot(ui, world.inventory.slot(addr));
                if let Some(action) = slot_gesture(ui, response) { input = Some((addr, action)); }
                if addr == FIRST { ui.label("+"); } else { arrow(ui); }
            }
            ui.add_enabled_ui(allowed, |ui| {
                let response = slot(ui, result);
                take = slot_gesture(ui, response);
            });
        });
        if anvil && result.is_some() { ui.label(format!("Cost: {cost} Levels")); }
        ui.separator();
        picked = compact_grid(ui, "station_pack", &pack(world));
        close = ui.button("Done").clicked() || done(ui);
    });
    if let Some((addr, action)) = input { gesture(world, addr, action, None); }
    if let Some((i, action)) = picked {
        let dst = [FIRST, SECOND].into_iter().find(|&a| world.inventory.slot(a).is_none());
        if action != Gesture::Transfer || dst.is_some() { gesture(world, pack_addr(i), action, dst); }
    }
    if let Some(action) = take && allowed && world.inventory.cursor.is_none() {
        let applied = if anvil { stations::use_anvil(world, FIRST, SECOND) }
            else if outcome.is_some() { stations::use_grindstone(world, FIRST, SECOND) }
            else { stations::salvage_in_slot(world, FIRST) };
        if applied { gesture(world, FIRST, action, None); }
    }
    close
}

fn stonecutter(ctx: &egui::Context, world: &mut World, _p: IVec3) -> bool {
    let input = world.inventory.slot(FIRST);
    let rows: Vec<(usize, ItemStack)> = STATIONS.stonecutter.iter().enumerate()
        .filter(|(_, c)| input.is_some_and(|s| s.item == c.input))
        .map(|(i, c)| (i, ItemStack::new(c.output, c.count))).collect();
    let (mut edit, mut picked, mut cut, mut close) = (None, None, None, false);
    window("Stonecutter").show(ctx, |ui| {
        ui.horizontal(|ui| {
            let response = slot(ui, input);
            edit = slot_gesture(ui, response);
            arrow(ui);
            for (i, stack) in &rows {
                let response = slot(ui, Some(*stack));
                if let Some(action) = slot_gesture(ui, response) { cut = Some((*i, action)); }
            }
        });
        ui.separator();
        picked = compact_grid(ui, "station_pack", &pack(world));
        close = ui.button("Done").clicked() || done(ui);
    });
    if let Some(action) = edit { gesture(world, FIRST, action, None); }
    if let Some((i, action)) = picked { gesture(world, pack_addr(i), action, Some(FIRST)); }
    if let Some((row, action)) = cut {
        stations::cut_from_slot(world, FIRST, row, action == Gesture::Transfer);
    }
    close
}

fn brewing_stand(ctx: &egui::Context, world: &mut World, p: IVec3) -> bool {
    let stand = world.block_data.stand(p);
    let water = brewing::has_water(world, p);
    let phase = brew_view::phase(&stand, water);
    let progress = f32::from(stand.steps) / f32::from(brewing::brew_steps());
    let stacks = pack(world);
    let (mut picked, mut take, mut close, mut load) = (None, None, false, Vec::new());
    window("Brewing Stand").show(ctx, |ui| {
        ui.set_width(brew_view::PAGE_WIDTH);
        ui.horizontal(|ui| {
            for (i, stack) in [stand.base, stand.modifier].into_iter().enumerate() {
                let response = slot(ui, stack);
                if let Some(gesture) = slot_gesture(ui, response) {
                    take = Some((i, gesture));
                }
            }
            ui.vertical(|ui| {
                ui.add_space(10.0);
                brew_view::gauge(ui, progress, brew_view::batch_colour(&stand), matches!(phase, brew_view::Phase::Brewing(_)));
            });
            let response = slot(ui, stand.output);
            if let Some(gesture) = slot_gesture(ui, response) {
                take = Some((2, gesture));
            }
        });
        ui.horizontal(|ui| {
            brew_view::pill(ui, &phase.label(), phase.colour());
            if water {
                brew_view::pill(ui, "Water", style::JADE);
            }
        });
        ui.separator();
        load = brew_view::recipes(ui, &stand, &stacks, pack_addr);
        ui.separator();
        picked = compact_grid(ui, "station_pack", &stacks);
        close = ui.button("Done").clicked() || done(ui);
    });
    if let Some((i, gesture)) = take {
        if gesture == Gesture::Transfer { brewing::take(world, p, i); }
        else { brewing::edit_slot(world, p, i, gesture == Gesture::Split); }
    }
    for addr in load {
        brewing::store(world, p, addr);
    }
    if let Some((i, gesture)) = picked {
        let addr = pack_addr(i);
        match gesture {
            Gesture::Transfer => brewing::store(world, p, addr),
            Gesture::Stack => world.inventory.click_slot(addr),
            Gesture::Split => world.inventory.right_click_slot(addr),
            Gesture::Gather => world.inventory.gather_to_cursor(addr),
            Gesture::Hotbar(i) => { world.inventory.swap_slot_with_hotbar(addr, usize::from(i)); }
        }
        world.sync_from_inventory();
    }
    close
}
