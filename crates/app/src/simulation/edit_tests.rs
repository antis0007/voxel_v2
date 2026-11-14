//! The player's break loop against a real world: residency, harvest, one
//! batch of simulation steps, repeat. Guards the click path from silent refusals.
use crate::{
    fluid::VoxelScalePreset,
    player::Player,
    simulation::{Budget, WorldSimulation},
    world::{self, BlockId as B, World, WorldType},
};
use glam::IVec3;
use std::collections::HashSet;

fn surface_cell(world: &World, x: i32, z: i32, top: i32) -> Option<IVec3> {
    (0..=24)
        .map(|dy| IVec3::new(x, top - dy, z))
        .find(|&p| world.get(p) != B::Air)
}

#[test]
fn forty_hand_breaks_all_land_and_report_released_water() {
    let mut world = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 7);
    let cfg = world.sample_config();
    let spawn = world::suggested_spawn(&cfg).expect("hills spawn");
    let mut sim = WorldSimulation::new();
    sim.reset_for_world(VoxelScalePreset::OneMeter);
    let mut player = Player::new();
    player.pos = spawn;
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    let dt = 1.0 / 60.0;
    let mut broken = Vec::new();
    let mut refused = Vec::new();
    for i in 0..40 {
        let x = spawn.x.floor() as i32 + 3 + i;
        let z = spawn.z.floor() as i32 + 3;
        let Some(cell) = surface_cell(&world, x, z, spawn.y.floor() as i32 + 6) else {
            continue;
        };
        let before = world.get(cell);
        if before.is_liquid() {
            continue;
        }
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(3);
        while !sim.prepare_tool_edit(&mut world, cell) {
            assert!(sim.tool_edit_waiting() && std::time::Instant::now()<deadline, "player source did not become ready");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let ok = world::expedition::harvest(&mut world, cell, &mut blocks, &mut chunks);
        let after = world.get(cell);
        if ok && after == B::Air {
            broken.push((cell, before));
        } else {
            refused.push((cell, before, ok, after));
        }
        // The source lane is a worker thread; give it wall time like a real frame.
        for _ in 0..15 {
            sim.update(&mut world, &player, dt, &mut blocks, &mut chunks, Budget::UNBOUNDED);
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
    }
    for _ in 0..360 {
        sim.update(&mut world, &player, dt, &mut blocks, &mut chunks, Budget::UNBOUNDED);
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    let wet: Vec<_> = broken
        .iter()
        .filter(|(c, _)| world.water_volume(*c) > 0)
        .map(|(c, b)| (*c, *b, world.water_volume(*c)))
        .collect();
    println!(
        "[break-loop] broken={} wet={} of {} refused={:?} wet_cells={:?} active_chunks={} env_scheduled={} env_blocked={}",
        broken.len(),
        wet.len(),
        broken.len(),
        refused,
        wet,
        world.activity.pending(),
        world.environment.scheduled.len(),
        world.environment.blocked
    );
    let first_due = world.environment.due.first().copied();
    let due_cell = first_due.map(|(_, key)| world::environment::position(key));
    let neighbours_resident: Vec<_> = due_cell
        .map(|p| {
            [IVec3::ZERO, IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z]
                .iter()
                .map(|&d| (p + d, world.chunks.contains_key(&world::chunk_coord_from_world_block(p + d))))
                .collect()
        })
        .unwrap_or_default();
    let light = due_cell
        .and_then(|p| world.environment.light.get(&world::environment::key(p)))
        .map(|q| (q.next_y, q.leaf_cells, q.shade));
    println!(
        "[break-loop] light_query={light:?} sky_summaries={} light_queries={} stats={:?}",
        world.environment.sky.len(),
        world.environment.light.len(),
        sim.stats()
    );
    println!(
        "[break-loop] sim_status={:?} first_due={first_due:?} due_cell={due_cell:?} neighbours_resident={neighbours_resident:?} env_tick={} target_tick={} serviced={}",
        sim.status,
        world.environment.tick,
        world.environment.target_tick,
        world.environment.serviced
    );
    assert!(refused.is_empty(), "refused breaks: {refused:?}");
}
