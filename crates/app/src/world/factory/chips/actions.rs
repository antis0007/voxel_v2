//! Library and installation requests from the UI, and the review fixture.
use super::{Action, Bay, BLANK, COPPER, COPPER_PER_BILL, MAX_PARTS, MAX_PROJECTS, MAX_REVISIONS, registry};
use crate::fast_hash::FastMap;
use crate::world::factory::Node;
use crate::world::factory::extraction::WATER_CAPACITY;
use engine_chip::fab::{Material, Provenance};
use engine_chip::library::ReleaseError;
use glam::IVec3;
use std::sync::Arc;

impl Bay {
    pub fn action(&mut self, nodes: &FastMap<IVec3, Node>, action: Action, pay: impl FnOnce(&[(usize, u32)]) -> bool) -> String {
        let registry = registry();
        match action {
            Action::NewProject(_, _) if self.library.projects.len() >= MAX_PROJECTS => "Project Limit".into(),
            Action::Release { .. } if self.library.revisions.len() >= MAX_REVISIONS => "Revision Limit".into(),
            Action::Prototype(_) | Action::StartLot { .. } if self.library.stock.len() + self.installed.len() >= MAX_PARTS => "Part Limit".into(),
            Action::NewProject(_, design) | Action::SaveDraft { design, .. } | Action::Release { design, .. }
                if design.sane(registry).is_err() =>
            {
                "Invalid Design".into()
            }
            Action::Scrap(serial) => {
                if Arc::make_mut(&mut self.library).scrap(serial) { "Part Scrapped".into() } else { String::new() }
            }
            Action::NewProject(name, design) => {
                Arc::make_mut(&mut self.library).add_project(&name, design);
                format!("Project {name}")
            }
            Action::SaveDraft { project, design } => {
                match Arc::make_mut(&mut self.library).edit(project, |d| *d = design) {
                    Some(()) => "Draft Saved".into(),
                    None => "No Such Project".into(),
                }
            }
            Action::Release { project, design, generation } => {
                let library = Arc::make_mut(&mut self.library);
                if library.project(project).is_none_or(|p| p.generation != generation) {
                    return "Draft Changed: Reopen".into();
                }
                library.edit(project, |d| *d = design);
                let g = library.project(project).map_or(0, |p| p.generation);
                match library.release(registry, project, g) {
                    Ok(id) => library.revision(id).map_or_else(String::new, |r| format!("Revision {} Released", r.letter())),
                    Err(ReleaseError::Failed(e)) => format!("Release Failed: {}", e.first().cloned().unwrap_or_default()),
                    Err(e) => format!("Release Failed: {e:?}"),
                }
            }
            Action::Prototype(revision) => {
                let Some(bill) = self.library.bill(revision) else { return "No Such Revision".into() };
                if !pay(&[BLANK, (COPPER, bill * COPPER_PER_BILL)]) {
                    return "Missing Materials".into();
                }
                match Arc::make_mut(&mut self.library).package(revision, Provenance::Prepared) {
                    Some(serial) => format!("Prototype #{serial}"),
                    None => "No Such Revision".into(),
                }
            }
            Action::StartLot { revision, lab } => {
                let Some(bill) = self.library.lot_bill(registry, revision, &lab) else { return "No Such Revision".into() };
                if self.library.lots.len() >= engine_chip::foundry::MAX_LOTS {
                    return "Lot Limit".into();
                }
                if !pay(&resources(&bill)) {
                    return "Missing Materials".into();
                }
                let now = self.now;
                match Arc::make_mut(&mut self.library).start_lot(registry, revision, &lab, now) {
                    Ok(id) => format!("Lot {id} Started"),
                    Err(e) => e.into(),
                }
            }
            Action::Install(p, serial) => match self.install(nodes, p, serial) {
                Ok(()) => "Controller Installed".into(),
                Err(e) => e.into(),
            },
            Action::Remove(p) => if self.remove(p) { "Controller Removed".into() } else { String::new() },
            Action::Reset(p) => {
                self.reset(p);
                "Controller Reset".into()
            }
            Action::Thresholds(p, lower, upper) => {
                self.set_thresholds(p, lower, upper);
                String::new()
            }
        }
    }
}

/// A chip library or installation request from the UI. Prototypes are built
/// at a workbench from the pack's materials.
pub fn chip_action(world: &mut crate::world::World, player: glam::Vec3, action: Action) {
    let prototype = matches!(action, Action::Prototype(_) | Action::StartLot { .. });
    if prototype && !super::super::at_bench(world, player) {
        world.expedition.say("Workbench Required");
        return;
    }
    let (factory, expedition) = (&mut world.factory, &mut world.expedition);
    let message = factory.chips.action(&factory.nodes, action, |cost| expedition.pay(cost));
    if prototype {
        world.reconcile_legacy_to_inventory();
    }
    if !message.is_empty() {
        world.expedition.say(message);
    }
}

/// Review fixture (VOXEL_REVIEW_FOCUS=pump or pile): both presets and a
/// Heat Guard Sentinel released; a Twin Watch in the pump, the Heat Guard in
/// the pile, a Sentinel in stock, the tank at 20%.
pub fn review_fixture(world: &mut crate::world::World, pump: IVec3, tank: IVec3, pile: IVec3) {
    let f = &mut world.factory;
    let mut guard = engine_chip::presets::sentinel(registry());
    guard.name = "Rod Guard".into();
    guard.firmware.task = engine_chip::firmware::Task::HeatGuard;
    for design in [engine_chip::presets::sentinel(registry()), engine_chip::presets::twin_watch(registry()), guard] {
        let lib = Arc::make_mut(&mut f.chips.library);
        let project = lib.add_project(&design.name.clone(), design);
        if let Ok(rev) = lib.release(registry(), project, 0) {
            lib.package(rev, Provenance::Prepared);
        }
    }
    if let Some(n) = f.nodes.get_mut(&pump) {
        n.energy = 64;
    }
    if let Some(n) = f.nodes.get_mut(&tank) {
        n.water = WATER_CAPACITY / 5;
    }
    // A Twin Watch lot in the clean hood, 100 s into the route.
    let lib = Arc::make_mut(&mut f.chips.library);
    if lib.start_lot(registry(), 2, "clean", 0).is_ok() {
        lib.advance_lots(registry(), 120 * 100);
        f.chips.now = 120 * 100;
    }
    let _ = f.chips.install(&f.nodes, pump, 2);
    if let Some(n) = f.nodes.get_mut(&pile) {
        n.energy = n.energy.max(64);
        n.progress = 620;
    }
    let _ = f.chips.install(&f.nodes, pile, 3);
}


/// Pack resources a fabrication bill draws: crystal for wafers and mask
/// glass, bio-ethanol for resist and solvent, tin dust as dopant, coal for
/// furnace steps, copper for metal and leadframes, steel for hood filters.
pub fn resources(bill: &[(Material, u32)]) -> Vec<(usize, u32)> {
    let mut out: Vec<(usize, u32)> = Vec::new();
    for &(m, n) in bill {
        let (r, per) = match m {
            Material::Wafer => (5, 64),
            Material::MaskBlank => (5, 16),
            Material::Resist => (21, 8),
            Material::Solvent => (21, 4),
            Material::Dopant => (10, 8),
            Material::Gas => (3, 8),
            Material::Metal => (COPPER, 8),
            Material::Filter => (15, 8),
        };
        match out.iter_mut().find(|(i, _)| *i == r) {
            Some((_, c)) => *c += n * per,
            None => out.push((r, n * per)),
        }
    }
    out
}
