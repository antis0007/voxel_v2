//! The workshop rescue uses the existing personal companion record.
use super::{Companion, GrowthEvent, SizeClass, emit_world};
use crate::world::{World, adventure::Adventure, chunk_coord_from_world_block, collides_world};
use glam::{IVec3, Vec3};

// Conjured lanterncaps use id 1. The saved id distinguishes the rescued one
// so existing rooted and logistics companions retain their behaviour.
pub(in crate::world::adventure) const ID: u32 = 2;

fn fits(world: &World, at: Vec3, size: SizeClass) -> bool {
    let scale = world.voxel_scale.meters();
    let half = size.half_width() * 1.25;
    let (lo, hi) = ((at - Vec3::new(half, 0.0, half)) / scale,
        (at + Vec3::new(half, size.height() + 0.09, half)) / scale);
    // The collision query can also inspect a fitting's adjacent cells. Leave
    // the cap in place while an edited chunk is paged out instead of reading
    // its record from disk on every simulation step.
    let first = chunk_coord_from_world_block(lo.floor().as_ivec3() - IVec3::ONE);
    let last = chunk_coord_from_world_block(hi.floor().as_ivec3() + IVec3::ONE);
    for z in first.z..=last.z {
        for y in first.y..=last.y {
            for x in first.x..=last.x {
                let chunk = IVec3::new(x, y, z);
                if !world.chunks.contains_key(&chunk) && world.regions.view.records.contains_key(&chunk) {
                    return false;
                }
            }
        }
    }
    let cell = (at / scale).floor().as_ivec3();
    !world.get(cell).is_liquid() && !collides_world(world, lo, hi)
}

fn nearby(world: &World, feet: Vec3, size: SizeClass) -> Option<Vec3> {
    if !feet.is_finite() { return None; }
    [Vec3::X, Vec3::Z, Vec3::NEG_X, Vec3::NEG_Z].into_iter()
        .map(|offset| feet + offset * 1.2 + Vec3::Y * 0.2)
        .find(|&at| fits(world, at, size) && super::super::line(world, feet + Vec3::Y * 0.5, at + Vec3::Y * 0.3))
}

pub(in crate::world::adventure) fn rescue(adventure: &mut Adventure, world: &World, feet: Vec3) -> bool {
    if adventure.companion.is_some() {
        return false;
    }
    let Some(at) = nearby(world, feet, SizeClass::Small) else { return false; };
    adventure.companion = Some(Companion::juvenile(ID, at));
    emit_world(adventure, GrowthEvent::Conjure, at);
    true
}

pub(in crate::world::adventure) fn follow(adventure: &mut Adventure, world: &World, player: &crate::player::Player, dt: f32) {
    let Some(cap) = adventure.companion.as_mut().filter(|cap| cap.id == ID && !cap.rooted
        && cap.crate_mark.is_none() && cap.pad_mark.is_none()) else { return; };
    if player.up_dir().dot(Vec3::Y) < 0.95 || adventure.death.is_some() {
        return;
    }
    let feet = player.pos * world.voxel_scale.meters();
    let delta = feet + Vec3::Y * 0.3 - cap.pos;
    let distance = delta.length();
    if !distance.is_finite() { return; }
    if distance > 7.0 {
        // Catch up after a blocked turn or a respawn, only to a clear perch
        // next to a grounded player. Ordinary steps never pass through walls.
        if player.on_ground && let Some(at) = nearby(world, feet, cap.size) {
            cap.pos = at;
        }
        return;
    }
    if distance <= 1.8 {
        return;
    }
    let step = delta * ((distance - 1.8).min(4.2 * dt) / distance);
    let next = cap.pos + step;
    if fits(world, next, cap.size) {
        cap.pos = next;
    }
}
