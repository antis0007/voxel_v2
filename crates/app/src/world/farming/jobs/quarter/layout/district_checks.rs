//! Connectivity, saved-plan rejection and native district access checks.
use super::*;
use anyhow::{Result,ensure};
use std::path::Path;

pub(super) fn run(root:&Path)->Result<()> {
    std::fs::create_dir_all(root)?;
    super::super::roof_check::matrix(root)?;
    let mut rows=Vec::new();
    for profile in 0..3 {for seed in [7,424242,8675309] {for style in 0..3 {
        let plan=district::generate(seed,profile,style,1,&scene::height).map_err(anyhow::Error::msg)?;
        ensure!(plan==district::generate(seed,profile,style,1,&scene::height).map_err(anyhow::Error::msg)?,"Nondeterministic district");
        ensure!(plan==ron::from_str::<Plan>(&ron::to_string(&plan)?)?,"District provenance changed on reload");
        ensure!(plan.parcels.iter().all(|p|p.tradition==style),"Incoherent district materials");
        ensure!(plan.design.as_ref().is_some_and(|d|d.version==2),"New roof design missing version gate");
        if style==1 {
            let mut downgraded=plan.clone();if let Some(d)=&mut downgraded.design {d.version=1;}
            ensure!(!downgraded.valid(),"Gated roof accepted by old design schema");
            for p in &mut downgraded.parcels {p.roof_access=false;}
            ensure!(downgraded.valid(),"Original closed-roof district rejected");
        }
        let mut state=super::super::State {frontier:None,layout:Some(plan.clone()),
            lots:vec![plan.parcels[0].lot(IVec3::ZERO,true)],housing_demand:4,residents:3,
            paused:false,completed:0,origin:[0;3],status:String::new()};
        ensure!(state.valid(),"Generated district state invalid");
        state.lots[0].origin[0]+=1;
        ensure!(!state.valid(),"Saved project detached from its district parcel");
        state.lots[0]=plan.parcels[0].lot(IVec3::ZERO,true);state.lots[0].layer=4;
        ensure!(state.valid(),"Valid in-progress district rejected");
        state.lots[0].variant^=1;
        ensure!(!state.valid(),"Saved project changed its district architecture");
        let mut broken=plan.clone();broken.parcels[2]=broken.parcels[1].clone();
        ensure!(!broken.valid(),"Overlapping saved parcels accepted");
        let mut broken=plan.clone();broken.roads[1].reverse();ensure!(!broken.valid(),"Disconnected saved frontage accepted");
        let mut broken=plan.clone();if let Some(d)=&mut broken.design {d.widths.clear();}ensure!(!broken.valid(),"Invalid saved widths accepted");
        rows.push(serde_json::json!({"profile":profile,"seed":seed,"style":style,"paving_m2":plan.paving().len(),"expansions":plan.expansions,"plan":plan}));
    }}}
    let legacy=super::generate(7,0,0,1,&scene::height).map_err(anyhow::Error::msg)?;
    ensure!(legacy.version==1&&legacy.design.is_none()&&legacy.valid(),"Legacy layout changed");
    let mut corrupt=legacy.clone();corrupt.parcels[0].origin[0]=i32::MIN;
    ensure!(!corrupt.valid(),"Extreme saved coordinate accepted");
    let mut corrupt=district::generate(7,0,0,1,&scene::height).map_err(anyhow::Error::msg)?;
    if let Some(d)=&mut corrupt.design {d.timber[0]=[corrupt.parcels[1].origin[0],1,corrupt.parcels[1].origin[1]];}
    ensure!(!corrupt.valid(),"Timber reserved on a building");
    let cliff=|p:Point|if p==[6,2] {Some(8)}else{scene::height(p)};
    ensure!(district::generate(7,0,0,1,&cliff).is_err(),"Cliff accepted inside public plaza");
    let blocked=|p:Point|if p==[4,20] {None}else{scene::height(p)};
    let detour=district::generate(7,0,0,1,&blocked).map_err(anyhow::Error::msg)?;
    ensure!(!detour.paving().contains(&[4,20]),"Widened boulevard crossed an exclusion");
    std::fs::write(root.join("district-plans.json"),serde_json::to_vec_pretty(&rows)?)?;
    println!("[city-district] PASS27 plans, coherent styles, connected streets, widened exclusion avoidance, save rejection and legacy defaults");
    Ok(())
}

pub(super) fn native_matrix(root:&Path)->Result<()> {
    for (profile,seed,style) in [(0,7,0),(1,424242,1),(2,8675309,2)] {
        let mut w=crate::world::World::with_seed_in(crate::world::WorldType::Superflat,
            crate::fluid::VoxelScalePreset::OneMeter,seed,&root.join(format!("district-{profile}-{seed}")));
        let base=IVec3::new(0,engine_worldgen::terrain_height(&w.sample_config(),0,0)+1,0);
        let plan=district::generate(seed,profile,style,1,&scene::height).map_err(anyhow::Error::msg)?;
        w.set_many(&scene::edits(&plan,base).map_err(anyhow::Error::msg)?);
        w.farming.jobs.quarter=Some(super::super::State {frontier:None,lots:vec![plan.parcels[0].lot(base,true)],
            layout:Some(plan),housing_demand:4,residents:0,paused:true,completed:0,origin:base.to_array(),status:String::new()});
        super::checks::native(&w,&mut crate::player::Player::new())?;
    }
    println!("[city-district] PASS3 native street-to-building matrices");
    Ok(())
}
