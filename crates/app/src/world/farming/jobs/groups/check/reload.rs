use super::*;
use crate::world::{storage,DayNightCycle};

pub(super) fn full(root:&Path,w:&mut World,p:&mut crate::player::Player,orders:&[Order]) -> Result<()> {
    ensure!(storage::save_directory() == root.join("disposable-saves"),"Group reload requires disposable save root");
    ensure!(!storage::quicksave_path().exists(),"Group reload requires a fresh save");
    let chunks:Vec<_>=w.chunks.keys().copied().collect();
    let jobs=ron::to_string(&w.farming.jobs)?;
    let roster=crew::saved_workers(w);
    let pile=IVec3::from(w.farming.jobs.stockpile.context("Missing group stockpile")?);
    let stock=w.block_data.chest(pile);
    let cells:Vec<_>=orders.iter().map(|o|(o.cell(),w.get(o.cell()))).collect();
    let day=DayNightCycle::new(0.4,1200.0);
    storage::set_autosave_enabled(false);
    let mut saves=storage::SaveService::new();saves.request(true);
    let started=Instant::now();
    while saves.busy() {
        saves.tick(Some(w),p,&day);
        ensure!(!saves.failed,"Group checkpoint write failed: {:?}",saves.status);
        ensure!(started.elapsed().as_secs() < 30,"Group checkpoint timed out");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let mut loaded=storage::load_review_checkpoint()?;
    for coord in &chunks {ensure!(loaded.world.admit_edit_chunk(*coord),"Missing restored group chunk");}
    ensure!(ron::to_string(&loaded.world.farming.jobs)? == jobs,"Full reload changed group/jobs ledger");
    ensure!(crew::saved_workers(&loaded.world) == roster,"Full reload changed worker identities");
    ensure!(loaded.world.block_data.chest(pile) == stock,"Full reload changed stockpile");
    ensure!(cells.iter().all(|(at,block)|loaded.world.get(*at) == *block),"Full reload changed paid blocks");
    *w=loaded.world;*p=loaded.player;
    std::fs::write(root.join("full-reload.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "passed":true,"residents":roster.len(),"paid_cells_checked":cells.len(),"group_jobs_exact":true,
        "physical_stock_exact":true,"elapsed_ms":started.elapsed().as_secs_f64()*1000.0}))?)?;
    println!("[group-check] PASS full world checkpoint preserves group identity, individual consent, workers and paid cells");
    Ok(())
}
