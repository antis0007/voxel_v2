//! Wing inhabitants: planned trader, residents, guards and hostiles as persistent actors.
use super::*;
use crate::world::adventure::Enemy;
use engine_worldgen::station::{self, Role};

/// Enemy slots at or above this belong to a habitat wing; freight guards use 0..6.
pub const SLOT_BASE: u32 = 1000;
const NEAR_M: f32 = 85.0;
const HOSTILE_KINDS: [u32; 5] = [10, 25, 29, 26, 23];
const VILLAGER: u32 = 40;
pub const MAX: usize = 32;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Resident {
    pub slot: u32,
    pub kind: u32,
    pub pos: Vec3,
    /// `None` until the actor first takes damage or is evicted.
    pub health: Option<f32>,
}

fn kind(role: Role, seed: u32, k: usize) -> u32 {
    match role {
        Role::Trader | Role::Resident => VILLAGER,
        Role::Guard => super::super::cargo::ROBOT,
        Role::Hostile => HOSTILE_KINDS[engine_core::hash::hash_u32(seed ^ k as u32) as usize % HOSTILE_KINDS.len()],
    }
}

/// Fills the roster once, after the wing's gas grid is live.
pub(super) fn populate(h: &mut Habitat) {
    let Some(seed) = h.wing else { return };
    if h.populated || !h.ready || h.gas.is_empty() { return; }
    h.populated = true;
    let at = (h.origin + geometry::wing_origin()).as_vec3();
    for (k, m) in station::plan(seed).markers.iter().take(MAX).enumerate() {
        h.residents.push(Resident {
            slot: SLOT_BASE + k as u32,
            kind: kind(m.role, seed, k),
            pos: at + m.at.as_vec3() + Vec3::new(0.5, 0.0, 0.5),
            health: None,
        });
    }
}

pub(super) fn advance(world: &mut World, h: &Habitat, player: &Player) {
    let eye = player.precise_eye(world.voxel_scale).as_vec3() * world.voxel_scale.meters();
    for r in &h.residents {
        if world.adventure.enemies.len() >= 80 { break; }
        if r.pos.distance(eye) > NEAR_M
            || world.adventure.enemies.iter().any(|e| e.site == h.origin && e.slot == r.slot)
            || !crate::world::adventure::habitat_space(world, r.pos, r.kind) { continue; }
        let mut e = Enemy::new(h.origin, r.slot, r.kind, r.pos, 0);
        if let Some(health) = r.health { e.health = health; }
        world.adventure.enemies.push(e);
    }
}

/// Called with the cargo guards: stores position and health, drops the dead.
pub fn remember(world: &mut World, enemies: &[Enemy]) {
    for e in enemies.iter().filter(|e| e.slot >= SLOT_BASE) {
        let Some(h) = world.orbital.habitats.habitats.iter_mut().find(|h| h.origin == e.site) else { continue };
        if e.health <= 0.0 { h.residents.retain(|r| r.slot != e.slot); }
        else if let Some(r) = h.residents.iter_mut().find(|r| r.slot == e.slot) {
            r.pos = e.pos;
            r.health = Some(e.health);
        }
    }
}
