//! Old Copper, the Thunderhen: a hen enlarged until her habits are weather.
//! Every action is a chicken behaviour. The Look is the sideways stare that
//! locks a mark and calls lightning; the Peck is a committed run with three
//! stabs; the Scratch tears two furrows behind her. Lightning lands on the
//! nearest conductor to the locked mark (`conductors`), so rods, cables and
//! metal blocks are the player's tools. Charge is finite: only roosting on
//! stormstones (`Crystal`) at the nest refills it.
use super::*;
use action::{Phase, Run};
use crate::world::BlockId as B;

pub(super) const KIND: u32 = 48;
/// Strikes stored at full charge; the comb reads it from a distance.
pub(super) const CHARGE_MAX: u8 = 3;
/// Fraction of the Look's preparation after which the mark no longer follows.
pub(super) const LOCK_AT: f32 = 0.55;
/// A conductor this close to the mark takes the strike instead.
pub(super) const GROUNDING_M: f32 = 4.0;
pub(super) const STRIKE_M: f32 = 2.2;
pub(super) const STRIKE_DAMAGE: f32 = 34.0;
pub(super) const PECK_DAMAGE: f32 = 14.0;
pub(super) const PECKS: u8 = 3;
pub(super) const SCRATCH_DAMAGE: f32 = 18.0;
/// Cells one scratch may remove; the furrows finish over later scratches.
pub(super) const SCRATCH_CELLS: usize = 48;
/// Lifetime furrow cap so a long fight cannot level the arena.
const FURROW_CAP: u32 = 2048;
/// Roosting at the nest with at least one stormstone refills one charge.
pub(super) const ROOST_S: f32 = 3.0;
pub(super) const NEST_M: f32 = 3.0;
const STORMSTONE_SCAN_S: f32 = 2.0;
const ENGAGE_M: f32 = 28.0;
const MELEE_M: f32 = 3.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(super) enum Action {
    Look,
    Peck,
    Scratch,
    Roost,
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(super) struct State {
    pub run: Option<Run<Action>>,
    pub charge: u8,
    /// Where the Look will strike; follows the player until `LOCK_AT`.
    pub mark: Vec3,
    pub locked: bool,
    /// Last landing point and seconds of flash left, for the telegraph pass.
    pub strike: Option<(Vec3, f32)>,
    pub pecks_left: u8,
    pub peck_gap: f32,
    pub furrowed: u32,
    /// Stormstones counted near the nest; rescanned every `STORMSTONE_SCAN_S`.
    pub stormstones: u8,
    pub scan: f32,
    pub ready: bool,
}

pub(super) fn action_name(e: &Enemy) -> Option<&'static str> {
    let hen = &e.hen;
    hen.run
        .as_ref()
        .map(|r| match (r.kind, r.phase()) {
            (_, Phase::Recover) => "Recovering",
            (Action::Look, _) if hen.locked => "Lightning locked — move off the mark",
            (Action::Look, _) => "The look — mark follows",
            (Action::Peck, _) => "Peck run — sidestep",
            (Action::Scratch, _) => "Scratch — rear furrows",
            (Action::Roost, _) => "Roosting — charge refills",
        })
        .or(Some(if hen.charge == 0 { "Discharged" } else { "Watching" }))
}

fn flat(v: Vec3) -> Vec3 {
    v * Vec3::new(1.0, 0.0, 1.0)
}

fn move_ground(e: &mut Enemy, world: &World, direction: Vec3, distance: f32) {
    let steps = (distance / 0.1).ceil().max(1.0) as u32;
    for _ in 0..steps {
        let q = e.pos + direction * distance / steps as f32;
        if free(world, q, KIND) && factory::solid_point(world, q - Vec3::Y * 0.15) {
            e.phase += q.distance(e.pos);
            e.pos = q;
        } else {
            break;
        }
    }
}

fn count_stormstones(world: &World, nest: Vec3) -> u8 {
    let scale = world.voxel_scale.meters();
    let origin = (nest / scale).floor().as_ivec3();
    let reach = ((NEST_M / scale).ceil() as i32).clamp(1, 6);
    let mut n = 0u8;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            for dz in -reach..=reach {
                if world.get(origin + IVec3::new(dx, dy, dz)) == B::Crystal {
                    n = n.saturating_add(1);
                }
            }
        }
    }
    n
}

