use crate::compile::{Fix, Severity, compile};
use crate::design::{Design, EditError, Pin, Site};
use crate::extract::extract;
use crate::library::fingerprint;
use crate::presets::{sentinel, twin_watch};
use crate::registry::{Registry, Role};
use crate::route::{route_all, route_dirty, route_nets};

fn reg() -> Registry {
    Registry::builtin()
}

fn part_of(d: &Design, module: &str) -> u16 {
    d.parts.iter().find(|p| p.module == module).map(|p| p.id).unwrap()
}

fn has(d: &Design, r: &Registry, text: &str) -> bool {
    compile(d, r).errors().any(|e| e.message.contains(text))
}

#[test]
fn both_presets_compile_and_differ_in_capability_and_cost() {
    let r = reg();
    let a = compile(&sentinel(&r), &r);
    let b = compile(&twin_watch(&r), &r);
    assert!(a.ok(), "{:?}", a.diagnostics);
    assert!(b.ok(), "{:?}", b.diagnostics);
    let (a, b) = (a.report.unwrap(), b.report.unwrap());
    assert_eq!((a.channels, b.channels), (1, 2));
    assert!(b.worst_response_ms * 4.0 < a.worst_response_ms, "{} vs {}", b.worst_response_ms, a.worst_response_ms);
    assert!(b.idle_w > a.idle_w && b.average_w > a.average_w);
    assert!(b.die_sites > a.die_sites && b.used_sites > a.used_sites);
    assert!(b.cycles_per_wake < 2 * a.cycles_per_wake, "the ALU keeps two channels cheap");
    assert!(a.sleeps && b.sleeps);
    assert!(a.clock_hz <= 4.0e6 && a.clock_hz > 1.0e6);
}

#[test]
fn placement_rejects_overlap_and_leaving_the_die() {
    let r = reg();
    let mut d = Design::new("t", "p8um", 8, 8, 1);
    let core = d.place(&r, "core8", 0, 0, 0).unwrap();
    assert_eq!(d.place(&r, "rom256", 2, 2, 0), Err(EditError::Overlaps(core)));
    assert_eq!(d.place(&r, "rom256", 6, 0, 0), Err(EditError::OutsideDie));
    assert_eq!(d.place(&r, "nonsense", 5, 5, 0), Err(EditError::UnknownModule));
    let alu = d.place(&r, "alu8", 4, 0, 1).unwrap();
    assert_eq!(d.rect(&r, d.part(alu).unwrap()), Some((4, 0, 3, 2)), "rotation swaps the footprint");
    assert!(d.relocate(&r, alu, 2, 0, 1).is_err());
}

#[test]
fn ports_are_typed_and_need_one_driver() {
    let r = reg();
    let mut d = sentinel(&r);
    let (osc, rom) = (part_of(&d, "rc_osc"), part_of(&d, "rom256"));
    d.connect(&r, Pin { part: osc, port: "clk".into() }, Pin { part: rom, port: "vdd".into() }).unwrap();
    route_dirty(&mut d, &r);
    let c = compile(&d, &r);
    assert!(c.errors().any(|e| e.message.contains("Type Mismatch")), "{:?}", c.diagnostics);
    assert!(!c.ok());
}

#[test]
fn unpowered_module_fails_with_a_connect_fix() {
    let r = reg();
    let mut d = sentinel(&r);
    let rom = part_of(&d, "rom256");
    d.disconnect(&Pin { part: rom, port: "vdd".into() });
    route_dirty(&mut d, &r);
    let c = compile(&d, &r);
    let e = c.errors().find(|e| e.message.contains("Unpowered")).unwrap();
    assert_eq!(e.fix, Some(Fix::Connect { part: rom, port: "vdd".into() }));
}

#[test]
fn no_program_memory_means_no_execution() {
    let r = reg();
    let mut d = sentinel(&r);
    let rom = part_of(&d, "rom256");
    d.remove(&r, rom).unwrap();
    route_dirty(&mut d, &r);
    let c = compile(&d, &r);
    let e = c.errors().find(|e| e.message.contains("No Program ROM")).unwrap();
    assert_eq!(e.fix, Some(Fix::Add(Role::Rom)));
    assert!(c.report.is_none());
}

