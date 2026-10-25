//! Same-machine contrast of the two presets and the 10k idle-controller
//! profile. Reuses the pump rig from `chips_tests.rs`.
use super::tests::rig;
use super::*;
use engine_chip::fab::Provenance;
use engine_chip::presets::{sentinel, twin_watch};

#[test]
fn ten_thousand_idle_controllers_cost_only_their_wakes() {
    let mut f = Factory::default();
    f.topology = true; // no fluid networks to build
    let lib = Arc::make_mut(&mut f.chips.library);
    let project = lib.add_project("S", sentinel(registry()));
    let rev = lib.release(registry(), project, 0).unwrap();
    for i in 0..10_000 {
        let p = IVec3::new(i % 100, 0, i / 100) * 2;
        let mut n = Node::new(Kind::Pump);
        n.energy = 64;
        f.nodes.insert(p, n);
        let serial = Arc::make_mut(&mut f.chips.library).package(rev, Provenance::Prepared).unwrap();
        f.chips.install(&f.nodes, p, serial).unwrap();
    }
    let start = std::time::Instant::now();
    f.step_controllers(1);
    let first = start.elapsed();
    // Within one 30-tick wake period every controller is serviced about
    // once (boot, then periodic wakes); idle ticks only peek the queue.
    let start = std::time::Instant::now();
    let mut idle_ticks = 0;
    for _ in 0..120 {
        let before = f.chips.queue.peek().map(|r| r.0.0);
        f.step_controllers(1);
        if before.is_some_and(|d| d > f.chips.now) {
            idle_ticks += 1;
        }
    }
    let second = start.elapsed();
    eprintln!("10k sentinels: boot tick {first:?}, next 120 ticks {second:?}, {idle_ticks} idle ticks");
    assert!(idle_ticks > 100, "{idle_ticks}");
    assert_eq!(f.chips.installed_count(), 10_000);
}

/// Same rig, same outflow, each preset in turn: what the machine does. A
/// light draw (60 units/s per tank) is within the pump's delivery; a heavy
/// one (120 units/s per tank) is not, and no controller changes that.
#[test]
fn contrast_two_designs_in_the_same_machine() {
    let mut rows = Vec::new();
    for (draw, every) in [("light", 2), ("heavy", 1)] {
        for design in [sentinel(registry()), twin_watch(registry())] {
            let name = design.name.clone();
            let mut r = rig();
            r.drain_every = every;
            let serial = r.part(design);
            r.install(serial);
            let (mut held, mut low1) = (0u32, u32::MAX);
            let ticks = 120 * 120;
            r.run(ticks, 1, |r| {
                held += u32::from(r.holding());
                if r.ticks > 120 * 60 {
                    low1 = low1.min(r.level(1));
                }
            });
            let d = r.s.world.factory.chips.diagnostics(r.pump).unwrap();
            let model = d.model.unwrap();
            let joules = (d.state.wakes * model.wake_nj + model.idle_nw * u64::from(ticks) / 120) as f64 * 1e-9;
            let pm = |v: u32| v * 1000 / WATER_CAPACITY;
            eprintln!(
                "{draw} {name}: tank0 {} tank1 {} permille, tank1 min (last 60 s) {}, pump held {:.0}% of ticks, {} wakes, {:.2} J in 120 s",
                pm(r.level(0)), pm(r.level(1)), pm(low1), held as f32 / ticks as f32 * 100.0, d.state.wakes, joules
            );
            rows.push((low1, held, d.state.wakes, joules));
        }
    }
    let (a, b) = (rows[0], rows[1]);
    assert!(b.0 >= WATER_CAPACITY * 3 / 10 && a.0 < WATER_CAPACITY / 10, "light draw: only the two-sensor part keeps tank 1 in band ({} vs {})", b.0, a.0);
    assert!(b.2 > 5 * a.2 && b.3 > a.3, "faster wakes cost more energy");
    let (c, e) = (rows[2], rows[3]);
    assert!(e.1 < c.1, "heavy draw: the two-sensor part keeps the pump running");
    assert!(e.0 < WATER_CAPACITY / 10, "and still cannot beat the pump's delivery rate");
}

