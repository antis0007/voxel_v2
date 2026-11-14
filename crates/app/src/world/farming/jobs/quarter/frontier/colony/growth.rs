//! Demand-led homes reserve stock in the existing physical chest.
use super::{jobs, known, production, B, IVec3, World};
use super::super::{Project, State as Site};
use crate::world::inventory::Item;
use serde::{Deserialize, Serialize};
mod site;
mod funding_check;
mod validation_check;
mod repeated;
mod repeated_check;
mod repeatable_check;
pub(crate) mod services;
use super::super::super::layout::{self, infill};
pub use infill::Scans;
#[cfg(test)] mod tests;
pub(crate) use funding_check::check as check_housing_growth_funding;
pub(crate) use site::check as check_housing_growth_sites;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    #[serde(default)] pub next_poll: u64,
    #[serde(default)] pub polls: u32,
    #[serde(default)] pub scans: Scans,
    pub version: u8,
    pub bootstrap: usize,
    pub stockpile: [i32; 3],
    pub proposal: Option<Project>,
    pub project: Option<usize>,
    pub complete: bool,
    pub waiting: String,
    pub chest_target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infill: Option<infill::InfillPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) services: Option<services::Services>,
}

impl State {
    pub fn new(stockpile: [i32; 3]) -> Self {
        Self { next_poll: 0, polls: 0, scans: Scans::default(), version: 1, bootstrap: 6, stockpile, proposal: None, project: None,
            complete: false, waiting: "Bootstrap Incomplete".into(), chest_target: None, infill: None, services: None }
    }
    fn due(&mut self, now: u64) -> bool {
        self.next_poll = self.next_poll.min(now.saturating_add(4*jobs::PULSE));
        now >= self.next_poll
    }
    pub(in super::super) fn valid(&self, s: &Site) -> bool {
        if self.version == 4 { return services::valid(self,s); }
        if self.services.is_some() { return false; }
        if matches!(self.version,2|3) { return repeated::valid(self,s); }
        self.infill.is_none() && self.version == 1 && self.bootstrap == 6 && self.waiting.len() <= 160
            && s.project_layers.len() == s.projects.len()
            && self.stockpile.iter().all(|n| n.unsigned_abs() < 1 << 27)
            && self.chest_target.is_none_or(|n| (1..=4096).contains(&n))
            && if let Some(i) = self.project {
                i == self.bootstrap && s.projects.len() == i + 1 && self.proposal.is_none()
                    && s.projects[i].home.as_ref().is_some_and(|h| h.capacity == 4)
                    && (!self.complete || s.project_layers[i] > s.projects[i].cells.iter()
                        .map(|o|s.projects[i].work_layer(*o)).max().unwrap_or(0)+1)
            } else {
                !self.complete && s.projects.len() == self.bootstrap
                    && self.proposal.as_ref().is_none_or(|p| site::valid_plan(s, p))
            }
    }
    pub(crate) fn active_project(&self) -> Option<usize> {
        self.project.filter(|_|!self.complete)
    }
    pub(crate) fn households_live(&self, s: &Site) -> bool {
        if self.version == 1 { return self.project.is_some(); }
        matches!(self.version,2|3|4) && s.projects.iter().take(self.bootstrap).enumerate().all(|(i,p)|
            s.project_layers.get(i).is_some_and(|l|*l>p.cells.iter().map(|o|p.work_layer(*o)).max().unwrap_or(0)+1))
    }
}

pub(crate) fn enable(w: &mut World) -> Result<(), String> {
    if std::env::var("VOXEL_CITY_GROWTH").is_ok_and(|v|v=="4") { return services::enable(w); }
    let version=match std::env::var("VOXEL_CITY_GROWTH").ok().as_deref() {Some("2")=>2,Some("3")=>3,_=>1};
    let mode=version>=2;
    let seed=layout::seed(w.seed);
    let j = &mut w.farming.jobs;
    let pile = j.stockpile.ok_or("Stockpile Unavailable")?;
    let city = j.quarter.as_mut().ok_or("Colony Unavailable")?;
    let origin=IVec3::from(city.origin);
    let s = city.frontier.as_mut().ok_or("Colony Unavailable")?;
    if s.growth.is_some() { return Ok(()); }
    if s.projects.len() != 6 || s.project_layers.len() != 6 {
        return Err("Six Bootstrap Projects Required".into());
    }
    let profile=if mode {layout::selected()?.unwrap_or(0)} else {0};
    let mut g=State::new(pile);
    if mode {
        let mut plan=infill::InfillPlan::new(origin,pile,seed,profile);
        if version==3 {plan.schema=2;}
        g.version=version;g.infill=Some(plan);
    }
    s.growth = Some(g);
    Ok(())
}

