//! Crate Fill on a processor, Heat Guard on a pile, and brownout hangs, on a
//! bare factory ticked at the world step.
use super::*;
use crate::world::factory::{CAPACITY, pile};
use engine_chip::fab::Provenance;
use engine_chip::firmware::Task;
use engine_chip::presets::{sentinel, twin_watch};
use engine_chip::runtime::Phase;

const DT: f32 = 1.0 / 120.0;

fn part(f: &mut Factory, mut design: Design, task: Task) -> u32 {
    design.firmware.task = task;
    let lib = Arc::make_mut(&mut f.chips.library);
    let project = lib.add_project(&design.name.clone(), design);
    let rev = lib.release(registry(), project, 0).unwrap();
    lib.package(rev, Provenance::Prepared).unwrap()
}

/// Crusher -> item pipe -> crate, the crusher charged every tick.
fn crusher_line() -> (Factory, IVec3, IVec3) {
    let mut f = Factory::default();
    let (crusher, crate_at) = (IVec3::ZERO, IVec3::new(2, 0, 0));
    let mut c = Node::new(Kind::Crusher);
    c.input[0] = 64 * 64;
    f.nodes.insert(crusher, c);
    f.nodes.insert(IVec3::X, Node::new(Kind::Pipe));
    f.nodes.insert(crate_at, Node::new(Kind::Storage));
    (f, crusher, crate_at)
}

fn run(f: &mut Factory, ticks: u32, mut each: impl FnMut(&mut Factory)) {
    for _ in 0..ticks {
        each(f);
        f.tick(DT, 1);
    }
}

#[test]
fn crate_fill_holds_a_crusher_while_its_crate_is_full() {
    let (mut f, crusher, crate_at) = crusher_line();
    let serial = part(&mut f, sentinel(registry()), Task::StockBand);
    f.chips.install(&f.nodes, crusher, serial).unwrap();
    f.nodes.get_mut(&crate_at).unwrap().input[9] = CAPACITY;
    run(&mut f, 120 * 2, |f| f.nodes.get_mut(&crusher).unwrap().energy = 64);
    let d = f.chips.diagnostics(crusher).unwrap();
    assert_eq!(d.sensors, vec![crate_at]);
    assert_eq!(f.nodes[&crusher].status, "Held by controller");
    f.nodes.get_mut(&crate_at).unwrap().input[9] = 0;
    run(&mut f, 120, |f| f.nodes.get_mut(&crusher).unwrap().energy = 64);
    assert_ne!(f.nodes[&crusher].status, "Held by controller", "an empty crate restarts it");
}

#[test]
fn a_tank_part_does_not_fit_a_crusher() {
    let (mut f, crusher, _) = crusher_line();
    let serial = part(&mut f, sentinel(registry()), Task::FillBand);
    assert_eq!(f.chips.install(&f.nodes, crusher, serial), Err("Task Mismatch"));
    assert_eq!(f.chips.library.stock.len(), 1, "the part stays in stock");
}

/// A fuelled pile with no coolant and a load that always drains its buffer.
fn hot_pile() -> (Factory, IVec3) {
    let mut f = Factory::default();
    let p = IVec3::ZERO;
    let mut g = Node::new(Kind::Generator);
    g.mode = 1;
    g.input[pile::FUEL] = 64 * 64;
    f.nodes.insert(p, g);
    (f, p)
}

#[test]
fn heat_guard_keeps_an_uncooled_pile_below_meltdown() {
    let (mut f, p) = hot_pile();
    let mut peak = 0;
    run(&mut f, 120 * 60, |f| {
        let n = f.nodes.get_mut(&p).unwrap();
        n.energy = 0;
        peak = peak.max(n.progress);
    });
    assert!(!f.nodes[&p].enabled, "unguarded: the pile melts down ({peak})");

    let (mut f, p) = hot_pile();
    let serial = part(&mut f, sentinel(registry()), Task::HeatGuard);
    f.chips.install(&f.nodes, p, serial).unwrap();
    let (mut peak, mut held) = (0, 0);
    run(&mut f, 120 * 60, |f| {
        let n = f.nodes.get_mut(&p).unwrap();
        // The load draws the whole buffer; the controller runs on the
        // burning fuel portion.
        n.energy = 0;
        peak = peak.max(n.progress);
        held += u32::from(n.status == "Held by controller");
    });
    let n = &f.nodes[&p];
    assert!(n.enabled, "guarded: no meltdown");
    assert!(peak < pile::WARN_HEAT, "peak heat {peak}");
    assert!(held > 0 && n.input[pile::FUEL] < 64 * 64, "it both ran and held: held {held} fuel {} peak {peak} status {}", n.input[pile::FUEL], n.status);
}

