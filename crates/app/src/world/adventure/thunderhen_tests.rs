use super::super::{Enemy, World};
use super::*;
use crate::world::WorldType;
use engine_core::VoxelScalePreset;
use glam::IVec3;

const SEED: u32 = 913_001;
const DT: f32 = 1.0 / 120.0;

fn arena() -> (World, Vec3) {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, SEED);
    let feet = Vec3::new(0.5, (w.terrain_height_at(0, 0) + 1) as f32, 0.5);
    let cfg = w.sample_config();
    w.sandbox.climate.sync(cfg, 0.5, 0.0);
    (w, feet)
}

fn hen(pos: Vec3) -> Enemy {
    let mut e = Enemy::new(IVec3::ZERO, 0, KIND, pos, 0);
    e.cooldown = 0.0;
    e.visible = true;
    e
}

fn cell(w: &World, p: Vec3) -> IVec3 {
    (p / w.voxel_scale.meters()).floor().as_ivec3()
}

/// Runs the hen through one whole action; `feet_at` places the player each
/// tick. Returns the damage dealt.
fn run_action(e: &mut Enemy, w: &mut World, mut feet_at: impl FnMut(&Enemy) -> Vec3) -> f32 {
    let mut total = 0.0;
    let mut sounds = Vec::new();
    let mut dirty = Vec::new();
    for _ in 0..1200 {
        let feet = feet_at(e);
        let (damage, _) = tick(e, w, feet, feet + Vec3::Y * 1.6, DT, &mut sounds, &mut dirty);
        total += damage;
        if e.hen.run.is_none() && e.attacks > 0 {
            break;
        }
    }
    total
}

#[test]
fn the_look_locks_then_strikes_the_mark() {
    let (mut w, feet) = arena();
    let mut e = hen(feet + Vec3::X * 8.0);
    let damage = run_action(&mut e, &mut w, |_| feet);
    assert!(e.hen.run.is_none());
    assert_eq!(e.hen.charge, CHARGE_MAX - 1, "one charge spent");
    assert_eq!(damage, STRIKE_DAMAGE, "a player standing on the mark is struck");
}

#[test]
fn moving_after_the_lock_avoids_the_strike() {
    let (mut w, feet) = arena();
    let mut e = hen(feet + Vec3::X * 8.0);
    let damage = run_action(&mut e, &mut w, |e| {
        if e.hen.locked { feet - Vec3::X * (STRIKE_M + 1.0) } else { feet }
    });
    assert_eq!(damage, 0.0);
    assert_eq!(e.hen.charge, CHARGE_MAX - 1, "the bolt still fires");
}

#[test]
fn a_conductor_near_the_mark_takes_the_strike() {
    let (mut w, feet) = arena();
    let rod = feet + Vec3::X * 3.0;
    w.set(cell(&w, rod), B::CopperBlock);
    let mut e = hen(feet + Vec3::X * 8.0);
    let damage = run_action(&mut e, &mut w, |_| feet);
    assert_eq!(damage, 0.0, "the rod grounds the bolt");
    assert!((conductors::landing(&w, feet, GROUNDING_M) - rod).length() < 1.0);
}

#[test]
fn conductors_ignore_stone_and_crystal() {
    let (mut w, feet) = arena();
    w.set(cell(&w, feet + Vec3::X), B::Crystal);
    w.set(cell(&w, feet - Vec3::X), B::Stone);
    assert_eq!(conductors::nearest(&w, feet, GROUNDING_M), None);
    assert!(conductors::conductive(B::PowerCable));
    assert!(conductors::conductive(B::GoldBlock));
    assert!(!conductors::conductive(B::Crystal));
}

#[test]
fn discharged_hen_roosts_on_stormstones_to_refill() {
    let (mut w, feet) = arena();
    let nest = feet + Vec3::X * 6.0;
    w.set(cell(&w, nest + Vec3::Z * 2.0 - Vec3::Y), B::Crystal);
    let mut e = hen(nest);
    e.home = nest;
    e.hen.ready = true;
    e.hen.charge = 0;
    e.hen.stormstones = 0;
    e.hen.scan = STORMSTONE_SCAN_S;
    // Out of sight and empty: the hen walks to the nest and roosts.
    e.visible = false;
    let mut sounds = Vec::new();
    let mut dirty = Vec::new();
    for _ in 0..1200 {
        tick(&mut e, &mut w, feet, feet + Vec3::Y * 1.6, DT, &mut sounds, &mut dirty);
        if e.hen.charge > 0 {
            break;
        }
    }
    assert_eq!(e.hen.charge, 1, "one roost refills one charge");
    assert!(e.hen.stormstones > 0);
}

#[test]
fn scratch_tears_furrows_behind_within_budget() {
    let (mut w, feet) = arena();
    let mut e = hen(feet + Vec3::X * 2.0);
    // Odd attack counts pick Scratch at melee range.
    e.attacks = 1;
    let behind = cell(&w, e.pos + Vec3::X * 1.5 - Vec3::Y * 0.5);
    assert!(w.get(behind).is_solid(), "the ground behind the hen starts solid");
    // Committed edits land only in resident chunks, as they do around a player.
    assert!(w.admit_edit_chunk(engine_core::chunk_coord_from_world_block(behind)));
    let mut sounds = Vec::new();
    let mut dirty = Vec::new();
    for _ in 0..600 {
        tick(&mut e, &mut w, feet, feet + Vec3::Y * 1.6, DT, &mut sounds, &mut dirty);
        if e.hen.furrowed > 0 && e.hen.run.is_none() {
            break;
        }
    }
    assert!(e.hen.furrowed > 0 && e.hen.furrowed as usize <= SCRATCH_CELLS, "furrowed {}", e.hen.furrowed);
    assert_eq!(dirty.len() as u32, e.hen.furrowed, "every removed cell is published");
    assert!(dirty.iter().all(|c| w.get(*c) == B::Air));
}

#[test]
fn stagger_interrupts_the_look() {
    let (mut w, feet) = arena();
    let mut e = hen(feet + Vec3::X * 8.0);
    let mut sounds = Vec::new();
    let mut dirty = Vec::new();
    tick(&mut e, &mut w, feet, feet + Vec3::Y * 1.6, DT, &mut sounds, &mut dirty);
    assert!(matches!(e.hen.run.as_ref().map(|r| r.kind), Some(Action::Look)));
    e.stagger = 1.0;
    tick(&mut e, &mut w, feet, feet + Vec3::Y * 1.6, DT, &mut sounds, &mut dirty);
    assert_eq!(e.hen.run.as_ref().map(|r| r.phase()), Some(Phase::Recover));
    assert_eq!(e.hen.charge, CHARGE_MAX, "an interrupted look spends nothing");
}
