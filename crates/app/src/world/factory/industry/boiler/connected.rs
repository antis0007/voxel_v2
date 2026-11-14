use super::*;
use crate::world::{World, factory::{Action, extraction}, industry::State, inventory::{Item, ItemStack}};
use engine_core::VoxelScalePreset;
use engine_voxel::WorldType;
fn world(scale: VoxelScalePreset) -> World { World::with_seed(WorldType::Superflat, scale, 7) }
const ACID: usize = 2;
const SCALES: [VoxelScalePreset; 3] = [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter];

#[test]
fn fired_boiler_modes_fuel_acceptance_and_remainder_recovery_are_authoritative() {
    for scale in SCALES {
        let mut w = world(scale); let p = IVec3::new(-16, 64, -16);
        let mut n = boiler(); n.water = 0; n.separator_water_mg = 123; n.burn = 7;
        n.input[3] = 16; w.factory.nodes.insert(p, n.clone());
        for (id, _, _) in COMBUSTION_FUELS { assert!(n.accepts(id)); }
        assert!(!n.accepts(13));
        factory::action(&mut w, p.as_vec3(), Action::SetRecipe(p, 0));
        assert_eq!(w.factory.nodes[&p].mode, MODE);
        w.factory.nodes.get_mut(&p).unwrap().burn = 0;
        let far = p.as_vec3() + glam::Vec3::X * (8.0 / scale.meters());
        factory::action(&mut w, far, Action::SetRecipe(p, 0)); assert_eq!(w.factory.nodes[&p].mode, MODE);
        for mode in [0, 1, 2, 3, 4, MODE, MODE + 1] {
            factory::action(&mut w, p.as_vec3(), Action::SetRecipe(p, mode));
            assert_eq!(w.factory.nodes[&p].mode, mode.min(MODE));
            assert_eq!(condensate(&w.factory.nodes[&p]), 123);
        }
        factory::action(&mut w, p.as_vec3(), Action::SetRecipe(p, 3));
        let before = ledger(&w.factory.nodes[&p]);
        factory::action(&mut w, p.as_vec3(), Action::Drain(p)); assert_eq!(ledger(&w.factory.nodes[&p]), before);
        w.inventory.hotbar[0] = Some(ItemStack::new(Item::Bucket(0), 1));
        factory::action(&mut w, far, Action::Drain(p)); assert_eq!(ledger(&w.factory.nodes[&p]), before);
        factory::action(&mut w, p.as_vec3(), Action::Drain(p));
        assert_eq!(condensate(&w.factory.nodes[&p]), 0);
        assert_eq!(w.inventory.hotbar[0].as_ref().unwrap().item, Item::Bucket(123));
    }
}

#[test]
fn fired_boiler_pipe_fill_respects_exact_capacity_and_pause_at_all_scales() {
    for size in [1, 2, 4] { for (kind, mode) in [(Kind::Generator, MODE), (Kind::Generator, 1), (Kind::WetScrubber, 2)] {
        let mut f = Factory::default(); let p = IVec3::ZERO; let q = IVec3::X * 2 * size;
        let mut source = Node::new(Kind::Tank); source.water = 300;
        let mut n = Node::new(kind); n.mode = mode; n.water = WATER_CAPACITY - 2; n.separator_water_mg = 1;
        n.enabled = false;
        f.nodes.insert(p, source); f.nodes.insert(IVec3::X * size, Node::new(Kind::FluidPipe)); f.nodes.insert(q, n);
        f.tick(0.25, size);
        assert_eq!(f.nodes[&q].water, WATER_CAPACITY - 2);
        f.nodes.get_mut(&q).unwrap().enabled = true; extraction::route_water(&mut f);
        assert_eq!((f.nodes[&p].water, f.nodes[&q].water, f.nodes[&q].separator_water_mg), (299, WATER_CAPACITY - 1, 1));
        extraction::route_water(&mut f); assert_eq!(f.nodes[&p].water, 299);
    }}
}