/// Tears two furrows behind the hen, one cell deep, bounded per call.
fn scratch(e: &mut Enemy, world: &mut World, dirty: &mut Vec<IVec3>) {
    if e.hen.furrowed >= FURROW_CAP || dirty.len() > 1856 {
        return;
    }
    let s = world.voxel_scale.meters();
    let back = -e.lunge;
    let right = Vec3::new(-e.lunge.z, 0.0, e.lunge.x);
    let floor = e.pos.y - s * 0.5;
    let along = (2.5 / s).ceil() as usize;
    let across = (0.8 / s).ceil().max(1.0) as usize;
    let mut edits = Vec::new();
    for lane in [-1.0f32, 1.0] {
        for i in 0..along {
            for j in 0..across {
                let p = e.pos
                    + back * (1.0 + i as f32 * s)
                    + right * (lane * 0.9 + (j as f32 - (across as f32 - 1.0) * 0.5) * s)
                    + Vec3::Y * floor
                    - Vec3::Y * e.pos.y;
                let cell = (p / s).floor().as_ivec3();
                let old = world.get(cell);
                if clawable(old) && !edits.iter().any(|(q, _)| *q == cell) {
                    edits.push((cell, old));
                }
                if edits.len() >= SCRATCH_CELLS {
                    break;
                }
            }
        }
    }
    let mut applied = 0u32;
    world.set_many_committed(
        &edits.iter().map(|(p, _)| (*p, B::Air)).collect::<Vec<_>>(),
        |p, _, _| {
            applied += 1;
            dirty.push(p);
        },
    );
    e.hen.furrowed += applied;
}

/// Claws move earth and loose stone, never masonry, metal or machines.
fn clawable(block: B) -> bool {
    block.is_solid() && block.material().physics.hardness < 0.6 && !block.is_model()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    sounds: &mut Vec<Sound>,
    dirty: &mut Vec<IVec3>,
) -> (f32, Vec3) {
    navigation::gravity(e, world, dt);
    if !e.hen.ready {
        e.hen.ready = true;
        e.hen.charge = CHARGE_MAX;
        e.hen.stormstones = count_stormstones(world, e.home);
    }
    e.hen.scan += dt;
    if e.hen.scan >= STORMSTONE_SCAN_S {
        e.hen.scan = 0.0;
        e.hen.stormstones = count_stormstones(world, e.home);
    }
    if let Some((p, left)) = e.hen.strike {
        e.hen.strike = (left > dt).then_some((p, left - dt));
    }
    let mut damage = 0.0;
    let mut direction = Vec3::ZERO;
    if let Some(mut run) = e.hen.run.take() {
        if e.stagger > 0.3 && run.phase() == Phase::Prepare && run.kind != Action::Roost {
            run.interrupt();
        }
        if run.kind == Action::Look && run.phase() == Phase::Prepare {
            if run.wind() < LOCK_AT {
                e.hen.mark = feet;
            } else if !e.hen.locked {
                e.hen.locked = true;
                sound(sounds, Cue::Spark, Some(e.hen.mark), 0.6);
            }
        }
        let release = run.advance(dt);
        e.tell = if run.phase() == Phase::Prepare { run.prepare - run.elapsed } else { 0.0 };
        if release {
            match run.kind {
                Action::Look if e.hen.charge > 0 => {
                    e.hen.charge -= 1;
                    let landing = conductors::landing(world, e.hen.mark, GROUNDING_M);
                    e.hen.strike = Some((landing, 0.35));
                    let d = feet - landing;
                    if flat(d).length() < STRIKE_M
                        && d.y.abs() < 2.5
                        && line(world, landing + Vec3::Y * 0.3, eye)
                    {
                        damage = STRIKE_DAMAGE;
                        direction = (eye - landing).normalize_or_zero() * damage;
                        run.hit = true;
                    }
                    sound(sounds, Cue::Burst, Some(landing), 1.0);
                }
                Action::Scratch => {
                    scratch(e, world, dirty);
                    sound(sounds, Cue::Swing, Some(e.pos), 0.8);
                }
                Action::Peck => sound(sounds, Cue::Swing, Some(e.center()), 0.7),
                _ => {}
            }
        }
        if run.phase() == Phase::Active {
            let d = flat(feet - e.pos);
            match run.kind {
                Action::Peck => {
                    move_ground(e, world, e.lunge, dt * 6.0);
                    e.hen.peck_gap -= dt;
                    let ahead = d.normalize_or_zero().dot(e.lunge) > 0.3;
                    if e.hen.pecks_left > 0
                        && e.hen.peck_gap <= 0.0
                        && ahead
                        && d.length() < 2.6
                        && (feet.y - e.pos.y).abs() < 2.4
                        && line(world, e.center(), eye)
                    {
                        e.hen.pecks_left -= 1;
                        e.hen.peck_gap = 0.15;
                        damage += PECK_DAMAGE;
                        direction += e.lunge * PECK_DAMAGE;
                        run.hit = true;
                    }
                }
                Action::Scratch => {
                    let behind = d.normalize_or_zero().dot(e.lunge) < -0.4;
                    if !run.hit && behind && d.length() < 3.6 && (feet.y - e.pos.y).abs() < 2.0 {
                        damage = SCRATCH_DAMAGE;
                        direction = -e.lunge * damage;
                        run.hit = true;
                    }
                }
                _ => {}
            }
        }
        if run.phase() == Phase::Done {
            if run.kind == Action::Roost && e.hen.stormstones > 0 && e.hen.charge < CHARGE_MAX {
                e.hen.charge += 1;
            }
            e.hen.locked = false;
            e.cooldown = 0.5;
        } else {
            e.hen.run = Some(run);
        }
        return (damage, direction);
    }
    e.tell = 0.0;
    if e.cooldown > 0.0 {
        return (0.0, Vec3::ZERO);
    }
    let distance = flat(feet - e.pos).length();
    let active = distance < ENGAGE_M && e.visible;
    let wants_charge = e.hen.charge == 0 || (!active && e.hen.charge < CHARGE_MAX);
    let at_nest = flat(e.home - e.pos).length() < NEST_M;
    let kind = if wants_charge && e.hen.stormstones > 0 {
        if !at_nest {
            move_ground(e, world, flat(e.home - e.pos).normalize_or_zero(), dt * 1.4);
            return (0.0, Vec3::ZERO);
        }
        Action::Roost
    } else if !active {
        if !at_nest {
            move_ground(e, world, flat(e.home - e.pos).normalize_or_zero(), dt * 0.8);
        }
        return (0.0, Vec3::ZERO);
    } else if distance < MELEE_M {
        if e.attacks.is_multiple_of(2) { Action::Peck } else { Action::Scratch }
    } else if e.hen.charge > 0 && e.attacks % 3 != 2 {
        Action::Look
    } else if distance < 9.0 {
        Action::Peck
    } else {
        move_ground(e, world, flat(feet - e.pos).normalize_or_zero(), dt * 1.4);
        return (0.0, Vec3::ZERO);
    };
    let toward = flat(feet - e.pos).normalize_or_zero();
    if kind != Action::Roost {
        e.lunge = toward;
        e.yaw = (-toward.x).atan2(-toward.z);
    }
    e.hen.mark = feet;
    e.hen.locked = false;
    e.hen.pecks_left = PECKS;
    e.hen.peck_gap = 0.0;
    let (prepare, active, recover) = match kind {
        Action::Look => (1.4, 0.08, 1.0),
        Action::Peck => (0.7, 0.6, 0.9),
        Action::Scratch => (0.9, 0.5, 1.2),
        Action::Roost => (0.6, ROOST_S, 0.8),
    };
    e.attacks = e.attacks.wrapping_add(1);
    e.hen.run = Some(Run::new(kind, e.attacks, prepare, active, recover));
    e.tell = prepare;
    e.attack = 8;
    sound(sounds, if kind == Action::Roost { Cue::Bloom } else { Cue::Warning }, Some(e.center()), 0.6);
    (0.0, Vec3::ZERO)
}

