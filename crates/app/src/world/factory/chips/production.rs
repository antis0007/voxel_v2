//! Authored laboratory batches and energy-backed wafer clocks. Stock is an
//! explicit bounded lab inventory; it never aliases the legacy resource table.
use super::super::{Kind, Node};
use super::{Bay, Installed, Key, MAX_PARTS, registry};
use crate::world::{
    BlockId,
    inventory::{Item, SurvivalInventory},
};
use engine_chip::{fab::Material, library::Library};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const STOCK_CAP: u32 = 4096;
pub const MAX_JOBS: usize = 64;
pub const SAND_CAP: u32 = 64;
pub const QUANTUM: u64 = 30; // 0.25 s at the authoritative 120 Hz.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Supply {
    Crude,
    Pure,
    Ingot,
    Wafer,
    Mask,
    Resist,
    Solvent,
    Dopant,
    Gas,
    Metal,
    Filter,
}
impl Supply {
    pub const ALL: [Self; 11] = [
        Self::Crude,
        Self::Pure,
        Self::Ingot,
        Self::Wafer,
        Self::Mask,
        Self::Resist,
        Self::Solvent,
        Self::Dopant,
        Self::Gas,
        Self::Metal,
        Self::Filter,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Crude => "Crude Silicon",
            Self::Pure => "Refined Silicon",
            Self::Ingot => "Crystal Ingot",
            Self::Wafer => "Polished Wafer",
            Self::Mask => "Mask Blank",
            Self::Resist => "Photoresist",
            Self::Solvent => "Cleaning Solvent",
            Self::Dopant => "Dopant Kit",
            Self::Gas => "Process Gas",
            Self::Metal => "Metal Target",
            Self::Filter => "Hood Filter",
        }
    }
    pub fn material(m: Material) -> Self {
        match m {
            Material::Wafer => Self::Wafer,
            Material::MaskBlank => Self::Mask,
            Material::Resist => Self::Resist,
            Material::Solvent => Self::Solvent,
            Material::Dopant => Self::Dopant,
            Material::Gas => Self::Gas,
            Material::Metal => Self::Metal,
            Material::Filter => Self::Filter,
        }
    }
}

pub struct Operation {
    pub name: &'static str,
    pub raw: &'static [(Item, u32)],
    pub inputs: &'static [(Supply, u32)],
    pub outputs: &'static [(Supply, u32)],
    pub seconds: u32,
    /// Integer 250 J units per 0.25 s, numerically equal to kW.
    pub power: u32,
}
use Supply::*;
pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "Silicon Smelting",
        raw: &[(Item::Block(BlockId::Sand), 2), (Item::Resource(3), 2)],
        inputs: &[],
        outputs: &[(Crude, 1)],
        seconds: 8,
        power: 8,
    },
    Operation {
        name: "Zone Refining",
        raw: &[],
        inputs: &[(Crude, 1)],
        outputs: &[(Pure, 1)],
        seconds: 12,
        power: 8,
    },
    Operation {
        name: "Crystal Growth",
        raw: &[],
        inputs: &[(Pure, 2)],
        outputs: &[(Ingot, 1)],
        seconds: 12,
        power: 8,
    },
    Operation {
        name: "Wafer Preparation",
        raw: &[],
        inputs: &[(Ingot, 1), (Solvent, 2)],
        outputs: &[(Wafer, 4)],
        seconds: 8,
        power: 4,
    },
    Operation {
        name: "Mask Glass",
        raw: &[(Item::Block(BlockId::Sand), 1)],
        inputs: &[],
        outputs: &[(Mask, 8)],
        seconds: 6,
        power: 4,
    },
    // Explicit procurement abstraction, not a reaction producing dopants from tin
    // or process gas from coal. Board barter orders prepared chemical supplies.
    Operation {
        name: "Reagent Order",
        raw: &[(Item::Resource(12), 2)],
        inputs: &[],
        outputs: &[(Resist, 64), (Solvent, 64), (Dopant, 16), (Gas, 64)],
        seconds: 8,
        power: 1,
    },
    Operation {
        name: "Metal Targets",
        raw: &[(Item::Resource(6), 1)],
        inputs: &[],
        outputs: &[(Metal, 32)],
        seconds: 4,
        power: 4,
    },
    Operation {
        name: "Hood Filters",
        raw: &[(Item::Resource(15), 1), (Item::Resource(4), 1)],
        inputs: &[],
        outputs: &[(Filter, 32)],
        seconds: 4,
        power: 2,
    },
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub site: Key,
    pub recipe: u8,
    pub remaining: u64,
    pub credit: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoweredLot {
    pub id: u32,
    pub site: Key,
    pub clock: u64,
    pub credit: u64,
    pub reserved: usize,
}
/// A placed machine maintains a shared lab-stock target. Resource inputs use
/// the existing pipe buffer (64 units per inventory item); sand has a block bin.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Order {
    pub site: Key,
    pub recipe: u8,
    pub target: u32,
    pub enabled: bool,
    pub sand: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Production {
    pub stock: [u32; 11],
    pub jobs: Vec<Job>,
    pub lots: Vec<PoweredLot>,
    #[serde(default)]
    pub orders: Vec<Order>,
}

