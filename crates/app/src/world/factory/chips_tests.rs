//! End-to-end controller tests on a real headless session: a pump over a
//! water pool feeds two tanks through fluid pipes, and released chips are
//! installed in that pump. The tank outflow is a test fixture standing in
//! for a consumer.
use super::*;
use engine_chip::fab::Provenance;
use crate::headless::Session;
use crate::world::BlockId;
use engine_chip::presets::{sentinel, twin_watch};
use engine_core::scale::VoxelScalePreset;
use engine_voxel::WorldType;

pub(super) struct Rig {
    pub(super) s: Session,
    pub(super) pump: IVec3,
    /// Channel order: position order of the tanks.
    pub(super) tanks: [IVec3; 2],
    /// The fixture outflow runs on every n-th tick.
    pub(super) drain_every: u32,
    pub(super) ticks: u32,
}

pub(super) fn rig() -> Rig {
    let mut s = Session::new(WorldType::Superflat, VoxelScalePreset::OneMeter, 7, None);
    let at = s.player.pos.floor().as_ivec3() + IVec3::new(6, 0, 6);
    let mut ground = at.y + 8;
    while ground > at.y - 32 && s.world.get(IVec3::new(at.x, ground, at.z)) == BlockId::Air {
        ground -= 1;
    }
    let mut pool = Vec::new();
    for y in ground - 1..=ground {
        for z in -2..=4 {
            for x in -2..=4 {
                pool.push((IVec3::new(at.x + x, y, at.z + z), BlockId::Water));
            }
        }
    }
    s.world.set_many(&pool);
    let up = ground + 1;
    let c = |x: i32, z: i32| IVec3::new(at.x + x, up, at.z + z);
    let w = &mut s.world;
    crate::world::factory::fixture_place(w, c(0, 0), Kind::Pump);
    crate::world::factory::fixture_place(w, c(1, 0), Kind::FluidPipe);
    crate::world::factory::fixture_place(w, c(1, 1), Kind::FluidPipe);
    crate::world::factory::fixture_place(w, c(2, 0), Kind::Tank);
    crate::world::factory::fixture_place(w, c(1, 2), Kind::Tank);
    Rig { s, pump: c(0, 0), tanks: [c(1, 2), c(2, 0)], drain_every: 1, ticks: 0 }
}

impl Rig {
    pub(super) fn chips(&mut self) -> &mut Bay {
        &mut self.s.world.factory.chips
    }
    /// Release a design and build one prepared part without a bench.
    pub(super) fn part(&mut self, design: Design) -> u32 {
        let bay = self.chips();
        let lib = Arc::make_mut(&mut bay.library);
        let project = lib.add_project(&design.name.clone(), design);
        let rev = lib.release(registry(), project, 0).unwrap();
        lib.package(rev, Provenance::Prepared).unwrap()
    }
    pub(super) fn install(&mut self, serial: u32) {
        let (pump, f) = (self.pump, &mut self.s.world.factory);
        f.chips.install(&f.nodes, pump, serial).unwrap();
    }
    pub(super) fn level(&self, i: usize) -> u32 {
        self.s.world.factory.nodes[&self.tanks[i]].water
    }
    /// Advance with the pump's buffer kept charged (a stand-in grid) and the
    /// tanks drained at `outflow` units per tick each.
    pub(super) fn run(&mut self, ticks: u32, outflow: u32, mut each: impl FnMut(&Self)) {
        for _ in 0..ticks {
            if let Some(n) = self.s.world.factory.nodes.get_mut(&self.pump) {
                n.energy = 64;
            }
            self.ticks += 1;
            for t in self.tanks.into_iter().filter(|_| self.ticks.is_multiple_of(self.drain_every)) {
                let n = self.s.world.factory.nodes.get_mut(&t).unwrap();
                n.water = n.water.saturating_sub(outflow);
            }
            self.s.tick();
            each(self);
        }
    }
    pub(super) fn holding(&self) -> bool {
        !self.s.world.factory.chips.allows(self.pump)
    }
}

