use super::super::*;
use anyhow::{Context, Result, ensure};
use crate::{player::{Input, Player}, simulation::{Budget, WorldSimulation, fixed}, world::{DayNightCycle, WorldType}};
use std::{collections::HashSet, path::Path};

pub(in super::super) fn run(root: &Path) -> Result<()> {
    std::fs::create_dir_all(root)?;
    let mut w=World::with_seed_in(WorldType::Superflat,crate::fluid::VoxelScalePreset::OneMeter,424242,root);
    let mut p=Player::new();scene::install(&mut w,&mut p).map_err(anyhow::Error::msg)?;
    super::super::check::transaction_checks(&w)?;
    let pile=IVec3::from(w.farming.jobs.stockpile.context("Missing stockpile")?);
    let base=pile-IVec3::Y;
    let roots:Vec<_>=w.farming.jobs.production.as_ref().context("Missing production")?.beds.iter()
        .filter(|b|b.crop==B::Sapling).map(|b|IVec3::from(b.at)).collect();
    ensure!(roots.len()==2,"Woodlot fixture must have two real plots");
    let residents=w.farming.jobs.production.as_ref().context("Missing production")?.workers.len();
    ensure!(residents==12&&super::super::super::crew::saved_workers(&w).len()==12,"Expected twelve physical workers");
    let initial_seeds=count(&w.block_data.chest(pile),Item::Block(B::CarrotCrop))+count(&w.block_data.chest(pile),Item::Block(B::BeetCrop));
    let orders=w.farming.jobs.build.clone();
    let initial_logs=world_logs(&w,base);
    let initial_player_wood=w.inventory.count_item(Item::Block(B::Log));
    let food_bill=&w.farming.jobs.production.as_ref().context("Missing production")?.bills[0];
    ensure!(craft_ready(&w,&w.farming.jobs,pile,food_bill).is_none(),"Cooking consumed planting reserves");
    let mut sim=WorldSimulation::new();sim.reset_for_world(w.voxel_scale);
    let mut day=DayNightCycle::new(0.4,1200.0);
    let mut input=Input::default();let mut blocks=HashSet::new();let mut chunks=HashSet::new();
    let mut grown=0u32;let mut carried_seed_reload=false;let mut felling_reload=false;
    let started=std::time::Instant::now();
    for step in 0..360_000 {
        ensure!(sim.prepare_step(),"Simulation authority could not advance");
        let growing:Vec<_>=roots.iter().copied().filter(|p|w.farming.plots.contains_key(&(p.x,p.y,p.z))).collect();
        fixed::gameplay(&mut w,&mut p,&mut input,false,|_,_,_,_|fixed::Actions::default());
        for p in growing { if w.get(p)==B::Log && !w.farming.plots.contains_key(&(p.x,p.y,p.z)) { grown+=1; } }
        fixed::finish(&mut sim,&mut w,&p,&mut day,&mut blocks,&mut chunks,Budget::UNBOUNDED,true,true);
        blocks.clear();chunks.clear();w.adventure.dirty_voxels.clear();
        let j=&w.farming.jobs;
        let seed=j.sowing.is_some_and(|b|b.crop==B::Sapling);
        let falling=w.sandbox.bodies.iter().any(|b|b.felled_tree);
        if (seed&&!carried_seed_reload)||(falling&&!felling_reload) {
            let bytes=farming::sidecar::encode(&w);let before=ron::to_string(j)?;
            let (state,_)=farming::sidecar::decode(&bytes)?;
            ensure!(ron::to_string(&state.jobs)?==before,"Woodlot jobs changed in sidecar reload");
            std::fs::write(root.join(if seed {"carried-sapling.vxf"} else {"felling-state.vxf"}),bytes)?;
            w.farming=state;carried_seed_reload|=seed;felling_reload|=falling;
        }
        let j=&w.farming.jobs;let s=j.production.as_ref().context("Lost production")?;
        let f=s.woodlot.as_ref().context("Lost woodlot")?;
        if step%12_000==0 { println!("[woodlot] step={step} grown={grown} planted={} felled={} bodies={} logs={} {} {}",f.planted,f.felled,
            w.sandbox.bodies.len(),count(&w.block_data.chest(pile),Item::Block(B::Log)),summary(&w).unwrap_or_default(),super::super::super::status(&w)); }
        if f.felled>=3 && f.planted>=4 && j.build.is_empty() && !falling && s.served.len()==residents&&s.served.values().all(|n|*n>=3)
            && count(&w.block_data.chest(pile),Item::Block(B::Log))>=4 { break; }
    }
    let j=&w.farming.jobs;let s=j.production.as_ref().context("Missing production")?;
    let f=s.woodlot.as_ref().context("Missing woodlot")?;let slots=w.block_data.chest(pile);
    let stored=|item|count(&slots,item)+super::super::super::crew::cargo_item(j,item)
        +w.inventory.dropped.iter().filter(|d|d.stack.item==item).map(|d|d.stack.count).sum::<u32>();
    let details=serde_json::json!({"jobs":j,"stock":slots,"grown":grown,"world_logs":world_logs(&w,base),
        "bodies":w.sandbox.bodies.len(),"dropped":w.inventory.dropped.iter().map(|d|format!("{:?} {:?}",d.pos,d.stack)).collect::<Vec<_>>()});
    std::fs::write(root.join("final-state.json"),serde_json::to_vec_pretty(&details)?)?;
    ensure!(j.valid()&&carried_seed_reload&&felling_reload,"Woodlot persistence checkpoints missing");
    ensure!(f.felled>=3&&f.planted>=4&&grown>=3,"No repeated plant-grow-fell cycle");
    ensure!(w.sandbox.bodies.iter().all(|b|!b.felled_tree),"Tree physics did not finish");
    ensure!(j.build.is_empty()&&orders.iter().all(|o|w.get(o.cell())==o.block()),"Paid shelter did not finish");
    ensure!(s.next_meal.len()==residents&&s.served.len()==residents&&s.served.values().all(|n|*n>=3),"All twelve residents must repeatedly eat");
    let timber=world_logs(&w,base)+stored(Item::Block(B::Log));
    let planks=orders.iter().filter(|o|o.block()==B::Planks).count() as u32+stored(Item::Block(B::Planks));
    let chests=orders.iter().filter(|o|o.block()==B::Chest).count() as u32+stored(Item::Block(B::Chest));
    ensure!((initial_logs+grown*4)*4==timber*4+planks+chests*8,"Grown timber, pickups, cargo, recipes and placement do not balance");
    let produce=stored(Item::Block(B::CarrotCrop))+stored(Item::Block(B::BeetCrop));
    ensure!(initial_seeds+s.counts.harvested==s.counts.planted+2*s.counts.meals+produce,"Crop/seed/meal accounting");
    ensure!(stored(s.meal)+s.counts.eaten==s.counts.meals,"Meal accounting");
    ensure!(w.inventory.count_item(Item::Block(B::Log))==initial_player_wood&&w.inventory.count_item(s.meal)==0,"Resident goods reached player inventory");
    let result=serde_json::json!({"passed":true,"active_residents":residents,"aggregate_residents":0,"steps":w.farming.clock,
        "wall_seconds":started.elapsed().as_secs_f64(),"tree_plots":roots.len(),"grown_trees":grown,"woodlot":f,"food":s.counts,
        "paid_placements":orders.len(),"served_per_resident":s.served,"standing_logs":world_logs(&w,base),"stored_logs":count(&slots,Item::Block(B::Log)),
        "timber_conserved":true,"food_conserved":true,"carried_sapling_reload":carried_seed_reload,"felling_sidecar_reload":felling_reload,
        "limits":"Two designated woodlot plots; finite initial saplings plus actual leaf drops; no general forest survey; no guarantee of indefinite seed supply; twelve physical residents; no multiplayer"});
    std::fs::write(root.join("woodlot-check.json"),serde_json::to_vec_pretty(&result)?)?;
    println!("[woodlot] PASS {result}");Ok(())
}

fn world_logs(w:&World,base:IVec3)->u32 {
    let mut n=0;
    for z in -18..=18 {for x in -18..=18 {for y in 0..=9 {n+=u32::from(w.get(base+IVec3::new(x,y,z))==B::Log);}}}
    n
}
