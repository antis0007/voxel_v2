use super::*;
use crate::{player::Player, simulation::{STEP_SECONDS, residency::Residency},
    world::{BlockId, factory}};
use engine_core::VoxelScalePreset;
use engine_voxel::WorldType;
use std::collections::HashSet;

const POS: IVec3 = IVec3::new(-16, 128, -16);
fn world(scale: VoxelScalePreset) -> World { World::with_seed(WorldType::Superflat, scale, 7) }
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
fn furnace(w: &World) -> IVec3 { POS + (IVec3::X + IVec3::Z) * factory::step(w) }
fn bridge(w: &World) -> IVec3 { furnace(w) - IVec3::Z }
fn dest(w: &World) -> IVec3 { furnace(w) + IVec3::X * factory::step(w) }
fn line(scale: VoxelScalePreset, item: u8, amount: u32) -> World {
    let mut w = world(scale); let size = factory::step(&w); let f = furnace(&w);
    for (at, kind, input, power_dir) in [(POS, Kind::Crusher, 1, -IVec3::Z), (f, Kind::Furnace, 5, IVec3::Y)] {
        let n = factory::fixture_place(&mut w, at, kind).unwrap();
        for face in 0..6 { sides::apply_role(n, face, Role::Off as u8); }
        sides::apply_role(n, input, Role::In as u8); sides::apply_role(n, 0, Role::Out as u8);
        factory::fixture_place(&mut w, at + power_dir * size, Kind::Cable).unwrap();
        let battery = factory::fixture_place(&mut w, at + power_dir * size * 2, Kind::Battery).unwrap();
        battery.upgrade = 1; battery.energy = factory::power::battery_capacity(1);
    }
    chest(&mut w, POS - IVec3::X, &[(item, amount)]);
    let (mid, out) = (bridge(&w), dest(&w)); chest(&mut w, mid, &[]); chest(&mut w, out, &[]);
    w
}
fn total(w: &World) -> u32 {
    let (c, f) = (&w.factory.nodes[&POS], &w.factory.nodes[&furnace(w)]);
    (0..3).map(|i| 2 * (count(w, POS - IVec3::X, i as u8) * 64 + c.input[i])
        + c.output[i + 9] + count(w, bridge(w), (i + 9) as u8) * 64
        + f.input[i + 9] + f.output[i + 6] + count(w, dest(w), (i + 6) as u8) * 64).sum()
}
fn run(w: &mut World, ticks: usize, expected: u32) {
    let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
    for _ in 0..ticks {
        w.factory.tick(STEP_SECONDS, factory::step(w));
        factory::tick_world(w, &Player::new(), STEP_SECONDS, &mut sources, &mut cursor, Budget::UNBOUNDED, &mut blocks, &mut chunks);
        assert_eq!(total(w), expected);
    }
}
#[test]
fn crusher_chests_complete_ore_doubling_at_all_scales() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        for item in 0..3 {
            let mut w = line(scale, item, 2); run(&mut w, 3000, 4 * 64);
            assert_eq!(count(&w, dest(&w), item + 6), 4, "{scale:?}, ore {item}");
            assert_eq!(count(&w, POS - IVec3::X, item), 0);
            assert_eq!(count(&w, bridge(&w), item + 9), 0);
        }
    }
}
#[test]
fn crusher_chests_filter_reservations_and_fractions_fund_one_batch() {
    let mut w = world(VoxelScalePreset::OneMeter); let mut n = Node::new(Kind::Crusher);
    chest(&mut w, POS + IVec3::X, &[(0, 8), (1, 8), (2, 8), (9, 8)]);
    n.filter = Some(1); n.input[1] = 1; w.factory.reserved.insert((POS, 1), 63);
    service(&mut w, &mut n); assert_eq!(n.input.iter().sum::<u32>(), 1);
    w.factory.reserved.clear(); service(&mut w, &mut n);
    assert_eq!(n.input[1], 65); assert_eq!(count(&w, POS + IVec3::X, 1), 7);
    n.input[1] = CAPACITY - 32; service(&mut w, &mut n); assert_eq!(n.input[1], CAPACITY - 32);
    n.filter = None; n.input = [0; RESOURCE_COUNT]; service(&mut w, &mut n);
    assert_eq!(&n.input[..3], &[64, 64, 64]); assert_eq!(n.input[9], 0);
    service(&mut w, &mut n); assert_eq!(&n.input[..3], &[64, 64, 64]);
}
#[test]
fn crusher_chests_paused_output_cadence_and_shared_allowance() {
    let mut w = world(VoxelScalePreset::OneMeter); let mut n = Node::new(Kind::Crusher); n.enabled = false;
    for dir in DIRS { chest(&mut w, POS + dir, &[(0, 8)]); }
    n.output[9] = 12 * 64 + 1; n.output[10] = 12 * 64; n.output[factory::GOLD_CONCENTRATE] = 12 * 64;
    service(&mut w, &mut n); assert_eq!(n.output.iter().sum::<u32>(), 20 * 64 + 1);
    assert_eq!(n.input.iter().sum::<u32>(), 0);
    transfer(&mut w, POS, &mut n, 1, Budget::UNBOUNDED);
    assert_eq!(n.output.iter().sum::<u32>(), 20 * 64 + 1);
    service(&mut w, &mut n); service(&mut w, &mut n);
    assert_eq!(n.output.iter().sum::<u32>(), 1);
    assert_eq!(DIRS.iter().map(|d| count(&w, POS + *d, 0)).sum::<u32>(), 48);
}
#[test]
fn crusher_chests_policies_and_physical_face_contacts() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        for role in [Role::In, Role::Out, Role::Off] {
            let mut w = world(scale); let size = factory::step(&w); let at = POS + IVec3::new(-1, size - 1, size - 1);
            let mut n = Node::new(Kind::Crusher); n.output[9] = 128;
            chest(&mut w, at, &[(0, 4)]); sides::apply_role(&mut n, 1, role as u8);
            policy::set(&mut w, at, policy::Policy { reserve: 3, target: 1, ..Default::default() });
            service(&mut w, &mut n);
            assert_eq!(n.input[0], if role == Role::In {64} else {0});
            assert_eq!(n.output[9], if role == Role::Out {64} else {128});
        }
    }
    let mut w = world(VoxelScalePreset::OneMeter); let mut n = Node::new(Kind::Crusher); n.output[9] = 128;
    let at = POS + IVec3::X; chest(&mut w, at, &[(0, 4)]);
    policy::set(&mut w, at, policy::Policy { enabled: false, ..Default::default() });
    service(&mut w, &mut n); assert_eq!((n.input[0], n.output[9]), (0, 128));
    policy::set(&mut w, at, policy::Policy { assigned: Some(Item::Resource(0)), ..Default::default() });
    service(&mut w, &mut n); assert_eq!((n.input[0], n.output[9]), (64, 128));
    n.input[0] = 0; w.set(at, BlockId::Stone);
    w.block_data.set_chest(at, vec![Some(ItemStack::new(Item::Resource(0), 4)); BlockId::Chest.chest_slots()]);
    service(&mut w, &mut n); assert_eq!((n.input[0], n.output[9]), (0, 128));
}
#[test]
fn crusher_chests_pause_and_power_loss_preserve_the_line() {
    let mut w = line(VoxelScalePreset::OneMeter, 2, 2);
    for n in w.factory.nodes.values_mut().filter(|n| n.kind == Kind::Battery) { n.enabled = false; }
    run(&mut w, 120, 256); assert_eq!(w.factory.nodes[&POS].progress, 0);
    for n in w.factory.nodes.values_mut().filter(|n| n.kind == Kind::Battery) { n.enabled = true; }
    run(&mut w, 180, 256); let c = w.factory.nodes.get_mut(&POS).unwrap();
    assert!(c.progress > 0); c.enabled = false;
    let before = (c.input, c.output, c.progress, c.energy, c.recipe);
    run(&mut w, 120, 256); let c = &w.factory.nodes[&POS];
    assert_eq!(before, (c.input, c.output, c.progress, c.energy, c.recipe));
    w.factory.nodes.get_mut(&POS).unwrap().enabled = true; run(&mut w, 3000, 256);
    assert_eq!(count(&w, dest(&w), 8), 4);
}
#[test]
fn crusher_chests_blocked_output_retains_paid_input_and_recovers() {
    let mut w = line(VoxelScalePreset::OneMeter, 0, 2); let mid = bridge(&w);
    w.block_data.set_chest(mid, vec![Some(ItemStack::new(Item::Block(BlockId::Stone), 64)); BlockId::Chest.chest_slots()]);
    let c = w.factory.nodes.get_mut(&POS).unwrap(); c.output[9] = CAPACITY; c.energy = 64;
    run(&mut w, 180, CAPACITY + 256);
    let c = &w.factory.nodes[&POS]; assert_eq!((c.input[0], c.output[9], c.progress, c.energy), (64, CAPACITY, 0, 64));
    chest(&mut w, mid, &[]); run(&mut w, 600, CAPACITY + 256);
    assert!(w.factory.nodes[&POS].output[9] < CAPACITY); assert!(count(&w, dest(&w), 6) > 0);
}
#[test]
fn crusher_chests_preserve_active_block_work_and_quartz_buffers() {
    let mut w = world(VoxelScalePreset::OneMeter); let mut n = Node::new(Kind::Crusher);
    let stone = factory::blocks::Material::Stone.index(); let gravel = factory::blocks::Material::Gravel.index();
    n.blocks.input[stone] = 1; n.progress = 5;
    chest(&mut w, POS + IVec3::X, &[(0, 2)]); service(&mut w, &mut n);
    assert_eq!((n.progress, n.recipe, n.blocks.input[stone], n.input[0]), (5, None, 1, 64));
    w.factory.nodes.insert(POS, n);
    for _ in 0..11 { w.factory.nodes.get_mut(&POS).unwrap().energy = 64; w.factory.tick(0.25, 1); }
    let n = &w.factory.nodes[&POS]; assert_eq!((n.blocks.input[stone], n.blocks.output[gravel]), (0, 1));
    assert_eq!(n.input[0], 64);
    let mut n = Node::new(Kind::Crusher); n.recipe = Some(recipes::QUARTZ_RECIPE); n.progress = 3;
    n.blocks.input[factory::blocks::Material::GoldQuartz.index()] = 1; n.output[factory::GOLD_CONCENTRATE] = 64;
    service(&mut w, &mut n);
    assert_eq!((n.recipe, n.progress, n.blocks.input[factory::blocks::Material::GoldQuartz.index()]), (Some(recipes::QUARTZ_RECIPE), 3, 1));
    assert_eq!(count(&w, POS + IVec3::X, factory::GOLD_CONCENTRATE as u8), 1);
}
#[test]
fn crusher_chests_paid_line_survives_save_codecs() {
    let mut w = line(VoxelScalePreset::HalfMeter, 1, 2); run(&mut w, 210, 256);
    assert!(w.factory.nodes[&POS].progress > 0);
    let before = w.factory.nodes[&POS].clone(); let mut bytes = Vec::new(); w.factory.encode(&mut bytes);
    let mut words = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    w.factory = factory::Factory::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")), factory::step(&w), factory::FACTORY_FORMAT).unwrap();
    w.block_data = block_data::sidecar::decode(&block_data::sidecar::encode(&w.block_data)).unwrap();
    let after = &w.factory.nodes[&POS];
    assert_eq!((before.input, before.output, before.progress, before.energy, before.recipe, before.faces), (after.input, after.output, after.progress, after.energy, after.recipe, after.faces));
    run(&mut w, 3000, 256); assert_eq!(count(&w, dest(&w), 7), 4);
}
#[test]
fn crusher_chests_fixed_authority_runs_the_complete_line() {
    let mut s = crate::headless::Session::new(WorldType::Superflat, VoxelScalePreset::OneMeter, 7, None);
    s.world = line(VoxelScalePreset::OneMeter, 0, 2); s.player.pos = POS.as_vec3() + glam::Vec3::splat(4.0);
    s.player.precise_pos = s.player.pos.as_dvec3(); let mut calls = 0;
    while s.ticks < 3000 { s.tick(); calls += 1; assert!(s.sim.authority_error.is_none()); assert!(calls < 150000); }
    assert_eq!(total(&s.world), 256); assert_eq!(count(&s.world, dest(&s.world), 6), 4);
}