impl Production {
    pub fn busy(&self, site: Key) -> bool {
        self.jobs.iter().any(|j| j.site == site) || self.lots.iter().any(|l| l.site == site)
    }
    pub fn owns(&self, site: Key) -> bool {
        self.busy(site) || self.orders.iter().any(|o| o.site == site)
    }
    pub fn progress(&self, site: Key) -> Option<(&'static str, f32)> {
        self.jobs
            .iter()
            .find(|j| j.site == site)
            .map(|j| {
                let op = &OPERATIONS[j.recipe as usize];
                (
                    op.name,
                    1.0 - j.remaining as f32 / (op.seconds * 120) as f32,
                )
            })
            .or_else(|| {
                self.orders.iter().find(|o| o.site == site).map(|o| {
                    let supply = OPERATIONS[o.recipe as usize].outputs[0].0;
                    (
                        supply.name(),
                        (self.stock[supply as usize] as f32 / o.target as f32).min(1.0),
                    )
                })
            })
    }
    pub fn pending(&self, supply: Supply) -> u32 {
        self.jobs
            .iter()
            .flat_map(|j| OPERATIONS[j.recipe as usize].outputs)
            .filter(|(s, _)| *s == supply)
            .map(|(_, n)| *n)
            .sum()
    }
    pub fn available(&self, inputs: &[(Supply, u32)]) -> bool {
        inputs.iter().all(|&(s, n)| self.stock[s as usize] >= n)
    }
    pub fn start(
        &mut self,
        site: Key,
        recipe: u8,
        inventory: &mut SurvivalInventory,
    ) -> Result<(), &'static str> {
        if self.orders.iter().any(|o| o.site == site) {
            return Err("Production Order Active");
        }
        self.check_start(site, recipe)?;
        let op = &OPERATIONS[recipe as usize];
        let mut paid = inventory.clone();
        for &(item, n) in op.raw {
            if !paid.remove_item(item, n) {
                return Err("Missing Materials");
            }
        }
        *inventory = paid;
        self.commit_job(site, recipe);
        Ok(())
    }

    fn check_start(&self, site: Key, recipe: u8) -> Result<(), &'static str> {
        let op = OPERATIONS.get(recipe as usize).ok_or("Unknown Process")?;
        if self.busy(site) {
            return Err("Fabricator Busy");
        }
        if self.jobs.len() >= MAX_JOBS {
            return Err("Batch Limit");
        }
        if !self.available(op.inputs) {
            return Err("Missing Lab Stock");
        }
        for &(s, n) in op.outputs {
            if self.stock[s as usize] + self.pending(s) + n > STOCK_CAP {
                return Err("Lab Stock Full");
            }
        }
        Ok(())
    }

    fn commit_job(&mut self, site: Key, recipe: u8) {
        let op = &OPERATIONS[recipe as usize];
        for &(s, n) in op.inputs {
            self.stock[s as usize] -= n;
        }
        self.jobs.push(Job {
            site,
            recipe,
            remaining: u64::from(op.seconds) * 120,
            credit: 0,
        });
    }

    fn start_order(&mut self, index: usize, node: &mut Node) -> Result<(), &'static str> {
        let order = &self.orders[index];
        let op = &OPERATIONS[order.recipe as usize];
        let supply = op.outputs[0].0;
        if self.stock[supply as usize] + self.pending(supply) >= order.target {
            return Err("Stock ready");
        }
        if node.energy < op.power {
            return Err("Needs cable / power");
        }
        self.check_start(order.site, order.recipe)
            .map_err(|e| match e {
                "Missing Lab Stock" => "Needs input",
                _ => "Output blocked",
            })?;
        // Check every input before committing any buffer or lab-stock debit.
        for &(item, n) in op.raw {
            let enough = match item {
                Item::Resource(r) => node.input[r as usize] >= n * 64,
                Item::Block(BlockId::Sand) => order.sand >= n,
                _ => false,
            };
            if !enough {
                return Err("Needs input");
            }
        }
        let (site, recipe) = (order.site, order.recipe);
        for &(item, n) in op.raw {
            match item {
                Item::Resource(r) => node.input[r as usize] -= n * 64,
                Item::Block(BlockId::Sand) => self.orders[index].sand -= n,
                _ => unreachable!("checked laboratory feed"),
            }
        }
        self.commit_job(site, recipe);
        Ok(())
    }

    pub fn bill(bill: &[(Material, u32)]) -> Vec<(Supply, u32)> {
        bill.iter()
            .map(|&(m, n)| (Supply::material(m), n))
            .collect()
    }

    pub fn validate(
        &self,
        library: &Library,
        now: u64,
        installed: usize,
    ) -> Result<(), &'static str> {
        if self.stock.iter().any(|&n| n > STOCK_CAP)
            || self.jobs.len() > MAX_JOBS
            || self.lots.len() > engine_chip::foundry::MAX_LOTS
            || self.orders.len() > MAX_JOBS
        {
            return Err("Invalid Lab Capacity");
        }
        let mut sites = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::new();
        let valid_site = |p: Key| {
            [p.0, p.1, p.2]
                .iter()
                .all(|v| v.unsigned_abs() <= 1_000_000)
        };
        let mut order_sites = std::collections::BTreeSet::new();
        for order in &self.orders {
            if !valid_site(order.site)
                || !order_sites.insert(order.site)
                || usize::from(order.recipe) >= OPERATIONS.len()
                || !(1..=STOCK_CAP).contains(&order.target)
                || order.sand > SAND_CAP
            {
                return Err("Invalid Production Order");
            }
            if self.lots.iter().any(|l| l.site == order.site)
                || self
                    .jobs
                    .iter()
                    .any(|j| j.site == order.site && j.recipe != order.recipe)
            {
                return Err("Conflicting Production Order");
            }
        }
        for lot in &library.lots {
            if lot.route_version > 1 {
                return Err("Unsupported Fabrication Route");
            }
            let rev = library
                .revision(lot.revision)
                .ok_or("Missing Wafer Revision")?;
            let steps = engine_chip::fab::manufacturing_route(rev.design.layers, lot.route_version);
            if usize::from(lot.step) > steps.len()
                || lot.records.len() != usize::from(lot.step)
                || lot
                    .records
                    .iter()
                    .enumerate()
                    .any(|(i, r)| usize::from(r.step) != i || r.kind != steps[i].kind)
                || !lot.survival.is_finite()
                || !(0.0..=1.0).contains(&lot.survival)
            {
                return Err("Invalid Wafer History");
            }
        }
        for j in &self.jobs {
            let Some(op) = OPERATIONS.get(j.recipe as usize) else {
                return Err("Invalid Process");
            };
            if !valid_site(j.site)
                || !sites.insert(j.site)
                || j.credit > QUANTUM
                || j.remaining == 0
                || j.remaining > u64::from(op.seconds) * 120
            {
                return Err("Invalid Lab Job");
            }
        }
        let mut reserved = library.stock.len() + installed;
        for l in &self.lots {
            if !valid_site(l.site)
                || !sites.insert(l.site)
                || !ids.insert(l.id)
                || l.credit > QUANTUM
                || l.clock > now
            {
                return Err("Invalid Powered Lot");
            }
            let lot = library
                .lots
                .iter()
                .find(|lot| lot.id == l.id)
                .ok_or("Missing Wafer Lot")?;
            if l.clock >= lot.busy_until {
                return Err("Invalid Wafer Clock");
            }
            if lot
                .records
                .last()
                .is_some_and(|r| r.kind == engine_chip::fab::StepKind::Package)
            {
                return Err("Completed Lot Still Assigned");
            }
            let expected = lot_capacity(library, lot.revision).ok_or("Invalid Wafer Revision")?;
            if l.reserved != expected {
                return Err("Invalid Die Reservation");
            }
            reserved += l.reserved;
        }
        if reserved > MAX_PARTS {
            return Err("Part Limit");
        }
        for s in Supply::ALL {
            let pending: u32 = self
                .jobs
                .iter()
                .map(|j| {
                    OPERATIONS[j.recipe as usize]
                        .outputs
                        .iter()
                        .filter(|(supply, _)| *supply == s)
                        .map(|(_, n)| *n)
                        .sum::<u32>()
                })
                .sum();
            if self.stock[s as usize] + pending > STOCK_CAP {
                return Err("Invalid Lab Reservation");
            }
        }
        Ok(())
    }
}

