use super::*;
use crate::{fluid::VoxelScalePreset, world::WorldType};
use wands::{EMPTY, ShotMods, evaluate_cast_block};

fn rig() -> (World, Player, Vec3, Adventure) {
    let scale = VoxelScalePreset::OneMeter;
    let w = World::with_seed(WorldType::Superflat, scale, 17);
    let mut p = Player::new();
    p.pos = Vec3::new(0.0, 150.0, 0.0) / scale.meters();
    p.flying = true;
    let eye = p.eye(scale) * scale.meters();
    let mut a = Adventure::default();
    a.wand = 3;
    a.rare_spells = !0;
    a.mana = 100.0;
    a.health = 100.0;
    a.loadout.aux_slots = [EMPTY; 4];
    (w, p, eye, a)
}

fn run(w: &mut World, p: &mut Player, frames: usize) {
    for _ in 0..frames {
        super::super::tick(w, p, 1.0 / 60.0, false, false, false);
    }
}

fn enemy_at(centre: Vec3, slot: u32) -> Enemy {
    let mut e = Enemy::new(IVec3::ZERO, slot, 0, centre, 1);
    let delta = centre - e.center();
    e.pos += delta;
    e
}

#[test]
fn bouncing_burst_reflects_off_a_wall_and_speeds_up() {
    let (mut w, mut p, eye, mut a) = rig();
    // A stone wall two metres east of the eye.
    for dy in -2..=2 {
        for dz in -2..=2 {
            let cell = (eye + Vec3::new(2.0, dy as f32, dz as f32)).floor().as_ivec3();
            w.set(cell, crate::world::BlockId::Stone);
        }
    }
    a.emit_spell(BOUNCING_BURST, eye, Vec3::X, None);
    assert_eq!(a.projectiles[0].mods.bounces, 6);
    let before = a.projectiles[0].velocity.x;
    w.adventure = a;
    run(&mut w, &mut p, 12);
    let shot = &w.adventure.projectiles[0];
    assert!(shot.velocity.x < 0.0, "bolt must head back west, vx {}", shot.velocity.x);
    assert!(shot.mods.bounces < 6, "a bounce was spent");
    assert!(shot.velocity.x.abs() > before, "each bounce adds speed");
}

#[test]
fn scatter_quad_casts_four_spells_in_a_wide_fan() {
    let (_, _, _, mut a) = rig();
    a.loadout.slots = [91, 0, 2, 0];
    a.loadout.aux_slots = [81, EMPTY, EMPTY, EMPTY];
    let eval = evaluate_cast_block(3, !0, 0, 0, &a.loadout, 0);
    assert_eq!(eval.actions.len(), 4);
    assert!(eval.spread_deg >= 18.0, "spread {}", eval.spread_deg);
}

#[test]
fn scatter_shot_fires_five_pellets_and_dig_bolt_bores() {
    let (_, _, eye, mut a) = rig();
    a.emit_spell(SCATTER_SHOT, eye, Vec3::NEG_Z, None);
    assert_eq!(a.projectiles.len(), SCATTER_PELLETS);
    let dirs: Vec<Vec3> = a.projectiles.iter().map(|s| s.velocity.normalize()).collect();
    assert!(dirs.iter().skip(1).all(|d| d.distance(dirs[0]) > 0.01), "pellets spread");
    assert!(dirs.iter().all(|d| d.dot(Vec3::NEG_Z) > 0.98), "a tight cone");
    a.projectiles.clear();
    a.emit_spell(DIG_BOLT, eye, Vec3::NEG_Z, None);
    assert_eq!(a.projectiles[0].bore_budget, 12);
    assert_eq!(spell_hit_damage(DIG_BOLT), 6.0, "dig bolt does not inherit the 22 floor");
}

#[test]
fn magic_missile_turns_toward_a_foe() {
    let (mut w, _, eye, mut a) = rig();
    a.enemies.push(enemy_at(eye + Vec3::new(4.0, 0.0, -8.0), 0));
    a.emit_spell(MAGIC_MISSILE, eye, Vec3::NEG_Z, None);
    let mut shot = a.projectiles.pop().unwrap();
    let speed = shot.velocity.length();
    for _ in 0..10 {
        a.arcana_flight(&mut w, &mut shot, 1.0 / 60.0);
    }
    assert!(shot.velocity.x > 0.5, "steered east, v {:?}", shot.velocity);
    assert!((shot.velocity.length() - speed).abs() < 1e-3, "keeps its speed");
    // Weak: after a sixth of a second it still mostly flies its own way.
    assert!(shot.velocity.normalize().dot(Vec3::NEG_Z) > 0.8);
}

#[test]
fn lodestone_pulls_foes_and_never_outlives_its_cap() {
    let (mut w, mut p, eye, mut a) = rig();
    // Long life three times over cannot stretch it past the cap.
    let mods = ShotMods { life_mul: 3.0, ..Default::default() };
    a.emit_spell_modified(LODESTONE, eye, Vec3::NEG_Z, None, mods);
    assert!(a.projectiles[0].life <= LODESTONE_MAX_LIFE);
    let mut shot = a.projectiles.pop().unwrap();
    a.enemies.push(enemy_at(shot.pos + Vec3::new(4.0, 0.0, 0.0), 0));
    a.arcana_flight(&mut w, &mut shot, 1.0 / 60.0);
    assert!(a.enemies[0].knock.x < 0.0, "foe dragged toward the orb");
    a.enemies.clear();
    a.projectiles.push(shot);
    w.adventure = a;
    run(&mut w, &mut p, (LODESTONE_MAX_LIFE * 60.0) as usize + 4);
    assert!(
        w.adventure.projectiles.iter().all(|s| s.spell != LODESTONE),
        "the attractor must expire"
    );
}

