use super::*;
use crate::{player::Player, simulation::{STEP_SECONDS, residency::Residency},
    world::{BlockId, factory}};
use engine_core::VoxelScalePreset;
use engine_voxel::WorldType;
use std::collections::HashSet;

const POS: IVec3 = IVec3::new(-16, 64, -16);
fn world() -> World { World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7) }
fn chest(w: &mut World, at: IVec3, items: &[(u8, u32)]) {
    w.set(at, BlockId::Chest);
    let mut slots = vec![None; BlockId::Chest.chest_slots()];
    for (slot, &(item, count)) in slots.iter_mut().zip(items) { *slot = Some(ItemStack::new(Item::Resource(item), count)); }
    w.block_data.set_chest(at, slots);
}
fn count(w: &World, at: IVec3, item: u8) -> u32 {
    w.block_data.chest(at).iter().flatten().filter(|s| s.item == Item::Resource(item)).map(|s| s.count).sum()
}
fn service(w: &mut World, n: &mut Node) { n.work_clock = INTERVAL; transfer(w, POS, n, factory::step(w), Budget::UNBOUNDED); }
fn run(w: &mut World, ticks: usize, powered: bool) {
    let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
    let player = Player::new();
    for _ in 0..ticks {
        if powered { w.factory.tick(STEP_SECONDS, factory::step(w)); }
        factory::tick_world(w, &player, STEP_SECONDS, &mut sources, &mut cursor, Budget::UNBOUNDED, &mut blocks, &mut chunks);
    }
}
fn powered(scale: VoxelScalePreset, mode: u32, item: u8, amount: u32) -> World {
    let mut w = World::with_seed(WorldType::Superflat, scale, 7);
    let size = factory::step(&w);
    let n = factory::fixture_place(&mut w, POS, Kind::Furnace).unwrap();
    n.mode = mode;
    sides::apply_role(n, 0, Role::Out as u8);
    sides::apply_role(n, 1, Role::In as u8);
    factory::fixture_place(&mut w, POS - IVec3::Z * size, Kind::Cable).unwrap();
    let battery = factory::fixture_place(&mut w, POS - IVec3::Z * size * 2, Kind::Battery).unwrap();
    battery.upgrade = 1; battery.energy = factory::power::battery_capacity(1);
    chest(&mut w, POS - IVec3::X, &[(item, amount)]);
    chest(&mut w, POS + IVec3::X * size, &[]);
    w
}

