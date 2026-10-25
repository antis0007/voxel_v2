use crate::compile::compile;
use crate::firmware::OPEN;
use crate::library::{Library, ReleaseError};
use crate::presets::{sentinel, twin_watch};
use crate::registry::Registry;
use crate::runtime::{Phase, State};

fn reg() -> Registry {
    Registry::builtin()
}

#[test]
fn revisions_are_immutable_and_releases_check_staleness() {
    let r = reg();
    let mut lib = Library::default();
    let p = lib.add_project("Tank", sentinel(&r));
    let g = lib.project(p).unwrap().generation;
    let a = lib.release(&r, p, g).unwrap();
    assert_eq!(lib.release(&r, p, g).unwrap(), a, "unchanged design reuses its revision");
    lib.edit(p, |d| d.firmware.sample_ms = 500);
    assert_eq!(lib.release(&r, p, g), Err(ReleaseError::Stale));
    let g = lib.project(p).unwrap().generation;
    let b = lib.release(&r, p, g).unwrap();
    assert_ne!(a, b);
    assert_eq!(lib.revision(a).unwrap().design.firmware.sample_ms, 250);
    assert_eq!(lib.revision(b).unwrap().letter(), "B");
    assert!(lib.model(b).unwrap().wake_ticks > lib.model(a).unwrap().wake_ticks);
    lib.edit(p, |d| d.parts.clear());
    let g = lib.project(p).unwrap().generation;
    assert!(matches!(lib.release(&r, p, g), Err(ReleaseError::Failed(_))));
    let s = lib.package(a, crate::fab::Provenance::Prepared).unwrap();
    let pkg = lib.take(s).unwrap();
    assert!(lib.take(s).is_none(), "a package leaves stock once");
    lib.put_back(pkg);
    lib.put_back(pkg);
    assert_eq!(lib.stock.len(), 1);
    lib.validate().unwrap();
    let text = ron::to_string(&lib).unwrap();
    let back: Library = ron::from_str(&text).unwrap();
    assert_eq!(back.revisions, lib.revisions);
}

#[test]
fn runtime_hysteresis_response_and_brownout() {
    let r = reg();
    let model = compile(&sentinel(&r), &r).report.unwrap().model;
    let mut s = State::new(0);
    let mut level = 100u16;
    let mut t = 0u64;
    // Power-on: boot, then the first wake schedules "on" after the response time.
    s.service(&model, t, true, |_| level);
    assert_eq!(s.phase, Phase::Booting);
    t = s.due();
    s.service(&model, t, true, |_| level);
    assert_eq!(s.phase, Phase::Running);
    assert!(!s.output, "output waits for the conversion and handler");
    t = s.due();
    s.service(&model, t, true, |_| level);
    assert!(s.output);
    // Inside the band it holds; above the upper threshold it turns off.
    level = 600;
    for _ in 0..4 {
        t = s.due();
        s.service(&model, t, true, |_| level);
    }
    assert!(s.output);
    level = 950;
    for _ in 0..4 {
        t = s.due();
        s.service(&model, t, true, |_| level);
    }
    assert!(!s.output);
    // Idle: nothing due until the next wake, many ticks away.
    assert!(s.due() >= t + u64::from(model.wake_ticks) - 1);
    // Brownout forces the safe state; idle energy stops accruing.
    level = 0;
    t = s.due();
    s.service(&model, t, false, |_| level);
    assert_eq!((s.phase, s.output), (Phase::Unpowered, false));
    let debt = s.debt_nj;
    t += 10_000;
    s.service(&model, t, false, |_| level);
    assert_eq!(s.debt_nj, debt);
    assert!(s.readings.iter().skip(1).all(|&x| x == OPEN));
}

#[test]
fn energy_is_paid_once() {
    let r = reg();
    let model = compile(&twin_watch(&r), &r).report.unwrap().model;
    let mut s = State::new(0);
    for t in (0..120 * 3600).step_by(model.wake_ticks as usize) {
        s.service(&model, t, true, |_| 500);
    }
    let unit = 250_000_000_000u64; // 250 J
    let total = s.debt_nj;
    let paid = s.pay(unit, 1000);
    assert_eq!(u64::from(paid) * unit + s.debt_nj, total);
    assert_eq!(s.pay(unit, 1000), 0, "nothing is charged twice");
}


#[test]
fn whole_units_are_paid_and_the_remainder_kept() {
    let unit = 250_000_000_000u64;
    let mut s = State::new(0);
    s.debt_nj = 3 * unit + unit / 2;
    assert_eq!(s.pay(unit, 2), 2, "capped by the supply");
    assert_eq!(s.debt_nj, unit + unit / 2);
    assert_eq!(s.pay(unit, 64), 1);
    assert_eq!(s.debt_nj, unit / 2);
}

#[test]
fn defer_applies_a_due_output_and_never_stalls() {
    let mut s = State::new(0);
    s.phase = Phase::Running;
    s.pending = Some((5, true));
    s.next_wake = 3;
    s.defer(6);
    assert!(s.output && s.pending.is_none());
    assert!(s.due() > 6);
}
