use crate::compile::compile;
use crate::fab::{Grade, Material, Provenance, StepKind, route};
use crate::foundry::die_mm2;
use crate::library::Library;
use crate::lot::dies_per_wafer;
use crate::presets::{sentinel, twin_watch};
use crate::registry::Registry;

fn released(r: &Registry) -> (Library, u32) {
    let mut lib = Library::default();
    let p = lib.add_project("S", sentinel(r));
    let rev = lib.release(r, p, 0).unwrap();
    (lib, rev)
}

/// Run every lot to completion at game time; returns packages made.
fn finish(lib: &mut Library, r: &Registry) -> u32 {
    let mut made = 0;
    for t in (0..120 * 600).step_by(30) {
        made += lib.advance_lots(r, t);
    }
    made
}

#[test]
fn the_route_orders_its_steps() {
    let steps = route(2);
    let at = |k: StepKind, m: Option<crate::fab::MaskLayer>| steps.iter().position(|s| s.kind == k && (m.is_none() || s.mask == m)).unwrap();
    use crate::fab::MaskLayer::*;
    assert!(at(StepKind::Coat, None) < at(StepKind::Expose, Some(Diffusion)));
    assert!(at(StepKind::Expose, Some(Diffusion)) < at(StepKind::Implant, None), "implant through the opened window");
    assert!(at(StepKind::Implant, None) < at(StepKind::Anneal, None));
    assert!(at(StepKind::Expose, Some(Metal(0))) < at(StepKind::Expose, Some(Via(1))));
    assert!(at(StepKind::Expose, Some(Via(1))) < at(StepKind::Expose, Some(Metal(1))));
    assert_eq!(steps.last().unwrap().kind, StepKind::Package);
    for (i, _) in steps.iter().enumerate().filter(|(_, s)| s.kind == StepKind::Expose) {
        assert_eq!(steps[i - 1].kind, StepKind::Coat, "resist before every exposure");
    }
    assert!(route(3).len() > steps.len());
}

#[test]
fn mask_sets_are_made_once_and_reused() {
    let r = Registry::builtin();
    let (mut lib, rev) = released(&r);
    let first = lib.lot_bill(&r, rev, "bench").unwrap();
    assert!(first.iter().any(|(m, _)| *m == Material::MaskBlank));
    lib.start_lot(&r, rev, "bench", 0).unwrap();
    let second = lib.lot_bill(&r, rev, "bench").unwrap();
    assert!(!second.iter().any(|(m, _)| *m == Material::MaskBlank), "the second lot reuses the masks");
    lib.start_lot(&r, rev, "bench", 0).unwrap();
    assert_eq!(lib.masks.len(), 1);
    assert_eq!(lib.lots[0].mask_set, lib.lots[1].mask_set);
    let clean = lib.lot_bill(&r, rev, "clean").unwrap();
    assert!(clean.iter().any(|(m, n)| *m == Material::Filter && *n > 0), "the clean hood costs filters");
}

#[test]
fn lots_package_graded_dies_that_run_the_same_firmware() {
    let r = Registry::builtin();
    let (mut lib, rev) = released(&r);
    lib.start_lot(&r, rev, "bench", 0).unwrap();
    let made = finish(&mut lib, &r);
    let o = lib.lots[0].outcome.unwrap();
    let dies = dies_per_wafer(12.0, die_mm2(&r, &lib.revision(rev).unwrap().design));
    assert_eq!(o.dies, dies);
    assert_eq!(made, u32::from(o.good));
    assert!(o.good > 0 && o.good < o.dies, "{o:?}");
    assert_eq!(o.bins.iter().sum::<u16>(), o.good);
    assert!(lib.stock.iter().all(|p| p.revision == rev && matches!(p.provenance, Provenance::Fabricated { lot: 1, .. })));
    assert_eq!(lib.lots[0].records.len(), route(2).len());
    lib.validate().unwrap();
    // A part's behaviour is the revision's model; the bin only rates it.
    assert!(lib.stock.iter().any(|p| p.grade != Grade::C));
    let _ = lib.model(rev).unwrap();
}

#[test]
fn a_clean_hood_yields_more_and_faster_parts_than_an_open_bench() {
    let r = Registry::builtin();
    let (mut lib, rev) = released(&r);
    for _ in 0..4 {
        lib.start_lot(&r, rev, "bench", 0).unwrap();
        lib.start_lot(&r, rev, "clean", 0).unwrap();
    }
    finish(&mut lib, &r);
    let sum = |lab: &str, f: &dyn Fn(&crate::lot::Outcome) -> u16| -> u32 {
        lib.lots.iter().filter(|l| l.lab == lab).map(|l| u32::from(f(&l.outcome.unwrap()))).sum()
    };
    let (bench, clean) = (sum("bench", &|o| o.good), sum("clean", &|o| o.good));
    let (bench_a, clean_a) = (sum("bench", &|o| o.bins[0]), sum("clean", &|o| o.bins[0]));
    eprintln!("4 lots each: bench {bench} good ({bench_a} A), clean {clean} good ({clean_a} A)");
    assert!(clean > bench && clean_a > bench_a);
}

#[test]
fn a_half_finished_lot_finishes_the_same_after_a_reload() {
    let r = Registry::builtin();
    let (mut lib, rev) = released(&r);
    lib.start_lot(&r, rev, "bench", 0).unwrap();
    for t in (0..120 * 40).step_by(30) {
        lib.advance_lots(&r, t);
    }
    assert!(lib.lots_active());
    let text = ron::to_string(&lib).unwrap();
    let mut reloaded: Library = ron::from_str(&text).unwrap();
    finish(&mut lib, &r);
    // Different service cadence after the reload: same outcome.
    for t in (120 * 40..120 * 600).step_by(997) {
        reloaded.advance_lots(&r, t);
    }
    reloaded.advance_lots(&r, 120 * 600);
    assert_eq!(lib.lots, reloaded.lots);
    assert_eq!(lib.stock, reloaded.stock);
}

#[test]
fn two_lots_from_one_mask_set_are_comparable_and_revisions_stay_fixed() {
    let r = Registry::builtin();
    let mut lib = Library::default();
    let p = lib.add_project("T", twin_watch(&r));
    let rev = lib.release(&r, p, 0).unwrap();
    lib.start_lot(&r, rev, "clean", 0).unwrap();
    // Editing the draft mid-lot does not touch the lot's revision.
    lib.edit(p, |d| d.parts.clear());
    lib.start_lot(&r, rev, "clean", 0).unwrap();
    finish(&mut lib, &r);
    let (a, b) = (lib.lots[0].outcome.unwrap(), lib.lots[1].outcome.unwrap());
    assert_eq!(a.dies, b.dies);
    assert!(a.good.abs_diff(b.good) <= a.dies / 3, "{a:?} {b:?}");
    assert!(compile(&lib.revision(rev).unwrap().design, &r).ok());
}

#[test]
fn library_text_from_before_fabrication_still_decodes() {
    let r = Registry::builtin();
    let (mut lib, rev) = released(&r);
    lib.package(rev, Provenance::Prepared).unwrap();
    let text = ron::to_string(&lib).unwrap();
    let old = [",grade:A", "masks:[],", "lots:[],", ",next_lot:0", "recovers:false,"]
        .iter()
        .fold(text.clone(), |t, f| {
            assert!(t.contains(f), "{f} not in {t}");
            t.replace(f, "")
        });
    let back: Library = ron::from_str(&old).unwrap();
    back.validate().unwrap();
    assert_eq!(back.stock[0].grade, Grade::A);
    assert!(back.revision(rev).unwrap().model.recovers, "old revisions restart after a brownout");
}
