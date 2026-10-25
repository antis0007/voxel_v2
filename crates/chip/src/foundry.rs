//! Wafer-lot production on a library: one mask set per revision, reused by
//! every lot; materials billed up front; lots advanced at game time; good
//! dies packaged into stock with their speed bin.
use crate::fab::{Material, Provenance, mask_set, route};
use crate::library::{Library, Package};
use crate::lot::WaferLot;
use crate::registry::Registry;

pub const MAX_LOTS: usize = 64;

/// Die area in mm² of a revision's layout on its process.
pub fn die_mm2(registry: &Registry, design: &crate::design::Design) -> f32 {
    let site = registry.process(&design.process).map_or(0.0, |p| p.site_um / 1000.0);
    f32::from(design.width) * f32::from(design.height) * site * site
}

impl Library {
    pub fn mask_of(&self, revision: u32) -> Option<&crate::fab::MaskSet> {
        self.masks.iter().find(|m| m.revision == revision)
    }

    /// Materials one lot of `revision` in `lab` consumes, masks included
    /// when the revision has none yet.
    pub fn lot_bill(&self, registry: &Registry, revision: u32, lab: &str) -> Option<Vec<(Material, u32)>> {
        let rev = self.revision(revision)?;
        let lab = registry.lab(lab)?;
        let steps = route(rev.design.layers);
        let mut bill: Vec<(Material, u32)> = vec![(Material::Wafer, 1)];
        let mut add = |m: Material, n: u32| match bill.iter_mut().find(|(k, _)| *k == m) {
            Some((_, c)) => *c += n,
            None => bill.push((m, n)),
        };
        if self.mask_of(revision).is_none() {
            add(Material::MaskBlank, steps.iter().filter(|s| s.mask.is_some()).count() as u32);
        }
        for s in &steps {
            for &(m, n) in s.kind.uses() {
                add(m, n);
            }
        }
        if lab.filters > 0 {
            add(Material::Filter, lab.filters * steps.len() as u32 / 8);
        }
        Some(bill)
    }

    /// Start a lot; the caller has paid `lot_bill`.
    pub fn start_lot(&mut self, registry: &Registry, revision: u32, lab: &str, now: u64) -> Result<u32, &'static str> {
        if self.lots.len() >= MAX_LOTS {
            return Err("Lot Limit");
        }
        registry.lab(lab).ok_or("Unknown Lab")?;
        let rev = self.revision(revision).ok_or("No Such Revision")?;
        let steps = route(rev.design.layers);
        if self.mask_of(revision).is_none() {
            let masks = mask_set(&rev.design, registry, revision, rev.fingerprint);
            self.masks.push(masks);
        }
        let mask = self.mask_of(revision).map_or(0, |m| m.id);
        self.next_lot += 1;
        let id = self.next_lot;
        self.lots.push(WaferLot::new(id, revision, mask, lab, now, steps[0]));
        Ok(id)
    }

    pub fn lots_active(&self) -> bool {
        self.lots.iter().any(|l| l.records.last().is_none_or(|r| r.kind != crate::fab::StepKind::Package))
    }

    /// Advance every lot to `now`. Returns the parts packaged.
    pub fn advance_lots(&mut self, registry: &Registry, now: u64) -> u32 {
        let mut made = 0;
        for i in 0..self.lots.len() {
            let lot = &self.lots[i];
            let (Some(rev), Some(lab)) = (self.revision(lot.revision), registry.lab(&lot.lab)) else { continue };
            let Some(process) = registry.process(&rev.design.process) else { continue };
            let Some(masks) = self.masks.iter().find(|m| m.id == lot.mask_set).cloned() else { continue };
            let steps = route(rev.design.layers);
            if lot.done(&steps) || now < lot.busy_until {
                continue;
            }
            let area = die_mm2(registry, &rev.design);
            let (revision, lot_id, mask_id) = (lot.revision, lot.id, lot.mask_set);
            let lab = lab.clone();
            let process = process.clone();
            let Some(dies) = self.lots[i].advance(now, &steps, &lab, &process, &masks, area) else { continue };
            for (die, grade) in dies {
                self.next_serial += 1;
                let serial = self.next_serial;
                let provenance = Provenance::Fabricated { mask_set: mask_id, lot: lot_id, die };
                self.stock.push(Package { serial, revision, provenance, grade });
                made += 1;
            }
        }
        made
    }
}
