//! Rejects altered parcel commitments and verifies unfunded waiting.
use super::*;

fn roundtrip(s:&Site,expected:bool)->anyhow::Result<()> {
    let bytes=ron::to_string(s)?;let restored:Site=ron::from_str(&bytes)?;
    ensure!(restored.valid()==expected,"Repeatable parcel validity mismatch");
    ensure!(ron::to_string(&restored)?==bytes,"Repeatable parcel changed on serialization");Ok(())
}

pub(super) fn check(w:&mut World,s:&Site,plan:&infill::InfillPlan,p:&Project)->anyhow::Result<()> {
    let mut proposed=s.clone();let g=proposed.growth.as_mut().context("Missing growth")?;
    g.infill=Some(plan.clone());g.proposal=Some(p.clone());roundtrip(&proposed,true)?;
    for mode in 0..14 {
        let mut bad=proposed.clone();let g=bad.growth.as_mut().context("Missing growth")?;
        let plan=g.infill.as_mut().context("Missing plan")?;
        let parcel=plan.parcels.first_mut().context("Missing parcel")?;
        match mode {
            0=>plan.schema=1,
            1=>parcel.origin[0]+=1,
            2=>parcel.turn=!parcel.turn,
            3=>parcel.stairs_right=!parcel.stairs_right,
            4=>parcel.footprint[0]-=1,
            5=>parcel.entrance[1]+=1,
            6=>parcel.access.reverse(),
            7=>parcel.anchor=[6,0],
            8=>parcel.provenance^=1,
            9=>parcel.project=Some(7),
            10=>g.version=2,
            11=>g.complete=true,
            12=>g.proposal.as_mut().context("Missing proposal")?.cells.reverse(),
            _=>bad.projects[0].cells[0].at[1]=i32::MAX,
        }
        roundtrip(&bad,false)?;
    }
    let first=p.cells.iter().map(|o|o.cell().y).min().context("Empty house")?;
    let terminal=p.cells.iter().map(|o|p.work_layer(*o)).max().context("Empty house")?+2;
    let mut funded=proposed.clone();let g=funded.growth.as_mut().context("Missing growth")?;
    g.proposal=None;g.project=Some(6);
    g.infill.as_mut().context("Missing plan")?.parcels[0].project=Some(6);
    funded.projects.push(p.clone());funded.project_layers.push(first);roundtrip(&funded,true)?;
    let mut bad=funded.clone();bad.growth.as_mut().context("Missing growth")?.project=None;roundtrip(&bad,false)?;
    bad=funded.clone();bad.growth.as_mut().context("Missing growth")?.complete=true;roundtrip(&bad,false)?;
    funded.project_layers[6]=terminal;funded.phase+=1;
    funded.growth.as_mut().context("Missing growth")?.project=None;roundtrip(&funded,true)?;
    unfunded(w,s,&proposed)?;
    println!("[city-repeatable] parcel-negatives=14 serial-ledger=1 no-demand=1 material-shortage=1");Ok(())
}

fn unfunded(w:&mut World,original:&Site,proposed:&Site)->anyhow::Result<()> {
    let saved_jobs=w.farming.jobs.clone();
    let pile=IVec3::from(saved_jobs.stockpile.context("Missing stockpile")?);
    let saved_stock=w.block_data.chest(pile);
    let result=(|| {
        let mut s=original.clone();let mut g=s.growth.take().context("Missing growth")?;
        let before=ron::to_string(&g)?;
        let status=super::super::repeated::step(w,&mut s,0,&mut g).map_err(anyhow::Error::msg)?;
        ensure!(status=="Housing Supplied" && ron::to_string(&g)?==before,"No demand performed planning or admission");
        let mut slots=saved_stock.clone();
        for slot in &mut slots {if slot.as_ref().is_some_and(|s|matches!(s.item,Item::Block(B::Planks|B::Chest))) {*slot=None;}}
        w.block_data.set_chest(pile,slots);
        let mut s=proposed.clone();let mut g=s.growth.take().context("Missing growth")?;
        let plan=ron::to_string(&g.infill)?;let scans=serde_json::to_value(&g.scans)?;
        for _ in 0..3 {
            let status=super::super::repeated::step(w,&mut s,40,&mut g).map_err(anyhow::Error::msg)?;
            ensure!(status.starts_with("Materials:") && g.project.is_none() && s.projects.len()==6
                && ron::to_string(&g.infill)?==plan && serde_json::to_value(&g.scans)?==scans,
                "Material shortage admitted work, changed a proposal or repeated a survey");
        }
        s.growth=Some(g);roundtrip(&s,true)?;Ok(())
    })();
    w.block_data.set_chest(pile,saved_stock);w.farming.jobs=saved_jobs;result
}