#[test]
fn crossing_on_different_layers_is_not_a_connection_but_same_layer_shorts() {
    let r = reg();
    let mut d = Design::new("x", "p8um", 10, 10, 2);
    let a = d.place(&r, "timer8", 0, 4, 0).unwrap(); // vdd (0,4), bus (1,4)
    let b = d.place(&r, "timer8", 8, 4, 0).unwrap(); // vdd (8,4), bus (9,4)
    let c = d.place(&r, "timer8", 4, 0, 1).unwrap(); // rotated: vdd (4,0), bus (4,1)
    let e = d.place(&r, "timer8", 4, 8, 1).unwrap(); // vdd (4,8), bus (4,9)
    let horizontal = d.connect(&r, Pin { part: a, port: "bus".into() }, Pin { part: b, port: "vdd".into() }).unwrap();
    let vertical = d.connect(&r, Pin { part: c, port: "bus".into() }, Pin { part: e, port: "vdd".into() }).unwrap();
    let h0: Vec<Site> = (1..=8).map(|x| Site::new(x, 4, 0)).collect();
    // Vertical net goes up a via, crosses (4,4) on layer 1, comes back down.
    let mut v1 = vec![Site::new(4, 1, 0), Site::new(4, 2, 0), Site::new(4, 2, 1)];
    v1.extend((3..=6).map(|y| Site::new(4, y, 1)));
    v1.extend([Site::new(4, 6, 0), Site::new(4, 7, 0), Site::new(4, 8, 0)]);
    d.draw(horizontal, vec![h0.clone()]);
    d.draw(vertical, vec![v1]);
    let x = extract(&d, &r);
    assert!(x.shorts.is_empty() && x.opens.is_empty() && x.illegal.is_empty(), "{x:?}");
    // The same crossing on layer 0 is a short.
    let v0: Vec<Site> = (1..=8).map(|y| Site::new(4, y, 0)).collect();
    d.draw(vertical, vec![v0]);
    assert_eq!(extract(&d, &r).shorts, vec![(horizontal.min(vertical), horizontal.max(vertical))]);
    // A gap without a via is an open, and a diagonal step is illegal.
    let mut gap = h0.clone();
    gap[3].layer = 1;
    d.draw(vertical, vec![]);
    d.draw(horizontal, vec![gap]);
    assert!(!extract(&d, &r).illegal.is_empty());
    d.draw(horizontal, vec![h0[..3].to_vec(), h0[4..].to_vec()]);
    assert_eq!(extract(&d, &r).opens, vec![horizontal]);
}

#[test]
fn routing_is_deterministic_and_incremental() {
    let r = reg();
    let a = twin_watch(&r);
    let b = twin_watch(&r);
    assert_eq!(a, b);
    assert_eq!(fingerprint(&a), fingerprint(&b));
    assert!(extract(&a, &r).clean());
    // Moving the SRAM only reroutes the nets it touches.
    let mut d = a.clone();
    let sram = part_of(&d, "sram64");
    let stale = d.relocate(&r, sram, 10, 7, 0).unwrap();
    let untouched: Vec<_> = d.nets.iter().filter(|n| !stale.contains(&n.id)).cloned().collect();
    assert!(!stale.is_empty() && !untouched.is_empty());
    let stats = route_dirty(&mut d, &r);
    assert!(stats.failed.is_empty(), "{stats:?}");
    assert_eq!(stats.routed.len(), stale.len());
    for n in &untouched {
        assert_eq!(d.nets.iter().find(|m| m.id == n.id).unwrap().paths, n.paths);
    }
    assert!(compile(&d, &r).ok());
}

#[test]
fn manual_wires_survive_reroute() {
    let r = reg();
    let mut d = sentinel(&r);
    let net = d.nets.iter().find(|n| n.name.starts_with("IMEM")).unwrap().id;
    let drawn = d.nets.iter().find(|n| n.id == net).unwrap().paths.clone();
    d.draw(net, drawn.clone());
    route_all(&mut d, &r);
    let n = d.nets.iter().find(|n| n.id == net).unwrap();
    assert!(n.manual);
    assert_eq!(n.paths, drawn);
}

#[test]
fn a_long_memory_route_lowers_the_safe_clock() {
    let r = reg();
    let near = twin_watch(&r);
    let mut far = near.clone();
    let rom = part_of(&far, "rom256");
    let sram = part_of(&far, "sram64");
    far.remove(&r, rom).unwrap();
    far.remove(&r, sram).unwrap();
    let core = part_of(&far, "core8");
    let rom = far.place(&r, "rom256", 11, 9, 0).unwrap();
    let sram = far.place(&r, "sram64", 10, 2, 0).unwrap();
    for (port, slave) in [("imem", rom), ("dmem", sram)] {
        far.connect(&r, Pin { part: core, port: port.into() }, Pin { part: slave, port: "bus".into() }).unwrap();
    }
    let supply = part_of(&far, "supply_pad");
    for slave in [rom, sram] {
        far.connect(&r, Pin { part: supply, port: "vdd".into() }, Pin { part: slave, port: "vdd".into() }).unwrap();
    }
    route_dirty(&mut far, &r);
    let (a, b) = (compile(&near, &r), compile(&far, &r));
    assert!(b.ok(), "{:?}", b.diagnostics);
    let (a, b) = (a.report.unwrap(), b.report.unwrap());
    assert!(b.critical_ns > a.critical_ns, "{} vs {}", b.critical_ns, a.critical_ns);
    assert!(b.clock_hz < a.clock_hz);
    assert!(b.handler_us > a.handler_us);
}

