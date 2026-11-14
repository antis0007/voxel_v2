//! Food-backed projects extend the resident production scene without material grants.
use super::{B, IVec3, Vec3, World};
use super::super::{construction::Order, production, known};
use crate::world::inventory::Item;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub(super) mod scene;
pub(super) mod check;
mod panel;
mod movement;
mod colony;
pub(crate) use colony::growth::{enable as enable_housing_growth, check_housing_growth_funding, check_housing_growth_sites};
pub(crate) use colony::growth::services::{HubPlan, hub_plan, hub_available, check_service_growth, enable as enable_service_growth};
pub(super) use panel::draw;
pub(super) use scene::install;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub cells: Vec<Order>,
    pub food_reserve: u32,
    pub meals_eaten: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<colony::housing::Home>,
}

impl Project {
    pub(super) fn work_layer(&self,order:Order)->i32 {
        if order.block()==B::Chest {
            self.cells.iter().filter(|o|o.block()!=B::Chest).map(|o|o.cell().y).max().unwrap_or(0)+1
        } else {order.cell().y}
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub growth: Option<colony::growth::State>,
    pub version: u8,
    pub projects: Vec<Project>,
    pub timber: Vec<[i32;3]>,
    /// Legacy scenes already admitted their entire timber list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timber_next: Option<usize>,
    pub phase: usize,
    pub layer: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_layers: Vec<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub households: Vec<colony::housing::Household>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub housing_overrides: Vec<u32>,
}

impl State {
    pub(super) fn valid(&self) -> bool {
        let mut cells=BTreeSet::new();
        let mut timber=BTreeSet::new();
        let bounded=|p:&[i32;3]|p.iter().all(|n|n.unsigned_abs() < 1<<27);
        if self.projects.iter().any(|p|p.cells.iter().any(|o|!bounded(&o.at))) {return false;}
        self.version==1 && self.growth.as_ref().is_none_or(|g|g.valid(self)) && colony::housing::valid(self) && !self.projects.is_empty()
            && self.projects.len()<=if self.growth.as_ref().is_some_and(|g|matches!(g.version,3|4)) {super::layout::infill::PROJECT_LIMIT}else{8}
            && (!self.growth.as_ref().is_some_and(|g|g.version==4)
                || self.projects.iter().map(|p|p.cells.len()).sum::<usize>()<=4096)
            && self.phase<=self.projects.len() && self.layer.unsigned_abs()<1<<27
            && if self.project_layers.is_empty() {self.projects.get(self.phase).map_or(self.layer==0,|p| {
                let low=p.cells.iter().map(|o|o.cell().y).min().unwrap_or(0);
                let high=p.cells.iter().map(|o|p.work_layer(*o)).max().unwrap_or(0)+1;
                (low..=high).contains(&self.layer)
            })} else {self.layer==0 && self.project_layers.len()==self.projects.len()
                && self.projects.iter().zip(&self.project_layers).all(|(p,&layer)| {
                    let low=p.cells.iter().map(|o|o.cell().y).min().unwrap_or(0);
                    let high=p.cells.iter().map(|o|p.work_layer(*o)).max().unwrap_or(0);
                    (low..=high+2).contains(&layer)
                }) && self.phase==self.projects.iter().zip(&self.project_layers)
                    .filter(|(p,layer)|**layer>p.cells.iter().map(|o|p.work_layer(*o)).max().unwrap_or(0)+1).count()}
            && self.timber.len()<=128 && self.timber.iter().all(|p|bounded(p)&&timber.insert(*p))
            && self.timber_next.is_none_or(|next|next<=self.timber.len())
            && self.projects.iter().all(|p|!p.name.is_empty()&&p.name.len()<=48
                && p.food_reserve<=64 && p.meals_eaten<=128 && !p.cells.is_empty()
                && p.cells.len()<=if self.growth.as_ref().is_some_and(|g|g.version==4) {1024}else{super::super::construction::LIMIT}
                && p.cells.iter().all(|o|bounded(&o.at)&&o.material().is_some()&&cells.insert(o.at)))
            && timber.is_disjoint(&cells)
    }
    pub(crate) fn home_complete(&self, w: &World, index: usize) -> bool {
        self.projects.get(index).is_some_and(|p|self.project_layers.get(index).is_some_and(|layer|
            *layer > p.cells.iter().map(|o|p.work_layer(*o)).max().unwrap_or(0)+1)
            && self.built(w,index) == p.cells.len())
    }
    pub(super) fn built(&self, w:&World, index:usize)->usize {
        self.projects.get(index).map_or(0,|p|p.cells.iter().filter(|o|known(w,o.cell())==Some(o.block())).count())
    }
}

pub(super) fn enabled()->bool {
    std::env::var("VOXEL_CITY_START_SITE").is_ok_and(|v|v=="1")
}

pub(super) fn advance(w:&mut World, city:&mut super::State) {
    if city.paused {city.status="Projects Paused".into();return;}
    if city.frontier.as_ref().is_some_and(|s|!s.project_layers.is_empty()) {colony::advance(w,city);return;}
    let Some(s)=city.frontier.as_mut() else{return;};
    admit_timber(w,s);
    let Some(project)=s.projects.get(s.phase) else {
        city.status="Site projects complete".into(); return;
    };
    if !w.farming.jobs.build.is_empty() {
        city.status=super::super::status(w);return;
    }
    let high=project.cells.iter().map(|o|project.work_layer(*o)).max().unwrap_or(0);
    if s.layer>high {
        if project.cells.iter().all(|o|known(w,o.cell())==Some(o.block())) {
            println!("[city-site] project-complete phase={} name={} cells={}",s.phase,project.name,project.cells.len());
            s.phase+=1; city.completed=s.phase as u32;
            s.layer=s.projects.get(s.phase).and_then(|p|p.cells.iter().map(|o|o.cell().y).min()).unwrap_or(0);
            city.status="Project complete".into();
        } else {city.status="Structure changed".into();}
        return;
    }
    let Some(production)=w.farming.jobs.production.as_ref() else {city.status="Production unavailable".into();return;};
    let Some(pile)=w.farming.jobs.stockpile else {city.status="Stockpile unavailable".into();return;};
    let meals=production::count(&w.block_data.chest(pile.into()),production.meal);
    // The entry gate is evaluated once. Later meals do not cancel paid work.
    let first=project.cells.iter().map(|o|o.cell().y).min().unwrap_or(0);
    if s.layer==first && project.cells.iter().all(|o|known(w,o.cell())!=Some(o.block()))
        && (meals<project.food_reserve || production.counts.eaten<project.meals_eaten) {
        city.status=format!("Food reserve {meals}/{} · Meals eaten {}/{}",project.food_reserve,production.counts.eaten,project.meals_eaten);
        return;
    }
    if let Some(production)=w.farming.jobs.production.as_mut() {
        for bill in &mut production.bills {
            if bill.recipe=="Chest" {bill.target=(s.phase+1) as u32;}
        }
    }
    // Furniture follows the roof, so builders never need to climb a chest below a low ceiling.
    let layer:Vec<_>=project.cells.iter().copied().filter(|o|project.work_layer(*o)==s.layer).collect();
    if layer.iter().any(|o|crate::world::farming::base::overlaps(w,o.cell(),o.cell())
        || known(w,o.cell()).is_none_or(|b|b!=B::Air&&b!=o.block())) {
        city.status="Project obstructed".into();return;
    }
    let orders:Vec<_>=layer.into_iter().filter(|o|known(w,o.cell())!=Some(o.block())).collect();
    if orders.is_empty() {s.layer+=1;} else {
        println!("[city-site] phase={} layer={} orders={}",s.phase,s.layer,orders.len());
        w.farming.jobs.build.extend(orders);
        w.farming.jobs.reset_task();
    }
    city.status=format!("{} · Layer {}",project.name,s.layer-IVec3::from(city.origin).y);
}

pub(crate) fn worker_radius(w:&World)->f32 {
    if w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref())
        .is_some_and(|s|!s.project_layers.is_empty()) {64.0} else {32.0}
}