#[test]
fn without_a_controller_the_pump_runs_until_its_buffer_is_full() {
    let mut r = rig();
    r.run(600, 0, |_| {});
    assert!(r.level(0) > 0, "the pump feeds the first tank");
    assert!(!r.holding());
}

#[test]
fn sentinel_holds_one_tank_in_its_band_and_ignores_the_second() {
    let mut r = rig();
    let serial = r.part(sentinel(registry()));
    r.install(serial);
    let mut held = 0;
    let mut peak = 0;
    r.run(120 * 40, 0, |r| {
        held += u32::from(r.holding());
        peak = peak.max(r.level(0));
    });
    let upper = WATER_CAPACITY * 9 / 10;
    assert!(held > 0, "the controller turned the pump off");
    assert!(r.level(0) >= upper, "channel 0 filled to the band: {}", r.level(0));
    assert!(r.level(1) < upper / 2, "channel 1 is not watched by a one-ADC part: {}", r.level(1));
    let d = r.s.world.factory.chips.diagnostics(r.pump).unwrap();
    assert_eq!(d.sensors, r.tanks.to_vec());
    assert!(d.state.wakes > 0 && d.state.readings[1] == OPEN);
    // Draining the watched tank below the lower threshold restarts the pump.
    r.s.world.factory.nodes.get_mut(&r.tanks[0]).unwrap().water = WATER_CAPACITY / 5;
    let mut restarted = None;
    let mut t = 0;
    r.run(120, 0, |r| {
        t += 1;
        if restarted.is_none() && !r.holding() {
            restarted = Some(t);
        }
    });
    let wake = r.s.world.factory.chips.library.model(1).unwrap().wake_ticks;
    assert!(restarted.is_some_and(|t| t <= wake + 2), "{restarted:?} within one wake of {wake} ticks");
    let _ = peak;
}

#[test]
fn twin_watch_fills_both_tanks_and_answers_faster() {
    let mut r = rig();
    let serial = r.part(twin_watch(registry()));
    r.install(serial);
    r.run(120 * 60, 0, |_| {});
    let upper = WATER_CAPACITY * 9 / 10;
    assert!(r.level(0) >= upper && r.level(1) >= upper, "{} {}", r.level(0), r.level(1));
    assert!(r.holding());
    r.s.world.factory.nodes.get_mut(&r.tanks[1]).unwrap().water = WATER_CAPACITY / 5;
    let mut t = 0;
    let mut restarted = None;
    r.run(60, 0, |r| {
        t += 1;
        if restarted.is_none() && !r.holding() {
            restarted = Some(t);
        }
    });
    let model = *r.s.world.factory.chips.library.model(1).unwrap();
    assert!(restarted.is_some_and(|t| t <= model.wake_ticks + 2), "{restarted:?}");
}

#[test]
fn swapping_parts_moves_packages_and_never_duplicates_them() {
    let mut r = rig();
    let a = r.part(sentinel(registry()));
    let b = r.part(twin_watch(registry()));
    r.install(a);
    assert_eq!(r.chips().library.stock.len(), 1);
    let (pump, f) = (r.pump, &mut r.s.world.factory);
    assert_eq!(f.chips.install(&f.nodes, pump, b), Err("Socket Occupied"));
    let pump = r.pump;
    assert!(r.chips().remove(pump));
    assert!(!r.chips().remove(pump));
    r.install(b);
    let stock: Vec<u32> = r.chips().library.stock.iter().map(|p| p.serial).collect();
    assert_eq!(stock, vec![a]);
    assert_eq!(r.chips().installed_count(), 1);
    // A machine that disappears returns its part to stock.
    r.s.world.factory.nodes.remove(&r.pump);
    r.run(5, 0, |_| {});
    assert_eq!(r.chips().installed_count(), 0);
    assert_eq!(r.chips().library.stock.len(), 2);
}

