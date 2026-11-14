//! A tool edit steps on the next cellular tick, ahead of queued backlog.
use super::*;
use crate::{player::Player, world::WorldType};

#[test]
fn edited_chunk_steps_before_a_full_backlog_chunk() {
    let mut world = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 7);
    let mut sim = WorldSimulation::new();
    sim.reset_for_world(VoxelScalePreset::OneMeter);
    let mut player = Player::new();
    // Open sky, so neither chunk holds anything that moves.
    player.pos = glam::Vec3::new(16.0, 336.0, 16.0);
    let backlog = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
    // More pending cells than one tick may step, queued before the edit.
    assert!(world.admit_edit_chunk(backlog));
    world.activity.merge(backlog, &[u64::MAX; crate::world::activity::WORDS]);
    let edit = backlog + IVec3::X;
    let cell = edit * crate::world::CHUNK_SIZE + IVec3::splat(16);
    let deadline=std::time::Instant::now()+std::time::Duration::from_secs(3);
    while !sim.prepare_tool_edit(&mut world, cell) {
        assert!(sim.tool_edit_waiting() && std::time::Instant::now()<deadline, "player source did not become ready");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(world.get(cell), B::Air);
    world.activity.wake(cell);
    assert_eq!(sim.edit_priority_chunks(), 1);
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    let before = world.activity.tick;
    // One cellular interval at 1 m is exactly one tick.
    sim.update(&mut world, &player, 0.08, &mut blocks, &mut chunks, crate::simulation::Budget::UNBOUNDED);
    assert_eq!(world.activity.tick, before + 1);
    assert!(!world.activity.contains(edit), "the edited chunk waited behind the backlog");
    assert!(world.activity.contains(backlog), "the tick cap still bounds the backlog chunk");
}