/// Ground marks drawn by the danger pass: the strike mark, the furrow lanes
/// and the bolt itself for a few frames after it lands.
pub(super) fn telegraph(e: &Enemy, m: &mut Model) {
    if let Some((p, left)) = e.hen.strike {
        let k = (left / 0.35).clamp(0.0, 1.0);
        m.glow_beam(p, p + Vec3::Y * 40.0, 0.12 + 0.2 * k, [0.85, 0.9, 1.0], 2.5 * k, k);
    }
    let Some(run) = e.hen.run.as_ref() else { return };
    if run.phase() != Phase::Prepare {
        return;
    }
    match run.kind {
        Action::Look => {
            let colour = if e.hen.locked { [0.85, 0.92, 1.0] } else { [0.55, 0.7, 1.0] };
            tactics::ring(m, e.hen.mark, STRIKE_M, colour, if e.hen.locked { 1.4 } else { 0.6 });
        }
        Action::Scratch => {
            let right = Vec3::new(-e.lunge.z, 0.0, e.lunge.x);
            for lane in [-0.9f32, 0.9] {
                let a = e.pos - e.lunge * 1.0 + right * lane + Vec3::Y * 0.1;
                m.beam(a, a - e.lunge * 2.5, 0.08, [1.0, 0.44, 0.12], 0.9);
            }
        }
        Action::Peck => {
            let a = e.pos + Vec3::Y * 0.1;
            m.beam(a, a + e.lunge * 4.5, 0.08, [1.0, 0.44, 0.12], 0.9);
        }
        Action::Roost => {}
    }
}

mod model;
pub(super) use model::model;

#[cfg(test)]
#[path = "thunderhen_tests.rs"]
mod tests;
