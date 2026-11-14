//! A paid service chest supports one finite, content-backed house extension.
use super::*;
pub(crate) mod plan;
mod state;
mod targets;
mod funding;
mod validation;
mod check;
#[cfg(test)] mod tests;
pub(crate) use state::{Services, Stage};
pub(crate) use check::check as check_service_growth;
pub(super) use validation::valid;
const ATTEMPT_LIMIT: u32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HubPlan { pub primary: IVec3, pub stockpile: IVec3, pub project: usize }

pub(crate) fn hub_plan(j: &jobs::State) -> Option<HubPlan> {
    let g=j.quarter.as_ref()?.frontier.as_ref()?.growth.as_ref()?;
    let service=g.services.as_ref().filter(|_|g.version==4)?;
    let plan=service.plan.as_ref()?;
    service.hub_project?;
    Some(HubPlan {primary:plan.primary.into(),stockpile:plan.hub.into(),project:service.base_project_count+1})
}

pub(crate) fn hub_available(w: &World, j: &jobs::State) -> bool {
    let Some(s)=j.quarter.as_ref().and_then(|q|q.frontier.as_ref()) else {return false;};
    let Some(service)=s.growth.as_ref().filter(|g|g.version==4).and_then(|g|g.services.as_ref()) else {return false;};
    let Some((plan,index))=service.plan.as_ref().zip(service.hub_project) else {return false;};
    j.stockpile==Some(plan.primary) && plan::hub_available(w,s,plan,index)
}

pub(crate) fn enable(w: &mut World) -> Result<(),String> {
    let mut city=w.farming.jobs.quarter.take().ok_or("Colony Unavailable")?;
    let result=(|| {
        let s=city.frontier.as_mut().ok_or("Colony Unavailable")?;
        let mut g=s.growth.clone().ok_or("Settled Growth 3 Required")?;
        if g.version==4 {return if valid(&g,s) {Ok(())}else{Err("Invalid Service Growth".into())};}
        let residents=w.farming.jobs.production.as_ref().map_or(0,|p|p.workers.iter()
            .filter(|id|jobs::crew::view(&w.farming.jobs,**id).is_some()).count());
        let housed=s.households.iter().filter(|h|s.home_complete(w,h.project)).map(|h|h.members.len()).sum::<usize>();
        state::upgrade(w,s,&mut g,residents.saturating_sub(housed) as u32)?;
        s.growth=Some(g);Ok(())
    })();
    w.farming.jobs.quarter=Some(city);result
}

pub(super) fn step(w: &mut World, s: &mut Site, demand: u32, g: &mut State) -> Result<String,String> {
    let mut service=g.services.take().ok_or("Service State Missing")?;
    let result=advance(w,s,demand,g,&mut service);
    g.services=Some(service);result
}

fn advance(w: &mut World, s: &mut Site, demand: u32, g: &mut State, service: &mut Services) -> Result<String,String> {
    if w.farming.jobs.stockpile!=Some(g.stockpile) {return Err("Stockpile Changed".into());}
    if !known(w,g.stockpile.into()).is_some_and(|b|b.chest_tier().is_some()) {return Err("Stockpile Unavailable".into());}
    match service.stage {
        Stage::Survey=>survey(w,s,demand,g,service),
        Stage::HubMaterials|Stage::HomeMaterials=>materials(w,s,demand,g,service),
        Stage::HubBuilding|Stage::HomeBuilding=>building(w,s,g,service),
        Stage::Complete=>Ok("Service Home Complete".into()),
    }
}