#[test]
fn a_brownout_hangs_sentinel_until_reset_and_twin_watch_recovers() {
    for (design, recovers) in [(sentinel(registry()), false), (twin_watch(registry()), true)] {
        let (mut f, crusher, _) = crusher_line();
        let serial = part(&mut f, design, Task::StockBand);
        f.chips.install(&f.nodes, crusher, serial).unwrap();
        run(&mut f, 120, |f| f.nodes.get_mut(&crusher).unwrap().energy = 64);
        assert_eq!(f.chips.diagnostics(crusher).unwrap().state.phase, Phase::Running);
        run(&mut f, 120, |f| f.nodes.get_mut(&crusher).unwrap().energy = 0);
        run(&mut f, 120, |f| f.nodes.get_mut(&crusher).unwrap().energy = 64);
        let phase = f.chips.diagnostics(crusher).unwrap().state.phase;
        assert_eq!(phase == Phase::Hung, !recovers, "{phase:?}");
        if !recovers {
            assert!(!f.chips.allows(crusher), "a hung part holds its machine");
            f.chips.action(&f.nodes, Action::Reset(crusher), |_| true);
            run(&mut f, 120, |f| f.nodes.get_mut(&crusher).unwrap().energy = 64);
            assert_eq!(f.chips.diagnostics(crusher).unwrap().state.phase, Phase::Running);
        }
    }
}

#[test]
fn a_wafer_lot_is_paid_once_runs_at_game_time_and_survives_a_save() {
    use super::production::{Production, QUANTUM, Supply};
    let mut w = crate::world::World::with_seed(engine_voxel::WorldType::Superflat,
        engine_core::VoxelScalePreset::OneMeter, 7);
    let p = IVec3::new(0, 64, 0); let q = p + IVec3::X * 16;
    for site in [p, q] { w.factory.nodes.insert(site, Node::new(Kind::Fabricator)); }
    let lib = Arc::make_mut(&mut w.factory.chips.library);
    let project = lib.add_project("S", sentinel(registry()));
    let rev = lib.release(registry(), project, 0).unwrap();
    let bill = Production::bill(&lib.lot_bill(registry(), rev, "bench").unwrap());
    assert!(bill.iter().any(|&(supply, n)| supply == Supply::Wafer && n > 0));
    for &(supply, n) in &bill { Arc::make_mut(&mut w.factory.chips.production).stock[supply as usize] += n; }
    chip_action(&mut w, p.as_vec3(), Action::StartLot { revision: rev, lab: "bench".into() });
    assert_eq!(w.expedition.message, "Batch Queued");
    assert!(w.factory.chips.production.stock.iter().all(|&n| n == 0));
    assert_eq!(w.factory.chips.production.lots.len(), 1);
    assert!(w.factory.chips.production.lots[0].reserved > 0);
    chip_action(&mut w, p.as_vec3(), Action::StartLot { revision: rev, lab: "bench".into() });
    assert_eq!(w.expedition.message, "Fabricator Busy");
    chip_action(&mut w, q.as_vec3(), Action::StartLot { revision: rev, lab: "bench".into() });
    assert_eq!(w.expedition.message, "Missing Lab Stock");
    assert_eq!(w.factory.chips.library.lots.len(), 1);

    let f = &mut w.factory;
    run(f, 120, |_| {});
    assert_eq!(f.chips.production.lots[0].clock, 0);
    assert_eq!(f.chips.production.lots[0].credit, 0);
    assert!(f.chips.library.lots[0].outcome.is_none());
    let power = |f: &mut Factory| { f.nodes.get_mut(&p).unwrap().energy = 64; };
    run(f, 37, power);
    assert_eq!(f.chips.production.lots[0].clock, 37);
    assert_eq!(f.chips.production.lots[0].credit, QUANTUM - 37 % QUANTUM);
    let mut bytes = Vec::new(); f.chips.encode(&mut bytes);
    let mut words = bytes.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c));
    let mut g = Factory { chips: Bay::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("eof"))).unwrap(),
        nodes: f.nodes.clone(), accumulator: f.accumulator, topology: true, ..Factory::default() };
    assert_eq!(g.chips.production.lots[0].clock, f.chips.production.lots[0].clock);
    assert_eq!(g.chips.production.lots[0].credit, f.chips.production.lots[0].credit);
    for _ in 0..120 * 600 {
        if f.chips.production.lots.is_empty() && g.chips.production.lots.is_empty() { break; }
        run(f, 1, power); run(&mut g, 1, power);
    }
    let (a, b) = (&f.chips.library, &g.chips.library);
    assert_eq!(a.lots, b.lots); assert_eq!(a.stock, b.stock);
    assert!(f.chips.production.lots.is_empty() && g.chips.production.lots.is_empty());
    assert!(f.chips.production.stock.iter().all(|&n| n == 0));
    let o = a.lots[0].outcome.unwrap();
    assert!(o.good > 0 && a.stock.len() == usize::from(o.good));
    let made = a.stock.len(); run(f, 120, power);
    assert_eq!(f.chips.library.stock.len(), made, "completed lots cannot package twice");
}

#[test]
fn parts_can_be_scrapped_and_bad_designs_are_refused() {
    let mut f = Factory::default();
    let serial = part(&mut f, sentinel(registry()), Task::FillBand);
    assert_eq!(f.chips.action(&f.nodes, Action::Scrap(serial), |_| true), "Part Scrapped");
    assert!(f.chips.library.stock.is_empty());
    let mut bad = sentinel(registry());
    bad.layers = 9;
    assert_eq!(f.chips.action(&f.nodes, Action::NewProject("Bad".into(), bad), |_| true), "Invalid Design");
    assert_eq!(f.chips.library.projects.len(), 1);
}
