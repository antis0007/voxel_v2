use super::*;
use crate::world::{WorldType, adventure::difficulty::Difficulty};
use engine_core::VoxelScalePreset;

const STEP: f32 = crate::simulation::STEP_SECONDS;

fn carried(world: &World) -> usize {
    let inv = &world.inventory;
    inv.hotbar.iter().chain(&inv.pack).chain(&inv.equipment).chain(&inv.craft_grid)
        .chain(std::iter::once(&inv.cursor)).filter(|s| s.is_some()).count()
}

#[test]
fn environment_damage_follows_difficulty() {
    let mut adventure = Adventure::default();
    adventure.set_difficulty(Difficulty::Peaceful);
    adventure.environment_damage(50.0, Cause::Fall);
    assert_eq!(adventure.health, 100.0);
    adventure.set_difficulty(Difficulty::Hard);
    adventure.environment_damage(10.0, Cause::Contact(BlockId::Lava));
    assert_eq!(adventure.health, 85.0);
    assert_eq!(adventure.last_cause, Cause::Contact(BlockId::Lava));
}

#[test]
fn death_drops_inventory_and_respawn_returns_to_the_bed() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
        let mut world = World::new(WorldType::Superflat, scale);
        let mut player = Player::new();
        let feet_m = Vec3::new(4.0, 70.0, -3.0);
        player.pos = feet_m / scale.meters();
        // One live tick first: it may hand the starter wand to the inventory.
        super::super::tick(&mut world, &mut player, STEP, false, false, false);
        let stacks = carried(&world);
        assert!(stacks > 0, "starter inventory");
        let bed = Vec3::new(20.0, 90.0, 12.0);
        world.adventure.respawn_point = Some(bed);
        world.adventure.environment_damage(500.0, Cause::Drowned);
        assert_eq!(world.adventure.health, 0.0);

        super::super::tick(&mut world, &mut player, STEP, false, false, false);
        let death = world.adventure.death.expect("death screen");
        assert_eq!(death.cause, Cause::Drowned);
        assert_eq!(death.cause.label(), "Drowned");
        assert_eq!(death.dropped as usize, stacks);
        assert!((death.pos - feet_m).length() < 1e-3);
        // Reconciliation after the tick must not refill the emptied slots.
        assert_eq!(carried(&world), 0, "{scale:?}: inventory left on the body");
        assert_eq!(world.inventory.dropped.len(), stacks);
        assert!(world.inventory.dropped.iter().all(|d| (d.pos - feet_m).length() < 1.5));

        // The death screen holds the body in place until the respawn button.
        super::super::tick(&mut world, &mut player, STEP, false, false, false);
        assert!(world.adventure.is_dead());
        assert_eq!(world.inventory.dropped.len(), stacks, "drops happen once");

        respawn(&mut world, &mut player);
        assert!(!world.adventure.is_dead());
        assert_eq!(world.adventure.health, world.adventure.max_health());
        let spawned = player.pos * scale.meters();
        assert!((spawned.x - bed.x).abs() < 1e-3 && (spawned.z - bed.z).abs() < 1e-3);
        assert!(spawned.y >= bed.y - 1e-3 && spawned.y <= bed.y + 1.0 + 1e-3, "{spawned:?}");
    }
}

#[test]
fn respawn_without_a_bed_uses_the_world_spawn() {
    let mut world = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
    let mut player = Player::new();
    player.pos = Vec3::new(40.0, 70.0, 40.0);
    world.adventure.environment_damage(500.0, Cause::Fall);
    super::super::tick(&mut world, &mut player, STEP, false, false, false);
    assert_eq!(world.adventure.death.map(|d| d.cause), Some(Cause::Fall));
    let spawn = world.suggested_spawn();
    respawn(&mut world, &mut player);
    assert!((player.pos.x - spawn.x).abs() < 1e-3 && (player.pos.z - spawn.z).abs() < 1e-3);
}

#[test]
fn keep_inventory_skips_the_drop() {
    let mut world = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
    world.rules.keep_inventory = true;
    let mut player = Player::new();
    player.pos = Vec3::new(4.0, 70.0, -3.0);
    super::super::tick(&mut world, &mut player, STEP, false, false, false);
    let stacks = carried(&world);
    assert!(stacks > 0, "starter inventory");
    world.adventure.environment_damage(500.0, Cause::Fall);
    super::super::tick(&mut world, &mut player, STEP, false, false, false);
    assert_eq!(world.adventure.death.map(|d| d.dropped), Some(0));
    assert_eq!(carried(&world), stacks);
    assert!(world.inventory.dropped.is_empty());
    respawn(&mut world, &mut player);
    assert_eq!(carried(&world), stacks);
}
