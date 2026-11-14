//! Reproduce the carrier stalled against a workbench after checkpoint reload.
use super::*;
use crate::world::adventure::wildlife::vehicle;

pub(super) fn check(root:&Path)->Result<()> {
    let mut w=World::with_seed_in(crate::world::WorldType::Superflat,crate::fluid::VoxelScalePreset::OneMeter,424242,&root.join("step-fixture"));
    let mut p=Player::new();scene::install(&mut w,&mut p).map_err(anyhow::Error::msg)?;
    let pile=IVec3::from(w.farming.jobs.stockpile.context("Missing step-fixture stockpile")?);
    let feet=pile.as_vec3()+Vec3::new(0.452564,0.000555,2.400808);
    let actor=w.adventure.enemies.iter_mut().find(|e|vehicle::villager_id(e)==Some(0))
        .context("Missing step-fixture resident")?;
    actor.pos=feet;actor.home=feet;
    w.adventure.enemies.retain(|e|vehicle::villager_id(e)==Some(0));
    let before=production::count(&w.block_data.chest(pile),Item::Block(B::Log));
    let j=&mut w.farming.jobs;
    j.helpers.clear();j.production=None;j.quarter=None;j.build.clear();j.mine.clear();j.claims.clear();
    j.worker=Some(0);j.worker_pos=Some(feet.to_array());j.shopping=None;j.sowing=None;j.building_material=false;
    j.carry=Some(crate::world::inventory::ItemStack::new(Item::Block(B::Log),1));
    j.reset_task();j.task=Some(jobs::Task::Store(pile));
    j.path.push_back(jobs::metres(pile+IVec3::Y+IVec3::Z,1.0));
    let mut sim=WorldSimulation::new();sim.reset_for_world(w.voxel_scale);
    let mut day=DayNightCycle::new(0.4,1200.0);let mut input=Input::default();
    let mut blocks=HashSet::new();let mut chunks=HashSet::new();let mut top=feet.y;let mut elapsed=0;
    for tick in 0..1200 {
        ensure!(sim.prepare_step(),"Step fixture cannot advance");
        fixed::gameplay(&mut w,&mut p,&mut input,false,|_,_,_,_|fixed::Actions::default());
        fixed::finish(&mut sim,&mut w,&p,&mut day,&mut blocks,&mut chunks,Budget::UNBOUNDED,true,true);
        blocks.clear();chunks.clear();w.adventure.dirty_voxels.clear();
        if let Some(actor)=vehicle::villager_actor(&w,0) {top=top.max(actor.pos.y);}
        elapsed=tick+1;
        if w.farming.jobs.carry.is_none() {break;}
    }
    ensure!(w.farming.jobs.carry.is_none(),"Carrier still pinned at the workbench leading edge");
    ensure!(top>=pile.y as f32+0.8,"Carrier did not physically ascend the workbench step");
    ensure!(production::count(&w.block_data.chest(pile),Item::Block(B::Log))==before+1,
        "Step recovery lost or duplicated cargo");
    std::fs::write(root.join("carrier-step-regression.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "passed":true,"ticks":elapsed,"start":feet.to_array(),"peak_y":top,"cargo_deposited":1}))?)?;
    println!("[city-colony-check] PASS physical workbench leading-edge step and cargo deposit ticks={elapsed}");
    Ok(())
}
