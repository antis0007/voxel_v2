use super::*;
use std::collections::BTreeMap;
use crate::world::{inventory::ItemStack,block_data,adventure::wildlife::vehicle};

#[derive(Deserialize)]
struct Content {production:production::State,projects:Vec<super::super::scene::Spec>,initial_saplings:u32}

pub(super) fn residents()->usize {
    std::env::var("VOXEL_CITY_RESIDENTS").ok().and_then(|s|s.parse().ok())
        .filter(|n|matches!(n,12|24|32|48|64)).unwrap_or(32)
}

pub(in super::super) fn install(w:&mut World,p:&mut crate::player::Player)->Result<(),String> {
    if std::env::args().any(|s|s=="--city-livelihood-woodlot") {return Err("Colony selects its own woodlot layout".into());}
    let content:Content=ron::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../assets/city_colony.ron")))
        .map_err(|e|e.to_string())?;
    let count=residents();
    if count>jobs::crew::CAPACITY||content.initial_saplings>64 {return Err("Colony roster exceeds capacity".into());}
    production::scene::install(w,p)?;
    w.rules.mob_spawning=false;
    let pile=IVec3::from(w.farming.jobs.stockpile.ok_or("Missing stockpile")?);
    let base=pile-IVec3::Y;
    let mut edits=Vec::new();
    for x in -24i32..=24 {for z in -24i32..=24 {
        if x.abs()>12||z.abs()>12 {
            edits.push((base+IVec3::new(x,0,z),B::Dirt));
            for y in 1..=8 {edits.push((base+IVec3::new(x,y,z),B::Air));}
        }
    }}
    for x in [-20,-12,-4] {for z in -12i32..=12 {
        if z.abs()>1 {edits.push((base+IVec3::new(x,0,z),B::Water));}
    }}
    for (at,block) in &w.farming.jobs.mine {
        if *block==B::Log as u16 {edits.push((IVec3::from(*at),B::Air));}
    }
    let timber:Vec<_>=(20..=22).flat_map(|x|(1..=2).flat_map(move|y|(-8..=8)
        .map(move|z|(base+IVec3::new(x,y,z)).to_array()))).take(100).collect();
    edits.extend(timber.iter().map(|p|(IVec3::from(*p),B::Log)));
    w.set_many(&edits);
    let mut prod=content.production;
    prod.workers=(0..count as u32).collect();
    for b in &mut prod.beds {b.at[1]+=base.y;}
    for b in &mut prod.bills {b.station[1]+=base.y;}
    // A felled trunk lands beyond its root. Keep the whole logging corridor
    // inside the haul service radius, including the actor's standing margin.
    if prod.beds.iter().filter(|b|b.crop==B::Sapling).any(|b|
        (IVec3::from(b.at)-pile).as_vec3().length()+4.5>jobs::RANGE) {
        return Err("Woodlot fall corridor exceeds hauling range".into());
    }
    if !prod.valid()||prod.beds.iter().any(|b|known(w,b.at.into())!=Some(B::Air)
        ||!known(w,IVec3::from(b.at)-IVec3::Y).is_some_and(|soil|b.row().is_some_and(|(_,c)|c.soil.contains(&soil)))) {
        return Err("Invalid colony planting ground".into());
    }
    let mut projects=Vec::new();let mut occupied=BTreeMap::new();
    for spec in content.projects {
        let mut project=super::super::scene::plan(spec,base)?;
        for o in &project.cells {
            if occupied.get(&o.at).is_some_and(|b|*b!=o.block()) {return Err("Conflicting project materials".into());}
        }
        project.cells.retain(|o|occupied.insert(o.at,o.block()).is_none());
        projects.push(project);
    }
    let layers=projects.iter().map(|p|p.cells.iter().map(|o|o.cell().y).min().unwrap_or(0)).collect();
    let mut site=State{growth:None,version:1,projects,timber,timber_next:Some(0),phase:0,layer:0,project_layers:layers,households:Vec::new(),housing_overrides:Vec::new()};
    if !site.valid(){return Err("Invalid parallel project ledger".into());}
    for o in site.projects.iter().flat_map(|p|&p.cells) {
        if known(w,o.cell())!=Some(B::Air)||(o.cell()-pile).as_vec3().length()>jobs::RANGE
            ||prod.beds.iter().any(|b|b.at[0]==o.at[0]&&b.at[2]==o.at[2]) {
            return Err(format!("Colony project blocked or outside service range at {:?}",o.at));
        }
    }
    for crop in [B::CarrotCrop,B::BeetCrop,B::Sapling] {
        let wanted=if crop==B::Sapling {content.initial_saplings} else {prod.beds.iter().filter(|b|b.crop==crop).count() as u32};
        let have=production::count(&w.block_data.chest(pile),Item::Block(crop));
        let mut stack=ItemStack::new(Item::Block(crop),wanted.saturating_sub(have));
        if stack.count>0 {block_data::store_hauled(w,pile,&mut stack);}
        if stack.count!=0 {return Err("Colony seed stockpile full".into());}
    }
    w.farming.jobs.production=Some(prod);
    w.farming.jobs.mine.clear();w.farming.jobs.build.clear();w.farming.jobs.claims.clear();
    w.farming.jobs.reset_task();
    super::super::admit_timber(w,&mut site);
    let mut spots:Vec<_>=(-18..=18).flat_map(|z|(-2..=1).map(move|x|base+IVec3::new(x,1,z))).collect();
    spots.sort_by_key(|at|(at.x*at.x+(at.z-base.z)*(at.z-base.z),at.z,at.x));
    for id in 3..count as u32 {
        let at=spots.iter().copied().find(|at|jobs::stand(w,*at,1.0)
            &&w.adventure.enemies.iter().all(|e|e.pos.distance_squared(jobs::metres(*at,1.0))>0.95*0.95)
            &&p.pos.distance_squared(jobs::metres(*at,1.0))>2.25).ok_or("No safe resident spawn")?;
        let feet=jobs::metres(at,1.0);
        vehicle::spawn_resident(w,id,feet);w.travel.homes.insert(id,feet);
        if !jobs::crew::add(w,id) {return Err(format!("Resident assignment failed at {id}"));}
    }
    for id in 0..count as u32 {
        let priorities=if (12..16).contains(&id) {[1,1,3]}else if (4..12).contains(&id) {[1,3,1]}else{[1,2,1]};
        jobs::set_priorities(w,id,priorities);
    }
    let planned=site.projects.iter().map(|p|p.cells.len()).sum::<usize>();
    w.farming.jobs.quarter=Some(super::super::super::State{frontier:Some(site),layout:None,lots:Vec::new(),
        housing_demand:0,residents:count as u32,paused:false,completed:0,origin:base.to_array(),status:"Preparing settlement".into()});
    if std::env::var("VOXEL_CITY_GROWTH").is_ok_and(|v|matches!(v.as_str(),"1"|"2"|"3")) { super::growth::enable(w)?; }
    if !w.farming.jobs.valid(){return Err("Invalid colony commitments".into());}
    p.pos=base.as_vec3()+Vec3::new(0.5,1.0,21.5);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
    println!("[city-colony] residents={count} crop_beds=96 woodlots=4 projects=6 paid_cells={planned}");
    Ok(())
}
