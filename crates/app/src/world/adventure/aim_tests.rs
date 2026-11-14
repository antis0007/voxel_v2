//! Crosshair trajectories must not depend on the planet chart address.
use super::*;
use crate::{fluid::VoxelScalePreset, world::WorldType};
fn caster(scale: VoxelScalePreset, at: Vec3, rotation: Quat) -> (World, Player) {
    let mut world = World::with_seed(WorldType::Superflat, scale, 74);
    world.rules.mob_spawning = false;
    world.adventure.creative = true;
    world.adventure.spawn_timer = 100.0;
    world.adventure.wand = 3;
    world.adventure.loadout.slots = [0, wands::EMPTY, wands::EMPTY, wands::EMPTY];
    world.adventure.loadout.aux_slots = [wands::EMPTY; 4];
    let mut player = Player::new();
    player.pos = at / scale.meters();
    player.precise_pos = player.pos.as_dvec3();
    player.gravity_frame = rotation;
    player.yaw = 0.013;
    player.pitch = 0.017;
    player.flying = true;
    (world, player)
}

#[test]
fn planet_wand_aim_preserves_direction_across_chart_addresses() {
    let mut worst = 0.0_f32;
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        for x in [0.5, -23_999.5, 65_536.5, 327_680.5] {
            for rotation in [Quat::IDENTITY, Quat::from_rotation_z(1.2), Quat::from_rotation_x(-1.1)] {
                let (mut world, mut player) = caster(scale, Vec3::new(x, 150.0, -17_000.5), rotation);
                let eye = player.eye(scale) * scale.meters();
                let direction = player.forward_dir();
                let mut adventure = std::mem::take(&mut world.adventure);
                adventure.emit_spell(0, eye, direction, None);
                let velocity = adventure.projectiles[0].velocity;
                world.adventure = adventure;
                for _ in 0..120 { tick(&mut world, &mut player, 1.0 / 120.0, false, false, false); }
                let shot = &world.adventure.projectiles[0];
                let expected = eye.as_dvec3() + velocity.as_dvec3();
                let error = (shot.pos.as_dvec3() - expected).length() as f32;
                worst = worst.max(error);
                assert!(error < 0.04, "scale={scale:?} x={x} rotation={rotation:?} error={error} m");
            }
        }
    }
    eprintln!("planet aim maximum endpoint error: {worst:.6} m");
}

#[test]
fn planet_wand_aim_spawn_origin_matches_camera() {
    let cfg = crate::world::WorldSampleConfig {
        world_type: WorldType::Planet,
        generator_version: engine_voxel::PLANET_GENERATOR_VERSION,
        seed: 74,
        voxel_scale: VoxelScalePreset::QuarterMeter,
        planet: None,
    };
    let spawn = crate::planet::spawn(&cfg) * cfg.voxel_meters();
    eprintln!("planet seed=74 generator={} spawn_m={spawn:?}", cfg.generator_version);
    let (mut world, mut player) = caster(cfg.voxel_scale, spawn + Vec3::Y * 10.0, Quat::IDENTITY);
    player.precise_pos += glam::DVec3::new(0.001, 0.0, 0.001);
    player.pos = player.precise_pos.as_vec3();
    let eye = player.eye(cfg.voxel_scale) * cfg.voxel_meters();
    let mut adventure = std::mem::take(&mut world.adventure);
    adventure.cast_wand(&mut world, &mut player, eye, Vec3::NEG_Z);
    let shot = &adventure.projectiles[0];
    let precise_eye = player.precise_eye(cfg.voxel_scale) * f64::from(cfg.voxel_meters());
    assert!(shot.pos.as_dvec3().distance(precise_eye) < 0.04);
    assert!(shot.velocity.normalize().dot(Vec3::NEG_Z) > 0.995);
}

