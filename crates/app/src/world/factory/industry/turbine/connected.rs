use super::*;
use crate::world::{World, factory::{Action, extraction}, industry::State};
use engine_core::VoxelScalePreset;
use engine_voxel::WorldType;

fn world(scale: VoxelScalePreset) -> World { World::with_seed(WorldType::Superflat, scale, 7) }

#[test]
fn turbine_modes_reach_and_bottles_work_at_all_scales() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut w = world(scale);
        let p = IVec3::new(-16, 64, -16);
        let mut n = turbine(777); n.water = 1; n.separator_water_mg = 123;
        w.factory.nodes.insert(p, n.clone());
        for mode in [0, 1, 2] {
            factory::action(&mut w, p.as_vec3(), Action::SetRecipe(p, mode));
            assert_eq!(ledger(&w.factory.nodes[&p]), ledger(&n));
            assert_eq!(w.factory.nodes[&p].mode, if mode == 0 { 0 } else { 1 });
        }
        let far = p.as_vec3() + glam::Vec3::X * (8.0 / scale.meters());
        factory::action(&mut w, far, Action::SetRecipe(p, 0));
        assert_eq!(w.factory.nodes[&p].mode, 1);
        w.inventory.hotbar[0] = Some(ItemStack::new(Item::Bucket(0), 1));
        factory::action(&mut w, far, Action::Drain(p));
        assert_eq!(ledger(&w.factory.nodes[&p]), ledger(&n));
        factory::action(&mut w, p.as_vec3(), Action::Drain(p));
        assert_eq!(condensate(&w.factory.nodes[&p]), 0);
        assert_eq!(w.inventory.hotbar[0].as_ref().unwrap().item, Item::Bucket(WATER_UNIT_MG + 123));
    }
}

#[test]
fn turbine_water_source_returns_to_tank_but_never_draws_coolant() {
    for size in [1, 2, 4] {
        let mut f = Factory::default();
        let p = IVec3::ZERO; let q = IVec3::X * size * 2;
        let mut n = turbine(0); n.water = 300; n.enabled = true;
        f.nodes.insert(p, n);
        f.nodes.insert(IVec3::X * size, Node::new(Kind::FluidPipe));
        f.nodes.insert(q, Node::new(Kind::Tank));
        f.tick(0.25, size);
        assert_eq!((f.nodes[&p].water, f.nodes[&q].water), (172, 128));
        f.nodes.get_mut(&p).unwrap().water = 0;
        extraction::route_water(&mut f);
        assert_eq!((f.nodes[&p].water, f.nodes[&q].water), (0, 128));
    }
}

#[test]
fn turbine_water_rate_is_shared_across_disconnected_networks() {
    let mut f = Factory::default();
    let mut n = turbine(0); n.water = 300;
    f.nodes.insert(IVec3::ZERO, n);
    for d in [IVec3::X, IVec3::NEG_X] {
        f.nodes.insert(d, Node::new(Kind::FluidPipe));
        f.nodes.insert(d * 2, Node::new(Kind::Tank));
    }
    f.tick(0.25, 1);
    assert_eq!(f.nodes[&IVec3::ZERO].water, 172);
    assert_eq!(f.nodes[&(IVec3::X * 2)].water + f.nodes[&(IVec3::NEG_X * 2)].water, 128);
}

#[test]
fn turbine_boiler_tank_duct_and_electrical_load_conserve_mass() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut w = world(scale); let size = factory::step(&w);
        let p = IVec3::new(-16, 64, -16);
        for (x, kind) in [(0, Kind::WetScrubber), (1, Kind::ExhaustDuct), (2, Kind::GasTank),
            (3, Kind::ExhaustDuct), (4, Kind::GasGenerator), (5, Kind::Cable), (6, Kind::Furnace)] {
            w.factory.nodes.insert(p + IVec3::X * x * size, Node::new(kind));
        }
        let b = w.factory.nodes.get_mut(&p).unwrap();
        b.mode = 2; b.water = 1; b.energy = 24;
        let t = p + IVec3::X * 4 * size;
        w.factory.nodes.get_mut(&t).unwrap().mode = 1;
        w.factory.nodes.get_mut(&(p + IVec3::X * 2 * size)).unwrap().mode = 3;
        let mut state = State::default();
        for _ in 0..160 {
            w.factory.tick(0.25, size);
            for _ in 0..8 { super::super::super::route(&mut w, &mut state); }
        }
        let gas: u64 = w.factory.nodes.values().map(|n| u64::from(n.exhaust[STEAM])).sum();
        let liquid: u64 = w.factory.nodes.values().map(condensate).sum();
        let generated = w.factory.stats.window(false).0.generated;
        assert_eq!(gas + liquid, u64::from(WATER_UNIT_MG));
        assert!(generated > 0 && generated <= 12, "generation {generated}");
        assert_eq!(w.factory.stats.window(false).0.drawn, 24);
        assert!(w.factory.nodes[&(p + IVec3::X * 6 * size)].energy > 0);
        assert_eq!(condensate(&w.factory.nodes[&t]), generated * 96);
        assert_eq!(state.imported, [0; crate::world::industry::SPECIES]);
    }
}

#[test]
fn turbine_gas_intake_is_mode_specific() {
    for mode in [0, 1] {
        let mut w = world(VoxelScalePreset::OneMeter);
        let p = IVec3::new(-16, 64, -16);
        let mut source = Node::new(Kind::GasTank);
        source.exhaust[FUEL] = 1152; source.exhaust[STEAM] = 1152;
        w.factory.nodes.insert(p, source);
        let mut n = turbine(0); n.mode = mode;
        w.factory.nodes.insert(p + IVec3::X, n);
        super::super::super::route(&mut w, &mut State::default());
        let n = &w.factory.nodes[&(p + IVec3::X)];
        assert_eq!((n.exhaust[FUEL], n.exhaust[STEAM]), if mode == 0 { (288, 0) } else { (0, 288) });
        for species in [FUEL, STEAM] {
            assert_eq!(w.factory.nodes.values().map(|n| n.exhaust[species]).sum::<u32>(), 1152);
        }
    }
}

#[test]
fn turbine_condensate_blocks_dismantle_until_recovered() {
    let mut w = world(VoxelScalePreset::OneMeter);
    let p = IVec3::new(-16, 64, -16);
    let n = factory::fixture_place(&mut w, p, Kind::GasGenerator).unwrap();
    n.separator_water_mg = 1152;
    let (mut blocks, mut chunks) = (std::collections::HashSet::new(), std::collections::HashSet::new());
    assert!(factory::dismantle(&mut w, p, &mut blocks, &mut chunks));
    assert_eq!(w.factory.nodes[&p].separator_water_mg, 1152);
    w.inventory.hotbar[0] = Some(ItemStack::new(Item::Bucket(0), 1));
    factory::action(&mut w, p.as_vec3(), Action::Drain(p));
    assert!(factory::dismantle(&mut w, p, &mut blocks, &mut chunks));
    assert!(!w.factory.nodes.contains_key(&p));
    assert_eq!(w.inventory.hotbar[0].as_ref().unwrap().item, Item::Bucket(1152));
}
