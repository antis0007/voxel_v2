//! Furnace Saint and Chain Astronomer. Heat yields to water; the beam yields to a pillar or a broken ring.
use super::*;
use crate::audio::Cue;
use crate::world::BlockId;
use engine_worldgen::hall;

pub(super) const FURNACE: u32 = hall::FURNACE;
pub(super) const ASTRONOMER: u32 = hall::ASTRONOMER;

pub(super) fn name(kind: u32) -> &'static str {
    if kind == FURNACE { "Furnace Saint" } else { "Chain Astronomer" }
}

fn wet(world: &World, feet: Vec3) -> bool {
    let c = (feet / world.voxel_scale.meters()).floor().as_ivec3();
    world.get(c) == BlockId::Water || world.get(c - IVec3::Y) == BlockId::Water
}

fn ring_broken(world: &World) -> bool {
    let cells = hall::ring_cells(&world.sample_config());
    cells.iter().filter(|p| world.get(**p) == BlockId::StarGlass).count() < 4
}

pub(super) fn tick(
    e: &mut Enemy,
    world: &World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    shots: &mut Vec<Projectile>,
    _hazards: &mut Vec<tactics::Hazard>,
    sounds: &mut Vec<Sound>,
    _dirty: &mut Vec<IVec3>,
    _navigation_budget: &mut usize,
) -> (f32, Vec3) {
    let to = feet - e.pos;
    let dist = to.length();
    e.yaw = to.x.atan2(to.z);
    if e.tell > 0.0 {
        e.tell = (e.tell - dt).max(0.0);
        if e.tell > 0.0 || e.attack != 1 {
            return (0.0, Vec3::ZERO);
        }
        e.attack = 0;
        let furnace = e.kind == FURNACE;
        let melee = dist < 2.6;
        let cooled = furnace && wet(world, feet);
        let broken = !furnace && ring_broken(world);
        let exposed = line(world, e.pos + Vec3::Y * 1.75, eye);
        if furnace && !melee && dist <= 9.0 && shots.len() < 96 {
            let origin = e.pos + Vec3::Y * 1.6;
            let aim = feet + Vec3::Y * 1.05;
            shots.push(Projectile {
                farm: None,
                pos: origin,
                velocity: (aim - origin).normalize_or_zero() * 7.0,
                hostile: true,
                payload: None,
                payload_mods: Default::default(),
                spell: 0,
                life: 2.4,
                mods: wands::ShotMods {
                    damage_mul: if cooled { 0.35 } else { 1.1 },
                    ..Default::default()
                },
                bore_budget: 0,
                trail_timer: 0.0,
                flight: None,
                splashed: false,
                follow: None,
            });
        }
        let mut damage = if !exposed {
            0.0
        } else if melee {
            if cooled { 3.0 } else if furnace { 14.0 } else { 7.0 }
        } else if furnace {
            // The projectile still has to reach the player. Standing in
            // water protects against the close heat pulse separately.
            if dist < 4.2 && !cooled { 5.0 } else { 0.0 }
        } else {
            if broken { 7.0 } else { 18.0 }
        };
        if dist > if furnace { 9.0 } else { 14.0 } {
            damage = 0.0;
        }
        e.cooldown = if cooled { 3.4 } else if furnace { 1.8 } else if broken { 4.2 } else { 1.7 };
        if damage > 0.0 {
            sound(sounds, if furnace { Cue::ForgeSlam } else { Cue::ArchiveRelease }, Some(e.center()), 0.8);
        }
        return (damage, to.normalize_or_zero());
    }
    e.cooldown = (e.cooldown - dt).max(0.0);
    if dist > 7.5 && e.charge <= 0.0 {
        let flat = Vec3::new(to.x, 0.0, to.z).normalize_or_zero();
        e.charge = 0.32;
        e.lunge = flat * 0.85;
    }
    let range = if e.kind == FURNACE { 9.0 } else { 14.0 };
    if e.cooldown == 0.0 && dist < range && dist > 0.4 {
        e.attack = 1;
        e.tell = if dist < 2.6 { 0.45 } else { 1.1 };
        sound(sounds, if e.kind == FURNACE { Cue::ForgeWindup } else { Cue::ArchiveWindup }, Some(e.center()), 0.55);
    }
    (0.0, Vec3::ZERO)
}

pub(super) fn model(e: &Enemy, m: &mut Model, _time: f32) {
    let mark = m.mark();
    let furnace = e.kind == FURNACE;
    let yaw = Quat::from_rotation_y(e.yaw);
    let energy = if e.tell > 0.0 { 1.4 } else { 0.25 };
    let body = if furnace { [0.62, 0.28, 0.12] } else { [0.16, 0.15, 0.18] };
    m.material_box(e.pos + Vec3::Y * 1.05, Vec3::new(0.72, 1.55, 0.42), yaw, body, 39.0, if furnace { energy } else { 0.0 });
    let head = if furnace { [0.95, 0.46, 0.16] } else { [0.55, 0.72, 0.95] };
    m.material_box(e.pos + Vec3::Y * 1.95, Vec3::new(0.38, 0.38, 0.38), yaw, head, 39.0, if furnace { energy } else { 0.8 });
    super::enemy_presentation::motion::pose_body(e, m, mark);
}