#[test]
fn planet_wand_aim_keeps_point_blank_collision() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let (mut world, mut player) = caster(scale, Vec3::new(327_679.95, 150.0, 0.125), Quat::IDENTITY);
        let eye = player.eye(scale) * scale.meters();
        let wall = IVec3::new((327_680.0 / scale.meters()) as i32, (eye.y / scale.meters()).floor() as i32, 0);
        world.set(wall, crate::world::BlockId::Stone);
        assert!(world.solid(wall), "wall fixture must exist");
        let mut adventure = std::mem::take(&mut world.adventure);
        adventure.emit_spell(0, eye, Vec3::X, None);
        assert_eq!(adventure.projectiles[0].pos, eye);
        world.adventure = adventure;
        tick(&mut world, &mut player, 1.0 / 120.0, false, false, false);
        assert!(world.adventure.projectiles.is_empty(), "{scale:?}: wall must not be skipped");
    }
}

#[test]
fn planet_wand_aim_hits_near_and_far_crosshair_cells() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        for distance in [2.0_f32, 26.0, 52.0] {
            let (mut world, mut player) = caster(scale, Vec3::new(327_680.8, 150.0, 17_000.125), Quat::IDENTITY);
            player.pitch = 0.0;
            let eye = player.eye(scale) * scale.meters();
            let direction = player.forward_dir();
            let target = ((eye.as_dvec3() + direction.as_dvec3() * f64::from(distance)) / f64::from(scale.meters())).floor().as_ivec3();
            world.set(target, crate::world::BlockId::Stone);
            assert!(world.solid(target), "target fixture must exist");
            let mut adventure = std::mem::take(&mut world.adventure);
            adventure.cast_wand(&mut world, &mut player, eye, direction);
            assert_eq!(adventure.projectiles.len(), 1);
            world.adventure = adventure;
            for _ in 0..(distance / 26.0 * 120.0).ceil() as usize + 3 {
                tick(&mut world, &mut player, 1.0 / 120.0, false, false, false);
            }
            assert!(world.adventure.projectiles.is_empty(), "{scale:?} at {distance} m: missed crosshair cell {target:?}");
        }
    }
}

#[test]
fn planet_wand_aim_drill_keeps_crosshair_path() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
        let (mut world, player) = caster(scale, Vec3::new(327_680.5, 150.0, -17_000.5), Quat::IDENTITY);
        let eye = player.eye(scale) * scale.meters();
        let direction = player.forward_dir();
        let mut adventure = std::mem::take(&mut world.adventure);
        adventure.cast_luminous_drill(&mut world, eye, direction, wands::ShotMods::default(), None);
        let beam = &adventure.lasers[0];
        let reach = wands::card_rules::DRILL_REACH;
        let expected = beam.a.as_dvec3() + direction.as_dvec3() * f64::from(reach);
        let error = beam.b.as_dvec3().distance(expected);
        assert!(error < 0.04, "{scale:?}: drill endpoint error {error} m");
    }
}

#[test]
fn planet_wand_aim_precision_resets_after_collision_correction() {
    let start = Vec3::new(327_680.5, 150.0, 0.5);
    let delta = Vec3::new(0.001, 0.0, -0.01);
    let mut flight = trajectory::Flight::new(delta);
    let mut at = start;
    for _ in 0..10 { at = flight.translate(at, delta); }
    flight.rebase(start);
    let same = flight.translate(start, Vec3::X * 0.01);
    flight.rebase(same);
    assert_eq!(flight.translate(same, Vec3::X * 0.01), same, "contact must discard a residual even when its rounded endpoint aliases");
    let corrected = at - Vec3::X * 5.0;
    let end = flight.translate(corrected, delta);
    assert!(end.as_dvec3().distance(corrected.as_dvec3() + delta.as_dvec3()) < 0.02);
}
#[cfg(feature = "multiplayer")]
#[test]
fn planet_wand_aim_replication_preserves_precision() {
    let (mut world, player) = caster(VoxelScalePreset::QuarterMeter, Vec3::new(327_680.5, 150.0, -17_000.5), Quat::IDENTITY);
    let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
    world.adventure.emit_spell_modified(0, eye, player.forward_dir(), None, wands::ShotMods::default());
    let shot = &mut world.adventure.projectiles[0];
    let step = shot.velocity / 120.0;
    for _ in 0..5 { shot.pos = shot.next_position(step); }
    let encoded = ron::to_string(shot).unwrap();
    let mut restored: Projectile = ron::from_str(&encoded).unwrap();
    for _ in 0..40 {
        shot.pos = shot.next_position(step);
        restored.pos = restored.next_position(step);
        assert_eq!(shot.pos, restored.pos);
    }
    let start = restored.pos;
    let expected = start.as_dvec3() + restored.velocity.as_dvec3() * 0.05;
    world.adventure.projectiles = vec![restored];
    Adventure::advance_multiplayer_projectiles(&mut world, 0.05);
    assert!(world.adventure.projectiles[0].pos.as_dvec3().distance(expected) < 0.04);
    #[derive(serde::Serialize)]
    struct LegacyFlight { normal: Vec3, distance: f32, snapline: Option<()> }
    let old = ron::to_string(&LegacyFlight { normal: Vec3::Y, distance: 0.0, snapline: None }).unwrap();
    let mut old: trajectory::Flight = ron::from_str(&old).unwrap();
    let mut at = Vec3::new(327_680.5, 0.0, 0.0);
    for _ in 0..100 { at = old.translate(at, Vec3::X * 0.001); }
    assert!((at.x - 327_680.6).abs() < 0.04);
}