pub fn lot_capacity(library: &Library, revision: u32) -> Option<usize> {
    let rev = library.revision(revision)?;
    let p = registry().process(&rev.design.process)?;
    Some(engine_chip::lot::dies_per_wafer(
        p.wafer_mm,
        engine_chip::foundry::die_mm2(registry(), &rev.design),
    ) as usize)
}

/// Spend a quantum before doing its work; unused paid ticks persist across saves.
/// No work accrues while disabled, controller-held, missing or out of energy.
fn powered_ticks(
    node: Option<&mut Node>,
    held: bool,
    ticks: u64,
    credit: &mut u64,
    power: u32,
) -> (u64, u32) {
    let Some(node) = node.filter(|n| n.kind == Kind::Fabricator) else {
        return (0, 0);
    };
    if !node.enabled || held {
        node.status = if held { "Held by controller" } else { "Paused" };
        return (0, 0);
    }
    let mut done = 0;
    let mut used = 0;
    // Callers supply fixed world steps. Cap catch-up work, never simulate a long
    // wall-clock absence or borrow unlimited input power from a stale buffer.
    while done < ticks.min(120) {
        if *credit == 0 {
            if node.energy < power {
                node.status = "Needs cable / power";
                break;
            }
            node.energy -= power;
            used += power;
            *credit = QUANTUM;
        }
        let work = (*credit).min(ticks.min(120) - done);
        *credit -= work;
        done += work;
        node.status = "Processing";
    }
    (done, used)
}

