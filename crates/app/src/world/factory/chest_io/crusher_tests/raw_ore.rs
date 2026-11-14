use super::*;
use factory::blocks::Material;

fn raw_count(w: &World, at: IVec3, block: BlockId) -> u32 {
    w.block_data.chest(at).iter().flatten().filter(|s| s.item == Item::Block(block)).map(|s| s.count).sum()
}
fn raw_chest(w: &mut World, at: IVec3, items: &[(BlockId, u32)]) {
    w.set(at, BlockId::Chest);
    let mut slots = vec![None; BlockId::Chest.chest_slots()]; let mut index = 0;
    for &(block, mut count) in items {
        while count > 0 {
            let amount = count.min(w.inventory.stack_limit(Item::Block(block)));
            slots[index] = Some(ItemStack::new(Item::Block(block), amount)); index += 1; count -= amount;
        }
    }
    w.block_data.set_chest(at, slots);
}
fn raw_line(scale: VoxelScalePreset, block: BlockId, units: u32) -> World {
    let resource = crate::world::expedition::ore_resource(block).unwrap();
    let mut w = line(scale, resource as u8, 0); let amount = units / factory::placer::cell_volume(factory::step(&w));
    raw_chest(&mut w, POS - IVec3::X, &[(block, amount)]); w
}
fn raw_total(w: &World) -> u32 {
    total(w) + factory::blocks::raw_ores().filter(|(_, i)| *i < 3)
        .map(|(m, _)| raw_count(w, POS - IVec3::X, m.block()) * 2 * factory::placer::cell_volume(factory::step(w))).sum::<u32>()
}
fn tick(w: &mut World, sources: &mut Residency, cursor: &mut usize, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) {
    w.factory.tick(STEP_SECONDS, factory::step(w));
    factory::tick_world(w, &Player::new(), STEP_SECONDS, sources, cursor, Budget::UNBOUNDED, blocks, chunks);
}
fn run_raw(w: &mut World, steps: usize, expected: u32) {
    let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
    for _ in 0..steps { tick(w, &mut sources, &mut cursor, &mut blocks, &mut chunks); assert_eq!(raw_total(w), expected); }
}
#[test]
fn quarry_chests_all_ore_variants_preserve_volume_at_every_scale() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        for (material, ore) in factory::blocks::raw_ores().filter(|(_, i)| *i < 3) {
            let mut w = raw_line(scale, material.block(), 128); run_raw(&mut w, 3000, 256);
            assert_eq!(count(&w, dest(&w), (ore + 6) as u8), 4, "{scale:?}, {material:?}");
            assert_eq!(raw_count(&w, POS - IVec3::X, material.block()), 0);
            assert_eq!(w.factory.nodes[&POS].blocks.input.iter().sum::<u32>(), 0);
        }
    }
}
#[test]
fn quarry_chests_actual_excavation_reaches_the_ingot_chest() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        for kind in [Kind::Quarry, Kind::MiningDrill] {
            for block in [BlockId::CopperOre, BlockId::Cassiterite, BlockId::Hematite] {
                let mut w = raw_line(scale, block, 0); let size = factory::step(&w); let volume = factory::placer::cell_volume(size);
                w.factory.nodes.get_mut(&POS).unwrap().enabled = false;
                let q = POS - (IVec3::X + IVec3::Z) * size;
                let node = factory::fixture_place(&mut w, q, kind).unwrap();
                node.enabled = true; node.mode = if kind == Kind::Quarry {0} else {1}; node.placer.enabled = false;
                for face in 0..6 { sides::apply_role(node, face, Role::Off as u8); }
                sides::apply_role(node, 4, Role::Out as u8);
                let mut sample = node.clone(); let cells = 64 / volume; let mut terrain = Vec::new();
                for i in 0..=cells {
                    sample.excavation = i; let at = factory::quarry::cell(q, &sample, size);
                    let planted = if i < cells {block} else {BlockId::Bedrock};
                    w.set(at, planted); assert_eq!(w.get(at), planted, "fixture admission at {at:?}, {scale:?}");
                    if i < cells { terrain.push(at); }
                }
                factory::fixture_place(&mut w, q - IVec3::X * size, Kind::Cable).unwrap();
                let battery = factory::fixture_place(&mut w, q - IVec3::X * size * 2, Kind::Battery).unwrap();
                battery.upgrade = 1; battery.energy = factory::power::battery_capacity(1);
                assert!(terrain.iter().all(|&at| w.get(at) == block), "fixture changed during power setup");
                let material = Material::from_block(block).unwrap();
                let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
                for _ in 0..2400 {
                    tick(&mut w, &mut sources, &mut cursor, &mut blocks, &mut chunks);
                    let unmined = terrain.iter().filter(|&&at| w.get(at) == block).count() as u32;
                    assert_eq!(raw_total(&w) + 2 * volume * (unmined + w.factory.nodes[&q].blocks.output[material.index()]), 128,
                        "{scale:?}, {kind:?}, {block:?}, unmined={unmined}, cursor={}, source={:?}, ore_out={:?}, blocks={:?}, q={q:?}, cells={terrain:?}",
                        w.factory.nodes[&q].excavation, w.block_data.chest(POS-IVec3::X), w.factory.nodes[&q].output, w.factory.nodes[&q].blocks.output);
                    if raw_count(&w, POS - IVec3::X, block) == cells { break; }
                }
                assert!(terrain.iter().all(|&at| w.get(at) == BlockId::Air), "{scale:?}, {kind:?}, {block:?}");
                assert_eq!(raw_count(&w, POS - IVec3::X, block), cells);
                assert_eq!(w.factory.nodes[&q].excavation, cells);
                w.factory.nodes.get_mut(&q).unwrap().enabled = false;
                w.factory.nodes.get_mut(&POS).unwrap().enabled = true; run_raw(&mut w, 3000, 128);
                let ore = crate::world::expedition::ore_resource(block).unwrap();
                assert_eq!(count(&w, dest(&w), (ore + 6) as u8), 2);
            }
        }
    }
}
#[test]
fn quarry_chests_share_item_allowance_and_one_batch_across_representations() {
    let mut w = world(VoxelScalePreset::QuarterMeter); let mut n = Node::new(Kind::Crusher);
    let at = POS + IVec3::X * 4; raw_chest(&mut w, at, &[(BlockId::TinOre, 64), (BlockId::IronOre, 64)]);
    let mut slots = w.block_data.chest(at); slots[2] = Some(ItemStack::new(Item::Resource(0), 1)); w.block_data.set_chest(at, slots);
    raw_chest(&mut w, POS - IVec3::X, &[(BlockId::CopperOre, 64), (BlockId::Cassiterite, 64)]);
    service(&mut w, &mut n); assert_eq!(&n.input[..3], &[64, 15, 0]);
    assert_eq!(raw_count(&w, POS - IVec3::X, BlockId::CopperOre), 64);
    for _ in 0..8 { service(&mut w, &mut n); }
    assert_eq!(&n.input[..3], &[64, 64, 64]);
    assert_eq!(raw_count(&w, POS - IVec3::X, BlockId::Cassiterite), 64);
}
#[test]
fn quarry_chests_partial_packets_capacity_filters_and_unrelated_blocks() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut w = world(scale); let size = factory::step(&w); let volume = factory::placer::cell_volume(size);
        let at = POS + IVec3::X * size; let mut n = Node::new(Kind::Crusher); n.filter = Some(1); n.input[1] = 1;
        raw_chest(&mut w, at, &[(BlockId::CopperOre, 8), (BlockId::TinOre, 8), (BlockId::CoalOre, 8), (BlockId::Crystal, 8)]);
        w.factory.reserved.insert((POS, 1), 63); service(&mut w, &mut n); assert_eq!(n.input.iter().sum::<u32>(), 1);
        w.factory.reserved.insert((POS, 1), 62); service(&mut w, &mut n); assert_eq!(n.input[1], 1 + volume);
        assert_eq!(raw_count(&w, at, BlockId::TinOre), 7); assert_eq!(n.input[0], 0);
        w.factory.reserved.insert((POS, 1), CAPACITY - volume); n.input[1] = volume - 1;
        service(&mut w, &mut n); assert_eq!(n.input[1], volume - 1);
        assert_eq!(raw_count(&w, at, BlockId::CoalOre), 8); assert_eq!(raw_count(&w, at, BlockId::Crystal), 8);
    }
    let mut w = world(VoxelScalePreset::OneMeter); raw_chest(&mut w, POS + IVec3::X, &[(BlockId::CopperOre, 4)]);
    let mut furnace = Node::new(Kind::Furnace); furnace.mode = 1; service(&mut w, &mut furnace);
    assert_eq!(furnace.input[0], 0);
}
#[test]
fn quarry_chests_policy_pause_and_negative_face_edge() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut w = world(scale); let size = factory::step(&w); let at = POS + IVec3::new(-1, size - 1, size - 1);
        let mut n = Node::new(Kind::Crusher); raw_chest(&mut w, at, &[(BlockId::CopperOre, 4)]);
        policy::set(&mut w, at, policy::Policy { reserve: 3, ..Default::default() });
        n.enabled = false; service(&mut w, &mut n); assert_eq!(n.input[0], 0);
        n.enabled = true; sides::apply_role(&mut n, 1, Role::Out as u8); service(&mut w, &mut n); assert_eq!(n.input[0], 0);
        sides::apply_role(&mut n, 1, Role::In as u8); service(&mut w, &mut n);
        assert_eq!(n.input[0], factory::placer::cell_volume(size)); assert_eq!(raw_count(&w, at, BlockId::CopperOre), 3);
        policy::set(&mut w, at, policy::Policy { enabled: false, ..Default::default() });
        n.input[0] = 0; service(&mut w, &mut n); assert_eq!(n.input[0], 0);
        policy::set(&mut w, at, policy::Policy::default()); let slots = w.block_data.chest(at);
        w.set(at, BlockId::Stone); w.block_data.set_chest(at, slots);
        service(&mut w, &mut n); assert_eq!(n.input[0], 0);
    }
}
#[test]
fn quarry_chests_partial_batch_save_and_full_output_recovery() {
    let mut w = raw_line(VoxelScalePreset::QuarterMeter, BlockId::Hematite, 128);
    run_raw(&mut w, 31, 256); assert_eq!(w.factory.nodes[&POS].input[2], 16);
    let mut bytes = Vec::new(); w.factory.encode(&mut bytes);
    let mut words = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    w.factory = factory::Factory::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")), 4, factory::FACTORY_FORMAT).unwrap();
    w.block_data = block_data::sidecar::decode(&block_data::sidecar::encode(&w.block_data)).unwrap();
    assert_eq!(w.factory.nodes[&POS].input[2], 16); assert_eq!(raw_total(&w), 256);
    let mid = bridge(&w); w.block_data.set_chest(mid, vec![Some(ItemStack::new(Item::Block(BlockId::Stone), 64)); BlockId::Chest.chest_slots()]);
    w.factory.nodes.get_mut(&POS).unwrap().output[11] = CAPACITY;
    run_raw(&mut w, 300, CAPACITY + 256);
    assert_eq!(w.factory.nodes[&POS].input[2], 64); assert_eq!(w.factory.nodes[&POS].progress, 0);
    chest(&mut w, mid, &[]); run_raw(&mut w, 36000, CAPACITY + 256);
    assert_eq!(count(&w, dest(&w), 8), CAPACITY / 64 + 4);
}

