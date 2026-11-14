//! Opt-in rendered assembler processing and disk checkpoint acceptance.
use super::review_fixture::{self as fixture, Ledger};
use crate::{player::Player, world::{self, World, factory as f}};
use anyhow::{Result, anyhow, ensure};
use std::path::PathBuf;

const BATCHES: u32 = 8;

pub struct Review {
    phase: u8,
    single: bool,
    expected: Option<Ledger>,
    capture: Option<&'static str>,
    root: PathBuf,
    frames_after_complete: u32,
}
impl Review {
    pub fn new() -> Option<Self> {
        let root = PathBuf::from(std::env::var_os("VOXEL_CAPTURE_DIR")?);
        let saves = PathBuf::from(std::env::var_os("VOXEL_SAVE_DIR")?);
        if saves != root.join("disposable-saves") { return None; }
        Some(Self { phase: 0, single: std::env::var("VOXEL_REVIEW_ASSEMBLER_BATCH").as_deref() == Ok("1"), expected: None, capture: None, root, frames_after_complete: 0 })
    }
    pub fn install(&mut self, w: &mut World, p: &mut Player) {
        world::storage::set_autosave_enabled(false);
        if let Err(error) = fixture::install(w, p, BATCHES) { self.fail(error); }
        else {
            if self.single { let at = fixture::machine(w); f::action(w, p.pos, f::Action::SetRecipe(at, super::SINGLE_BATCH)); }
            self.capture = Some("assembler-start.png");
        }
    }
    fn fail(&mut self, error: anyhow::Error) {
        eprintln!("[assembler-review] FAIL {error:#}");
        self.phase = 4;
    }
    pub fn frame(&mut self, w: &mut World, p: &mut Player, day: &mut world::DayNightCycle,
        saves: &mut world::storage::SaveService) -> bool {
        match self.step(w, p, day, saves) {
            Ok(reloaded) => reloaded,
            Err(error) => { self.fail(error); false }
        }
    }
    fn step(&mut self, w: &mut World, p: &mut Player, day: &mut world::DayNightCycle,
        saves: &mut world::storage::SaveService) -> Result<bool> {
        if self.phase == 4 { return Ok(false); }
        let ledger = Ledger::capture(w).ok_or_else(|| anyhow!("assembler missing"))?;
        ensure!(ledger.conserved(BATCHES), "inventory conservation failed: {ledger:?}");
        if self.single && self.phase >= 2 {
            self.batch_step(w, p, &ledger)?;
            return Ok(false);
        }
        match self.phase {
            0 if ledger.progress >= 3 => {
                let at = fixture::machine(w);
                if self.single {
                    for mode in [super::SINGLE_BATCH, 0, super::SINGLE_BATCH] {
                        f::action(w, p.pos, f::Action::SetRecipe(at, mode));
                    }
                    ensure!(Ledger::capture(w).as_ref() == Some(&ledger), "mode switch reset paid work");
                }
                f::action(w, p.pos, f::Action::Toggle(at));
                self.expected = Ledger::capture(w);
                ensure!(self.expected.as_ref().is_some_and(|l| !l.enabled), "pause action refused");
                saves.request(true);
                saves.tick(Some(w), p, day);
                self.phase = 1;
                self.capture = Some("assembler-paused.png");
                eprintln!("[assembler-review] paused mid-batch and requested checkpoint");
                fixture::report(w);
            }
            1 => {
                saves.tick(Some(w), p, day);
                if saves.busy() { return Ok(false); }
                ensure!(!saves.failed, "checkpoint failed: {:?}", saves.status);
                let saved = world::storage::load_review_checkpoint()?;
                ensure!(Ledger::capture(&saved.world) == self.expected, "checkpoint ledger changed");
                *w = saved.world; *p = saved.player; *day = saved.day_night;
                let at = fixture::machine(w);
                f::action(w, p.pos, f::Action::Toggle(at));
                self.phase = 2;
                self.capture = Some("assembler-reloaded.png");
                eprintln!("[assembler-review] disk reload matched; resumed production");
                return Ok(true);
            }
            2 if ledger.complete(BATCHES) => {
                self.phase = 3;
                self.capture = Some("assembler-complete.png");
                fixture::report(w);
                eprintln!("[assembler-review] PASS batches={BATCHES} voxel_m={}", w.voxel_scale.meters());
            }
            3 => { self.frames_after_complete += 1; }
            _ => {}
        }
        Ok(false)
    }
    fn batch_step(&mut self, w: &mut World, p: &Player, ledger: &Ledger) -> Result<()> {
        match self.phase {
            2 if ledger.batch_complete(1, BATCHES) => {
                self.expected = Some(ledger.clone()); self.phase = 3;
                self.frames_after_complete = 0; self.capture = Some("assembler-batch-one.png");
            }
            3 | 6 => {
                ensure!(Some(ledger) == self.expected.as_ref(), "stopped batch changed inventory or state");
                self.frames_after_complete += 1;
                if self.frames_after_complete >= 240 {
                    if self.phase == 3 {
                        let at = fixture::machine(w);
                        f::action(w, p.pos, f::Action::Toggle(at));
                        self.phase = 5;
                    } else {
                        self.capture = Some("assembler-complete.png"); self.phase = 7;
                        fixture::report(w);
                        eprintln!("[assembler-review] PASS single_batch=2 stocked={BATCHES} voxel_m={}", w.voxel_scale.meters());
                    }
                }
            }
            5 if ledger.batch_complete(2, BATCHES) => {
                self.expected = Some(ledger.clone()); self.phase = 6; self.frames_after_complete = 0;
            }
            7 => { self.frames_after_complete += 1; }
            _ => {}
        }
        Ok(())
    }
    pub fn capture(&mut self) -> Option<PathBuf> { Some(self.root.join(self.capture.take()?)) }
    pub fn done(&self) -> bool { self.phase == 4 || if self.single { self.phase == 7 && self.frames_after_complete >= 360 } else { self.phase == 3 && self.frames_after_complete >= 120 } }
}
