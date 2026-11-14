use super::super::{Adventure, Enemy, night};
use super::*;
use crate::world::WorldType;
use engine_core::{Budget, VoxelScalePreset};
use glam::IVec3;

const SEED: u32 = 2_738_958_700;
const MIDNIGHT: f32 = 0.0;
const NOON: f32 = 0.5;

fn flat_world(time_of_day: f32) -> (World, Vec3) {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, SEED);
    let feet = Vec3::new(0.5, (w.terrain_height_at(0, 0) + 1) as f32, 0.5);
    let cfg = w.sample_config();
    w.sandbox.climate.sync(cfg, time_of_day, 0.0);
    (w, feet)
}

fn night_body(kind: u32, pos: Vec3) -> Enemy {
    let mut e = Enemy::new(IVec3::new(0, 0, night::NIGHT_Z), 1, kind, pos, 0);
    e.cooldown = 0.0;
    e
}

#[test]
fn creeper_fuse_arms_in_reach_and_bursts_after_the_fuse() {
    let (w, feet) = flat_world(MIDNIGHT);
    let eye = feet + Vec3::Y * 1.6;
    let mut e = night_body(CREEPER, feet + Vec3::X * 2.0);
    let mut sounds = Vec::new();
    assert!(fuse(&mut e, &w, feet, eye, 0.05, &mut sounds).is_none());
    assert!(fusing(&e), "fuse starts inside FUSE_M");
    assert_eq!(sounds.len(), 1, "one hiss when the fuse starts");
    let mut burst = None;
    let mut ticks = 0;
    while burst.is_none() && ticks < 100 {
        burst = fuse(&mut e, &w, feet, eye, 0.05, &mut sounds);
        ticks += 1;
    }
    assert!(burst.is_some(), "the fuse bursts");
    assert!((ticks as f32 * 0.05 - FUSE_S).abs() < 0.11, "burst after {ticks} ticks");
    assert!(e.health <= 0.0, "the body is spent by its own blast");
}

#[test]
fn creeper_fuse_drops_when_the_player_walks_away() {
    let (w, feet) = flat_world(MIDNIGHT);
    let eye = feet + Vec3::Y * 1.6;
    let mut e = night_body(CREEPER, feet + Vec3::X * 2.0);
    let mut sounds = Vec::new();
    fuse(&mut e, &w, feet, eye, 0.05, &mut sounds);
    assert!(fusing(&e));
    let far = feet - Vec3::X * (ABORT_M + 1.0);
    assert!(fuse(&mut e, &w, far, far + Vec3::Y * 1.6, 0.05, &mut sounds).is_none());
    assert!(!fusing(&e), "a lost target resets the fuse");
    assert!(e.health > 0.0);
}

#[test]
fn creeper_does_not_arm_out_of_reach() {
    let (w, feet) = flat_world(MIDNIGHT);
    let eye = feet + Vec3::Y * 1.6;
    let mut e = night_body(CREEPER, feet + Vec3::X * (FUSE_M + 1.0));
    let mut sounds = Vec::new();
    for _ in 0..40 {
        assert!(fuse(&mut e, &w, feet, eye, 0.05, &mut sounds).is_none());
    }
    assert!(!fusing(&e));
    assert!(sounds.is_empty());
}

#[test]
fn burst_hurts_the_player_and_opens_a_crater() {
    let (mut w, feet) = flat_world(MIDNIGHT);
    let eye = feet + Vec3::Y * 1.6;
    let mut a = Adventure::default();
    a.creative = false;
    a.set_difficulty(super::super::difficulty::Difficulty::Normal);
    a.invulnerable = 0.0;
    let health = a.health;
    let at = feet + Vec3::X * 1.5 + Vec3::Y * 0.9;
    let floor = (feet - Vec3::Y * 0.5).floor().as_ivec3();
    assert!(w.solid(floor), "superflat ground under the player");
    // Terrain edits need a resident chunk.
    let chunk = floor.div_euclid(IVec3::splat(crate::world::CHUNK_SIZE as i32));
    assert!(w.admit_edit_chunk(chunk), "chunk {chunk:?} admitted");
    a.burst(&mut w, at, eye);
    assert!(a.health < health, "blast damage lands: {} -> {}", health, a.health);
    let crater = (at - Vec3::Y * 1.0).floor().as_ivec3();
    assert!(
        !w.solid(crater),
        "the blast removes ground below its centre: {:?} at {crater:?}, {} dirty cells",
        w.get(crater),
        a.dirty_voxels.len()
    );
}

#[test]
fn stalk_goal_sits_behind_the_player() {
    let feet = Vec3::new(10.0, 4.0, 10.0);
    let goal = stalk_goal(feet, Vec3::new(0.0, 0.0, -1.0));
    assert!((goal - Vec3::new(10.0, 4.0, 12.0)).length() < 1e-4);
}

#[test]
fn skeletons_burn_in_daylight_and_creepers_do_not() {
    let (w, feet) = flat_world(NOON);
    let mut a = Adventure::default();
    a.set_difficulty(super::super::difficulty::Difficulty::Normal);
    a.enemies.push(night_body(SKELETON, feet + Vec3::X * 30.0));
    a.enemies.push(night_body(CREEPER, feet + Vec3::Z * 30.0));
    let before: Vec<f32> = a.enemies.iter().map(|e| e.health).collect();
    night::tick(&mut a, &w, feet, Budget::UNBOUNDED);
    let skeleton = a.enemies.iter().find(|e| e.kind == SKELETON).expect("skeleton stays");
    let creeper = a.enemies.iter().find(|e| e.kind == CREEPER).expect("creeper stays");
    assert!(skeleton.health < before[0], "open daylight burns the skeleton");
    assert!(skeleton.status.burn > 0.0);
    assert_eq!(creeper.health, before[1], "the creeper is untouched by day");
}

#[test]
fn skeletons_do_not_burn_under_cover() {
    let (mut w, feet) = flat_world(NOON);
    let pos = feet + Vec3::X * 30.0;
    let cell = pos.floor().as_ivec3();
    for dy in 3..6 {
        w.set(cell + IVec3::Y * dy, crate::world::BlockId::Stone);
    }
    let mut a = Adventure::default();
    a.enemies.push(night_body(SKELETON, pos));
    let before = a.enemies[0].health;
    night::tick(&mut a, &w, feet, Budget::UNBOUNDED);
    assert_eq!(a.enemies[0].health, before, "a roof shields the body");
}

#[test]
fn night_packs_rise_beside_their_first_body() {
    let (w, feet) = flat_world(MIDNIGHT);
    let mut a = Adventure::default();
    a.set_difficulty(super::super::difficulty::Difficulty::Hard);
    for _ in 0..200 {
        night::tick(&mut a, &w, feet, Budget::UNBOUNDED);
    }
    let packed: Vec<&Enemy> = a
        .enemies
        .iter()
        .filter(|e| super::super::bestiary::species(e.kind).is_some_and(|s| s.pack > 1))
        .collect();
    assert!(!packed.is_empty(), "no pack species rose in 200 passes");
    // The night cap can cut a pack short, so one companion pair is the proof.
    let paired = packed.iter().any(|e| {
        packed.iter().any(|o| o.slot != e.slot && (o.pos - e.pos).length() < 6.0)
    });
    assert!(paired, "no pack body rose beside another");
}