fn survey(w: &mut World, s: &Site, demand: u32, g: &mut State, service: &mut Services) -> Result<String,String> {
    if demand==0 {return Ok("Housing Supplied".into());}
    let count=plan::candidate_count();
    if count==0 {return Err("Service Candidates Unavailable".into());}
    if service.cursor>=count {
        service.retry_at=service.retry_at.min(w.farming.clock.saturating_add(infill::RETRY));
        if w.farming.clock<service.retry_at {return Ok("Service Sites Exhausted: Retry Pending".into());}
        service.cursor=0;service.retry_at=0;
    }
    service.attempts=service.attempts.saturating_add(1).min(ATTEMPT_LIMIT);
    g.scans.candidates=g.scans.candidates.saturating_add(1);
    service.reserved_budget.reserve();
    let Some(plan)=plan::select(w,s,g.stockpile,service.cursor)? else {
        service.cursor+=1;
        if service.cursor==count {service.retry_at=w.farming.clock.saturating_add(infill::RETRY);}
        return Ok("Service Site Survey".into());
    };
    let hub=plan::hub_project(&plan)?;
    let home=plan::home_project(&plan)?;
    if !plan::valid(&plan,s,service.base_project_count)
        || s.projects.iter().map(|p|p.cells.len()).sum::<usize>()+hub.cells.len()+home.cells.len()>4096 {
        return Err("Service Plan Limit".into());
    }
    g.proposal=Some(hub);service.plan=Some(plan);service.stage=Stage::HubMaterials;
    Ok("Service Chest Materials".into())
}

fn materials(w: &mut World, s: &mut Site, demand: u32, g: &mut State, service: &mut Services) -> Result<String,String> {
    if demand==0 {
        if let Some(prod)=w.farming.jobs.production.as_mut() {service.targets.restore(prod)?;}
        return Ok("Housing Supplied".into());
    }
    let plan=service.plan.as_ref().ok_or("Service Plan Missing")?;
    if service.stage==Stage::HomeMaterials
        && !plan::hub_available(w,s,plan,service.hub_project.ok_or("Service Chest Missing")?) {
        return Ok("Service Chest Unavailable".into());
    }
    let proposal=g.proposal.as_ref().ok_or("Proposal Missing")?;
    let cost=funding::requirements(proposal)?;
    let slots=w.block_data.chest(g.stockpile.into());
    if !funding::paid(&cost,&slots) {
        funding::request(w,service,g.stockpile,&cost)?;
        return Ok(funding::status(&cost,&slots));
    }
    service.reserved_budget.reserve();
    if !plan::available(w,s,plan)? {return Ok("Service Site Obstructed".into());}
    let prod=w.farming.jobs.production.as_mut().ok_or("Production Unavailable")?;
    service.targets.restore(prod)?;
    state::admit(s,g,service)?;
    println!("[city-service] funded stage={:?} project={:?} cells={}",service.stage,g.project, cost.iter().map(|(_,n)|n).sum::<u32>());
    Ok("Service Materials Reserved".into())
}

fn building(w: &World, s: &Site, g: &mut State, service: &mut Services) -> Result<String,String> {
    let i=g.project.ok_or("Service Project Missing")?;
    let p=s.projects.get(i).ok_or("Service Project Missing")?;
    if service.stage==Stage::HomeBuilding
        && !plan::hub_available(w,s,service.plan.as_ref().ok_or("Service Plan Missing")?,
            service.hub_project.ok_or("Service Chest Missing")?) {return Ok("Service Chest Unavailable".into());}
    if s.home_complete(w,i) {
        if w.farming.jobs.build.iter().any(|o|p.cells.contains(o))
            || w.farming.jobs.claims.iter().any(|c|p.cells.contains(&c.order)) {return Ok("Service Project Reconciliation".into());}
        if service.stage==Stage::HomeBuilding && !super::super::housing::usable(w,s,i) {
            return Ok("Home Entrance Blocked".into());
        }
        state::finished(s,g,service)?;
        return Ok(if g.complete {"Service Home Complete"}else{"Service Chest Complete: Home Materials"}.into());
    }
    if p.cells.iter().any(|o|crate::world::farming::base::overlaps(w,o.cell(),o.cell())
        || known(w,o.cell()).is_none_or(|b|b!=B::Air && b!=o.block())) {return Ok("Service Project Obstructed".into());}
    Ok("Service Construction Active".into())
}
