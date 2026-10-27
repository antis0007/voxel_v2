//! `afk_vs_adventure_factory`: a machine's ledger is a function of simulated
//! time only, never of where the player stands or whether the machine's chunk
//! is near the player. Origin, now reference only:
//! `docs/parked/07_PLAYER_FREEDOM_EXPLORATION_ECONOMY.md` §8 and §11.
use super::{fixture_place, Kind, RESOURCE_COUNT};
use crate::headless::Session;
use engine_core::VoxelScalePreset;
use engine_voxel::WorldType;
use glam::Vec3;

/// Furnace recipe 9: dust 9 to ingot 6, 64 per batch, 12 work steps of 0.25 s.
const DUST: usize = 9;
const INGOT: usize = 6;
const BATCHES: u32 = 10;
/// 30 s of simulation: 120 work steps, exactly `BATCHES` batches.
const TICKS: u32 = 120 * 30;
const SEED: u32 = 7;

#[derive(Debug, PartialEq, Eq)]
struct Ledger {
    input: [u32; RESOURCE_COUNT],
    output: [u32; RESOURCE_COUNT],
    energy: u32,
    progress: u32,
    status: &'static str,
}

/// Places a loaded furnace `machine_m` from the spawn, then moves the player
/// by `player_m` before running `TICKS` fixed steps.
fn run(machine_m: Vec3, player_m: Vec3) -> Ledger {
    let mut s = Session::new(WorldType::Superflat, VoxelScalePreset::OneMeter, SEED, None);
    // One-metre cells: player metres and voxel cells share units.
    let cell = (s.player.pos + machine_m).floor().as_ivec3();
    let node = fixture_place(&mut s.world, cell, Kind::Furnace).expect("furnace fixture");
    node.input[DUST] = 64 * BATCHES;
    node.energy = 100_000;
    s.player.pos += player_m;
    s.player.precise_pos = s.player.pos.as_dvec3();
    s.player.vel = Vec3::ZERO;
    let target = s.ticks + u64::from(TICKS);
    let mut calls = 0u32;
    while s.ticks < target {
        s.tick();
        calls += 1;
        assert!(s.sim.authority_error.is_none(), "{:?}", s.sim.authority_error);
        assert!(calls < TICKS * 50, "simulation stalled at tick {}", s.ticks);
    }
    let n = &s.world.factory.nodes[&cell];
    Ledger { input: n.input, output: n.output, energy: n.energy, progress: n.progress, status: n.status }
}

#[test]
fn afk_vs_adventure_factory_ledger_is_independent_of_player_distance() {
    let beside = run(Vec3::new(4.0, 0.0, 0.0), Vec3::ZERO);
    let player_far = run(Vec3::new(4.0, 0.0, 0.0), Vec3::new(500.0, 0.0, 0.0));
    let machine_far = run(Vec3::new(600.0, 0.0, 0.0), Vec3::ZERO);

    assert_eq!(beside.output[INGOT], 64 * BATCHES, "{beside:?}");
    assert_eq!(beside.input[DUST], 0, "{beside:?}");
    assert_eq!(beside, player_far);
    assert_eq!(beside, machine_far);
}

#[test]
fn afk_vs_adventure_factory_mid_run_ledger_matches() {
    // Half the input: production stops at "Needs input" in every configuration
    // at the same tick, so progress and status also have to agree.
    fn half(machine_m: Vec3, player_m: Vec3) -> Ledger {
        let mut s = Session::new(WorldType::Superflat, VoxelScalePreset::OneMeter, SEED, None);
        let cell = (s.player.pos + machine_m).floor().as_ivec3();
        let node = fixture_place(&mut s.world, cell, Kind::Furnace).expect("furnace fixture");
        node.input[DUST] = 64 * BATCHES / 2;
        node.energy = 100_000;
        s.player.pos += player_m;
        s.player.precise_pos = s.player.pos.as_dvec3();
        s.player.vel = Vec3::ZERO;
        let target = s.ticks + u64::from(TICKS);
        let mut calls = 0u32;
        while s.ticks < target {
            s.tick();
            calls += 1;
            assert!(calls < TICKS * 50, "simulation stalled at tick {}", s.ticks);
        }
        let n = &s.world.factory.nodes[&cell];
        Ledger { input: n.input, output: n.output, energy: n.energy, progress: n.progress, status: n.status }
    }
    let beside = half(Vec3::new(4.0, 0.0, 0.0), Vec3::ZERO);
    let player_far = half(Vec3::new(4.0, 0.0, 0.0), Vec3::new(500.0, 0.0, 0.0));
    assert_eq!(beside.output[INGOT], 64 * BATCHES / 2, "{beside:?}");
    assert_eq!(beside.status, "Needs input");
    assert_eq!(beside, player_far);
}
