use super::*;
use crate::world::{factory::{self, Factory, extraction::WATER_CAPACITY, industry::turbine::condensate}};
use glam::IVec3;
const SOOT: usize = 1;

fn boiler() -> Node { let mut n = Node::new(Kind::Generator); n.mode = MODE; n.water = 1; n }
fn ledger(n: &Node) -> (u64, [u32; SPECIES], [u32; factory::RESOURCE_COUNT], u32, u32, u32) {
    (condensate(n), n.exhaust, n.input, n.burn, n.energy, n.fuel_resource)
}
fn decode(f: &Factory, version: u8) -> anyhow::Result<Factory> {
    let mut bytes = Vec::new(); f.encode_format(&mut bytes, version);
    let mut words = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    Factory::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")), 1, version)
}

#[test]
fn fired_boiler_each_fuel_pays_exact_steam_and_pollution_without_electricity() {
    let b = &CONFIG.fired_boiler;
    assert_eq!(*b, ron::from_str(&ron::to_string(b).unwrap()).unwrap());
    for (id, portion, credit) in COMBUSTION_FUELS {
        let mut n = boiler(); n.input[id] = portion; n.energy = 2000;
        let start = condensate(&n);
        for _ in 0..credit.div_ceil(b.max_fuel_per_tick) { step(&mut n); }
        assert_eq!((n.input[id], n.burn, n.energy, n.flow, n.fuel_resource), (0, 0, 2000, 0, id as u32));
        assert_eq!(start - condensate(&n), u64::from(credit * b.steam_mg_per_energy));
        for (gas, rate) in rates(id as u32).into_iter().enumerate() { assert_eq!(n.exhaust[gas], credit * rate); }
        let before = ledger(&n); step(&mut n); assert_eq!(ledger(&n), before);
        assert_eq!(n.status, "Needs Fuel");
    }
}

#[test]
fn fired_boiler_dry_paused_full_and_partial_water_never_overpay() {
    for case in 0..5 {
        let mut n = boiler(); n.input[3] = 16;
        match case { 0 => n.enabled = false, 1 => n.water = 0,
            2 => { n.water = 0; n.separator_water_mg = 95; },
            3 => n.exhaust[SOOT] = GAS_CAPACITY,
            _ => n.exhaust[SOOT] = GAS_CAPACITY - 103 }
        let before = ledger(&n); step(&mut n); assert_eq!(ledger(&n), before, "case {case}");
    }
    let mut n = boiler(); n.water = 0; n.separator_water_mg = 96 * 3 + 17; n.input[3] = 16;
    step(&mut n);
    assert_eq!((n.burn, n.input[3], condensate(&n), n.exhaust[STEAM]), (397, 0, 17, 288));
    let before = ledger(&n); step(&mut n); assert_eq!(ledger(&n), before);
    n.water = 1; step(&mut n); assert_eq!(n.burn, 365);
}

#[test]
fn fired_boiler_capacity_includes_pollution_and_keeps_last_paid_credit() {
    let mut n = boiler(); n.burn = 2; n.fuel_resource = 20;
    n.exhaust[SOOT] = GAS_CAPACITY - rates(20).iter().sum::<u32>();
    let mass = condensate(&n); step(&mut n);
    assert_eq!((n.burn, n.exhaust.iter().sum::<u32>(), mass - condensate(&n)), (1, GAS_CAPACITY, 96));
    let before = ledger(&n); step(&mut n); assert_eq!(ledger(&n), before);
}

#[test]
fn fired_boiler_factory_step_has_no_direct_power_and_keeps_ordinary_combustion() {
    for mode in [0, MODE] {
        let mut f = Factory::default(); let mut n = boiler(); n.mode = mode; n.input[3] = 16;
        f.nodes.insert(IVec3::ZERO, n); f.tick(0.25, 1);
        let n = &f.nodes[&IVec3::ZERO];
        assert_eq!((n.energy, n.flow, n.burn), if mode == 0 { (16, 16, 384) } else { (0, 0, 368) });
        assert_eq!(n.exhaust[STEAM], if mode == 0 { 32 } else { 3072 });
        f.nodes.get_mut(&IVec3::ZERO).unwrap().enabled = false;
        for _ in 0..3 { f.tick(0.25, 1); }
        assert_eq!(f.stats.window(false).0.generated, if mode == 0 { 16 } else { 0 });
    }
}