fn admit_timber(w:&mut World,s:&mut State) {
    let Some(mut next)=s.timber_next else {return;};
    while next<s.timber.len() && w.farming.jobs.mine.len()<super::super::LIMIT {
        let at=IVec3::from(s.timber[next]);
        let Some(block)=known(w,at) else {break;};
        if block==B::Log && !crate::world::farming::base::overlaps(w,at,at)
            && !w.farming.jobs.mine.iter().any(|(p,_)|*p==at.to_array()) {
            w.farming.jobs.mine.push((at.to_array(),B::Log as u16));
        }
        // Each source is considered once; later player replacements are not new work.
        next+=1;
    }
    s.timber_next=Some(next);
}

/// Unpaid cells are backed by chest stock or valid physical cargo claims.
pub(crate) fn growth_required(w: &World, j: &super::super::State, p: &Project, item: Item) -> u32 {
    p.cells.iter().filter(|o|o.material() == Some(item) && known(w,o.cell()) != Some(o.block()))
        .filter(|o|!j.claims.iter().any(|c|c.order == **o && j.build.contains(&c.order)
            && super::super::crew::view(j,c.worker).is_some_and(|a|a.carry.is_some_and(|stack|
                stack.item == item && j.claims.iter().filter(|other|other.worker == c.worker).count() <= stack.count as usize))))
        .count() as u32
}
