//! Player progression: item wear, experience, hunger saturation and status effects.
//! Persists in `sidecar::FILE` beside the world, not in the save body.
pub mod effects;
pub mod sidecar;
pub mod thresholds;
pub(crate) mod study;
pub mod table;
#[cfg(test)]
mod tests;
pub mod wear;
pub mod xp;

use super::World;
use super::inventory::{FoodKind, Item};
use crate::audio::{Cue, Event};
pub use effects::{Effects, Kind};
pub use wear::{Slot, Wear};

#[derive(Clone, Debug, Default)]
pub struct State {
    /// Total experience points; the level is derived from it.
    pub xp: u32,
    /// Permanent field records travel with the actor, including across realms.
    pub lore: u32,
    pub(crate) lessons: u8,
    pub(crate) horticulture: bool,
    /// Hidden hunger buffer drained before hunger, never above hunger.
    pub saturation: f32,
    pub effects: Effects,
    pub wear: Wear,
    /// Kill XP queued while the adventure is detached from the world; granted in `step`.
    pub pending_xp: u32,
    /// Health and strike timer after the previous step, to detect hits taken and landed.
    last_health: Option<f32>,
    last_hit: f32,
    was_dead: bool,
    /// Seconds toward the next Self-repair point; runtime only.
    regrow: f32,
}

/// Seconds between Self-repair points: one per stack on every living tool.
const REGROW_SECONDS: f32 = 10.0;

/// Living parts (the Self-repair trait) knit one durability point per stack
/// on every unbroken assembled tool the player owns.
fn regrow_living_tools(world: &mut World) {
    for tool in &mut world.adventure.tool_bank {
        if tool.is_broken() {
            continue;
        }
        let stacks: u32 = tool.traits().unwrap_or_default().iter()
            .filter(|t| t.effect == engine_tools::TraitEffect::SelfRepair)
            .map(|t| u32::from(t.stacks)).sum();
        if stacks == 0 {
            continue;
        }
        if let Ok(stats) = tool.stats() {
            tool.durability_left = (tool.durability_left + stacks).min(stats.durability);
        }
    }
}

/// HUD snapshot published to the UI each frame.
#[derive(Clone, Debug, Default)]
pub struct View {
    pub level: u32,
    pub progress: f32,
    pub effects: Vec<effects::Active>,
    /// Remaining and maximum durability for every durable stack.
    pub durability: Vec<(Slot, u32, u32)>,
}

impl State {
    pub fn level(&self) -> u32 {
        xp::level_of(self.xp).0
    }

    /// Takes `amount` of hunger drain out of saturation first and returns what reaches hunger.
    pub fn absorb_drain(&mut self, amount: f32) -> f32 {
        let amount = amount.max(0.0) * if study::Lesson::Wayfaring.learned(self.lessons, self.horticulture) { 0.85 } else { 1.0 };
        let from_saturation = amount.min(self.saturation);
        self.saturation -= from_saturation;
        amount - from_saturation
    }

    /// Adds the food's saturation (capped by `hunger`) and its effects.
    pub fn eat(&mut self, food: FoodKind, hunger: f32) {
        let Some(row) = super::inventory::food::get(food) else { return };
        let bonus = if !food.is_potion() && study::Lesson::Provisioning.learned(self.lessons, self.horticulture) { 1.25 } else { 1.0 };
        self.saturation = (self.saturation + row.saturation * bonus).min(hunger).max(0.0);
        for grant in &row.effects {
            self.effects.add(grant.kind, grant.seconds, grant.level);
        }
    }

    /// Death loses all XP unless the world keeps inventories; effects and saturation always end.
    pub fn on_death(&mut self, keep_inventory: bool) {
        if !keep_inventory {
            self.xp = 0;
        }
        self.effects.clear();
        self.saturation = 0.0;
    }

    pub fn view(&self, inv: &super::inventory::SurvivalInventory) -> View {
        let (level, _) = xp::level_of(self.xp);
        let durability = [Slot::Offhand, Slot::Cursor]
            .into_iter()
            .chain((0..super::inventory::HOTBAR_SLOTS as u8).map(Slot::Hotbar))
            .chain((0..super::inventory::PACK_SLOTS as u8).map(Slot::Pack))
            .chain((0..super::inventory::EQUIP_SLOTS as u8).map(Slot::Equipment))
            .filter_map(|s| self.wear.durability(inv, s).map(|(left, max)| (s, left, max)))
            .collect();
        View { level, progress: xp::progress(self.xp), effects: self.effects.list.clone(), durability }
    }
}

fn cue(world: &mut World, cue: Cue, gain: f32) {
    if world.adventure.sounds.len() < 32 {
        world.adventure.sounds.push(Event { cue, pos: None, gain, pitch: 1.0 });
    }
}