#[test]
fn soaked_strike_doubles_damage_on_wet_foes_only() {
    let mods = ShotMods { crit_wet: true, ..Default::default() };
    let mut dry = enemy_at(Vec3::ZERO, 0);
    assert_eq!(Adventure::arcana_hit_mul(&mods, &dry), 1.0);
    dry.status.wet = 2.0;
    assert_eq!(Adventure::arcana_hit_mul(&mods, &dry), 2.0);
    assert_eq!(Adventure::arcana_hit_mul(&ShotMods::default(), &dry), 1.0);
}

#[test]
fn skewer_passes_through_two_foes() {
    let (mut w, mut p, eye, mut a) = rig();
    a.loadout.slots = [90, 0, EMPTY, EMPTY];
    let eval = evaluate_cast_block(3, !0, 0, 0, &a.loadout, 0);
    assert_eq!(eval.actions[0].mods.pierce, 2);
    let mods = eval.actions[0].mods;
    for (i, z) in [-2.0_f32, -4.0, -6.0].into_iter().enumerate() {
        let mut e = enemy_at(eye + Vec3::new(0.0, 0.0, z), i as u32);
        e.health = 500.0;
        a.enemies.push(e);
    }
    a.emit_spell_modified(0, eye, Vec3::NEG_Z, None, mods);
    w.adventure = a;
    run(&mut w, &mut p, 30);
    let hurt = w.adventure.enemies.iter().filter(|e| e.health < 500.0).count();
    assert_eq!(hurt, 3, "the bolt went through two foes and stopped in the third");
}

#[test]
fn on_bounce_and_on_proximity_attach_a_payload() {
    let (mut w, _, eye, mut a) = rig();
    a.loadout.slots = [92, 0, 3, EMPTY];
    let eval = evaluate_cast_block(3, !0, 0, 0, &a.loadout, 0);
    assert_eq!(eval.actions[0].payload, Some(3));
    assert!(eval.actions[0].mods.bounce_release && eval.actions[0].mods.bounces >= 1);

    a.loadout.slots = [93, 0, 2, EMPTY];
    let eval = evaluate_cast_block(3, !0, 0, 0, &a.loadout, 0);
    assert_eq!(eval.actions[0].payload, Some(2));
    assert!(eval.actions[0].mods.proximity);
    a.emit_spell_modified(0, eye, Vec3::NEG_Z, Some(2), eval.actions[0].mods);
    let mut shot = a.projectiles.pop().unwrap();
    a.arcana_flight(&mut w, &mut shot, 1.0 / 60.0);
    assert!(shot.payload.is_some(), "no foe near, payload held");
    a.enemies.push(enemy_at(shot.pos + Vec3::new(0.0, 0.0, -2.0), 0));
    a.arcana_flight(&mut w, &mut shot, 1.0 / 60.0);
    assert!(shot.payload.is_none(), "payload released beside the foe");
    assert!(a.projectiles.iter().any(|s| s.spell == 2), "frost lance fired");
}

#[test]
fn freezing_shard_and_lightning_arc_carry_their_elements() {
    let (_, _, eye, mut a) = rig();
    a.emit_spell(FREEZING_SHARD, eye, Vec3::NEG_Z, None);
    a.emit_spell(LIGHTNING_ARC, eye, Vec3::NEG_Z, None);
    a.emit_spell(ACID_GLOB, eye, Vec3::NEG_Z, None);
    assert_eq!(a.projectiles[0].mods.element, Some(Element::Frost));
    assert!(a.projectiles[1].mods.chain);
    assert_eq!(a.projectiles[1].mods.element, Some(Element::Shock));
    assert!(a.projectiles[1].life < 0.5, "the arc is short-range");
    assert!(a.projectiles[2].mods.gravity > 0.0, "acid arcs");
}

#[test]
fn explosive_payload_and_undertow_change_the_impact() {
    let (mut w, _, eye, mut a) = rig();
    let at = eye + Vec3::new(0.0, 0.0, -10.0);
    a.enemies.push(enemy_at(at + Vec3::new(3.0, 0.0, 0.0), 0));
    a.enemies.push(enemy_at(at + Vec3::new(1.0, 0.0, 0.0), 1));
    let mods = ShotMods { pull: true, explosive: true, ..Default::default() };
    a.emit_spell_modified(0, at, Vec3::NEG_Z, None, mods);
    let shot = a.projectiles.pop().unwrap();
    let before = a.enemies[1].health;
    a.arcana_impact(&mut w, &shot, eye);
    assert!(a.enemies[0].knock.x < 0.0, "undertow drags the far foe in");
    assert!(a.enemies[1].health < before, "blast hurts the near foe");
    assert!(a.take_terrain_blast().is_some(), "a crater is queued");
}
