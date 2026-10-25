//! Side-by-side characterisation of the two preset controllers, plus router
//! and compile timings. `cargo run --release -p engine_chip --example chip_report`
use engine_chip::compile::compile;
use engine_chip::presets::{sentinel, twin_watch};
use engine_chip::registry::Registry;
use engine_chip::route::{route_all, route_dirty};
use engine_chip::runtime::State;
use std::time::Instant;

fn main() {
    let registry = Registry::builtin();
    let designs = [sentinel(&registry), twin_watch(&registry)];
    for d in &designs {
        let c = compile(d, &registry);
        let Some(r) = c.report else {
            println!("{}: failed {:?}", d.name, c.diagnostics);
            continue;
        };
        println!("== {} ({}x{} sites, {} layers)", d.name, d.width, d.height, d.layers);
        println!("  die {:.2} mm², modules {} of {} sites, wire {} sites, {} vias", r.die_mm2, r.used_sites, r.die_sites, r.wire_sites, r.vias);
        println!("  critical {:.0} ns ({}), safe clock {:.2} MHz, running {:.2} MHz", r.critical_ns, r.critical_path, r.max_clock_hz / 1e6, r.clock_hz / 1e6);
        println!("  {} cycles/wake, {:.0} instr/s, handler {:.1} µs, busy {:.1} µs, capacity {:.0} wakes/s", r.cycles_per_wake, r.instructions_per_s, r.handler_us, r.busy_us, r.max_wake_hz);
        println!("  sensors {}, outputs {}, ROM {} B, RAM {} B, sleeps {}", r.channels, r.outputs, r.rom_bytes, r.ram_bytes, r.sleeps);
        println!("  wake every {:.0} ms, worst response {:.1} ms", r.wake_ms, r.worst_response_ms);
        println!("  idle {:.2} mW, {:.2} µJ/wake, average {:.2} mW", r.idle_w * 1e3, r.wake_uj, r.average_w * 1e3);
        println!("  model {:?}", r.model);
        for w in c.diagnostics {
            println!("  {:?}: {}", w.severity, w.message);
        }
    }
    let mut d = designs[1].clone();
    let n = 200;
    let t = Instant::now();
    for _ in 0..n {
        route_all(&mut d, &registry);
    }
    let route_us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    let t = Instant::now();
    for _ in 0..n {
        std::hint::black_box(compile(&d, &registry));
    }
    let compile_us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!("Twin Watch full reroute {route_us:.0} µs, compile {compile_us:.0} µs (mean of {n})");

    // Incremental: move the SRAM one site and route only the nets it left stale.
    let sram = d.parts.iter().find(|p| p.module == "sram64").map(|p| p.id).unwrap_or(0);
    let t = Instant::now();
    let mut stale = 0;
    for i in 0..n {
        let mut e = d.clone();
        stale = e.relocate(&registry, sram, 10, 6 + (i % 2) as u8, 0).map_or(0, |s| s.len());
        route_dirty(&mut e, &registry);
        std::hint::black_box(&e);
    }
    let dirty_us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!("move SRAM + reroute {stale} dirty nets {dirty_us:.0} µs (includes a design clone)");

    // Runtime: 10k parts sharing one frozen model, serviced at every wake for 10 s.
    let Some(model) = compile(&designs[0], &registry).report.map(|r| r.model) else { return };
    let mut states: Vec<State> = (0..10_000).map(|_| State::new(0)).collect();
    let t = Instant::now();
    let mut services = 0u64;
    for tick in 0..1200u64 {
        for s in states.iter_mut().filter(|s| s.due() <= tick) {
            s.service(&model, tick, true, |_| 500);
            services += 1;
        }
    }
    let ns = t.elapsed().as_secs_f64() * 1e9;
    println!("10k Sentinel states, 1200 ticks: {services} services, {:.0} ns/service incl. due scan", ns / services as f64);

    // Fabrication: mask set on the first lot, then 64 lots run to the end.
    let mut lib = engine_chip::library::Library::default();
    let project = lib.add_project("T", designs[1].clone());
    let Ok(rev) = lib.release(&registry, project, 0) else { return };
    let t = Instant::now();
    for i in 0..64 {
        let _ = lib.start_lot(&registry, rev, if i % 2 == 0 { "bench" } else { "clean" }, 0);
    }
    let start_us = t.elapsed().as_secs_f64() * 1e6;
    let t = Instant::now();
    let mut made = 0;
    for tick in (0..120 * 400).step_by(12) {
        made += lib.advance_lots(&registry, tick);
    }
    let run_ms = t.elapsed().as_secs_f64() * 1e3;
    let good: u32 = lib.lots.iter().filter_map(|l| l.outcome).map(|o| u32::from(o.good)).sum();
    println!("64 lots: start {start_us:.0} µs (1 mask set), run {run_ms:.1} ms over 4000 services, {made} parts ({good} good dies)");
}
