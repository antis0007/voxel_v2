//! Read-only correspondence between completed project claims and loaded cells.
use super::*;

pub(super) fn run(root:&Path,w:&mut World)->Result<()> {
    let phase=site(w)?.phase;
    let projects=site(w)?.projects.iter().map(|p|(p.name.clone(),p.cells.clone())).collect::<Vec<_>>();
    let chunks=projects.iter().flat_map(|(_,cells)|cells.iter().map(|o|{
        let c=crate::world::chunk_coord_from_world_block(o.cell());(c.x,c.y,c.z)
    })).collect::<BTreeSet<_>>();
    for (x,y,z) in chunks {ensure!(w.admit_edit_chunk(IVec3::new(x,y,z)),"Recovery audit chunk unavailable");}
    let mut missing_claimed=0usize;
    let rows=projects.iter().enumerate().map(|(i,(name,cells))| {
        let matching=cells.iter().filter(|o|known(w,o.cell())==Some(o.block())).count();
        let unknown=cells.iter().filter(|o|known(w,o.cell()).is_none()).count();
        let claimed_complete=i<phase;
        if claimed_complete {missing_claimed+=cells.len().saturating_sub(matching);}
        serde_json::json!({"project":i,"name":name,"claimed_complete":claimed_complete,
            "cells":cells.len(),"matching_cells":matching,"unknown_cells":unknown})
    }).collect::<Vec<_>>();
    let report=serde_json::json!({"loaded":true,"phase":phase,"projects":rows,
        "completed_claim_cells_missing":missing_claimed,"consistent_complete_geometry":missing_claimed==0,
        "food_balance":food_balance(w),"timber_balance":ledger::timber(w),
        "scope":"Loaded a copied checkpoint without simulation or saving; checks completed construction claims against its referenced physical cells, not all save invariants"});
    std::fs::write(root.join("checkpoint-recovery-audit.json"),serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