#[test]
fn planet_wand_aim_actual_spawn_trajectory() {
    let cfg = crate::world::WorldSampleConfig {
        world_type: WorldType::Planet,
        generator_version: engine_voxel::PLANET_GENERATOR_VERSION,
        seed: 74,
        voxel_scale: VoxelScalePreset::OneMeter,
        planet: None,
    };
    let spawn = crate::planet::spawn(&cfg);
    let (mut world, mut player) = caster(cfg.voxel_scale, spawn + Vec3::Y * 10.0, Quat::IDENTITY);
    player.yaw = 0.1;
    let eye = player.eye(cfg.voxel_scale);
    let direction = player.forward_dir();
    let mut adventure = std::mem::take(&mut world.adventure);
    adventure.cast_wand(&mut world, &mut player, eye, direction);
    let expected = eye.as_dvec3() + adventure.projectiles[0].velocity.as_dvec3();
    world.adventure = adventure;
    for _ in 0..120 { tick(&mut world, &mut player, 1.0 / 120.0, false, false, false); }
    let error = world.adventure.projectiles[0].pos.as_dvec3().distance(expected);
    eprintln!("actual planet spawn 26 m shot endpoint error: {error:.6} m");
    assert!(error < 0.004, "actual planet spawn shot drifted {error} m");
}

#[test]
fn planet_wand_aim_curved_paths_keep_chart_precision() {
    for path in 1..=3 {
        let offset = Vec3::new(327_680.5, 150.0, -17_000.5);
        let mut near = Vec3::ZERO;
        let mut far = offset;
        let mut near_velocity = Vec3::new(0.13, 0.27, -26.0);
        let mut far_velocity = near_velocity;
        let mut near_flight = trajectory::Flight::new(near_velocity);
        let mut far_flight = trajectory::Flight::new(far_velocity);
        for _ in 0..720 {
            let a = near_flight.advance(&mut near_velocity, 1.0/360.0, path);
            let b = far_flight.advance(&mut far_velocity, 1.0/360.0, path);
            near = near_flight.translate(near, a);
            far = far_flight.translate(far, b);
        }
        assert_eq!(near_velocity, far_velocity);
        assert!((far.as_dvec3() - offset.as_dvec3()).distance(near.as_dvec3()) < 0.04);
    }
}
#[cfg(feature = "multiplayer")]
#[test]
fn planet_wand_aim_replica_tether_still_returns() {
    let (mut world, player) = caster(VoxelScalePreset::OneMeter, Vec3::new(327_680.5, 150.0, 0.5), Quat::IDENTITY);
    let eye = player.eye(world.voxel_scale);
    let direction = Vec3::NEG_Z;
    world.set(IVec3::new(327_680, eye.y.floor() as i32, -1), crate::world::BlockId::Stone);
    world.adventure.emit_spell_modified(snapline::SPELL, eye, direction, None, wands::ShotMods::default());
    let mut flight = trajectory::Flight::new(direction);
    flight.snapline = Some(snapline::Tether::new(eye, 0.0));
    world.adventure.projectiles[0].flight = Some(flight);
    let mut returning = false;
    for _ in 0..100 {
        world.adventure.replica_projectile_lead = 0.0;
        Adventure::advance_multiplayer_projectiles(&mut world, 1.0 / 120.0);
        if world.adventure.projectiles.is_empty() {
            assert!(returning, "outbound contact must turn the tether around, not destroy it");
            return;
        }
        returning |= world.adventure.projectiles[0].tether().is_some_and(|t| t.returning);
    }
    panic!("replica tether did not return to its anchor");
}

