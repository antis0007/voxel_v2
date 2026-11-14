//! `--multiplayer-bot-work`: a scripted windowless joiner for end-to-end survival checks.
//! It mines, places, crafts, flies up in creative, falls in survival and respawns, logging `[bot]` lines.
use super::*;
use crate::world::inventory::{SurvivalInventory, all_recipes};

/// Drives ordered chest requests through a real client session, waiting for
/// each replicated result before sending the next gesture.
pub(super) fn chest_slots(session: &mut Session, world: &World, player: &Player, phase: &mut u8) {
    use world::{block_data::{ChestAction, SlotGesture}, inventory::Item, scanner::commands::Action};
    if *phase >= 4 { return; }
    let eye = player.eye(world.voxel_scale);
    let m = world.voxel_scale.meters();
    let Some(p) = world.block_data.entries.iter().find_map(|(&(x,y,z), entry)| {
        let p = IVec3::new(x,y,z);
        (matches!(entry, world::block_data::Entry::Chest(_))
            && (p.as_vec3() + Vec3::splat(0.5) - eye).length() * m <= 6.0).then_some(p)
    }) else { return };
    let slots = world.block_data.chest(p);
    let count = |i: usize| slots.get(i).copied().flatten()
        .filter(|s| s.item == Item::Block(BlockId::Stone)).map_or(0, |s| s.count);
    let cursor = world.inventory.cursor.filter(|s| s.item == Item::Block(BlockId::Stone)).map_or(0, |s| s.count);
    let request = match *phase {
        0 if count(0) == 8 && cursor == 0 => ChestAction::Chest(0, SlotGesture::Split),
        1 if count(0) == 4 && cursor == 4 => ChestAction::Chest(1, SlotGesture::Split),
        2 if count(1) == 1 && cursor == 3 => ChestAction::Chest(0, SlotGesture::Stack),
        3 if count(0) == 7 && count(1) == 1 && cursor == 0 => {
            eprintln!("[slot-flow-multiplayer] split=4 place-one=1 merge=7 cursor=empty");
            *phase = 4;
            return;
        }
        _ => return,
    };
    session.queue_scanner(p, Action::Chest(request));
    *phase += 1;
}

#[derive(Default)]
pub struct Bot {
    ready_at: Option<Instant>,
    next_log: f32,
    crafted: bool,
    asked_creative: bool,
    asked_survival: bool,
    respawned: bool,
}

fn summary(inventory: &SurvivalInventory) -> String {
    let mut parts = Vec::new();
    for stack in inventory.hotbar.iter().chain(inventory.pack.iter()).flatten() {
        parts.push(format!("{}x{}", stack.item.name(), stack.count));
    }
    parts.join(",")
}

/// The first hotbar slot holding a placeable block.
fn block_slot(inventory: &SurvivalInventory) -> Option<usize> {
    inventory.hotbar.iter().position(|s| s.is_some_and(|s| s.item.placeable_block().is_some()))
}

impl Bot {
    pub fn input(&mut self, session: &mut Session, world: &World, player: &mut Player) -> Input {
        let mut input = Input::default();
        if !session.ready() {
            return input;
        }
        let t = self.ready_at.get_or_insert_with(Instant::now).elapsed().as_secs_f32();
        if t >= self.next_log {
            self.next_log += 2.0;
            let feet = player.pos.floor().as_ivec3();
            eprintln!(
                "[bot] t={t:.0} at={:?} below={:?} health={:.1} hand={} items={}",
                player.pos.round(),
                world.get(feet - IVec3::Y),
                world.adventure.health,
                session.hand,
                summary(&world.inventory)
            );
        }
        match t {
            t if (3.0..9.0).contains(&t) => {
                player.pitch = -1.5;
                input.break_block = true;
            }
            t if (9.0..13.0).contains(&t) => {
                if let Some(slot) = block_slot(&world.inventory) {
                    session.hand = slot;
                }
                // Aim beyond the player's body, including diagonal spawns.
                player.yaw = 0.0;
                player.pitch = -0.55;
                input.place_pressed = t.fract() < 0.05;
            }
            t if t >= 13.0 && !self.crafted => {
                self.crafted = true;
                let found = all_recipes().iter().enumerate().find(|(_, recipe)| {
                    let mut trial = world.inventory.clone();
                    trial.stage_recipe_into_grid(recipe, false) && trial.craft_from_grid(false, false).is_some()
                });
                match found {
                    Some((index, recipe)) => {
                        eprintln!("[bot] craft recipe={}", recipe.name);
                        session.queue_inventory(vec![inventory::Command::Stage(index), inventory::Command::Craft(false)]);
                    }
                    None => eprintln!("[bot] craft none available"),
                }
            }
            t if t >= 15.0 && !self.asked_creative => {
                self.asked_creative = true;
                session.social.send("/gamemode creative");
            }
            // A short climb: the fall hurts without killing.
            t if (16.0..16.6).contains(&t) => {
                player.flying = true;
                input.jump = true;
            }
            t if t >= 16.6 && !self.asked_survival => {
                self.asked_survival = true;
                eprintln!("[bot] high at={:?} health={:.1}", player.pos.round(), world.adventure.health);
                session.social.send("/gamemode survival");
            }
            _ => {}
        }
        if world.adventure.is_dead() && !self.respawned {
            self.respawned = true;
            eprintln!("[bot] dead; respawning");
            let _ = session.route_ui(crate::egui_ui::UiAction::Respawn);
        }
        input
    }
}
