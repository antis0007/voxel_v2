//! Opt-in scheduler measurements and powered fixture coverage.
use super::{review_fixture as fixture, *};
use crate::{player::Player, simulation::{STEP_SECONDS, residency::Residency}, world::{World, BlockId, factory}};
use engine_core::{Budget, VoxelScalePreset};
use engine_voxel::WorldType;
use glam::IVec3;
use std::{collections::HashSet, time::{Duration, Instant}};

#[test]
fn assembler_review_powered_fixture_finishes_at_supported_scales() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut w = World::with_seed(WorldType::Superflat, scale, 617);
        let mut player = Player::new();
        fixture::install(&mut w, &mut player, 2).unwrap();
        let size = factory::step(&w);
        let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
        for _ in 0..720 {
            w.factory.tick(STEP_SECONDS, size);
            factory::tick_world(&mut w, &player, STEP_SECONDS, &mut sources, &mut cursor,
                Budget::UNBOUNDED, &mut blocks, &mut chunks);
            assert!(fixture::Ledger::capture(&w).unwrap().conserved(2));
        }
        assert!(fixture::Ledger::capture(&w).unwrap().complete(2), "scale {scale:?}");
    }
}

#[test]
#[ignore = "wall-clock scheduler measurement; run without concurrent builds"]
fn assembler_load_wall_budget_drains_all_nodes_without_loss() {
    for machines in [32, 128, 512] {
        for micros in [750, 100] {
            let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 617);
            let mut positions = Vec::new();
            for i in 0..machines {
                let at = IVec3::new((i % 32) * 3, 192, (i / 32) * 3);
                let chest = at + IVec3::X;
                w.set(chest, BlockId::Chest);
                w.block_data.set_chest(chest, vec![None; BlockId::Chest.chest_slots()]);
                let mut n = Node::new(Kind::Assembler);
                n.enabled = false;
                n.target = all_recipes().iter().position(|r| r.name == "Hand-wound induction coil").unwrap() as u32 + 1;
                n.output[16] = 32 * 64;
                w.factory.nodes.insert(at, n);
                positions.push((at, chest));
            }
            let player = Player::new();
            let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
            let mut times = Vec::new();
            let mut remaining = machines as u32 * 32 * 64;
            for _ in 0..1200 {
                let start = Instant::now();
                factory::tick_world(&mut w, &player, STEP_SECONDS, &mut sources, &mut cursor,
                    Budget::wall(Duration::from_micros(micros)), &mut blocks, &mut chunks);
                times.push(start.elapsed().as_nanos() as u64);
                remaining = positions.iter().map(|(at, _)| w.factory.nodes[at].output[16]).sum();
                if remaining == 0 { break; }
            }
            let deposited: u32 = positions.iter().map(|(_, chest)| w.block_data.chest(*chest).iter().flatten()
                .filter(|s| s.item == Item::Resource(16)).map(|s| s.count).sum::<u32>()).sum();
            assert_eq!(remaining + deposited * 64, machines as u32 * 32 * 64);
            times.sort_unstable();
            eprintln!("[assembler-load] machines={machines} budget_us={micros} steps={} remaining_units={remaining} p50_us={:.1} p95_us={:.1} max_us={:.1}",
                times.len(), times[times.len()/2] as f64 / 1000.0, times[times.len()*95/100] as f64 / 1000.0,
                times.last().unwrap().to_owned() as f64 / 1000.0);
            if micros == 750 { assert_eq!(remaining, 0, "production budget starved {machines} machines"); }
        }
    }
}
