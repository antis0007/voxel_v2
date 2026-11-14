//! Environmental harm measured by the player step: falls, drowning, contact.
//! Damage is only queued here; the caller applies it where health lives, so the
//! single-player and host paths share one rule.
use super::{AIR_SECONDS, Player};
use crate::world::{GRAVITY, World, adventure::death::Cause, player_aabb};
use engine_content::BlockId;
use glam::{IVec3, Vec3};

/// Metres a body can drop without harm.
pub const SAFE_FALL_METRES: f32 = 5.0;
/// Health lost per metre fallen past the safe drop.
pub const FALL_DAMAGE_PER_METRE: f32 = 3.0;

/// Health a landing at `speed` metres per second costs.
pub fn fall_damage(speed: f32) -> f32 {
    // Free-fall height is v^2 / 2g in metres, so the threshold is the same
    // height at every voxel scale.
    let height = speed * speed / (2.0 * GRAVITY);
    (height - SAFE_FALL_METRES).max(0.0) * FALL_DAMAGE_PER_METRE
}

impl Player {
    /// Queues damage and its cause for the caller to apply.
    pub fn harm(&mut self, amount: f32, cause: Cause) {
        if amount > 0.0 {
            if cause == Cause::Fall && self.fall_ward_seconds > 0.0 {
                self.fall_ward_seconds = 0.0;
                return;
            }
            self.pending_damage += amount;
            self.pending_cause = Some(cause);
        }
    }

    /// Takes this step's environmental damage with its cause.
    pub fn take_pending_harm(&mut self) -> Option<(f32, Cause)> {
        let cause = self.pending_cause.take();
        let amount = std::mem::take(&mut self.pending_damage);
        (amount > 0.0).then(|| (amount, cause.unwrap_or_default()))
    }

    /// Remaining breath, 1.0 when full.
    pub fn breath(&self) -> f32 {
        (1.0 - self.air_seconds / AIR_SECONDS).clamp(0.0, 1.0)
    }
}

/// Queues contact damage from the most harmful block the body overlaps.
pub(super) fn contact(player: &mut Player, world: &World, origin: IVec3, dt: f32) {
    // The current authoritative player mode also exempts grounded builders.
    if world.adventure.creative || player.flying {
        return;
    }
    let (lo, hi) = player_aabb(player.pos, world.voxel_scale, player.scale, player.crouch_t);
    let lo = lo.floor().as_ivec3();
    let hi = (hi - Vec3::splat(1e-4)).floor().as_ivec3();
    let mut worst: Option<(f32, BlockId)> = None;
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let block = world.get(IVec3::new(x, y, z) + origin);
                let rate = block.contact_damage();
                if rate > worst.map_or(0.0, |(best, _)| best) {
                    worst = Some((rate, block));
                }
            }
        }
    }
    if let Some((rate, block)) = worst {
        let resistance = if world.adventure.discoveries & (1 << 3) != 0
            && matches!(block, BlockId::Fire | BlockId::Lava) { 0.5 } else { 1.0 };
        player.harm(rate * dt * resistance, Cause::Contact(block));
    }
}

/// Seconds unlit the darkness meter holds at most.
pub const HUSH_MEMORY_S: f32 = 60.0;
/// Block light at or above this keeps the Hush away.
pub const HUSH_SAFE_LIGHT: u8 = 4;

/// Light the Hush will not enter: a lit cell, a held torch, the lamp, or a
/// glowing block within about 2 m. Generated chunks carry no light nibbles
/// until an edit relights them, so foxfire and shafts count by proximity.
pub fn lit(player: &Player, world: &World, origin: IVec3) -> bool {
    let scale = world.voxel_scale.meters();
    let eye = (player.eye(world.voxel_scale)).floor().as_ivec3() + origin;
    let feet = (player.pos + player.up_dir() * (0.3 / scale)).floor().as_ivec3() + origin;
    let cell_light = crate::world::block_light::level_at(world, eye).max(crate::world::block_light::level_at(world, feet));
    if cell_light >= HUSH_SAFE_LIGHT
        || world.expedition.lamp_enabled()
        || crate::mesh::models::held_item::offhand_torch(world)
        || crate::mesh::models::held_item::main_hand_torch(world, player)
    {
        return true;
    }
    let reach = ((2.0 / scale).ceil() as i32).clamp(2, 8);
    let step = (reach / 2).max(1);
    (-reach..=reach).step_by(step as usize).any(|dy| {
        (-reach..=reach).any(|dz| (-reach..=reach).any(|dx| world.get(feet + IVec3::new(dx, dy, dz)).is_light_source()))
    })
}

/// The Lightless darkness meter. Light drains it four times faster than the
/// dark fills it; the Hush colony (`world::lightless_life`) reads it to decide
/// when to close in and does the biting.
pub(super) fn darkness(player: &mut Player, world: &World, origin: IVec3, dt: f32) {
    if world.world_type.realm() != Some(engine_voxel::RealmKind::Lightless) || world.adventure.creative {
        player.dark_seconds = 0.0;
        return;
    }
    player.dark_seconds = if lit(player, world, origin) {
        (player.dark_seconds - dt * 4.0).max(0.0)
    } else {
        (player.dark_seconds + dt).min(HUSH_MEMORY_S)
    };
}

#[cfg(test)]
#[path = "hazards_tests.rs"]
mod tests;