pub(super) fn advance(w: &mut World, s: &mut Site, origin: IVec3, demand: u32) {
    let Some(mut g) = s.growth.take() else { return; };
    if g.complete || !g.due(w.farming.clock) { s.growth = Some(g); return; }
    g.next_poll = w.farming.clock.saturating_add(4*jobs::PULSE);
    g.polls = g.polls.saturating_add(1);
    g.waiting = match g.version {
        4=>services::step(w,s,demand,&mut g),
        2|3=>repeated::step(w,s,demand,&mut g),
        _=>step(w,s,origin,demand,&mut g),
    }.unwrap_or_else(|e| e);
    s.growth = Some(g);
}

fn step(w: &mut World, s: &mut Site, origin: IVec3, demand: u32, g: &mut State) -> Result<String, String> {
    if g.complete { return Ok("Home Complete".into()); }
    if let Some(i) = g.project {
        let p = s.projects.get(i).ok_or("Project Missing")?;
        let high = p.cells.iter().map(|o| p.work_layer(*o)).max().unwrap_or(0);
        if s.project_layers.get(i).is_some_and(|l| *l > high + 1)
            && p.cells.iter().all(|o| known(w, o.cell()) == Some(o.block())) {
            g.complete = true;
            return Ok("Home Complete".into());
        }
    } else {
        if s.projects.iter().take(g.bootstrap).enumerate().any(|(i, p)|
            s.built(w, i) != p.cells.len() || s.project_layers[i] <= p.cells.iter().map(|o|p.work_layer(*o)).max().unwrap_or(0) + 1) {
            return Ok("Bootstrap Incomplete".into());
        }
        if demand == 0 { return Ok("Housing Supplied".into()); }
        if g.proposal.is_none() {
            g.proposal = Some(site::select(w, s, origin, &mut g.scans)?);
            return Ok("Proposal Ready".into());
        }
    }
    if w.farming.jobs.stockpile != Some(g.stockpile) { return Err("Stockpile Changed".into()); }
    let pile = IVec3::from(g.stockpile);
    if known(w, pile).is_none_or(|b| b.chest_tier().is_none()) { return Err("Stockpile Unavailable".into()); }
    let p = g.project.and_then(|i| s.projects.get(i)).or(g.proposal.as_ref()).cloned().ok_or("Proposal Missing")?;
    let slots = w.block_data.chest(pile);
    let need = |item| super::super::growth_required(w, &w.farming.jobs, &p, item);
    let planks = need(Item::Block(B::Planks));
    let chests = need(Item::Block(B::Chest));
    request(w, g, planks, chests)?;
    let have_planks = production::count(&slots, Item::Block(B::Planks));
    let have_chests = production::count(&slots, Item::Block(B::Chest));
    if have_planks < planks || have_chests < chests {
        return Ok(format!("Materials: Planks {have_planks}/{planks} | Chests {have_chests}/{chests}"));
    }
    if g.project.is_none() {
        site::available(w, s, &p, &mut g.scans)?;
        let p = g.proposal.take().ok_or("Proposal Missing")?;
        let layer = p.cells.iter().map(|o|o.cell().y).min().ok_or("Proposal Empty")?;
        g.project = Some(s.projects.len());
        s.projects.push(p);
        s.project_layers.push(layer);
        println!("[city-growth] funded project={} planks={planks} chests={chests}", s.projects.len()-1);
        return Ok("Materials Reserved".into());
    }
    let p = &s.projects[g.project.ok_or("Project Missing")?];
    if p.cells.iter().any(|o| crate::world::farming::base::overlaps(w,o.cell(),o.cell())
        || known(w,o.cell()).is_none_or(|b| b != B::Air && b != o.block())) {
        return Ok("Project Obstructed".into());
    }
    Ok("Construction Active".into())
}

fn request(w: &mut World, g: &mut State, planks: u32, chests: u32) -> Result<(), String> {
    let pile = IVec3::from(g.stockpile);
    let chest = Item::Block(B::Chest);
    let have = production::count(&w.block_data.chest(pile), chest);
    let cargo = jobs::crew::cargo_item(&w.farming.jobs, chest);
    let missing = chests.saturating_sub(have + cargo);
    let prod = w.farming.jobs.production.as_mut().ok_or("Production Unavailable")?;
    let made = prod.made.get("Chest").copied().unwrap_or(0);
    let target = made.checked_add(missing).filter(|n| *n <= 4096).ok_or("Production Limit")?;
    let bill = prod.bills.iter_mut().find(|b| b.recipe == "Chest" && b.once).ok_or("Chest Bill Missing")?;
    bill.target = bill.target.max(target);
    g.chest_target = Some(bill.target);
    let chest_recipe = crate::world::inventory::all_recipes().iter().find(|r|r.name == "Chest").ok_or("Chest Recipe Missing")?;
    let ingredient = chest_recipe.ingredients.iter().find(|(i,_)|*i == Item::Block(B::Planks)).map_or(0,|(_,n)|*n);
    let target = planks.saturating_add(missing.saturating_mul(ingredient)).min(4096);
    let mut found = false;
    for b in &mut prod.bills {
        if b.recipe == "Oak planks" { b.target = b.target.max(target); found = true; }
    }
    if !found { return Err("Plank Bill Missing".into()); }
    Ok(())
}