#[test]
fn crusher_chests_paused_scheduler_keeps_fraction_and_source() {
    let mut w = line(VoxelScalePreset::OneMeter, 0, 2);
    let c = w.factory.nodes.get_mut(&POS).unwrap(); c.enabled = false; c.output[9] = 129;
    run(&mut w, 29, 385); assert_eq!(w.factory.nodes[&POS].output[9], 129);
    run(&mut w, 2, 385); assert_eq!(w.factory.nodes[&POS].output[9], 1);
    assert_eq!(w.factory.nodes[&POS].input[0], 0); assert_eq!(count(&w, POS - IVec3::X, 0), 2);
}
#[test]
fn crusher_chests_held_controller_keeps_processing_off_and_drains_output() {
    use engine_chip::{fab::Provenance, firmware::Task, presets::sentinel};
    let mut w = line(VoxelScalePreset::OneMeter, 0, 2);
    sides::apply_role(w.factory.nodes.get_mut(&POS).unwrap(), 2, Role::Out as u8);
    factory::fixture_place(&mut w, POS + IVec3::Y, Kind::Pipe).unwrap();
    let store = factory::fixture_place(&mut w, POS + IVec3::Y * 2, Kind::Storage).unwrap();
    store.filter = Some(9); store.input[9] = CAPACITY;
    let mut design = sentinel(factory::chips::registry()); design.firmware.task = Task::StockBand;
    let library = std::sync::Arc::make_mut(&mut w.factory.chips.library);
    let project = library.add_project(&design.name.clone(), design);
    let revision = library.release(factory::chips::registry(), project, 0).unwrap();
    let serial = library.package(revision, Provenance::Prepared).unwrap();
    w.factory.chips.install(&w.factory.nodes, POS, serial).unwrap();
    w.factory.nodes.get_mut(&POS).unwrap().output[9] = 128;
    run(&mut w, 240, 384);
    let c = &w.factory.nodes[&POS];
    assert!(!w.factory.chips.allows(POS)); assert_eq!((c.progress, c.recipe, c.input[0], c.output[9]), (0, None, 64, 0));
    assert_eq!(w.factory.nodes[&(POS + IVec3::Y * 2)].input[9], CAPACITY);
    w.factory.chips.remove(POS); run(&mut w, 3000, 384); assert_eq!(count(&w, dest(&w), 6), 6);
}
#[test]
#[ignore = "wall-clock scheduler measurement; run without concurrent builds"]
fn crusher_chests_load_budget_drains_without_loss() {
    use std::time::{Duration, Instant};
    for machines in [32, 128, 512] { for micros in [750, 100] {
        let mut w = world(VoxelScalePreset::OneMeter); let mut positions = Vec::new();
        for i in 0..machines {
            let at = IVec3::new((i % 32) * 3, 192, (i / 32) * 3);
            let mut n = Node::new(Kind::Crusher); n.enabled = false; n.output[9] = 32 * 64;
            w.factory.nodes.insert(at, n); chest(&mut w, at + IVec3::X, &[]); positions.push(at);
        }
        let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
        let player = Player::new(); let mut times = Vec::new(); let mut remaining = machines as u32 * 32 * 64;
        for _ in 0..1200 {
            let start = Instant::now();
            factory::tick_world(&mut w, &player, STEP_SECONDS, &mut sources, &mut cursor, Budget::wall(Duration::from_micros(micros)), &mut blocks, &mut chunks);
            times.push(start.elapsed().as_nanos() as u64);
            remaining = positions.iter().map(|at| w.factory.nodes[at].output[9]).sum();
            if remaining == 0 { break; }
        }
        let deposited: u32 = positions.iter().map(|at| count(&w, *at + IVec3::X, 9)).sum();
        assert_eq!(remaining + deposited * 64, machines as u32 * 32 * 64); assert_eq!(remaining, 0);
        times.sort_unstable();
        eprintln!("[crusher-load] machines={machines} budget_us={micros} steps={} p50_us={:.1} p95_us={:.1} max_us={:.1}",
            times.len(), times[times.len()/2] as f64/1000.0, times[times.len()*95/100] as f64/1000.0, *times.last().unwrap() as f64/1000.0);
    }}
}

mod raw_ore;