#[test]
fn quarry_chests_reserved_packet_delivers_without_a_second_funded_batch() {
    let mut w = raw_line(VoxelScalePreset::QuarterMeter, BlockId::CopperOre, 64);
    w.factory.reserved.insert((POS, 0), 16);
    let mut n = w.factory.nodes.remove(&POS).unwrap();
    for _ in 0..4 { service(&mut w, &mut n); }
    assert_eq!(n.input[0], 48); assert_eq!(raw_count(&w, POS-IVec3::X, BlockId::CopperOre), 16);
    w.factory.nodes.insert(POS, n);
    w.factory.packets.push(factory::transit::Packet { item: 0, amount: 16, src: POS + IVec3::Y*8, dst: POS,
        path: vec![POS + IVec3::Y*8, POS], progress: 0.0 });
    w.factory.advance_packets(0.25);
    assert_eq!(w.factory.nodes[&POS].input[0], 64); assert_eq!(w.factory.input_reserved(POS, 0), 0);
    assert!(w.factory.packets.is_empty()); assert!(w.factory.take_refunds().is_empty());
    run_raw(&mut w, 3000, 160);
    assert_eq!(count(&w, dest(&w), 6), 2); assert_eq!(w.factory.nodes[&POS].input[0], 16);
}
#[test]
fn quarry_chests_fixed_authority_processes_physical_raw_ore() {
    let mut s = crate::headless::Session::new(WorldType::Superflat, VoxelScalePreset::HalfMeter, 7, None);
    s.world = raw_line(VoxelScalePreset::HalfMeter, BlockId::Malachite, 64);
    s.player.pos = POS.as_vec3() + glam::Vec3::splat(4.0); s.player.precise_pos = s.player.pos.as_dvec3();
    let mut calls = 0;
    while s.ticks < 3000 { s.tick(); calls += 1; assert!(s.sim.authority_error.is_none()); assert!(calls < 150000); }
    assert_eq!(raw_total(&s.world), 128); assert_eq!(count(&s.world, dest(&s.world), 6), 2);
}
#[test]
#[ignore = "wall-clock raw-intake measurement; run without concurrent builds"]
fn quarry_chests_loaded_raw_intake_obeys_shared_budget() {
    use std::time::{Duration, Instant};
    for machines in [32, 128, 512] { for micros in [750, 100] {
        let mut w = world(VoxelScalePreset::QuarterMeter); let mut positions = Vec::new();
        for i in 0..machines {
            let at = IVec3::new((i % 32) * 12, 192, (i / 32) * 12);
            w.factory.nodes.insert(at, Node::new(Kind::Crusher));
            raw_chest(&mut w, at + IVec3::X * 4, &[(BlockId::CopperOre, 64)]); positions.push(at);
        }
        let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
        let player = Player::new(); let mut times = Vec::new(); let mut buffered = 0;
        for _ in 0..1200 {
            let start = Instant::now();
            factory::tick_world(&mut w, &player, STEP_SECONDS, &mut sources, &mut cursor,
                Budget::wall(Duration::from_micros(micros)), &mut blocks, &mut chunks);
            times.push(start.elapsed().as_nanos() as u64);
            buffered = positions.iter().map(|at| w.factory.nodes[at].input[0]).sum::<u32>();
            if buffered == machines as u32 * 64 { break; }
        }
        let remaining: u32 = positions.iter().map(|at| raw_count(&w, *at + IVec3::X * 4, BlockId::CopperOre)).sum();
        times.sort_unstable();
        eprintln!("[quarry-intake-load] machines={machines} budget_us={micros} steps={} buffered={buffered} remaining={remaining} p50_us={:.1} p95_us={:.1} max_us={:.1}",
            times.len(), times[times.len()/2] as f64/1000.0, times[times.len()*95/100] as f64/1000.0, *times.last().unwrap() as f64/1000.0);
        assert_eq!(buffered + remaining, machines as u32 * 64); assert_eq!(remaining, 0);
    }}
}
