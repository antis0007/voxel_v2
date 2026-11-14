//! Native probes preserve physical stock while testing saved search state.
use super::*;
use anyhow::{Context,ensure};
#[path="repeatable_negative.rs"] mod negative;

fn valid_roundtrip(s:&Site,plan:&infill::InfillPlan,expected:bool)->anyhow::Result<()> {
    let mut trial=s.clone();
    let g=trial.growth.as_mut().context("Missing growth")?;
    g.infill=Some(plan.clone());
    let bytes=ron::to_string(&trial)?;let restored:Site=ron::from_str(&bytes)?;
    ensure!(restored.valid()==expected,"Repeatable saved search validity mismatch");
    ensure!(ron::to_string(&restored)?==bytes,"Repeatable search changed on sidecar round trip");
    Ok(())
}

fn solve(w:&World,s:&Site,plan:&mut infill::InfillPlan)->anyhow::Result<(bool,Scans)> {
    let mut scans=Scans::default();
    for _ in 0..2000 {
        let previous=scans.clone();
        let result=infill::poll(w,s,plan,&mut scans);
        ensure!(scans.candidates-previous.candidates<=64 && scans.path_searches-previous.path_searches<=2
            && scans.path_node_budget-previous.path_node_budget<=40002,"Probe exceeded per-poll budget");
        valid_roundtrip(s,plan,true)?;
        match result {Ok(true)=>return Ok((true,scans)),Ok(false)=>{},Err(e)=>{
            ensure!(e.starts_with("No Space"),"Unexpected repeatable survey result: {e}");
            return Ok((false,scans));
        }}
    }
    anyhow::bail!("Repeatable probe did not finish within 2000 bounded polls")
}

pub(super) fn check(w:&mut World)->anyhow::Result<()> {
    let s=w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).context("Missing site")?.clone();
    let g=s.growth.as_ref().context("Missing repeatable state")?;
    ensure!(g.version==3 && s.projects.len()==6,"Repeatable probes require unfunded six-house bootstrap");
    let original=g.infill.clone().context("Missing repeatable plan")?;
    let mut plan=original.clone();let mut scans=Scans::default();
    ensure!(!infill::poll(w,&s,&mut plan,&mut scans).map_err(anyhow::Error::msg)?,"First bounded poll admitted before ranking");
    ensure!(plan.searching() && scans.candidates==64,"Candidate sweep did not preserve bounded cursor");
    valid_roundtrip(&s,&plan,true)?;
    for mode in 0..4 {
        let mut bad=serde_json::to_value(&plan)?;
        match mode {
            0=>bad["search"]["cursor"]=serde_json::json!(9605),
            1=>bad["search"]["routing"]=serde_json::json!(true),
            2=>bad["schema"]=serde_json::json!(1),
            _=>bad["surveyed"]=serde_json::json!(false),
        }
        valid_roundtrip(&s,&serde_json::from_value(bad)?,false)?;
    }
    let (ready,total)=solve(w,&s,&mut plan)?;
    ensure!(ready && plan.parcels.len()==1,"Repeatable search did not select exactly one parcel");
    let parcel=plan.pending().context("Missing selected parcel")?;
    infill::revalidate(w,&s,&plan,parcel,&mut Scans::default()).map_err(anyhow::Error::msg)?;
    let project=parcel.build(IVec3::from(plan.base)).map_err(anyhow::Error::msg)?;
    negative::check(w,&s,&plan,&project)?;
    let cell=project.cells.first().context("Empty proposed house")?.cell();
    let previous=known(w,cell).context("Unknown proposal terrain")?;
    w.set(cell,B::Stone);
    let rejected=infill::revalidate(w,&s,&plan,parcel,&mut Scans::default()).is_err();
    let preserved=known(w,cell)==Some(B::Stone);w.set(cell,previous);
    ensure!(rejected&&preserved,"Pre-funding obstruction admitted or erased");
    blocked(w,&s,&original)?;
    disconnected(w,&s,&original)?;
    println!("[city-repeatable] PASS candidates={} paths={} cursor_reload=1 invalid_cursor=1 obstruction=1 cooldown=1",total.candidates,total.path_searches);
    Ok(())
}

fn blocked(w:&mut World,s:&Site,original:&infill::InfillPlan)->anyhow::Result<()> {
    let base=IVec3::from(original.base);let mut changes=Vec::new();
    for x in (-24..=24).step_by(4) {for z in (-24..=24).step_by(4) {
        let at=base+IVec3::new(x,1,z);
        if known(w,at)==Some(B::Air) {changes.push((at,B::Stone));}
    }}
    w.set_many(&changes);
    let result=(|| {
        let mut plan=original.clone();let (ready,mut scans)=solve(w,s,&mut plan)?;
        ensure!(!ready && !plan.searching() && plan.pending().is_none(),"Blocked land admitted a parcel");
        ensure!(changes.iter().all(|(at,b)|known(w,*at)==Some(*b)),"Survey changed blocked land");
        let before=serde_json::to_value(&scans)?;
        ensure!(!infill::poll(w,s,&mut plan,&mut scans).map_err(anyhow::Error::msg)?
            && serde_json::to_value(&scans)?==before,"No-space retry rescanned before its fixed deadline");
        valid_roundtrip(s,&plan,true)?;Ok(())
    })();
    w.set_many(&changes.iter().map(|(at,_)|(*at,B::Air)).collect::<Vec<_>>());
    result?;
    let mut plan=original.clone();let (ready,_)=solve(w,s,&mut plan)?;
    ensure!(ready,"Available ground did not permit another survey");Ok(())
}


fn disconnected(w:&mut World,s:&Site,original:&infill::InfillPlan)->anyhow::Result<()> {
    let base=IVec3::from(original.base);
    let cells:Vec<_>=s.projects.iter().flat_map(|p|&p.cells)
        .filter(|o|o.cell().y==base.y+1 && o.block()==B::Planks)
        .map(|o|o.cell()+IVec3::Y).filter(|at|known(w,*at)==Some(B::Air)).collect();
    w.set_many(&cells.iter().map(|at|(*at,B::Stone)).collect::<Vec<_>>());
    let mut plan=original.clone();let mut scans=Scans::default();
    let result=infill::poll(w,s,&mut plan,&mut scans);
    let preserved=cells.iter().all(|at|known(w,*at)==Some(B::Stone));
    let before=serde_json::to_value(&scans);
    let waiting=infill::poll(w,s,&mut plan,&mut scans);
    w.set_many(&cells.iter().map(|at|(*at,B::Air)).collect::<Vec<_>>());
    ensure!(result.is_err_and(|e|e=="Street Disconnected: No Paid Frontage") && preserved
        && waiting==Ok(false) && before?==serde_json::to_value(&scans)?,
        "Missing paid frontage was misreported, changed or repeatedly scanned");
    valid_roundtrip(s,&plan,true)?;Ok(())
}
