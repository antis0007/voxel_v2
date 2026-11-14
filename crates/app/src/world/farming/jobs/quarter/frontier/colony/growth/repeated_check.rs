//! Native probes exercise generated provenance without admitting real work.
use super::*;
use anyhow::{Context, ensure};

pub(super) fn check(w: &mut World) -> anyhow::Result<()> {
    let s=w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).context("Missing site")?.clone();
    let g=s.growth.as_ref().context("Missing growth")?;
    ensure!(g.version==2 && s.projects.len()==6,"Infill probes require unfunded version 2");
    let mut plan=g.infill.clone().context("Missing plan")?;
    let mut scans=Scans::default();
    infill::generate(w,&s,&mut plan,&mut scans).map_err(anyhow::Error::msg)?;
    ensure!(!plan.parcels.is_empty() && plan.parcels.len()<=2 && plan.valid(&s),"Generated plan rejected");
    ensure!(scans.candidates<=9604 && scans.path_searches<=infill::ROUTES
        && scans.path_node_budget<=u64::from(infill::ROUTES)*20_000,"Infill survey exceeded bounds");
    let parcel=plan.pending().context("Missing candidate")?;
    infill::revalidate(w,&s,&plan,parcel,&mut scans).map_err(anyhow::Error::msg)?;
    let project=parcel.build(IVec3::from(plan.base)).map_err(anyhow::Error::msg)?;
    let mut candidate=s.clone();
    let state=candidate.growth.as_mut().context("Missing growth")?;
    state.infill=Some(plan.clone());state.proposal=Some(project.clone());
    round_trip(&candidate,true)?;
    mutations(&candidate)?;
    serial(w,&candidate)?;
    let blocked=project.cells.first().context("Empty house")?.cell();
    let previous=known(w,blocked).context("Unloaded candidate")?;
    w.set(blocked,B::Stone);
    let invalid=infill::revalidate(w,&s,&plan,parcel,&mut Scans::default()).is_err();
    let mut fallback=plan.clone();
    let alternative=infill::generate(w,&s,&mut fallback,&mut Scans::default());
    let preserved=known(w,blocked)==Some(B::Stone);
    w.set(blocked,previous);
    ensure!(invalid && preserved,"Obstruction was accepted or changed");
    if alternative.is_ok() {
        ensure!(fallback.parcels.iter().all(|p|p.build(IVec3::from(plan.base)).is_ok_and(|p|
            p.cells.iter().all(|o|o.cell()!=blocked))),"Fallback retained obstructed cell");
    }
    all_blocked(w,&s,&plan)?;
    println!("[city-infill] site-check parcels={} candidates={} cells={} paths={} node_budget={} provenance=1 serial=1 obstruction=1 all_blocked=1",
        plan.parcels.len(),scans.candidates,scans.cells,scans.path_searches,scans.path_node_budget);
    Ok(())
}

fn all_blocked(w: &mut World, s: &Site, original: &infill::InfillPlan) -> anyhow::Result<()> {
    let base=IVec3::from(original.base);
    let mut changes=Vec::new();
    // Every six-cell building envelope intersects this lattice.
    for x in (-24..=24).step_by(4) {for z in (-24..=24).step_by(4) {
        let at=base+IVec3::new(x,1,z);
        if known(w,at)==Some(B::Air) {changes.push((at,B::Stone));}
    }}
    w.set_many(&changes);
    let mut plan=original.clone();
    let result=infill::generate(w,s,&mut plan,&mut Scans::default());
    let preserved=changes.iter().all(|(at,block)|known(w,*at)==Some(*block));
    w.set_many(&changes.iter().map(|(at,_)|(*at,B::Air)).collect::<Vec<_>>());
    ensure!(result.is_err() && preserved && plan.pending().is_none(),"All-blocked survey admitted or cleared land");
    ensure!(plan.retry_at==w.farming.clock.saturating_add(infill::RETRY),"Exhausted survey lacks bounded retry");
    infill::generate(w,s,&mut plan,&mut Scans::default()).map_err(anyhow::Error::msg)?;
    ensure!(plan.pending().is_some(),"Cleared obstruction did not permit replanning");
    Ok(())
}

fn round_trip(s: &Site, accepted: bool) -> anyhow::Result<()> {
    let encoded=ron::to_string(s)?;
    let decoded:Site=ron::from_str(&encoded)?;
    ensure!(decoded.valid()==accepted,"Infill saved-state validity mismatch");
    ensure!(encoded==ron::to_string(&decoded)?,"Infill serialization changed on reload");
    Ok(())
}

fn mutations(original: &Site) -> anyhow::Result<()> {
    for mode in 0..13 {
        let mut s=original.clone();
        let g=s.growth.as_mut().context("Missing growth")?;
        let plan=g.infill.as_mut().context("Missing plan")?;
        let p=plan.parcels.first_mut().context("Missing parcel")?;
        match mode {
            0=>plan.schema=2,
            1=>p.origin[0]+=1,
            2=>p.turn=!p.turn,
            3=>p.stairs_right=!p.stairs_right,
            4=>p.footprint[0]-=1,
            5=>p.entrance[1]+=1,
            6=>p.access.reverse(),
            7=>p.anchor=[8,0],
            8=>p.provenance^=1,
            9=>p.project=Some(7),
            10=>g.version=1,
            11=>g.complete=true,
            _=>g.proposal.as_mut().context("Missing proposal")?.cells.reverse(),
        }
        round_trip(&s,false)?;
    }
    let mut s=original.clone();
    let plan=s.growth.as_mut().and_then(|g|g.infill.as_mut()).context("Missing plan")?;
    plan.parcels.push(plan.parcels[0].clone());
    round_trip(&s,false)?;
    Ok(())
}

fn serial(w: &World, original: &Site) -> anyhow::Result<()> {
    let mut s=original.clone();
    let count=s.growth.as_ref().and_then(|g|g.infill.as_ref()).context("Missing plan")?.parcels.len();
    for n in 0..count {
        let g=s.growth.as_mut().context("Missing growth")?;
        let plan=g.infill.as_mut().context("Missing plan")?;
        let p=plan.parcels[n].build(IVec3::from(plan.base)).map_err(anyhow::Error::msg)?;
        let first=p.cells.iter().map(|o|o.cell().y).min().context("Empty house")?;
        let terminal=p.cells.iter().map(|o|p.work_layer(*o)).max().context("Empty house")?+2;
        plan.parcels[n].project=Some(6+n);
        g.project=Some(6+n);g.proposal=None;
        s.projects.push(p);s.project_layers.push(first);
        round_trip(&s,true)?;
        let mut bad=s.clone();
        bad.growth.as_mut().context("Missing growth")?.project=None;
        round_trip(&bad,false)?;
        s.project_layers[6+n]=terminal;s.phase+=1;
        round_trip(&s,true)?;
        s.growth.as_mut().context("Missing growth")?.project=None;
        round_trip(&s,true)?;
        if n==0 && count==2 {
            let mut replanned=s.growth.as_ref().and_then(|g|g.infill.clone()).context("Missing plan")?;
            let paid=replanned.parcels[0].clone();
            infill::generate(w,&s,&mut replanned,&mut Scans::default()).map_err(anyhow::Error::msg)?;
            ensure!(replanned.parcels[0]==paid && replanned.valid(&s),"Second-home replan changed or overlapped paid history");
        }
    }
    if count==2 {
        s.growth.as_mut().context("Missing growth")?.complete=true;
        round_trip(&s,true)?;
        s.projects.swap(6,7);round_trip(&s,false)?;
    }
    Ok(())
}