impl super::super::Factory {
    pub(super) fn step_fabrication(&mut self, ticks: u64) {
        let Bay {
            library,
            installed,
            production,
            now,
            ..
        } = &mut self.chips;
        // Cache only recipe routing on nodes, never inventory or ownership.
        // This is reconstructed after load, before the pipe network runs.
        for order in &production.orders {
            if let Some(n) =
                self.nodes
                    .get_mut(&glam::IVec3::new(order.site.0, order.site.1, order.site.2))
                && n.kind == Kind::Fabricator
            {
                n.lab_recipe = Some(order.recipe);
            }
        }
        let schedule = *now / QUANTUM != now.saturating_sub(ticks) / QUANTUM;
        if production.jobs.is_empty()
            && production.lots.is_empty()
            && (!schedule || production.orders.is_empty())
        {
            return;
        }
        let production = Arc::make_mut(production);
        let held = |site: &Key| {
            installed
                .get(site)
                .is_some_and(|c: &Installed| !c.state.output)
        };
        let mut used = 0;
        if schedule {
            let count = production.orders.len();
            for offset in 0..count {
                // Rotate priority deterministically when machines share scarce stock.
                let i = (offset + (*now / QUANTUM) as usize % count) % count;
                let order = &production.orders[i];
                if production.busy(order.site) {
                    continue;
                }
                let Some(node) = self
                    .nodes
                    .get_mut(&glam::IVec3::new(order.site.0, order.site.1, order.site.2))
                    .filter(|n| n.kind == Kind::Fabricator)
                else {
                    continue;
                };
                node.status = if !order.enabled || !node.enabled {
                    "Paused"
                } else if held(&order.site) {
                    "Held by controller"
                } else {
                    production
                        .start_order(i, node)
                        .err()
                        .unwrap_or("Processing")
                };
            }
        }
        for job in &mut production.jobs {
            let op = &OPERATIONS[job.recipe as usize];
            let p = glam::IVec3::new(job.site.0, job.site.1, job.site.2);
            let (work, energy) = powered_ticks(
                self.nodes.get_mut(&p),
                held(&job.site),
                ticks.min(job.remaining),
                &mut job.credit,
                op.power,
            );
            used += energy;
            job.remaining -= work;
            if job.remaining == 0 {
                for &(s, n) in op.outputs {
                    production.stock[s as usize] += n;
                }
            }
        }
        production.jobs.retain(|j| j.remaining != 0);
        let mut done = Vec::new();
        for powered in &mut production.lots {
            let p = glam::IVec3::new(powered.site.0, powered.site.1, powered.site.2);
            let (work, energy) = powered_ticks(
                self.nodes.get_mut(&p),
                held(&powered.site),
                ticks,
                &mut powered.credit,
                4,
            );
            used += energy;
            powered.clock += work;
            if work > 0 {
                Arc::make_mut(library).advance_lot(registry(), powered.id, powered.clock);
            }
            if library
                .lots
                .iter()
                .find(|l| l.id == powered.id)
                .is_none_or(|l| {
                    l.records
                        .last()
                        .is_some_and(|r| r.kind == engine_chip::fab::StepKind::Package)
                })
            {
                done.push(powered.id);
            }
        }
        production.lots.retain(|l| !done.contains(&l.id));
        self.stats.power(0, used);
    }
}
