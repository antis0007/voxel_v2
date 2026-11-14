use super::*;

pub(super) fn valid(g: &State, s: &Site) -> bool {
    let Some(plan)=&g.infill else {return false;};
    if !matches!((g.version,plan.schema),(2,1)|(3,2))
        || g.bootstrap!=6 || g.waiting.len()>160 || g.stockpile!=plan.stockpile
        || s.project_layers.len()!=s.projects.len() || !plan.valid(s)
        || g.chest_target.is_some_and(|n|!(1..=4096).contains(&n)) {return false;}
    let funded=plan.parcels.iter().filter(|p|p.project.is_some()).count();
    let terminal=|i:usize|s.projects.get(i).zip(s.project_layers.get(i)).is_some_and(|(p,l)|
        *l>p.cells.iter().map(|o|p.work_layer(*o)).max().unwrap_or(0)+1);
    if (6..6+funded).any(|i|Some(i)!=g.project && !terminal(i)) {return false;}
    if let Some(i)=g.project {
        if funded==0 || i!=5+funded || g.proposal.is_some() || g.complete
            || (g.version==3 && (plan.searching() || plan.pending().is_some())) {return false;}
    }
    if let Some(p)=&g.proposal {
        if !plan.pending().and_then(|q|q.build(IVec3::from(plan.base)).ok()).is_some_and(|q|infill::same(p,&q))
            || g.complete || g.project.is_some() {return false;}
    }
    !g.complete || (funded==limit(g)-6 && g.project.is_none() && g.proposal.is_none()
        && plan.pending().is_none() && !plan.searching())
}

fn limit(g: &State) -> usize {if g.version==3 {infill::PROJECT_LIMIT}else{8}}
fn full(g: &State) -> String {
    if g.version==3 {"Project Ledger Full".into()}else{"Infill Limit: 2 Homes Complete".into()}
}

pub(super) fn step(w: &mut World, s: &mut Site, demand: u32, g: &mut State) -> Result<String,String> {
    if let Some(i)=g.project {
        let p=s.projects.get(i).ok_or("Project Missing")?;
        if s.home_complete(w,i) {
            if w.farming.jobs.build.iter().any(|o|p.cells.contains(o))
                || w.farming.jobs.claims.iter().any(|c|p.cells.contains(&c.order)) {
                return Ok("Project Reconciliation".into());
            }
            g.project=None;
            if g.version==3 {g.infill.as_mut().ok_or("Infill Plan Missing")?.restart();}
            if s.projects.len()==limit(g) {g.complete=true;return Ok(full(g));}
            return Ok("Home Complete: Housing Review".into());
        }
        if p.cells.iter().any(|o|crate::world::farming::base::overlaps(w,o.cell(),o.cell())
            || known(w,o.cell()).is_none_or(|b|b!=B::Air && b!=o.block())) {
            return Ok("Project Obstructed".into());
        }
    } else {
        if !g.households_live(s) || (0..g.bootstrap).any(|i|!s.home_complete(w,i)) {
            return Ok("Bootstrap Incomplete".into());
        }
        if demand==0 {return Ok("Housing Supplied".into());}
        if s.projects.len()>=limit(g) {g.complete=true;return Ok(full(g));}
        if g.proposal.is_none() {
            let plan=g.infill.as_mut().ok_or("Infill Plan Missing")?;
            if plan.pending().is_none() {
                if g.version==3 {
                    if !infill::poll(w,s,plan,&mut g.scans)? {
                        return Ok(if plan.search.as_ref().is_some_and(|search|search.paused) {
                            "Street Search Budget: Retry Pending".into()
                        } else if plan.searching() {"Site Survey In Progress".into()}
                        else {g.waiting.clone()});
                    }
                } else {
                    plan.retry_at=plan.retry_at.min(w.farming.clock.saturating_add(infill::RETRY));
                    if plan.surveyed && w.farming.clock<plan.retry_at {return Ok(g.waiting.clone());}
                    infill::generate(w,s,plan,&mut g.scans)?;
                }
            }
            let parcel=plan.pending().ok_or("No Space: Local Capacity Exhausted")?;
            g.proposal=Some(parcel.build(IVec3::from(plan.base))?);
            return Ok("Proposal Ready".into());
        }
    }
    if w.farming.jobs.stockpile!=Some(g.stockpile) {return Err("Stockpile Changed".into());}
    let pile=IVec3::from(g.stockpile);
    if known(w,pile).is_none_or(|b|b.chest_tier().is_none()) {return Err("Stockpile Unavailable".into());}
    let p=g.project.and_then(|i|s.projects.get(i)).or(g.proposal.as_ref()).cloned().ok_or("Proposal Missing")?;
    let planks=super::super::super::growth_required(w,&w.farming.jobs,&p,Item::Block(B::Planks));
    let chests=super::super::super::growth_required(w,&w.farming.jobs,&p,Item::Block(B::Chest));
    request(w,g,planks,chests)?;
    let slots=w.block_data.chest(pile);
    let have_planks=production::count(&slots,Item::Block(B::Planks));
    let have_chests=production::count(&slots,Item::Block(B::Chest));
    if have_planks<planks || have_chests<chests {
        return Ok(format!("Materials: Planks {have_planks}/{planks} | Chests {have_chests}/{chests}"));
    }
    if g.project.is_some() {return Ok("Construction Active".into());}
    let plan=g.infill.as_mut().ok_or("Infill Plan Missing")?;
    let parcel=plan.pending().ok_or("Proposal Parcel Missing")?;
    if let Err(e)=infill::revalidate(w,s,plan,parcel,&mut g.scans) {
        plan.parcels.retain(|p|p.project.is_some());
        if g.version==3 {plan.search=None;}
        plan.retry_at=w.farming.clock.saturating_add(4*jobs::PULSE);
        g.proposal=None;
        return Ok(e);
    }
    let index=s.projects.len();
    let layer=p.cells.iter().map(|o|o.cell().y).min().ok_or("Proposal Empty")?;
    plan.parcels.iter_mut().find(|p|p.project.is_none()).ok_or("Proposal Parcel Missing")?.project=Some(index);
    g.project=Some(index);g.proposal=None;
    s.projects.push(p);s.project_layers.push(layer);
    println!("[city-infill] funded project={index} planks={planks} chests={chests}");
    Ok("Materials Reserved".into())
}
