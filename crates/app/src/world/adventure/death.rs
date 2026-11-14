//! Player death: the cause, the dropped inventory and the respawn.
use super::{Adventure, World, sound};
use crate::{
    audio::Cue,
    player::Player,
    world::inventory::DroppedStack,
};
use engine_content::BlockId;
use glam::Vec3;

/// The save loader rejects more dropped stacks than this.
const MAX_DROPPED: usize = 1024;

/// What last hurt the player; the death screen title.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub enum Cause {
    /// Enemy hits, blasts and anything without its own entry.
    #[default]
    Wounds,
    Fall,
    Drowned,
    /// A block that burns on contact, named by its material.
    Contact(BlockId),
    ManaPoisoning,
    Pollution,
    Suffocation,
    /// Taken by the Hush after too long unlit in the Lightless.
    Hush,
}

impl Cause {
    pub fn label(self) -> &'static str {
        match self {
            Self::Wounds => "Wounds",
            Self::Fall => "Fall",
            Self::Drowned => "Drowned",
            Self::Contact(block) => block.display_name(),
            Self::ManaPoisoning => "Mana poisoning",
            Self::Pollution => "Pollution",
            Self::Suffocation => "Suffocation",
            Self::Hush => "The Hush",
        }
    }
}

/// A death waiting on the respawn button.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Death {
    pub cause: Cause,
    /// Feet, in metres.
    pub pos: Vec3,
    pub dropped: u32,
    /// False until the inventory has been thrown at `pos`.
    settled: bool,
}

impl Adventure {
    pub fn is_dead(&self) -> bool {
        self.death.is_some()
    }

    /// Environmental harm (fall, drowning, contact) after difficulty scaling.
    pub fn environment_damage(&mut self, amount: f32, cause: Cause) {
        // Long invulnerability is spawn protection; the shorter combat
        // flinch window must not turn lava or drowning off after every hit.
        if self.creative || self.death.is_some() || self.invulnerable > 1.0 {
            return;
        }
        let amount = amount * self.difficulty.incoming_damage();
        let amount = if self.discoveries & (1 << 12) != 0 && self.ward > 0.0 && self.ward_hits > 0
            && matches!(cause, Cause::Contact(BlockId::Fire | BlockId::Lava)) {
            amount * 0.75
        } else { amount };
        if amount.is_nan() || amount <= 0.0 {
            return;
        }
        let amount = self.soak(amount);
        self.health = (self.health - amount).max(0.0);
        self.last_cause = cause;
        // Contact and drowning arrive every tick; one cue per flinch, not a buzz.
        if self.hurt <= 0.0 {
            sound(&mut self.sounds, Cue::Hurt, None, 0.7);
            self.damage_amount = amount;
        } else {
            self.damage_amount = self.damage_amount.max(amount);
        }
        self.hurt = self.hurt.max(0.35);
        self.damage_direction = Vec3::ZERO;
    }

    /// Records the death at `feet` (metres) and clears combat state.
    pub(super) fn die(&mut self, world: &mut World, feet: Vec3) {
        let lost_resonance = self.resonance.active.then_some(self.resonance.salvage);
        self.resonance.end();
        self.last_death_pos = Some(feet);
        self.health = 0.0;
        self.projectiles.retain(|p| p.farm.is_some());
        self.hazards.clear();
        self.wells.clear();
        self.melee_buffer = 0.0;
        self.strike_delay = 0.0;
        self.melee_cd = 0.0;
        for e in &self.enemies {
            super::wildlife::vehicle::remember_driver(world, e);
            if e.kind == super::engine::KIND && e.health > 0.0 {
                self.engines.insert(e.site, super::engine::snapshot(e));
            }
        }
        #[cfg(feature = "multiplayer")]
        let shared_encounter = world.multiplayer.is_some();
        #[cfg(not(feature = "multiplayer"))]
        let shared_encounter = false;
        if !shared_encounter {
            self.enemies.retain(|e| e.habitat.is_some());
        }
        self.blooms.clear();
        self.arcs.clear();
        self.crosses.clear();
        self.lasers.clear();
        self.particles.clear();
        self.splashes.clear();
        self.remains.clear();
        self.ward = 0.0;
        self.ward_hits = 0;
        sound(&mut self.sounds, Cue::Hurt, None, 1.0);
        self.death = Some(Death { cause: self.last_cause, pos: feet, dropped: 0, settled: false });
        if let Some(lost) = lost_resonance {
            world.expedition.say(format!("Salvage lost: {lost}"));
        }
    }
}

