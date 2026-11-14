use super::*;
use crate::world::{BlockId, factory, industry::{Mass, MAX_CELLS}};
use engine_core::VoxelScalePreset;
use engine_voxel::WorldType;

const P: IVec3 = IVec3::new(-16, 64, -16);
const SOOT: usize = 1;
const SCALES: [VoxelScalePreset; 3] = [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter];
const DIRECTIONS: [IVec3; 5] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::Z, IVec3::NEG_Z];
fn world(scale: VoxelScalePreset) -> World { World::with_seed(WorldType::Superflat, scale, 7) }
fn node(w: &mut World, p: IVec3, kind: Kind, mass: [u32; SPECIES]) {
    let mut n = Node::new(kind); n.exhaust = mass; w.factory.nodes.insert(p, n);
}
fn stock(w: &World) -> Mass {
    let mut mass = [0; SPECIES];
    for n in w.factory.nodes.values() {
        for i in 0..SPECIES { mass[i] += u64::from(n.exhaust[i]) + u64::from(n.residue[i]); }
        assert!(n.exhaust.iter().sum::<u32>() <= GAS_CAPACITY);
    }
    mass
}
fn accounted(w: &World, s: &State) -> Mass {
    let mut mass = stock(w);
    for i in 0..SPECIES { mass[i] += s.imported[i] - s.withdrawn[i]; }
    mass
}
fn pair(scale: VoxelScalePreset, direction: IVec3, mass: [u32; SPECIES]) -> (World, IVec3) {
    let mut w = world(scale); let q = P + direction * factory::step(&w);
    node(&mut w, P, Kind::ExhaustDuct, mass); node(&mut w, q, Kind::Chimney, [0; SPECIES]); (w, q)
}

#[test]
fn chimney_release_drains_new_single_milligrams_at_all_scales_and_orders() {
    for scale in SCALES { for direction in DIRECTIONS { for species in 0..SPECIES {
        let mut mass = [0; SPECIES]; mass[species] = 1;
        let (mut w, _) = pair(scale, direction, mass); let mut s = State::default();
        route(&mut w, &mut s);
        assert_eq!(s.imported, mass.map(u64::from)); assert_eq!(stock(&w), [0; SPECIES]);
        for _ in 0..4 { route(&mut w, &mut s); }
        assert_eq!(accounted(&w, &s), mass.map(u64::from));
    }}}
}

#[test]
fn chimney_release_drains_resident_auto_residue_within_two_passes() {
    for scale in SCALES { for direction in DIRECTIONS {
        let (mut w, q) = pair(scale, direction, [0; SPECIES]);
        w.factory.nodes.get_mut(&q).unwrap().exhaust = [1; SPECIES];
        let mut s = State::default();
        for _ in 0..2 { route(&mut w, &mut s); assert_eq!(accounted(&w, &s), [1; SPECIES]); }
        assert_eq!(stock(&w), [0; SPECIES]); assert_eq!(s.imported, [1; SPECIES]);
    }}
}

#[test]
fn chimney_release_blocked_or_submerged_top_retains_mass_until_opened() {
    for scale in SCALES { for block in [BlockId::Stone, BlockId::Water, BlockId::Lava] {
        let (mut w, q) = pair(scale, IVec3::X, [1; SPECIES]); let size = factory::step(&w);
        let top = q + IVec3::new(size / 2, size, size / 2); w.set(top, block);
        let mut s = State::default();
        for _ in 0..8 { route(&mut w, &mut s); assert_eq!(accounted(&w, &s), [1; SPECIES]); }
        assert_eq!(s.imported, [0; SPECIES]);
        w.set(top, BlockId::Air);
        for _ in 0..2 { route(&mut w, &mut s); }
        assert_eq!(s.imported, [1; SPECIES]); assert_eq!(stock(&w), [0; SPECIES]);
    }}
}