#[test]
fn brownout_holds_the_pump_and_power_is_paid_once() {
    let mut r = rig();
    let serial = r.part(twin_watch(registry()));
    r.install(serial);
    r.run(240, 0, |_| {});
    assert!(!r.holding(), "empty tanks: pumping");
    let pump = r.pump;
    let wake = r.s.world.factory.chips.library.model(1).unwrap().wake_ticks;
    for _ in 0..wake * 2 {
        r.s.world.factory.nodes.get_mut(&pump).unwrap().energy = 0;
        r.s.tick();
    }
    let d = r.s.world.factory.chips.diagnostics(pump).unwrap();
    assert_eq!(d.state.phase, engine_chip::runtime::Phase::Unpowered);
    assert!(r.holding(), "no supply: safe state");
    // A long powered run: total paid units plus outstanding debt equal the
    // model's energy; nothing is debited twice.
    let mut r = rig();
    let serial = r.part(twin_watch(registry()));
    r.install(serial);
    let mut charged = 0u64;
    r.run(1, 0, |_| {});
    r.s.world.factory.topology = true;
    let hours: u32 = 120 * 600;
    for _ in 0..hours {
        let n = r.s.world.factory.nodes.get_mut(&r.pump).unwrap();
        n.energy = 64;
        r.s.world.factory.step_controllers(1);
        charged += u64::from(64 - r.s.world.factory.nodes[&r.pump].energy);
    }
    let d = r.s.world.factory.chips.diagnostics(r.pump).unwrap();
    let model = d.model.unwrap();
    let expected = model.idle_nw * u64::from(hours) / 120 + d.state.wakes * model.wake_nj;
    let drawn = charged * UNIT_NJ + d.state.debt_nj;
    assert!(drawn.abs_diff(expected) < expected / 100 + 1_000_000, "{drawn} vs {expected}");
}

#[test]
fn a_prototype_without_materials_mints_nothing() {
    let mut f = Factory::default();
    f.chips.action(&f.nodes, Action::NewProject("S".into(), sentinel(registry())), |_| true);
    let design = f.chips.library.project(1).unwrap().draft.clone();
    f.chips.action(&f.nodes, Action::Release { project: 1, design, generation: 0 }, |_| true);
    let mut bill = Vec::new();
    let said = f.chips.action(&f.nodes, Action::Prototype(1), |cost| {
        bill = cost.to_vec();
        false
    });
    assert_eq!(said, "Missing Materials");
    assert!(f.chips.library.stock.is_empty());
    assert_eq!(bill, vec![BLANK, (COPPER, f.chips.library.bill(1).unwrap() * COPPER_PER_BILL)]);
    // A stale editor generation cannot release.
    let design = f.chips.library.project(1).unwrap().draft.clone();
    let said = f.chips.action(&f.nodes, Action::Release { project: 1, design, generation: 0 }, |_| true);
    assert_eq!(said, "Draft Changed: Reopen");
}

#[test]
fn controller_energy_is_debited_from_the_machine_in_whole_units() {
    let mut r = rig();
    let serial = r.part(sentinel(registry()));
    r.install(serial);
    let key = (r.pump.x, r.pump.y, r.pump.z);
    r.run(10, 0, |_| {});
    let f = &mut r.s.world.factory;
    f.topology = true;
    f.chips.installed.get_mut(&key).unwrap().state.debt_nj += 3 * UNIT_NJ + UNIT_NJ / 2;
    f.nodes.get_mut(&r.pump).unwrap().energy = 2;
    let before = f.chips.installed[&key].state.debt_nj;
    for _ in 0..40 {
        f.step_controllers(1);
    }
    let after = f.chips.installed[&key].state.debt_nj;
    assert_eq!(f.nodes[&r.pump].energy, 0, "two units taken");
    assert!(after >= before - 2 * UNIT_NJ && after < before, "{before} -> {after}");
    // The empty buffer now browns the part out: machine held.
    for _ in 0..40 {
        f.step_controllers(1);
    }
    assert_eq!(f.chips.installed[&key].state.phase, engine_chip::runtime::Phase::Unpowered);
    assert!(!f.chips.allows(r.pump));
}
