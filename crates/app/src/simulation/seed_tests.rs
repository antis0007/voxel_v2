//! Load-time seeds wake only near the player; distant ones wait without loss.
use super::*;
use crate::world::WorldType;

fn world() -> World {
    World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 7)
}

fn one_cell() -> Box<Mask> {
    let mut mask = Box::new([0u64; crate::world::activity::WORDS]);
    mask[0] = 1;
    mask
}

#[test]
fn distant_seeds_wait_until_the_player_comes_near() {
    let mut world = world();
    let mut sim = WorldSimulation::new();
    let player = IVec3::new(5, 40, 5);
    let near = chunk_coord_from_world_block(player) + IVec3::new(SEED_RADIUS, 0, -SEED_RADIUS);
    let far = chunk_coord_from_world_block(player) + IVec3::new(SEED_RADIUS + 1, 0, 0);
    sim.seed(&mut world, player, near, one_cell());
    sim.seed(&mut world, player, far, one_cell());
    assert_eq!(world.activity.pending(), 1, "only the chunk inside the radius wakes at load");
    assert_eq!(sim.dormant_chunks(), 1);

    // Standing still keeps the distant seed asleep.
    sim.wake_dormant(&mut world, chunk_coord_from_world_block(player));
    assert_eq!(world.activity.pending(), 1);

    // Walking one chunk toward it brings it inside the radius.
    sim.wake_dormant(&mut world, chunk_coord_from_world_block(player) + IVec3::X);
    assert_eq!(world.activity.pending(), 2, "the held seed is merged, not lost");
    assert_eq!(sim.dormant_chunks(), 0);
}

#[test]
fn cave_water_under_the_player_waits_until_the_player_digs_down() {
    let mut world = world();
    let mut sim = WorldSimulation::new();
    let player = IVec3::new(5, 40, 5);
    let center = chunk_coord_from_world_block(player);
    let cave = center - IVec3::Y * (SEED_DEPTH + 1);
    sim.seed(&mut world, player, cave, one_cell());
    assert_eq!(world.activity.pending(), 0, "a cave floor below the reach stays asleep");
    sim.wake_dormant(&mut world, center - IVec3::Y);
    assert_eq!(world.activity.pending(), 1, "descending one chunk wakes it");
}

#[test]
fn repeated_distant_seeds_for_one_chunk_merge() {
    let mut world = world();
    let mut sim = WorldSimulation::new();
    let player = IVec3::ZERO;
    let far = IVec3::new(-(SEED_RADIUS + 3), -1, 0);
    let mut other = Box::new([0u64; crate::world::activity::WORDS]);
    other[1] = 1;
    sim.seed(&mut world, player, far, one_cell());
    sim.seed(&mut world, player, far, other);
    assert_eq!(sim.dormant_chunks(), 1);
    sim.wake_dormant(&mut world, far);
    assert_eq!(world.activity.pending(), 2, "both cells survive the merge");
}

#[test]
fn reset_forgets_held_seeds() {
    let mut world = world();
    let mut sim = WorldSimulation::new();
    sim.seed(&mut world, IVec3::ZERO, IVec3::new(40, 0, 40), one_cell());
    sim.reset_for_world(VoxelScalePreset::OneMeter);
    assert_eq!(sim.dormant_chunks(), 0);
}
