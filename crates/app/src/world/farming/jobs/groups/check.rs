//! Native checks use the same decision and physical transfer functions.
use super::*;
use anyhow::{Context,Result,ensure};
use std::{path::Path,time::Instant};
mod policy;
mod reload;

pub(crate) fn run(root:&Path) -> Result<()> {
    ensure!(crate::world::storage::save_directory() == root.join("disposable-saves"),"Group check requires disposable save root");
    std::fs::create_dir_all(root)?;
    policy::run()?;
    conservation(root)?;
    autonomous(root)?;
    Ok(())
}

fn conservation(root:&Path) -> Result<()> {
    use crate::world::{block_data,inventory::{Item,ItemStack}};
    let mut w=World::with_seed_in(crate::world::WorldType::Superflat,crate::fluid::VoxelScalePreset::OneMeter,8,&root.join("ledger"));
    let y=engine_worldgen::terrain_height(&w.sample_config(),0,0)+3;
    let pile=IVec3::new(1,y,1);
    w.set(pile,B::Chest);
    let mut stock=ItemStack::new(Item::Block(B::Log),3);
    block_data::store_hauled(&mut w,pile,&mut stock);
    let orders:Vec<_>=(4..7).map(|x|Order::new(IVec3::new(x,y,4),B::Log)).collect();
    for o in &orders {w.set(o.cell()-IVec3::Y,B::Stone);w.set(o.cell(),B::Air);}
    let mut j=Jobs {worker:Some(0),stockpile:Some(pile.to_array()),build:orders.clone(),..Jobs::default()};
    j.groups=policy::formed(model::Goal::at(orders[0].at))?;
    super::super::construction::fetch(&mut w,&mut j,pile,orders[0]);
    ensure!(j.carry.is_some_and(|s|s.count == 3),"Missing paid cargo");
    w.farming.jobs=j;
    let bytes=crate::world::farming::sidecar::encode(&w);
    let (restored,_)=crate::world::farming::sidecar::decode(&bytes)?;
    ensure!(restored.jobs.groups == w.farming.jobs.groups,"Group identity reload mismatch");
    w.farming=restored;
    let mut j=std::mem::take(&mut w.farming.jobs);
    super::super::construction::place(&mut w,&mut j,orders[0],Vec3::splat(-100.0));
    let carry=j.carry;
    let blocks=j.groups.circles[0].paid_blocks;
    ensure!(blocks == 1,"Physical commit not attributed to circle");
    super::super::construction::place(&mut w,&mut j,orders[0],Vec3::splat(-100.0));
    ensure!(j.carry == carry && j.groups.circles[0].paid_blocks == blocks,"Repeated commit duplicated contribution");
    j.groups.leave(0,10_000);
    ensure!(j.carry == carry && j.claims.len() == 2,"Leaving a circle mutated cargo/claims");
    let mut returned=j.carry.take().context("Missing returned cargo")?;
    block_data::store_hauled(&mut w,pile,&mut returned);
    ensure!(returned.count == 0 && w.block_data.chest(pile).iter().flatten().map(|s|s.count).sum::<u32>() == 2,
        "Group exit duplicated or lost physical goods");
    println!("[group-check] PASS paid cargo, group sidecar, repeated placement and exit conservation");
    Ok(())
}