#[test]
fn fired_boiler_tank_duct_turbine_load_and_pollution_line_conserves() {
    for scale in SCALES {
        let mut w = world(scale); let size = factory::step(&w); let p = IVec3::new(-16, 64, -16);
        for (x, kind) in [(-2, Kind::Tank), (-1, Kind::FluidPipe), (0, Kind::Generator), (1, Kind::ExhaustDuct),
            (2, Kind::GasTank), (3, Kind::ExhaustDuct), (4, Kind::GasGenerator), (5, Kind::Cable), (6, Kind::Furnace)] {
            w.factory.nodes.insert(p + IVec3::X * x * size, Node::new(kind));
        }
        let mut chimney = Node::new(Kind::Chimney); chimney.gas_vent_side = 3;
        w.factory.nodes.insert(p + (IVec3::X + IVec3::Z) * size, chimney);
        w.factory.nodes.get_mut(&(p - IVec3::X * 2 * size)).unwrap().water = 128;
        let b = w.factory.nodes.get_mut(&p).unwrap(); b.mode = MODE; b.input[3] = 16;
        b.gas_vent_side = 1; b.gas_vent_species = (STEAM + 1) as u8;
        let duct = w.factory.nodes.get_mut(&(p + IVec3::X * size)).unwrap();
        duct.gas_vent_side = 1; duct.gas_vent_species = (STEAM + 1) as u8;
        w.factory.nodes.get_mut(&(p + IVec3::X * 2 * size)).unwrap().mode = 3;
        let t = p + IVec3::X * 4 * size; w.factory.nodes.get_mut(&t).unwrap().mode = 1;
        let mut state = State::default();
        for _ in 0..160 {
            w.factory.tick(0.25, size);
            for _ in 0..8 { factory::industry::route(&mut w, &mut state); }
        }
        let gas: u64 = w.factory.nodes.values().map(|n| u64::from(n.exhaust[STEAM])).sum();
        let liquid: u64 = w.factory.nodes.values().map(condensate).sum();
        let generated = w.factory.stats.window(false).0.generated;
        assert_eq!(gas + liquid, 128 * u64::from(WATER_UNIT_MG));
        assert!(generated > 0 && generated <= 400, "generation {generated}");
        assert_eq!(condensate(&w.factory.nodes[&t]), generated * 96);
        assert_eq!(generated * 96 + gas, 400 * 96);
        assert_eq!(state.imported[STEAM], 0);
        for (species, expected) in [(SOOT, 400 * 6), (ACID, 400 * 2)] {
            assert_eq!(state.imported[species], expected);
            assert_eq!(w.factory.nodes.values().map(|n| u64::from(n.exhaust[species])).sum::<u64>() + state.imported[species], expected);
        }
        assert!(w.factory.nodes[&(p + IVec3::X * 6 * size)].energy > 0);
        assert_eq!((w.factory.nodes[&p].input[3], w.factory.nodes[&p].burn, w.factory.nodes[&p].energy), (0, 0, 0));
    }
}

#[test]
fn fired_boiler_existing_controller_hold_cannot_be_bypassed_by_mode_switch() {
    let mut w = world(SCALES[0]); let p = IVec3::ZERO;
    let mut n = boiler(); n.mode = 1; n.input[3] = 16; w.factory.nodes.insert(p, n);
    let registry = factory::chips::registry();
    let mut design = engine_chip::presets::sentinel(registry); design.firmware.task = engine_chip::firmware::Task::HeatGuard;
    let lib = std::sync::Arc::make_mut(&mut w.factory.chips.library);
    let project = lib.add_project("Boiler Hold Test", design); let rev = lib.release(registry, project, 0).unwrap();
    let serial = lib.package(rev, engine_chip::fab::Provenance::Prepared).unwrap();
    w.factory.chips.install(&w.factory.nodes, p, serial).unwrap();
    factory::action(&mut w, p.as_vec3(), Action::SetRecipe(p, MODE));
    assert!(!w.factory.chips.allows(p)); let before = ledger(&w.factory.nodes[&p]);
    w.factory.tick(0.25, 1); assert_eq!(ledger(&w.factory.nodes[&p]), before);
    assert_eq!(w.factory.nodes[&p].status, "Controller Hold");
}

#[test]
fn fired_boiler_closed_and_ruptured_outlet_preserve_accounted_water() {
    let mut w = world(SCALES[0]); let p = IVec3::new(-16, 64, -16);
    let mut n = boiler(); n.input[3] = 128; let start = condensate(&n); w.factory.nodes.insert(p, n);
    let q = p + IVec3::X; let mut duct = Node::new(Kind::ExhaustDuct); duct.target = 1 << 1;
    w.factory.nodes.insert(q, duct); let mut state = State::default();
    for _ in 0..100 { w.factory.tick(0.25, 1); factory::industry::route(&mut w, &mut state); }
    let before = ledger(&w.factory.nodes[&p]);
    assert_eq!(w.factory.nodes[&p].status, "Exhaust Full"); assert_eq!(w.factory.nodes[&q].exhaust[STEAM], 0);
    w.factory.tick(0.25, 1); assert_eq!(ledger(&w.factory.nodes[&p]), before);
    let duct = w.factory.nodes.get_mut(&q).unwrap(); duct.target = 0; duct.ruptured = true; duct.gas_vent = true;
    w.factory.nodes.get_mut(&p).unwrap().enabled = false;
    for _ in 0..80 { factory::industry::route(&mut w, &mut state); }
    // Paused source stays closed even when the attached duct leaks.
    assert_eq!(ledger(&w.factory.nodes[&p]), before);
    w.factory.nodes.get_mut(&p).unwrap().enabled = true;
    for _ in 0..80 { factory::industry::route(&mut w, &mut state); }
    assert!(state.imported[STEAM] > 0);
    assert_eq!(w.factory.nodes.values().map(|n| condensate(n) + u64::from(n.exhaust[STEAM])).sum::<u64>()
        + state.imported[STEAM], start);
    assert_eq!(w.factory.nodes[&p].burn, before.3);
}
