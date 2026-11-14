use super::*;
use crate::world::{factory::{self, Factory}, industry::{FUEL, OXYGEN}, inventory::{Item, ItemStack}};
use glam::IVec3;

fn turbine(steam: u32) -> Node {
    let mut n = Node::new(Kind::GasGenerator);
    n.mode = 1;
    n.exhaust[STEAM] = steam;
    n
}
fn step(n: Node) -> Node {
    let mut f = Factory::default();
    f.nodes.insert(IVec3::ZERO, n);
    super::super::burn_gas(&mut f);
    f.nodes.remove(&IVec3::ZERO).unwrap()
}
fn ledger(n: &Node) -> ([u32; crate::world::industry::SPECIES], u64, u32) {
    (n.exhaust, condensate(n), n.energy)
}
fn decode(f: &Factory, version: u8) -> anyhow::Result<Factory> {
    let mut bytes = Vec::new();
    f.encode_format(&mut bytes, version);
    let mut words = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    Factory::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")), 1, version)
}

#[test]
fn turbine_one_boiler_batch_is_lossy_and_mass_exact() {
    let n = step(turbine(1152));
    assert_eq!((n.energy, n.flow, n.exhaust[STEAM], condensate(&n)), (12, 12, 0, 1152));
    assert_eq!(n.energy * 2, super::super::PROCESS_POWER);
    let t = &CONFIG.steam_turbine;
    let ron = ron::to_string(t).unwrap();
    assert_eq!(*t, ron::from_str(&ron).unwrap());
}

#[test]
fn turbine_retains_other_gases_and_subquantum_steam() {
    let mut n = turbine(1152 + 95);
    n.exhaust[FUEL] = 128;
    n.exhaust[OXYGEN] = 500;
    let n = step(n);
    assert_eq!((n.energy, n.exhaust[STEAM], n.exhaust[FUEL], n.exhaust[OXYGEN]), (12, 95, 128, 500));
    assert_eq!(condensate(&n), 1152);
    assert_eq!(ledger(&n), ledger(&step(n.clone())));
}

#[test]
fn turbine_full_gas_buffer_can_generate_at_bounded_rate() {
    let mut f = Factory::default();
    f.nodes.insert(IVec3::ZERO, turbine(super::super::GAS_CAPACITY));
    f.tick(0.25, 1);
    let n = &f.nodes[&IVec3::ZERO];
    assert_eq!((n.energy, n.flow, n.status), (16, 16, "Generating"));
    assert_eq!(condensate(n) + u64::from(n.exhaust[STEAM]), u64::from(super::super::GAS_CAPACITY));
}

#[test]
fn turbine_pause_rupture_and_full_buffers_never_consume() {
    for case in 0..4 {
        let mut n = turbine(1152);
        match case { 0 => n.enabled = false, 1 => n.ruptured = true,
            2 => n.energy = GENERATOR_BUFFER, _ => n.water = WATER_CAPACITY }
        assert_eq!(ledger(&n), ledger(&step(n.clone())), "case {case}");
    }
}

#[test]
fn turbine_rollover_and_capacity_use_exact_wide_arithmetic() {
    let mut n = turbine(1152);
    n.separator_water_mg = WATER_UNIT_MG - 100;
    let n = step(n);
    assert_eq!((n.water, n.separator_water_mg), (1, 1052));
    let mut n = turbine(1152);
    n.water = WATER_CAPACITY - 1;
    n.separator_water_mg = WATER_UNIT_MG - 200;
    let n = step(n);
    assert_eq!((n.energy, n.exhaust[STEAM], n.water, n.separator_water_mg), (2, 960, WATER_CAPACITY - 1, WATER_UNIT_MG - 8));
    assert_eq!(ledger(&n), ledger(&step(n.clone())));
}

