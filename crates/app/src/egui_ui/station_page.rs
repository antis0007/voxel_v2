//! Anvil, grindstone, stonecutter and brewing stand pages.
use super::block_page::{grid, slot};
use super::presentation as style;
use crate::world::inventory::{HOTBAR_SLOTS, ItemStack, PACK_SLOTS, SlotAddr};
use crate::world::stations::{self, brewing, table::STATIONS};
use crate::world::{BlockId, World};
use glam::IVec3;

type Inputs = (IVec3, [Option<SlotAddr>; 2]);

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
        _ => return false,
    };
    if close {
        *open = None;
    }
    true
}

/// Anvil or grindstone: two inputs that point at inventory slots, and the result.
fn two_input(ctx: &egui::Context, world: &mut World, p: IVec3, block: BlockId) -> bool {
    let id = egui::Id::new("station_inputs");
    let (_, mut inputs): Inputs = ctx.data_mut(|d| d.get_temp(id)).filter(|(at, _): &Inputs| *at == p).unwrap_or((p, [None; 2]));
    for input in &mut inputs {
        if input.is_some_and(|a| world.inventory.slot(a).is_none()) {
            *input = None;
        }
    }
    let anvil = block == BlockId::Anvil;
    let (result, cost) = match inputs {
        [Some(a), Some(b)] => {
            let outcome = if anvil { stations::anvil_preview(world, a, b) } else { stations::grindstone_preview(world, a, b) };
            (outcome.and_then(|_| world.inventory.slot(a)), outcome.map_or(0, |o| o.levels))
        }
        [Some(a), None] if !anvil => {
            let refund = stations::durable(world, a).and_then(|(item, left, max)| stations::salvage(item, left, max));
            (refund, 0)
        }
        _ => (None, 0),
    };
    let level = world.progression.level();
    let (mut picked, mut cleared, mut take, mut close) = (None, None, false, false);
    window(block.display_name()).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (i, input) in inputs.iter().enumerate() {
                if slot(ui, input.and_then(|a| world.inventory.slot(a))).clicked() {
                    cleared = Some(i);
                }
                if i == 0 {
                    ui.label("+");
                } else {
                    arrow(ui);
                }
            }
            take = slot(ui, result).clicked() && result.is_some();
        });
        if anvil {
            let colour = if cost > level { style::MUTED } else { style::PAPER };
            ui.label(egui::RichText::new(format!("Cost: {cost} levels · Level {level}")).color(colour));
        }
        ui.separator();
        picked = grid(ui, "station_pack", &pack(world));
        close = ui.button("Done").clicked() || done(ui);
    });
    if let Some(i) = cleared {
        inputs[i] = None;
    }
    if let Some(i) = picked {
        let addr = pack_addr(i);
        let free = if inputs[0].is_none() { 0 } else { 1 };
        inputs[free] = Some(addr);
    }
    if take {
        // Emptied inputs are dropped from the selection on the next frame.
        let _ = match inputs {
            [Some(a), Some(b)] if anvil => stations::use_anvil(world, a, b),
            [Some(a), Some(b)] => stations::use_grindstone(world, a, b),
            [Some(a), None] => stations::use_salvage(world, a),
            _ => false,
        };
    }
    ctx.data_mut(|d| d.insert_temp(id, (p, inputs)));
    close
}

fn stonecutter(ctx: &egui::Context, world: &mut World, p: IVec3) -> bool {
    let id = egui::Id::new("station_inputs");
    let (_, mut inputs): Inputs = ctx.data_mut(|d| d.get_temp(id)).filter(|(at, _): &Inputs| *at == p).unwrap_or((p, [None; 2]));
    let input = inputs[0].and_then(|a| world.inventory.slot(a));
    let rows: Vec<(usize, ItemStack)> = STATIONS
        .stonecutter
        .iter()
        .enumerate()
        .filter(|(_, c)| input.is_some_and(|s| s.item == c.input))
        .map(|(i, c)| (i, ItemStack::new(c.output, c.count)))
        .collect();
    let (mut picked, mut cut, mut cleared, mut close) = (None, None, false, false);
    window("Stonecutter").show(ctx, |ui| {
        ui.horizontal(|ui| {
            cleared = slot(ui, input).clicked();
            arrow(ui);
            for (i, stack) in &rows {
                if slot(ui, Some(*stack)).clicked() {
                    cut = Some(*i);
                }
            }
        });
        ui.separator();
        picked = grid(ui, "station_pack", &pack(world));
        close = ui.button("Done").clicked() || done(ui);
    });
    if cleared {
        inputs[0] = None;
    }
    if let Some(i) = picked {
        inputs[0] = Some(pack_addr(i));
    }
    if let (Some(addr), Some(row)) = (inputs[0], cut) {
        stations::use_stonecutter(world, addr, row);
    }
    ctx.data_mut(|d| d.insert_temp(id, (p, inputs)));
    close
}

fn brewing_stand(ctx: &egui::Context, world: &mut World, p: IVec3) -> bool {
    let stand = world.block_data.stand(p);
    let water = brewing::has_water(world, p);
    let progress = f32::from(stand.steps) / f32::from(brewing::brew_steps());
    let (mut picked, mut take, mut close) = (None, None, false);
    window("Brewing stand").show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (i, stack) in [stand.base, stand.modifier].into_iter().enumerate() {
                if slot(ui, stack).clicked() {
                    take = Some(i);
                }
            }
            ui.add(egui::ProgressBar::new(progress).desired_width(72.0));
            if slot(ui, stand.output).clicked() {
                take = Some(2);
            }
        });
        let state = if water { "Water: adjacent" } else { "Water: none" };
        ui.label(egui::RichText::new(state).color(if water { style::PAPER } else { style::MUTED }));
        ui.separator();
        picked = grid(ui, "station_pack", &pack(world));
        close = ui.button("Done").clicked() || done(ui);
    });
    if let Some(i) = take {
        brewing::take(world, p, i);
    }
    if let Some(i) = picked {
        brewing::store(world, p, pack_addr(i));
    }
    close
}
