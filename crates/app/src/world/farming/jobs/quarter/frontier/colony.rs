//! Concurrent paid projects share the existing inventory and resident jobs.
use super::*;
use crate::world::farming::jobs;
pub(super) mod scene;
pub(crate) mod growth;
pub(super) mod check;
pub(in super::super) mod housing;

pub(super) fn enabled()->bool {std::env::var("VOXEL_CITY_COLONY").is_ok_and(|s|s=="1")}

pub(super) fn advance(w:&mut World,city:&mut super::super::State) {
    let Some(s)=city.frontier.as_mut() else{return;};
    housing::service(w,s);
    let housed: usize = s.households.iter().map(|h| h.members.len()).sum();
    city.housing_demand = w.farming.jobs.production.as_ref().map_or(0, |p| p.workers.len().saturating_sub(housed)) as u32;
    growth::advance(w,s,IVec3::from(city.origin),city.housing_demand);
    super::admit_timber(w,s);
    let Some(pile)=w.farming.jobs.stockpile else{return;};
    let Some(prod)=w.farming.jobs.production.as_ref() else{return;};
    let meals=production::count(&w.block_data.chest(pile.into()),prod.meal);
    let eaten=prod.counts.eaten;
    let pending:BTreeSet<_>=w.farming.jobs.build.iter().map(|o|o.at).collect();
    for (i,project) in s.projects.iter().enumerate() {
        let layer=s.project_layers[i];
        let high=project.cells.iter().map(|o|project.work_layer(*o)).max().unwrap_or(0);
        if layer>high+1 || project.cells.iter().any(|o|pending.contains(&o.at)) {continue;}
        if layer>high {
            if project.cells.iter().all(|o|known(w,o.cell())==Some(o.block())) {
                s.project_layers[i]=high+2;s.phase+=1;city.completed=s.phase as u32;
                println!("[city-colony] completed={} name={} cells={}",s.phase,project.name,project.cells.len());
            }
            continue;
        }
        let first=project.cells.iter().map(|o|o.cell().y).min().unwrap_or(0);
        if layer==first && (meals<project.food_reserve||eaten<project.meals_eaten) {continue;}
        let cells:Vec<_>=project.cells.iter().copied().filter(|o|project.work_layer(*o)==layer).collect();
        if cells.iter().any(|o|crate::world::farming::base::overlaps(w,o.cell(),o.cell())
            ||known(w,o.cell()).is_none_or(|b|b!=B::Air&&b!=o.block())) {continue;}
        let orders:Vec<_>=cells.into_iter().filter(|o|known(w,o.cell())!=Some(o.block())).collect();
        if orders.is_empty() {s.project_layers[i]+=1;continue;}
        if orders.len()+w.farming.jobs.build.len()>jobs::construction::LIMIT {continue;}
        println!("[city-colony] project={i} layer={layer} orders={}",orders.len());
        w.farming.jobs.build.extend(orders);
        w.farming.jobs.reset_task();
    }
    city.status=if let Some(g) = &s.growth {g.waiting.clone()} else if s.phase==s.projects.len() {"Settlement projects complete".into()}
        else {format!("Projects {}/{} · {}",s.phase,s.projects.len(),jobs::status(w))};
}