#[test]
fn planet_wand_aim_rebound_resets_precision_at_the_wall() {
    let (mut world, mut player) = caster(VoxelScalePreset::OneMeter, Vec3::new(327_679.75, 150.0, 0.5), Quat::IDENTITY);
    let eye = player.eye(world.voxel_scale);
    world.set(IVec3::new(327_680, eye.y.floor() as i32, 0), crate::world::BlockId::Stone);
    let mods = wands::ShotMods { bounces: 1, ..Default::default() };
    world.adventure.emit_spell_modified(0, eye, Vec3::X, None, mods);
    for _ in 0..8 {
        tick(&mut world, &mut player, 1.0 / 120.0, false, false, false);
        if world.adventure.projectiles[0].mods.bounces == 0 { break; }
    }
    let shot = &world.adventure.projectiles[0];
    assert!(shot.velocity.x < 0.0 && shot.pos.x < 327_680.0);
    let expected = shot.pos.as_dvec3() + shot.velocity.as_dvec3() / 120.0;
    tick(&mut world, &mut player, 1.0 / 120.0, false, false, false);
    assert!(world.adventure.projectiles[0].pos.as_dvec3().distance(expected) < 0.04);
}

#[test]
fn planet_wand_aim_phase_exit_does_not_resume_inside_wall() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
        for direction in [Vec3::X, Vec3::new(0.1, 0.0, -1.0).normalize()] {
            let (mut world, mut player) = caster(scale, Vec3::new(327_679.96875, 150.0, 0.5), Quat::IDENTITY);
            let s = scale.meters();
            let eye = player.eye(scale) * s;
            for z in -(12.0 / s) as i32..=(2.0 / s) as i32 {
                world.set(IVec3::new((327_680.0 / s) as i32, (eye.y / s).floor() as i32, z), crate::world::BlockId::Stone);
            }
            let mods = wands::ShotMods { phase: 1, ..Default::default() };
            world.adventure.emit_spell_modified(0, eye, direction, None, mods);
            for _ in 0..20 {
                tick(&mut world, &mut player, 1.0 / 120.0, false, false, false);
                if world.adventure.projectiles[0].mods.phase == 0 { break; }
            }
            let shot = &world.adventure.projectiles[0];
            assert!(shot.velocity.x > 0.0 && shot.pos.x >= 327_680.0 + s);
            let expected = shot.pos.as_dvec3() + shot.velocity.as_dvec3() / 120.0;
            tick(&mut world, &mut player, 1.0 / 120.0, false, false, false);
            assert!(world.adventure.projectiles[0].pos.as_dvec3().distance(expected) < 0.04);
        }
    }
}

#[test]
fn planet_wand_aim_precise_trace_hits_thin_walls_from_both_sides() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        for x in [-1_310_722, -1, 0, 1_310_724] {
            let mut world = World::with_seed(WorldType::Superflat, scale, 74);
            let cell = IVec3::new(x, (150.0 / scale.meters()) as i32, 0);
            world.set(cell, crate::world::BlockId::Stone);
            assert!(world.solid(cell));
            let center = (cell.as_dvec3() + glam::DVec3::splat(0.5)) * f64::from(scale.meters());
            for sign in [-1.0, 1.0] {
                let start = center + glam::DVec3::X * sign * (f64::from(scale.meters()) * 0.5 + 0.005);
                let end = center - glam::DVec3::X * sign * 3.0;
                let hit = trace::solid_contact_precise(&world, start, end).expect("thin wall");
                assert_eq!(hit.cell, cell);
                assert_eq!(hit.normal.x, sign as f32);
            }
        }
    }
}
