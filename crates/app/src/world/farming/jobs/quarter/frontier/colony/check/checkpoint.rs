use super::*;
use crate::world::storage;

pub(super) fn reload(root:&Path,w:&mut World,p:&mut Player,day:&mut DayNightCycle,fresh:bool)->Result<()> {
    ensure!(storage::save_directory()==root.join("disposable-saves"),"Checkpoint must use this run's disposable save root");
    ensure!(!fresh||!storage::quicksave_path().exists(),"Checkpoint requires a fresh disposable save root");
    let resident_chunks:Vec<_>=w.chunks.keys().copied().collect();
    let jobs=ron::to_string(&w.farming.jobs)?;
    let travel=crate::world::travel::sidecar::encode(&w.travel);
    crate::world::travel::sidecar::decode(&travel)?;
    let roster=jobs::crew::saved_workers(w);
    let pile=IVec3::from(w.farming.jobs.stockpile.context("Missing stockpile")?);
    let stock=w.block_data.chest(pile);
    let cells:Vec<_>=site(w)?.projects.iter().flat_map(|p|p.cells.iter().map(|o|(o.cell(),w.get(o.cell())))).collect();
    storage::set_autosave_enabled(false);
    let mut saves=storage::SaveService::new();saves.request(true);let started=Instant::now();
    while saves.busy() {
        saves.tick(Some(w),p,day);
        ensure!(!saves.failed,"Checkpoint write failed: {:?}",saves.status);
        ensure!(started.elapsed().as_secs()<30,"Checkpoint timed out");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let mut loaded=storage::load_review_checkpoint()?;
    // The live streamer admits the visible working set after load. This
    // headless driver has no camera streamer, so restore its prior residency.
    for coord in &resident_chunks {
        ensure!(loaded.world.admit_edit_chunk(*coord),"Checkpoint working chunk could not be admitted: {coord:?}");
    }
    ensure!(crate::world::travel::sidecar::encode(&loaded.world.travel)==travel,"Resident homes changed");
    ensure!(ron::to_string(&loaded.world.farming.jobs)?==jobs,"Jobs changed across full reload");
    ensure!(jobs::crew::saved_workers(&loaded.world)==roster,"Resident identities changed");
    ensure!(loaded.world.block_data.chest(pile)==stock,"Stockpile changed");
    ensure!(cells.iter().all(|(at,block)|loaded.world.get(*at)==*block),"Construction geometry changed");
    *w=loaded.world;*p=loaded.player;*day=loaded.day_night;
    std::fs::write(root.join(if fresh {"checkpoint-reload.json"} else {"final-checkpoint-reload.json"}),serde_json::to_vec_pretty(&serde_json::json!({
        "passed":true,"residents":roster.len(),"homes":w.travel.homes.len(),"jobs_preserved":true,
        "stockpile_preserved":true,"project_cells_preserved":cells.len(),"resident_chunks_restored":resident_chunks.len(),"wall_seconds":started.elapsed().as_secs_f64()}))?)?;
    Ok(())
}