fn autonomous(root:&Path) -> Result<()> {
    use crate::world::{adventure::wildlife::vehicle,block_data,inventory::{Item,ItemStack}};
    use crate::{player::{Input,Player},simulation::fixed};
    let mut w=World::with_seed_in(crate::world::WorldType::Superflat,crate::fluid::VoxelScalePreset::OneMeter,424242,&root.join("disposable-saves"));
    w.rules.mob_spawning=false;
    let y=engine_worldgen::terrain_height(&w.sample_config(),0,0)+2;
    let mut edits=Vec::new();
    for x in -8..=22 {for z in -8..=12 {
        edits.push((IVec3::new(x,y-1,z),B::Stone));
        for h in 0..5 {edits.push((IVec3::new(x,y+h,z),B::Air));}
    }}
    w.set_many(&edits);
    let pile=IVec3::new(0,y,0);w.set(pile,B::Chest);
    let orders:Vec<_>=(8..=20).flat_map(|x|(3..=10).map(move|z|Order::new(IVec3::new(x,y,z),B::Log))).collect();
    let mut stock=ItemStack::new(Item::Block(B::Log),orders.len() as u32);
    block_data::store_hauled(&mut w,pile,&mut stock);
    ensure!(stock.count == 0,"Native fixture storage too small");
    for id in 0..8u32 {
        let feet=super::super::metres(IVec3::new((id%4) as i32-3,y,(id/4) as i32-3),1.0);
        vehicle::spawn_resident(&mut w,id,feet);w.travel.homes.insert(id,feet);
        if id == 0 {super::super::assign(&mut w,id);} else {ensure!(crew::add(&mut w,id),"Native fixture roster");}
        super::super::set_priorities(&mut w,id,[0,0,1]);
    }
    w.farming.jobs.stockpile=Some(pile.to_array());w.farming.jobs.build=orders.clone();
    w.farming.jobs.groups.enabled=true;
    let mut p=Player::new();p.pos=Vec3::new(-5.5,y as f32,-5.5);p.precise_pos=p.pos.as_dvec3();
    let mut input=Input::default();let mut samples=Vec::new();let mut activity=std::collections::BTreeSet::new();
    let mut reloaded=false;
    for step in 0..24_000 {
        let start=Instant::now();
        fixed::gameplay(&mut w,&mut p,&mut input,false,|_,_,_,_|fixed::Actions::default());
        samples.push(start.elapsed().as_secs_f64()*1000.0);
        if step%120 == 0 {
            ensure!(w.farming.jobs.valid(),"Invalid autonomous state at {step}");
            for (id,_) in crew::saved_workers(&w) {
                if crew::view(&w.farming.jobs,id).is_some_and(|a|a.routine.productive > 0) {activity.insert(id);}
            }
            if !reloaded && w.farming.jobs.groups.accepted > 0 {
                let bytes=crate::world::farming::sidecar::encode(&w);
                let (f,_)=crate::world::farming::sidecar::decode(&bytes)?;
                ensure!(f.jobs.groups == w.farming.jobs.groups,"Autonomous group reload mismatch");
                w.farming=f;reloaded=true;
            }
        }
        if w.farming.jobs.build.is_empty() || step > 4000 && reloaded
            && w.farming.jobs.groups.circles.iter().any(|g|g.paid_blocks >= 4) {break;}
        w.adventure.dirty_voxels.clear();
    }
    let built=orders.iter().filter(|o|known(&w,o.cell()) == Some(o.block())).count();
    let chest=w.block_data.chest(pile).iter().flatten().map(|s|s.count).sum::<u32>();
    let cargo=crew::cargo(&w.farming.jobs);
    ensure!(built as u32+chest+cargo == orders.len() as u32,"Autonomous goods not conserved");
    samples.sort_by(f64::total_cmp);
    let p95=samples[samples.len()*95/100];let p99=samples[samples.len()*99/100];
    ensure!(w.farming.jobs.groups.scanned_orders <= w.farming.jobs.groups.reviews*16,"Candidate work exceeded review budget");
    ensure!(w.farming.jobs.groups.goal_checks <= w.farming.jobs.groups.reviews*256,"Goal membership work exceeded review budget");
    reload::full(root,&mut w,&mut p,&orders)?;
    let groups=report(&w);
    let result=serde_json::json!({"roster":8,"productive_agents":activity,"steps":samples.len(),"paid_blocks":built,
        "chest":chest,"cargo":cargo,"group_reload":reloaded,"gameplay_p95_ms":p95,"gameplay_p99_ms":p99,"groups":groups});
    std::fs::write(root.join("autonomous-groups.json"),serde_json::to_vec_pretty(&result)?)?;
    println!("[group-check] active={} built={built} circles={} accepted={} p95={p95:.3}ms p99={p99:.3}ms",
        activity.len(),w.farming.jobs.groups.circles.len(),w.farming.jobs.groups.accepted);
    ensure!(activity.len() >= 6 && built >= 32,"Insufficient physical worker activity");
    ensure!(w.farming.jobs.groups.accepted > 0 && reloaded,"No autonomous proposal accepted");
    ensure!(w.farming.jobs.groups.circles.iter().any(|g|g.paid_blocks > 0),"No paid group work observed");
    ensure!(w.farming.jobs.groups.selections > 0,"No shared-goal task selection");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test] fn voluntary_group_policy_replays() { super::policy::run().unwrap(); }
    #[test] fn voluntary_group_cargo_reloads() {
        super::conservation(&std::env::temp_dir().join("axiom-group-cargo-tests")).unwrap();
    }
}