/// Throws the carried inventory at a new death position.
///
/// Runs after `Adventure::update` has been put back on the world, so the
/// legacy resource mirrors are refreshed from the emptied inventory before
/// reconciliation could restore them.
pub(super) fn settle(world: &mut World) {
    let Some(death) = world.adventure.death.filter(|d| !d.settled) else {
        return;
    };
    let dropped = if world.rules.keep_inventory { 0 } else { drop_inventory(world, death.pos) };
    world.adventure.death = Some(Death { dropped, settled: true, ..death });
    world.sync_from_inventory();
}

/// Moves every carried stack into the world around `feet`; returns the count.
fn drop_inventory(world: &mut World, feet: Vec3) -> u32 {
    let scale = world.voxel_scale.meters();
    let inventory = &mut world.inventory;
    let slots = inventory
        .hotbar
        .iter_mut()
        .chain(inventory.pack.iter_mut())
        .chain(inventory.equipment.iter_mut())
        .chain(std::iter::once(&mut inventory.offhand))
        .chain(inventory.craft_grid.iter_mut())
        .chain(std::iter::once(&mut inventory.cursor));
    let mut stacks = Vec::new();
    for slot in slots {
        if inventory.dropped.len() + stacks.len() >= MAX_DROPPED {
            break;
        }
        if let Some(stack) = slot.take() {
            stacks.push(stack);
        }
    }
    let count = stacks.len() as u32;
    for (index, stack) in stacks.into_iter().enumerate() {
        let pos = scatter(feet, index);
        let free = world.get((pos / scale).floor().as_ivec3()).is_empty();
        let pos = if free { pos } else { feet + Vec3::Y * 0.5 };
        world.inventory.dropped.push(DroppedStack { pos, stack });
    }
    count
}

/// A fixed spiral around the feet, so drops are reproducible without `rand`.
fn scatter(feet: Vec3, index: usize) -> Vec3 {
    let angle = index as f32 * 2.399_963;
    let radius = 0.2 + 0.12 * (index as f32).sqrt();
    feet + Vec3::new(angle.cos() * radius, 0.5, angle.sin() * radius)
}

/// Leaves the death screen at the bed or world spawn with restored vitals.
pub fn respawn(world: &mut World, player: &mut Player) {
    let Some(death) = world.adventure.death.take() else {
        return;
    };
    let scale = world.voxel_scale.meters();
    let Some(crate::world::spawn::Resolved { feet, bed_unavailable }) = crate::world::spawn::resolve(world, player) else {
        world.adventure.death = Some(death);
        world.expedition.say("Spawn blocked");
        return;
    };
    player.pos = feet;
    player.precise_pos = feet.as_dvec3();
    player.anchor = None;
    player.gravity_frame = glam::Quat::IDENTITY;
    player.vel = Vec3::ZERO;
    player.air_seconds = 0.0;
    player.movement.fall_speed = 0.0;
    player.fall_ward_seconds = 0.0;
    player.dark_seconds = 0.0;
    let _ = player.take_pending_harm();
    let spawn_m = feet * scale;
    let adventure = &mut world.adventure;
    adventure.health = adventure.max_health();
    adventure.mana = adventure.max_mana();
    adventure.hunger = 100.0;
    adventure.hurt = 0.0;
    adventure.damage_amount = 0.0;
    adventure.damage_direction = Vec3::ZERO;
    adventure.invulnerable = 5.0;
    adventure.last_cause = Cause::Wounds;
    adventure.spawn_voxel_burst(spawn_m + Vec3::Y * 0.9, [0.98, 0.78, 0.34], 26, 2.8, 0.045);
    sound(&mut adventure.sounds, Cue::Mend, Some(spawn_m), 0.85);
    if bed_unavailable {
        world.expedition.say("Bed unavailable");
    } else {
        world.expedition.say(format!("Death location: {:.0} m", (death.pos - spawn_m).length()));
    }
}

#[cfg(test)]
#[path = "death_tests.rs"]
mod tests;