#[test]
fn turbine_bottling_is_atomic_and_preserves_overflow() {
    let mut n = turbine(0);
    store(&mut n, 2 * u64::from(water_tools::CAPACITY_UL) + 123);
    let mut inv = SurvivalInventory::default();
    let before = condensate(&n);
    assert!(bottle(&mut n, &mut inv).is_err());
    assert_eq!(condensate(&n), before);
    for amount in [water_tools::CAPACITY_UL, water_tools::CAPACITY_UL, 123] {
        inv.hotbar[0] = Some(ItemStack::new(Item::Bucket(0), 1));
        let before = condensate(&n);
        bottle(&mut n, &mut inv).unwrap();
        assert_eq!(inv.hotbar[0].as_ref().unwrap().item, Item::Bucket(amount));
        assert_eq!(condensate(&n) + u64::from(amount), before);
    }
    assert_eq!(condensate(&n), 0);
}

#[test]
fn turbine_save_and_legacy_combustion_keep_stocks() {
    for mode in [0, 1] {
        let mut f = Factory::default();
        let mut n = turbine(777);
        n.mode = mode; n.water = 63; n.separator_water_mg = 195; n.energy = 17;
        f.nodes.insert(IVec3::ZERO, n.clone());
        let loaded = decode(&f, factory::FACTORY_FORMAT).unwrap();
        assert_eq!(ledger(&loaded.nodes[&IVec3::ZERO]), ledger(&n));
        assert_eq!(loaded.nodes[&IVec3::ZERO].mode, mode);
    }
    let mut f = Factory::default();
    let mut n = turbine(16); n.mode = 0; n.exhaust[FUEL] = 128;
    f.nodes.insert(IVec3::ZERO, n.clone());
    for version in [14, 20, 21] {
        let loaded = decode(&f, version).unwrap();
        assert_eq!(ledger(&loaded.nodes[&IVec3::ZERO]), ledger(&n));
        let burned = step(loaded.nodes[&IVec3::ZERO].clone());
        assert_eq!((burned.energy, burned.exhaust[FUEL], burned.exhaust[STEAM]), (16, 0, 32));
    }
    f.nodes.get_mut(&IVec3::ZERO).unwrap().separator_water_mg = WATER_UNIT_MG;
    assert!(decode(&f, factory::FACTORY_FORMAT).is_err());
    let n = f.nodes.get_mut(&IVec3::ZERO).unwrap();
    n.water = WATER_CAPACITY; n.separator_water_mg = 1;
    assert!(decode(&f, factory::FACTORY_FORMAT).is_err());
}

#[test]
fn turbine_generation_statistics_count_once() {
    let mut f = Factory::default();
    for x in 0..3 { f.nodes.insert(IVec3::new(x * 4, 0, 0), turbine(1152)); }
    for _ in 0..4 { f.tick(0.25, 1); }
    assert_eq!(f.stats.window(false).0.generated, 36);
}

#[path = "connected.rs"]
mod connected;

#[test]
#[ignore = "wall-clock conversion measurement; run without concurrent builds"]
fn turbine_generation_load_is_bounded_and_conservative() {
    use std::time::Instant;
    for count in [32, 512, 2048] {
        let mut f = Factory::default();
        for x in 0..count { f.nodes.insert(IVec3::new(x, 0, 0), turbine(0)); }
        let mut times = Vec::new();
        for _ in 0..240 {
            for n in f.nodes.values_mut() { n.energy = 0; n.exhaust[STEAM] = 1536; }
            let before: u64 = f.nodes.values().map(condensate).sum();
            let start = Instant::now();
            super::super::burn_gas(&mut f);
            times.push(start.elapsed().as_nanos() as u64);
            let after: u64 = f.nodes.values().map(condensate).sum();
            assert_eq!(after - before, count as u64 * 1536);
            assert!(f.nodes.values().all(|n| n.energy == 16 && n.exhaust[STEAM] == 0));
        }
        times.sort_unstable();
        eprintln!("[turbine-load] machines={count} passes=240 p50_us={:.2} p95_us={:.2} max_us={:.2}",
            times[120] as f64 / 1000.0, times[228] as f64 / 1000.0, times[239] as f64 / 1000.0);
    }
}
