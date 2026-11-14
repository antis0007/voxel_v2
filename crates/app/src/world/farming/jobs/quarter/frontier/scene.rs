use super::*;
#[path = "building.rs"]
pub(super) mod building;
#[path = "building_check.rs"]
pub(super) mod building_check;

#[derive(Deserialize)]
struct Content { timber_min:[i32;3], timber_max:[i32;3], projects:Vec<Spec> }
#[derive(Deserialize)]
pub(super) struct Spec { name:String,origin:[i32;3],size:[u8;2],#[serde(default)] turn:bool,
    #[serde(default)] stairs_right:bool,food_reserve:u32,meals_eaten:u32,path:Vec<[i32;3]>,
    #[serde(default="home_capacity")] home_capacity:u8,
    #[serde(default="one_floor")] floors:u8, #[serde(default)] home_floor:u8,
    #[serde(default)] variant_seed:u32 }

fn home_capacity()->u8 {4}
fn one_floor()->u8 {1}

pub(in super::super) fn infill(origin: [i32; 3], turn: bool, stairs_right: bool,
    path: Vec<[i32; 3]>, base: IVec3) -> Result<Project, String> {
    plan(Spec { name: "Infill house".into(), origin, size: [4,4], turn, stairs_right,
        food_reserve: 0, meals_eaten: 0, path, home_capacity: 4, floors: 1,
        home_floor: 0, variant_seed: 0 }, base)
}

pub(super) fn plan(spec:Spec,base:IVec3)->Result<Project,String> {
    let [width,depth]=spec.size.map(i32::from);
    if !(4..=7).contains(&width)||!(4..=8).contains(&depth)||spec.path.len()>32||spec.home_capacity>8
        ||!(1..=2).contains(&spec.floors)||spec.home_floor>=spec.floors {
        return Err("Invalid site building dimensions".into());
    }
    let origin=base+IVec3::from(spec.origin);
    let sign=if spec.turn {-1}else{1};
    let at=|x,y,z|origin+IVec3::new(x*sign,y,z*sign);
    let mut cells=Vec::new();
    if spec.floors==2 {
        cells.extend(building::plan(width,depth,spec.stairs_right,engine_core::hash::hash_u32(spec.variant_seed))?.into_iter()
            .map(|[x,y,z]|Order::new(at(x,y,z),B::Planks)));
    } else {
    for y in 0..=3 {for z in 0..depth {for x in 0..width {
        let wall=x==0||x==width-1||z==0||z==depth-1;
        let door=x==width/2&&z==0&&(y==1||y==2);
        let window=y==2&&((z==depth/2&&(x==0||x==width-1))||(z==depth-1&&x==width/2));
        if y==0||y==3||(wall&&!door&&!window) {
            cells.push(Order::new(at(x,y,z),B::Planks));
        }
    }}}
    }
    cells.push(Order::new(at(width/2,0,-1),B::Planks));
    cells.push(Order::new(at(1,1,depth-2),B::Chest));
    let lanes=if spec.stairs_right {[width,width+1]}else{[-2,-1]};
    if spec.floors==1 { for x in lanes {for step in 0..=3 {for y in 0..=step {
        cells.push(Order::new(at(x,y,1+step),B::Planks));
    }}}
    }
    cells.extend(spec.path.iter().map(|p|Order::new(base+IVec3::from(*p),B::Planks)));
    // A worker on an unfinished window ledge needs a one-metre descent outside.
    // The paid apron also connects frontage and the permanent roof-access steps.
    for z in -1..=depth {for x in -1..=width {
        if (x==-1||x==width||z==-1||z==depth)&&!cells.iter().any(|o|o.cell()==at(x,0,z)) {
            cells.push(Order::new(at(x,0,z),B::Planks));
        }
    }}
    if cells.len()>super::super::super::construction::LIMIT {
        return Err("Site building exceeds paid project limit".into());
    }
    let (entry,destination)=if spec.home_floor==1 {
        let wall=if spec.stairs_right {width-1}else{0};
        let outward=if spec.stairs_right {1}else{-1};
        (at(wall+outward,4,4),at(wall-outward,4,4))
    } else {(at(width/2,1,-1),at(width/2,1,1))};
    let home=(spec.home_capacity>0).then_some(super::colony::housing::Home{
        capacity:spec.home_capacity,entry:entry.to_array(),destination:destination.to_array()});
    Ok(Project{name:spec.name,cells,food_reserve:spec.food_reserve,meals_eaten:spec.meals_eaten,home})
}

pub(in super::super) fn install(w:&mut World,p:&mut crate::player::Player)->Result<(),String> {
    if super::colony::enabled() {return super::colony::scene::install(w,p);}
    production::scene::install(w,p)?;
    // The controlled founding scene keeps ambient creatures out of its work sites.
    w.rules.mob_spawning=false;
    // This plan uses permanent timber access steps, not the starter shelter's earthwork.
    w.farming.jobs.mine.retain(|(_,block)|*block==B::Log as u16);
    let starter:Vec<_>=w.farming.jobs.mine.iter().map(|(p,_)|IVec3::from(*p)).collect();
    if starter.iter().any(|p|known(w,*p)!=Some(B::Log)||known(w,*p+IVec3::Z*2)!=Some(B::Air)) {
        return Err("Starter timber relocation obstructed".into());
    }
    let mut relocation=Vec::new();
    for at in &starter {relocation.extend([(*at,B::Air),(*at+IVec3::Z*2,B::Log)]);}
    w.set_many(&relocation);
    for (p,_) in &mut w.farming.jobs.mine {p[2]+=2;}
    super::super::super::set_priorities(w,0,[1,2,1]);
    let pile=IVec3::from(w.farming.jobs.stockpile.ok_or("Missing production chest")?);
    let base=pile-IVec3::Y;
    let source=if building_check::enabled() {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../assets/city_two_storey_site.ron"))
    } else {include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../assets/city_start_site.ron"))};
    let content:Content=ron::from_str(source)
        .map_err(|e|e.to_string())?;
    let mut projects=Vec::new();
    for spec in content.projects {projects.push(plan(spec,base)?);}
    let min=IVec3::from(content.timber_min);let max=IVec3::from(content.timber_max);
    if min.y!=1||!(1..=2).contains(&max.y)||min.x>max.x||min.z>max.z||min.abs().max_element()>12||max.abs().max_element()>12 {
        return Err("Invalid finite timber source".into());
    }
    let additional:Vec<_>=(min.x..=max.x).flat_map(|x|(min.y..=max.y).flat_map(move|y|
        (min.z..=max.z).map(move|z|base+IVec3::new(x,y,z)))).collect();
    let mut timber:Vec<_>=w.farming.jobs.mine.iter().map(|(p,_)|*p).collect();
    timber.extend(additional.iter().map(|p|p.to_array()));
    let layer=projects.first().and_then(|p|p.cells.iter().map(|o|o.cell().y).min()).ok_or("No site projects")?;
    let mut site=State{growth:None,version:1,projects,timber,timber_next:Some(0),phase:0,layer,project_layers:Vec::new(),households:Vec::new(),housing_overrides:Vec::new()};
    if !site.valid() {return Err("Invalid site project ledger".into());}
    for project in &site.projects {
        for o in &project.cells {
            if known(w,o.cell())!=Some(B::Air)||(o.cell()-pile).as_vec3().length()>super::super::super::RANGE {
                return Err("Site project is obstructed or beyond stockpile reach".into());
            }
            if w.farming.jobs.production.as_ref().is_some_and(|s|s.beds.iter().any(|b|b.at[0]==o.at[0]&&b.at[2]==o.at[2])) {
                return Err("Site project overlaps a crop bed".into());
            }
        }
    }
    if additional.iter().any(|p|known(w,*p)!=Some(B::Air)) {return Err("Timber source obstructed".into());}
    w.set_many(&additional.iter().map(|p|(*p,B::Log)).collect::<Vec<_>>());
    w.farming.jobs.mine.clear();
    super::admit_timber(w,&mut site);
    // The fresh scene has not run a work tick; replace its starter orders with this persisted plan.
    w.farming.jobs.build.clear();
    w.farming.jobs.reset_task();
    let timber_count=site.timber.len();
    w.farming.jobs.quarter=Some(super::super::State{frontier:Some(site),layout:None,lots:Vec::new(),
        housing_demand:0,residents:3,paused:false,completed:0,origin:base.to_array(),status:"Gathering supplies".into()});
    if !w.farming.jobs.valid() {return Err("Invalid initial site commitments".into());}
    p.yaw=0.15;
    println!("[city-site] installed residents=3 timber={timber_count} existing_buildings=0 projects=3");
    Ok(())
}
