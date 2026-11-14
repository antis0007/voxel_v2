use super::*;
use crate::{player::Player,world::DayNightCycle};

pub(super) fn full(root:&Path,w:&mut World,p:&mut Player,day:&mut DayNightCycle)->Result<()> {
    ensure!(storage::save_directory()==root.join("disposable-saves"),"Checkpoint is not disposable");
    let chunks:Vec<_>=w.chunks.keys().copied().collect();
    let jobs=ron::to_string(&w.farming.jobs)?;
    let travel=crate::world::travel::sidecar::encode(&w.travel);
    let roster=crew::saved_workers(w);
    let snapshots:Vec<_>=roster.iter().map(|&(id,saved)| {
        let actor=vehicle::villager_actor(w,id);
        serde_json::json!({"id":id,"saved_position":saved.to_array(),
            "actor_position":actor.map(|a|a.pos.to_array()),"kind":actor.map(|a|a.kind),
            "body_metres":actor.map(|a|vehicle::resident_dimensions(a.kind).to_array())})
    }).collect();
    ensure!(roster.iter().all(|(id,_)|vehicle::villager_actor(w,*id).is_some_and(|a|a.kind==vehicle::VILLAGER)),
        "Authored resident kind changed before checkpoint");
    std::fs::write(root.join("pre-rest-checkpoint-residents.json"),serde_json::to_vec_pretty(&snapshots)?)?;
    let pile=IVec3::from(w.farming.jobs.stockpile.context("Missing stockpile")?);
    let stock=w.block_data.chest(pile);
    let geometry:Vec<_>=w.farming.jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref()).into_iter()
        .flat_map(|s|s.projects.iter()).flat_map(|p|p.cells.iter()).map(|o|(o.cell(),w.get(o.cell()))).collect();
    let balances=(super::food_balance(w),super::timber_balance(w));
    storage::set_autosave_enabled(false);
    let mut saves=storage::SaveService::new();saves.request(true);let at=Instant::now();
    let mut polls=0u64;let mut maximum_tick_ms=0.0f64;
    while saves.busy() {
        let tick=Instant::now();saves.tick(Some(w),p,day);polls+=1;
        maximum_tick_ms=maximum_tick_ms.max(tick.elapsed().as_secs_f64()*1000.0);
        ensure!(!saves.failed,"Daily checkpoint write failed");
        ensure!(at.elapsed().as_secs()<30,"Daily checkpoint timed out: {} polls, maximum tick {:.3} ms; {}; {}",
            polls,maximum_tick_ms,saves.diagnostic_state(),w.region_status());
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let mut loaded=storage::load_review_checkpoint()?;
    for at in chunks {ensure!(loaded.world.admit_edit_chunk(at),"Checkpoint chunk unavailable");}
    ensure!(ron::to_string(&loaded.world.farming.jobs)?==jobs,"Full reload changed routine or inventory");
    ensure!(crate::world::travel::sidecar::encode(&loaded.world.travel)==travel,"Full reload changed homes");
    ensure!(crew::saved_workers(&loaded.world)==roster,"Full reload changed identities or positions");
    ensure!(loaded.world.block_data.chest(pile)==stock,"Full reload changed physical stock");
    ensure!(geometry.iter().all(|(at,block)|loaded.world.get(*at)==*block),"Full reload changed paid or obstructed geometry");
    ensure!((super::food_balance(&loaded.world),super::timber_balance(&loaded.world))==balances,
        "Full reload changed physical food or timber, including cargo, drops and felled bodies");
    *w=loaded.world;*p=loaded.player;*day=loaded.day_night;Ok(())
}
