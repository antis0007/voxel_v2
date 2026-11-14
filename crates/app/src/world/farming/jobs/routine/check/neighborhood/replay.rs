//! Replays an exact failing saved roster before any test interventions.
use super::*;

pub(super) fn run(root:&Path)->Result<()> {
    ensure!(storage::save_directory()==root.join("disposable-saves"),"Worker replay requires disposable saves");
    let loaded=storage::load_review_checkpoint()?;
    let (mut w,mut player,mut day)=(loaded.world,loaded.player,loaded.day_night);
    let chunks=materialize::admit(&mut w)?;
    let saved_growth=ron::to_string(&site(&w)?.growth)?;
    let saved_version=site(&w)?.growth.as_ref().map(|g|g.version);
    if saved_version.is_some() {
        super::super::super::super::enable_housing_growth(&mut w).map_err(anyhow::Error::msg)?;
        ensure!(ron::to_string(&site(&w)?.growth)?==saved_growth,"Enable silently migrated an existing growth save");
        report::sidecar(&mut w,root,"retained-growth-version.vxf")?;
    }
    let ids=w.farming.jobs.production.as_ref().context("Missing saved roster")?.workers.clone();
    ensure!(ids.len()==64,"Worker replay requires 64 residents");
    let mut pending=materialize::Pending::new(&w,0,"exact-saved-worker-replay");
    let mut simulation=WorldSimulation::new();simulation.reset_for_world(w.voxel_scale);
    let mut input=Input::default();let mut blocks=HashSet::new();let mut changed=HashSet::new();
    for step in 0..=182 {
        ensure!(simulation.prepare_step(),"Saved replay simulation unavailable");
        let mut observed=Ok(None);
        fixed::gameplay(&mut w,&mut player,&mut input,false,|hook,w,_,_| {
            if matches!(hook,fixed::Hook::BeforeAdventure) {observed=pending.observe(root,w,step);}
            fixed::Actions::default()
        });
        if let Some(materialization)=observed? {
            let mut activity=evidence::Activity::default();activity.sample(&w,&ids,step)?;
            ensure!(w.farming.jobs.valid(),"Saved replay lost commitments");
            let report=serde_json::json!({"passed":true,"chunks":chunks,"identities":ids,"materialization":materialization,"saved_growth_version":saved_version,"existing_growth_unchanged_by_enable":true});
            std::fs::write(root.join("worker-reload-check.json"),serde_json::to_vec_pretty(&report)?)?;
            println!("[infill-reload] PASS 64 saved residents, exact pose");return Ok(());
        }
        fixed::finish(&mut simulation,&mut w,&player,&mut day,&mut blocks,&mut changed,Budget::UNBOUNDED,true,true);
        blocks.clear();changed.clear();w.adventure.dirty_voxels.clear();
    }
    anyhow::bail!("Saved worker replay timed out")
}
