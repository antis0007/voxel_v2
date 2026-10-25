use super::tests::uniform;
use super::*;
use engine_core::VoxelScalePreset;

#[test]
fn environment_full_queue_can_retire_unrelated_events() {
    let mut w = uniform(VoxelScalePreset::OneMeter, B::Stone);
    w.environment.fixed_steps = 120;
    assert!(w.environment.schedule((8, 8, 8), 1));
    for x in 1..env::MAX_EVENTS as i32 {
        assert!(w.environment.schedule((x + 8, 99_999, 0), 1));
    }
    let mut sources = Residency::default();
    update(
        &mut w,
        &mut sources,
        IVec3::splat(16),
        &mut HashSet::new(),
        &mut HashSet::new(),
        super::super::Budget::UNBOUNDED,
    );
    // Geometry dependencies need no event slots. Read-only admission must not
    // prevent consuming the first event when all slots are occupied.
    assert!(w.environment.serviced > 0);
    assert!(w.environment.scheduled.len() < env::MAX_EVENTS);
}

#[test]
fn environment_bulk_water_does_not_allocate_substrate_work() {
    let mut w = uniform(VoxelScalePreset::OneMeter, B::Stone);
    let a = IVec3::splat(8);
    let b = a + IVec3::X;
    w.set(a, B::Water);
    w.set(b, B::Air);
    w.environment.due.clear();
    w.environment.scheduled.clear();
    assert!(w.apply_water_pair(a, 4, b, 4, 4));
    assert!(w.environment.scheduled.is_empty());
    assert!(w.environment.hydraulic_work.is_empty());
    assert_eq!(w.water_microlitres(a) + w.water_microlitres(b), 1_000_000_000);
    // Actual receiving substrate activates infiltration and detachment.
    w.set(b - IVec3::Y, B::Dirt);
    w.environment.due.clear();
    w.environment.scheduled.clear();
    assert!(w.apply_water_pair(a, 3, b, 5, 1));
    assert!(w.environment.scheduled.contains_key(&env::key(b - IVec3::Y)));
    assert!(w.environment.scheduled.contains_key(&env::key(b)));
}

#[test]
fn environment_external_wakes_do_not_extend_an_overdue_step() {
    let mut state = State { tick: 1, target_tick: 7, ..Default::default() };
    state.wake(IVec3::ZERO);
    assert_eq!(state.due.first().unwrap().0, 8);
    state.event_tick = Some(2);
    state.wake(IVec3::X);
    assert_eq!(state.scheduled[&env::key(IVec3::X)], 3);
}

#[test]
fn environment_sky_summary_preserves_stacked_openings_and_invalidates_edits() {
    let mut w = uniform(VoxelScalePreset::QuarterMeter, B::Air);
    let p = IVec3::splat(8);
    w.set(p, B::Stone);
    w.set(p + IVec3::Y * 3, B::Leaves);
    let summary = env::sky::Summary::new(w.chunks.get(&IVec3::ZERO).unwrap(), 0);
    assert_eq!(summary.above(p - IVec3::Y), (true, 1));
    assert_eq!(summary.above(p + IVec3::Y), (false, 1));
    assert_eq!(summary.above(p + IVec3::Y * 4), (false, 0));
    assert_eq!(summary.above(p + IVec3::X), (false, 0));
    w.environment.sky.insert((0, 0, 0), summary);
    w.set(p, B::Air);
    assert!(w.environment.sky.is_empty());
}

#[test]
fn environment_last_flow_keeps_suspended_grains_due_until_settled() {
    let mut w = uniform(VoxelScalePreset::OneMeter, B::Stone);
    let p = IVec3::splat(8);
    w.set(p, B::Water);
    w.environment.sediment.insert(env::key(p), 1_000.into());
    w.environment.hydraulic_work.insert(env::key(p), 100);
    let mut blocks = HashSet::new();
    let mut chunks = HashSet::new();
    assert!(sediment::process(&mut w, p, &mut blocks, &mut chunks));
    assert!(w.environment.hydraulic_work.is_empty());
    assert!(sediment::process(&mut w, p, &mut blocks, &mut chunks));
    assert!(w.environment.sediment[&env::key(p)].bed > 0);
    assert_eq!(w.environment.sediment_at(p), 1_000);
}

#[test]
fn environment_waking_dormant_soil_does_not_integrate_absent_time() {
    let mut w = uniform(VoxelScalePreset::OneMeter, B::Stone);
    let p = IVec3::splat(8);
    w.set(p, B::Dirt);
    w.environment.due.clear();
    w.environment.scheduled.clear();
    w.environment.tick = 100;
    w.environment.target_tick = 100;
    w.environment.wake(p);
    assert_eq!(w.environment.patches[&env::key(p)].last_tick, 100);
    assert_eq!(w.environment.scheduled[&env::key(p)], 101);
    // The same active patch can take slow steps. A new edit advances its due
    // time without resetting elapsed eligible time or changing water stock.
    w.environment.complete_event(101, env::key(p));
    w.environment.event_tick = Some(101);
    w.environment.wake_after(p, 8);
    assert_eq!(w.environment.scheduled[&env::key(p)], 109);
    w.environment.event_tick = Some(104);
    w.environment.wake(p);
    assert_eq!(w.environment.scheduled[&env::key(p)], 105);
    assert_eq!(w.environment.patches[&env::key(p)].last_tick, 101);
}

#[test]
fn environment_full_queue_retains_steam_at_its_actual_source() {
    let mut w = uniform(VoxelScalePreset::OneMeter, B::Air);
    let p = IVec3::splat(8);
    w.set(p, B::Steam);
    w.sandbox
        .set(p, crate::simulation::sandbox::ThermalCell { temperature: 200.0, gas: 4, ..Default::default() });
    for x in 0..env::MAX_EVENTS as i32 {
        w.environment.wake(IVec3::new(x + 1_000, 99_999, 0));
    }
    let mut sources = Residency::default();
    super::super::thermal::step(
        &mut sources,
        &mut w,
        p,
        &mut HashSet::new(),
        &mut HashSet::new(),
        &mut HashSet::new(),
    );
    assert_eq!(w.get(p), B::Steam);
    assert_eq!(w.sandbox.cell(p).gas, 4);
    for d in [IVec3::Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
        assert_eq!(w.get(p + d), B::Air);
        assert_eq!(w.sandbox.cell(p + d).gas, 0);
    }
}