#[test]
fn furnace_chests_feed_only_selected_modes_and_one_funded_batch() {
    for (mode, id, amount) in [(0, 9, 1), (1, 0, 1), (recipes::GOLD_MODE, factory::GOLD_CONCENTRATE as u8, 16)] {
        let mut w = world(); let mut n = Node::new(Kind::Furnace); n.mode = mode;
        chest(&mut w, POS + IVec3::X, &[(0, 32), (9, 32), (factory::GOLD_CONCENTRATE as u8, 32)]);
        service(&mut w, &mut n);
        assert_eq!(n.input[id as usize], amount * 64);
        assert_eq!(n.input.iter().sum::<u32>(), amount * 64);
        service(&mut w, &mut n);
        assert_eq!(count(&w, POS + IVec3::X, id), 32 - amount);
    }
}
#[test]
fn furnace_chests_reservations_fractions_and_filter_never_overbook() {
    let mut w = world(); let mut n = Node::new(Kind::Furnace);
    chest(&mut w, POS + IVec3::X, &[(9, 8), (10, 8), (11, 8)]);
    n.filter = Some(9); n.input[9] = 1; w.factory.reserved.insert((POS, 9), 63);
    service(&mut w, &mut n); assert_eq!(n.input.iter().sum::<u32>(), 1);
    w.factory.reserved.clear();
    service(&mut w, &mut n); assert_eq!(n.input[9], 65);
    assert_eq!(count(&w, POS + IVec3::X, 9), 7);
    n.input[9] = CAPACITY - 32;
    service(&mut w, &mut n); assert_eq!(n.input[9], CAPACITY - 32);
    assert_eq!(count(&w, POS + IVec3::X, 9), 7);
}
#[test]
fn furnace_chests_shared_gold_intake_and_mixed_output_allowances() {
    let mut w = world(); let mut n = Node::new(Kind::Furnace); n.mode = recipes::GOLD_MODE;
    for dir in DIRS { chest(&mut w, POS + dir, &[(factory::GOLD_CONCENTRATE as u8, 8)]); }
    n.output[6] = 12 * 64; n.output[7] = 12 * 64; n.output[8] = 12 * 64;
    service(&mut w, &mut n);
    assert_eq!(n.input[factory::GOLD_CONCENTRATE], 16 * 64);
    assert_eq!(n.output.iter().sum::<u32>(), 20 * 64);
    assert_eq!(DIRS.iter().map(|d| count(&w, POS + *d, factory::GOLD_CONCENTRATE as u8)).sum::<u32>(), 32);
    transfer(&mut w, POS, &mut n, 1, Budget::UNBOUNDED);
    assert_eq!(n.output.iter().sum::<u32>(), 20 * 64);
}
#[test]
fn furnace_chests_policies_roles_and_physical_chests_are_respected() {
    for role in [Role::In, Role::Out, Role::Off] {
        let mut w = world(); let mut n = Node::new(Kind::Furnace); let at = POS + IVec3::X;
        chest(&mut w, at, &[(9, 4)]); n.output[6] = 128;
        sides::apply_role(&mut n, 0, role as u8);
        policy::set(&mut w, at, policy::Policy { reserve: 3, target: 1, ..Default::default() });
        service(&mut w, &mut n);
        assert_eq!(n.input[9], if role == Role::In { 64 } else { 0 });
        assert_eq!(n.output[6], if role == Role::Out { 64 } else { 128 });
    }
    let mut w = world(); let mut n = Node::new(Kind::Furnace); let at = POS + IVec3::X;
    chest(&mut w, at, &[(9, 4)]); n.output[6] = 128;
    policy::set(&mut w, at, policy::Policy { enabled: false, ..Default::default() });
    service(&mut w, &mut n); assert_eq!((n.input[9], n.output[6]), (0, 128));
    policy::set(&mut w, at, policy::Policy { assigned: Some(Item::Resource(9)), ..Default::default() });
    service(&mut w, &mut n); assert_eq!((n.input[9], n.output[6]), (64, 128));
    n.input[9] = 0; w.set(at, BlockId::Stone);
    // A stale container record is not a physical input.
    w.block_data.set_chest(at, vec![Some(ItemStack::new(Item::Resource(9), 4)); BlockId::Chest.chest_slots()]);
    service(&mut w, &mut n); assert_eq!((n.input[9], n.output[6]), (0, 128));
}
#[test]
fn furnace_chests_paused_scheduler_preserves_partial_output_and_input() {
    let mut w = world(); let mut n = Node::new(Kind::Furnace); n.enabled = false; n.output[6] = 129;
    chest(&mut w, POS + IVec3::X, &[(9, 2)]); w.factory.nodes.insert(POS, n);
    run(&mut w, 29, false); assert_eq!(count(&w, POS + IVec3::X, 6), 0);
    run(&mut w, 2, false); assert_eq!(count(&w, POS + IVec3::X, 6), 2);
    assert_eq!(w.factory.nodes[&POS].output[6], 1);
    assert_eq!(count(&w, POS + IVec3::X, 9), 2);
}
#[test]
fn furnace_chests_leave_filtered_pipe_stock_orders_unchanged() {
    let mut w = world(); let mut n = Node::new(Kind::Furnace);
    n.target = stock_order::Order { item: 6, count: 4 }.target(); n.output[6] = 128;
    chest(&mut w, POS + IVec3::X, &[(9, 4)]);
    let revision = w.block_data.storage_revision;
    service(&mut w, &mut n);
    assert_eq!((n.input[9], n.output[6]), (0, 128));
    assert_eq!(w.block_data.storage_revision, revision);
}
#[test]
fn furnace_chests_powered_ore_dust_and_gold_finish_at_every_scale() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        for (mode, input, amount, output) in [(0, 9, 2, 6), (1, 0, 2, 6), (recipes::GOLD_MODE, factory::GOLD_CONCENTRATE as u8, 32, factory::GOLD_INGOT as u8)] {
            let mut w = powered(scale, mode, input, amount);
            run(&mut w, 1300, true);
            assert_eq!(count(&w, POS + IVec3::X * factory::step(&w), output), 2, "{scale:?}, mode {mode}");
            assert_eq!(count(&w, POS - IVec3::X, input), 0);
            assert_eq!(w.factory.nodes[&POS].input.iter().sum::<u32>() + w.factory.nodes[&POS].output.iter().sum::<u32>(), 0);
        }
    }
}
#[test]
fn furnace_chests_fine_scale_nonanchor_face_contacts_work() {
    for scale in [VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut w = World::with_seed(WorldType::Superflat, scale, 7); let size = factory::step(&w);
        let mut n = Node::new(Kind::Furnace); n.output[6] = 64;
        let at = POS + IVec3::new(-1, size - 1, size - 1);
        chest(&mut w, at, &[(9, 2)]); service(&mut w, &mut n);
        assert_eq!((n.input[9], n.output[6]), (64, 0));
        assert_eq!(count(&w, at, 6), 1);
    }
}
#[test]
fn furnace_chests_save_codecs_resume_paid_work_without_loss() {
    let mut w = powered(VoxelScalePreset::OneMeter, 0, 9, 2);
    run(&mut w, 151, true);
    assert!(w.factory.nodes[&POS].progress > 0);
    let before = w.factory.nodes[&POS].clone();
    let mut bytes = Vec::new(); w.factory.encode(&mut bytes);
    let mut words = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    w.factory = factory::Factory::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")), 1, factory::FACTORY_FORMAT).unwrap();
    let sidecar = block_data::sidecar::encode(&w.block_data);
    w.block_data = block_data::sidecar::decode(&sidecar).unwrap();
    let after = &w.factory.nodes[&POS];
    assert_eq!((before.input, before.output, before.energy, before.progress, before.recipe, before.faces), (after.input, after.output, after.energy, after.progress, after.recipe, after.faces));
    run(&mut w, 900, true);
    assert_eq!(count(&w, POS + IVec3::X, 6), 2);
    assert_eq!(count(&w, POS - IVec3::X, 9), 0);
}