/// Adds experience and plays the level cue on each level gained.
pub fn grant_xp(world: &mut World, amount: u32) {
    if amount == 0 || world.adventure.creative || world.rules.mode.creative_tools() {
        return;
    }
    let before = world.progression.level();
    world.progression.xp = world.progression.xp.saturating_add(amount);
    if world.progression.level() > before {
        cue(world, Cue::Treasure, 0.6);
    }
}

/// Wears the stack in `slot`; announces and removes it when it breaks.
pub fn spend(world: &mut World, slot: Slot, amount: u32) {
    if world.adventure.creative || world.rules.mode.creative_tools() {
        return;
    }
    if let Slot::Hotbar(i)=slot {
        if let Some(Item::Assembled{id,..})=world.inventory.hotbar.get(i as usize).copied().flatten().map(|s|s.item) {
            if let Some(tool)=world.adventure.tool_bank.get_mut(id as usize) {
                if !tool.is_broken() {tool.wear(amount);tool.gain_xp(amount);}
            }
            return;
        }
    }
    if let Some(item) = world.progression.wear.spend(&mut world.inventory, slot, amount) {
        broke(world, item);
        if let Slot::Hotbar(i) = slot
            && world.inventory.refill_hotbar(i as usize, item) {
            // The replacement keeps its own wear; rebind its entry to the hotbar slot.
            world.progression.wear.reconcile(&world.inventory);
            world.sync_from_inventory();
        }
    }
}

fn broke(world: &mut World, item: Item) {
    cue(world, Cue::Shatter, 0.8);
    world.expedition.say(format!("{} Broken", item.name()));
}

/// Survival block break: wears the held tool and grants the block's mining XP.
pub fn on_harvest(world: &mut World, block: super::BlockId) {
    let hand = world.adventure.hand_slot as u8;
    spend(world, Slot::Hotbar(hand), 1);
    let base = table::mined_xp(block);
    let bonus = u32::from(base > 0 && study::Lesson::Prospecting.learned(world.progression.lessons, world.progression.horticulture));
    grant_xp(world, base.saturating_add(bonus));
}

/// Per fixed step, after the adventure update: wear from hits, death loss, effect ticks.
pub fn step(world: &mut World, dt: f32) {
    let pending = std::mem::take(&mut world.progression.pending_xp);
    grant_xp(world, pending);
    let p = &mut world.progression;
    p.wear.reconcile(&world.inventory);
    let dead = world.adventure.health <= 0.0;
    if dead && !p.was_dead {
        p.on_death(world.rules.keep_inventory);
    }
    p.was_dead = dead;
    let health = world.adventure.health;
    let taken = p.last_health.map_or(0.0, |last| last - health);
    let struck = world.adventure.hit > p.last_hit;
    p.last_hit = world.adventure.hit;
    if !dead && struck {
        let hand = world.adventure.hand_slot as u8;
        spend(world, Slot::Hotbar(hand), 1);
    }
    if !dead && taken > 0.5 {
        // One point per piece for every 20 health lost, at least one.
        let amount = ((taken / 20.0).floor() as u32).max(1);
        for i in 0..super::inventory::EQUIP_SLOTS as u8 {
            spend(world, Slot::Equipment(i), amount);
        }
    }
    let p = &mut world.progression;
    for (kind, seconds, level) in world.adventure.pending_effects.drain(..) {
        if !dead {
            p.effects.add(kind, seconds, level);
        }
    }
    if !dead {
        let delta = p.effects.tick(dt, world.adventure.health);
        if delta != 0.0 && !world.adventure.creative && !world.rules.mode.creative_tools() {
            let max = world.adventure.max_health();
            world.adventure.health = (world.adventure.health + delta).clamp(0.0, max.max(world.adventure.health));
        }
    }
    p.saturation = p.saturation.min(world.adventure.hunger).max(0.0);
    world.adventure.resist_level = p.effects.level(Kind::Resistance);
    world.adventure.absorption = world.adventure.absorption.clamp(0.0, p.effects.absorption_cap());
    p.last_health = Some(world.adventure.health);
    p.regrow += dt;
    if p.regrow >= REGROW_SECONDS {
        p.regrow -= REGROW_SECONDS;
        if !dead && world.rules.systems.enabled("forging") {
            regrow_living_tools(world);
        }
    }
}

/// Absorption fills to its cap when the effect is gained, not while it lasts.
pub fn fill_absorption(world: &mut World) {
    let cap = world.progression.effects.absorption_cap();
    world.adventure.absorption = world.adventure.absorption.max(cap);
}
