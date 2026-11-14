use super::tests::uniform;
use super::*;
use crate::simulation::WorldSimulation;
use engine_core::VoxelScalePreset;
use std::time::Duration;

/// Resident edited bytes above the default 64 MiB budget, with the dig and the
/// placement target outside the filled area so their chunks must be loaded.
fn over_budget_world() -> World {
    let mut w = uniform(VoxelScalePreset::HalfMeter, B::Dirt);
    let chunk_data = w.chunks.get(&IVec3::ZERO).unwrap().clone();
    for x in 20..60 {
        for z in 20..60 {
            w.chunks.insert(IVec3::new(x, 0, z), chunk_data.clone());
        }
    }
    assert!(w.edited_resident_bytes() > 64 * 1024 * 1024);
    w
}

#[test]
fn environment_dig_progresses_while_edited_bytes_exceed_budget() {
    let mut w = over_budget_world();
    let player = IVec3::new(31, 12, 31);
    for y in (1..=11).rev() {
        w.set(IVec3::new(31, y, 31), B::Air);
    }
    let mut sources = Residency::default();
    let mut blocks = HashSet::new();
    let mut chunks = HashSet::new();
    for _ in 0..1_200 {
        sources.poll(&mut w);
        update(&mut w, &mut sources, player, &mut blocks, &mut chunks, super::super::Budget::UNBOUNDED);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(w.environment.lag_seconds() <= 1, "environment {} s behind", w.environment.lag_seconds());
    assert!(w.environment.serviced > 50, "only {} events serviced", w.environment.serviced);
}

#[test]
fn tool_placement_into_unloaded_chunk_is_admitted_while_over_budget() {
    let mut w = over_budget_world();
    let mut sim = WorldSimulation::new();
    let cell = IVec3::new(-5, 1, -5);
    assert!(!w.chunks.contains_key(&chunk(cell)));
    let admitted = (0..2_000).any(|_| {
        let done = sim.prepare_tool_edit(&mut w, cell);
        std::thread::sleep(Duration::from_millis(1));
        done
    });
    assert!(admitted, "placement stayed deferred while edited bytes exceed the budget");
}