#[test]
fn chimney_release_paused_and_closed_intake_preserve_stock() {
    for scale in SCALES { for paused in [false, true] {
        let (mut w, q) = pair(scale, IVec3::X, [1; SPECIES]);
        if paused { let n = w.factory.nodes.get_mut(&q).unwrap(); n.enabled = false; n.exhaust = [1; SPECIES]; }
        else { w.factory.nodes.get_mut(&P).unwrap().target = 1; }
        let mut s = State::default();
        for _ in 0..4 { route(&mut w, &mut s); }
        assert_eq!(w.factory.nodes[&P].exhaust, [1; SPECIES]); assert_eq!(s.imported, [0; SPECIES]);
        w.factory.nodes.get_mut(&q).unwrap().enabled = true; w.factory.nodes.get_mut(&P).unwrap().target = 0;
        for _ in 0..2 { route(&mut w, &mut s); }
        assert_eq!(s.imported, [if paused { 2 } else { 1 }; SPECIES]);
    }}
}

#[test]
fn chimney_release_full_atmospheric_budget_never_deletes_rejected_gas() {
    let (mut w, _) = pair(SCALES[0], IVec3::X, [1; SPECIES]); let mut s = State::default();
    for x in 0..MAX_CELLS { s.air.insert((10_000 + x as i32, 64, 0, 1), [1; SPECIES]); }
    for _ in 0..4 { route(&mut w, &mut s); assert_eq!(accounted(&w, &s), [1; SPECIES]); }
    assert_eq!(s.imported, [0; SPECIES]); assert_eq!(s.air.len(), MAX_CELLS);
    s.air.pop_first();
    for _ in 0..2 { route(&mut w, &mut s); }
    assert_eq!(s.imported, [1; SPECIES]); assert_eq!(stock(&w), [0; SPECIES]);
}

#[test]
fn chimney_release_selected_vent_and_rupture_keep_species_and_pause_rules() {
    for scale in SCALES {
        let mut w = world(scale); let mut mass = [0; SPECIES]; mass[STEAM] = 23; mass[SOOT] = 17;
        node(&mut w, P, Kind::GasTank, mass);
        let n = w.factory.nodes.get_mut(&P).unwrap(); n.enabled = false; n.gas_vent = true; n.gas_vent_species = (STEAM + 1) as u8;
        let mut s = State::default(); route(&mut w, &mut s);
        assert_eq!(s.imported[STEAM], 23); assert_eq!(s.imported[SOOT], 0); assert_eq!(accounted(&w, &s), mass.map(u64::from));
        w.factory.nodes.get_mut(&P).unwrap().ruptured = true;
        route(&mut w, &mut s); assert_eq!(stock(&w), [0; SPECIES]); assert_eq!(s.imported, mass.map(u64::from));
    }
}

#[test]
fn chimney_release_selected_component_face_is_not_an_atmospheric_outlet() {
    for scale in SCALES {
        let mut w = world(scale); let q = P + IVec3::X * factory::step(&w);
        node(&mut w, P, Kind::ExhaustDuct, [16; SPECIES]); node(&mut w, q, Kind::GasTank, [0; SPECIES]);
        let n = w.factory.nodes.get_mut(&P).unwrap(); n.gas_vent = true; n.gas_vent_side = 1;
        let mut s = State::default();
        for _ in 0..8 { route(&mut w, &mut s); assert_eq!(accounted(&w, &s), [16; SPECIES]); }
        assert_eq!(s.imported, [0; SPECIES]); assert!(w.factory.nodes[&q].exhaust.iter().all(|&v| v > 0));
    }
}

#[test]
fn chimney_release_disconnected_producer_keeps_adjacent_closed_port_contract() {
    for state in 0..3 {
        let mut w = world(SCALES[0]); node(&mut w, P, Kind::Generator, [1; SPECIES]);
        if state != 0 {
            let q = P + IVec3::X; node(&mut w, q, Kind::ExhaustDuct, [0; SPECIES]);
            let n = w.factory.nodes.get_mut(&q).unwrap();
            if state == 1 { n.enabled = false; } else { n.target = 1 << 1; }
        }
        let mut s = State::default(); route(&mut w, &mut s);
        assert_eq!(s.imported, if state == 0 { [1; SPECIES] } else { [0; SPECIES] });
        assert_eq!(accounted(&w, &s), [1; SPECIES]);
    }
}