#[test]
fn furnace_chests_full_output_and_legacy_gold_blocks_survive_pause() {
    let mut w = world(); let mut n = Node::new(Kind::Furnace); n.enabled = false;
    n.output[6] = 128; n.blocks.output[factory::blocks::Material::GoldBlock.index()] = 2;
    chest(&mut w, POS + IVec3::X, &[]);
    w.block_data.set_chest(POS + IVec3::X, vec![Some(ItemStack::new(Item::Resource(9), 64)); BlockId::Chest.chest_slots()]);
    w.factory.nodes.insert(POS, n); run(&mut w, 60, false);
    assert_eq!(w.factory.nodes[&POS].output[6], 128);
    chest(&mut w, POS + IVec3::X, &[]); run(&mut w, 60, false);
    assert_eq!(count(&w, POS + IVec3::X, 6), 2);
    assert_eq!(w.block_data.chest(POS + IVec3::X).iter().flatten().filter(|s| s.item == Item::Block(BlockId::GoldBlock)).map(|s| s.count).sum::<u32>(), 2);
}
#[test]
#[ignore = "wall-clock scheduler measurement; run without concurrent builds"]
fn furnace_chests_load_budget_drains_without_loss() {
    use std::time::{Duration, Instant};
    for machines in [32, 128, 512] { for micros in [750, 100] {
        let mut w = world(); let mut positions = Vec::new();
        for i in 0..machines {
            let at = IVec3::new((i % 32) * 3, 192, (i / 32) * 3);
            let mut n = Node::new(Kind::Furnace); n.enabled = false; n.output[6] = 32 * 64;
            w.factory.nodes.insert(at, n); chest(&mut w, at + IVec3::X, &[]); positions.push(at);
        }
        let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
        let player = Player::new(); let mut times = Vec::new(); let mut remaining = machines as u32 * 32 * 64;
        for _ in 0..1200 {
            let start = Instant::now();
            factory::tick_world(&mut w, &player, STEP_SECONDS, &mut sources, &mut cursor, Budget::wall(Duration::from_micros(micros)), &mut blocks, &mut chunks);
            times.push(start.elapsed().as_nanos() as u64);
            remaining = positions.iter().map(|at| w.factory.nodes[at].output[6]).sum();
            if remaining == 0 { break; }
        }
        let deposited: u32 = positions.iter().map(|at| count(&w, *at + IVec3::X, 6)).sum();
        assert_eq!(remaining + deposited * 64, machines as u32 * 32 * 64);
        times.sort_unstable();
        eprintln!("[furnace-load] machines={machines} budget_us={micros} steps={} remaining_units={remaining} p50_us={:.1} p95_us={:.1} max_us={:.1}",
            times.len(), times[times.len()/2] as f64/1000.0, times[times.len()*95/100] as f64/1000.0, *times.last().unwrap() as f64/1000.0);
        if micros == 750 { assert_eq!(remaining, 0); }
    }}
}

#[test]
fn furnace_chests_fixed_authority_runs_the_complete_chest_line() {
    let mut session = crate::headless::Session::new(WorldType::Superflat, VoxelScalePreset::OneMeter, 7, None);
    session.world = powered(VoxelScalePreset::OneMeter, 0, 9, 2);
    session.player.pos = POS.as_vec3() + glam::Vec3::new(4.0, 1.0, 4.0);
    session.player.precise_pos = session.player.pos.as_dvec3();
    let mut calls = 0;
    while session.ticks < 1300 {
        session.tick(); calls += 1;
        assert!(session.sim.authority_error.is_none());
        assert!(calls < 65000, "simulation stalled");
    }
    assert_eq!(count(&session.world, POS + IVec3::X, 6), 2);
    assert_eq!(count(&session.world, POS - IVec3::X, 9), 0);
}