#[test]
fn fired_boiler_current_save_preserves_all_modes_and_legacy_combustion() {
    let mut f = Factory::default(); let mut n = boiler(); n.water = 64; n.input[3] = 16;
    n.separator_water_mg = 123; n.energy = 7; n.exhaust[STEAM] = 19;
    for mode in 0..=MODE {
        n.mode = mode; f.nodes.insert(IVec3::ZERO, n.clone());
        let loaded = decode(&f, factory::FACTORY_FORMAT).unwrap();
        assert_eq!(ledger(&loaded.nodes[&IVec3::ZERO]), ledger(&n));
        assert_eq!(loaded.nodes[&IVec3::ZERO].mode, mode);
    }
    n.mode = MODE; n.input[3] = 16; step(&mut n); f.nodes.insert(IVec3::ZERO, n.clone());
    assert_eq!(ledger(&decode(&f, 21).unwrap().nodes[&IVec3::ZERO]), ledger(&n));
    n.mode = 0; n.burn = 0; n.water = 64; n.separator_water_mg = 0; f.nodes.insert(IVec3::ZERO, n.clone());
    for version in [14, 20] { assert_eq!(ledger(&decode(&f, version).unwrap().nodes[&IVec3::ZERO]), ledger(&n)); }
    let n = f.nodes.get_mut(&IVec3::ZERO).unwrap(); n.water = WATER_CAPACITY; n.separator_water_mg = 1;
    assert!(decode(&f, 21).is_err());
    f.nodes.get_mut(&IVec3::ZERO).unwrap().separator_water_mg = WATER_UNIT_MG;
    assert!(decode(&f, 21).is_err());
}

#[test]
#[ignore = "wall-clock conversion measurement; run without concurrent builds"]
fn fired_boiler_conversion_load_is_bounded_and_conservative() {
    use std::time::Instant;
    let _ = &*CONFIG;
    for count in [32, 512, 2048] {
        let mut nodes: Vec<_> = (0..count).map(|_| boiler()).collect();
        let mut times = Vec::new();
        for _ in 0..240 {
            for n in &mut nodes { n.exhaust = [0; SPECIES]; n.burn = 32; }
            let before: u64 = nodes.iter().map(condensate).sum();
            let start = Instant::now(); for n in &mut nodes { step(n); }
            times.push(start.elapsed().as_nanos() as u64);
            assert_eq!(before - nodes.iter().map(condensate).sum::<u64>(), count as u64 * 3072);
            assert!(nodes.iter().all(|n| n.burn == 0 && n.exhaust[STEAM] == 3072 && n.energy == 0));
        }
        times.sort_unstable();
        eprintln!("[fired-boiler-load] machines={count} passes=240 p50_us={:.2} p95_us={:.2} max_us={:.2}",
            times[120] as f64 / 1000.0, times[228] as f64 / 1000.0, times[239] as f64 / 1000.0);
    }
}

#[path = "connected.rs"]
mod connected;

#[test]
fn fired_boiler_exact_backpressure_owns_status_near_capacity() {
    for enabled in [false, true] {
        let mut f = Factory::default(); let mut n = boiler(); n.enabled = enabled;
        n.burn = 32; n.exhaust[SOOT] = GAS_CAPACITY - 104; f.nodes.insert(IVec3::ZERO, n);
        f.tick(0.25, 1); let n = &f.nodes[&IVec3::ZERO];
        assert_eq!(n.status, if enabled { "Making Steam" } else { "Paused" });
        assert_eq!(n.burn, if enabled { 31 } else { 32 });
    }
}
