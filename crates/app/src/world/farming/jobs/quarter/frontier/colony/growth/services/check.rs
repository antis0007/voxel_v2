use super::*;
use anyhow::{Context,ensure};

pub(crate) fn check(w: &mut World) -> anyhow::Result<serde_json::Value> {
    let s=w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).context("Missing service site")?.clone();
    let g=s.growth.as_ref().context("Missing service growth")?;
    ensure!(g.version==4 && s.valid(),"Service checkpoint invalid");
    roundtrip(&s,true)?;
    for mode in 0..13 {
        let mut bad=s.clone();let g=bad.growth.as_mut().context("Missing growth")?;
        let service=g.services.as_mut().context("Missing service state")?;
        match mode {
            0=>g.version=3,
            1=>g.complete=!g.complete,
            2=>g.stockpile[0]+=1,
            3=>service.base_project_count=5,
            4=>service.cursor=plan::candidate_count()+1,
            5=>service.attempts=ATTEMPT_LIMIT+1,
            6=>service.targets.woodlot.original=257,
            7=>service.targets.bills[0].target.managed=Some(4097),
            8=>service.home_project=Some(100),
            9=>g.infill.as_mut().context("Missing prefix")?.schema=1,
            10=>g.infill.as_mut().context("Missing prefix")?.surveyed=false,
            11=>service.stage=if service.stage==Stage::Complete {Stage::HubMaterials}else{Stage::Complete},
            _=>service.reserved_budget.path_node_budget+=1,
        }
        roundtrip(&bad,false)?;
    }
    let production=w.farming.jobs.production.as_ref().context("Missing production")?.clone();
    let mut targets=targets::Targets::capture(&production).map_err(anyhow::Error::msg)?;
    let mut trial=production.clone();
    let slots=w.block_data.chest(g.stockpile.into());
    targets.apply(&mut trial,256,1024,1024).map_err(anyhow::Error::msg)?;
    targets.restore(&mut trial).map_err(anyhow::Error::msg)?;
    ensure!(ron::to_string(&trial)?==ron::to_string(&production)?,"Service targets did not restore");
    ensure!(w.block_data.chest(g.stockpile.into())==slots,"Funding probe changed stock");
    let service=g.services.as_ref().context("Missing service state")?;
    let mut no_demand=g.clone();let mut trial_site=s.clone();
    if matches!(service.stage,Stage::Survey|Stage::HubMaterials|Stage::HomeMaterials) {
        let before=ron::to_string(&trial_site.projects)?;
        let saved=w.farming.jobs.production.clone();
        let result=step(w,&mut trial_site,0,&mut no_demand);
        w.farming.jobs.production=saved;
        ensure!(result==Ok("Housing Supplied".into()) && before==ron::to_string(&trial_site.projects)?,"No-demand probe admitted work");
    }
    if let Some(plan)=&service.plan {
        for mode in 0..5 {
            let mut bad=s.clone();let p=bad.growth.as_mut().and_then(|g|g.services.as_mut())
                .and_then(|s|s.plan.as_mut()).context("Missing service plan")?;
            match mode {0=>p.provenance^=1,1=>p.origin[0]+=1,2=>p.primary_access[0][0]+=1,
                3=>p.capacity=4,_=>p.anchor_project=service.base_project_count};
            roundtrip(&bad,false)?;
        }
        let home=plan::home_project(plan).map_err(anyhow::Error::msg)?;
        let bill=funding::requirements(&home).map_err(anyhow::Error::msg)?;
        ensure!(bill.iter().map(|(_,n)|*n as usize).sum::<usize>()==home.cells.len()
            && !funding::paid(&bill,&[]),"Service funding omitted materials");
    }
    Ok(serde_json::json!({"stage":format!("{:?}",service.stage),"roundtrip":true,
        "corrupt_state_rejected":13,"target_restore":true,"stock_unchanged":true,
        "reserved_budget":service.reserved_budget,
        "per_call_reserved_caps":{"path_nodes":1100,"chunk_admissions":1},
        "plan_checked":service.plan.is_some(),"base_projects":service.base_project_count}))
}

fn roundtrip(s: &Site, expected: bool) -> anyhow::Result<()> {
    let bytes=ron::to_string(s)?;let restored:Site=ron::from_str(&bytes)?;
    ensure!(restored.valid()==expected,"Service saved-state validation mismatch");
    ensure!(ron::to_string(&restored)?==bytes,"Service save changed on round trip");
    Ok(())
}