#[test]
fn chimney_release_filter_capture_and_fan_reintake_conserve_mass() {
    let mut w = world(SCALES[0]); node(&mut w, P, Kind::DustFilter, [1000; SPECIES]);
    let n = w.factory.nodes.get_mut(&P).unwrap(); n.input[4] = 16; n.gas_vent = true;
    let mut s = State::default();
    for _ in 0..32 { route(&mut w, &mut s); assert_eq!(accounted(&w, &s), [1000; SPECIES]); }
    assert!(w.factory.nodes[&P].residue[SOOT] > 0); assert!(s.imported[SOOT] > 0);
    assert!(s.withdrawn.iter().any(|&mass| mass > 0));
    let mut physical = stock(&w);
    for mass in s.air.values() { for i in 0..SPECIES { physical[i] += mass[i]; } }
    assert_eq!(physical, [1000; SPECIES]);
}

#[test]
fn chimney_release_multi_inlet_capacity_and_roundtrip_keep_exact_residue() {
    let mut w = world(SCALES[0]); let mut mass = [0; SPECIES]; mass[SOOT] = GAS_CAPACITY - 3;
    node(&mut w, P, Kind::Chimney, mass);
    for d in DIRS { node(&mut w, P + d, Kind::ExhaustDuct, [1000; SPECIES]); }
    let initial = stock(&w); let mut s = State::default();
    for _ in 0..8 { route(&mut w, &mut s); assert_eq!(accounted(&w, &s), initial); }
    let mut w = world(SCALES[0]); node(&mut w, P, Kind::Chimney, [1; SPECIES]);
    node(&mut w, P + IVec3::NEG_X, Kind::ExhaustDuct, [0; SPECIES]);
    let mut s = State::default(); route(&mut w, &mut s);
    let mut bytes = Vec::new(); w.factory.encode(&mut bytes);
    let mut words = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    w.factory = Factory::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")), 1, factory::FACTORY_FORMAT).unwrap();
    route(&mut w, &mut s); assert_eq!(accounted(&w, &s), [1; SPECIES]); assert_eq!(stock(&w), [0; SPECIES]);
}

#[test]
#[ignore = "wall-clock routing measurement; run without competing native workloads"]
fn chimney_release_load_is_bounded_and_conservative() {
    use std::time::Instant;
    let _ = &*CONFIG;
    for count in [16, 256, 512] {
        let mut w = world(SCALES[0]); let mut sources = Vec::new();
        for i in 0..count {
            let p = P + IVec3::new((i % 32) * 4, 0, (i / 32) * 4); sources.push(p);
            let mut mass = [0; SPECIES]; mass[SOOT] = 1;
            node(&mut w, p, Kind::ExhaustDuct, mass); node(&mut w, p + IVec3::X, Kind::Chimney, [0; SPECIES]);
        }
        let mut s = State::default(); route(&mut w, &mut s); let mut times = Vec::new();
        for pass in 0..64 {
            for p in &sources { w.factory.nodes.get_mut(p).unwrap().exhaust[SOOT] = 1; }
            let start = Instant::now(); route(&mut w, &mut s); times.push(start.elapsed().as_nanos() as u64);
            assert_eq!(s.imported[SOOT], (pass + 2) * count as u64); assert_eq!(stock(&w), [0; SPECIES]);
        }
        times.sort_unstable();
        eprintln!("[chimney-release-load] nodes={} passes=64 p50_us={:.2} p95_us={:.2} max_us={:.2}",
            count * 2, times[32] as f64 / 1000.0, times[60] as f64 / 1000.0, times[63] as f64 / 1000.0);
    }
}