#[test]
fn an_alu_off_the_core_bus_adds_no_throughput() {
    let r = reg();
    let with = twin_watch(&r);
    let mut without = with.clone();
    let alu = part_of(&without, "alu8");
    without.disconnect(&Pin { part: alu, port: "op".into() });
    route_dirty(&mut without, &r);
    let c = compile(&without, &r);
    assert!(c.diagnostics.iter().any(|d| d.severity == Severity::Warning && d.message.contains("ALU Unused")));
    let (a, b) = (compile(&with, &r).report.unwrap(), c.report.unwrap());
    assert!(b.cycles_per_wake > a.cycles_per_wake);
    assert!(b.idle_w >= a.idle_w * 0.99, "the stranded ALU still leaks");
}

#[test]
fn combinational_loops_are_rejected() {
    let r = reg();
    let mut d = twin_watch(&r);
    let alu = part_of(&d, "alu8");
    let other = d.place(&r, "alu8", 11, 9, 1).unwrap_or_else(|_| d.place(&r, "alu8", 1, 7, 0).unwrap());
    d.connect(&r, Pin { part: alu, port: "carry_out".into() }, Pin { part: other, port: "carry_in".into() }).unwrap();
    d.connect(&r, Pin { part: other, port: "carry_out".into() }, Pin { part: alu, port: "carry_in".into() }).unwrap();
    route_dirty(&mut d, &r);
    assert!(has(&d, &r, "Combinational Loop"));
}

#[test]
fn firmware_must_fit_the_hardware() {
    let r = reg();
    let mut d = sentinel(&r);
    d.firmware.sample_ms = 50;
    let c = compile(&d, &r);
    assert!(c.errors().any(|e| e.fix == Some(Fix::SampleAtLeast(125))), "{:?}", c.diagnostics);
    d.firmware.sample_ms = 250;
    d.firmware.channels = 2;
    assert!(has(&d, &r, "Sensors 2 / 1"));
    assert!(has(&d, &r, "RAM 5 / 4 B"));
}

#[test]
fn pads_must_touch_the_die_edge() {
    let r = reg();
    let mut d = twin_watch(&r);
    let gpio = part_of(&d, "gpio4");
    d.relocate(&r, gpio, 1, 8, 0).unwrap();
    route_dirty(&mut d, &r);
    let c = compile(&d, &r);
    assert!(c.errors().any(|e| e.fix == Some(Fix::MoveToEdge(gpio))));
}

#[test]
fn an_enclosed_port_on_one_layer_is_unroutable_and_says_why() {
    let r = reg();
    let mut d = Design::new("tight", "p8um", 8, 4, 1);
    let core = d.place(&r, "core8", 0, 0, 0).unwrap(); // vdd at (0,0): a body corner
    let rom = d.place(&r, "rom256", 5, 0, 0).unwrap();
    let net = d.connect(&r, Pin { part: core, port: "vdd".into() }, Pin { part: rom, port: "vdd".into() }).unwrap();
    let stats = route_nets(&mut d, &r, &[net]);
    assert_eq!(stats.failed, vec![net]);
    let c = compile(&d, &r);
    assert!(c.diagnostics.iter().any(|x| x.fix == Some(Fix::AddLayer)));
    // A second layer lets the wire leave over the module.
    d.layers = 2;
    assert_eq!(route_nets(&mut d, &r, &[net]).routed, vec![net]);
}

#[test]
fn removing_a_module_leaves_no_dangling_nets() {
    let r = reg();
    let mut d = twin_watch(&r);
    let alu = part_of(&d, "alu8");
    d.remove(&r, alu).unwrap();
    assert!(d.nets.iter().all(|n| n.pins.len() >= 2));
    route_dirty(&mut d, &r);
    let c = compile(&d, &r);
    assert!(c.ok(), "{:?}", c.diagnostics);
}
