//! Coherent six-parcel districts with surveyed street widths and services.
use super::*;
use serde::{Deserialize, Serialize};
#[path = "district_validation.rs"]
mod validation;
pub(super) use validation::valid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Use { Housing, MarketHall, Workshop, Storage }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Design {
    pub version: u8,
    pub widths: Vec<u8>,
    pub plaza: [i32; 4],
    pub uses: Vec<Use>,
    pub merchant: [i32; 3],
    pub merchant_feet: [i32; 3],
    pub timber: Vec<[i32; 3]>,
}

#[derive(Deserialize)]
struct Content { widths: Vec<u8>, plaza: [i32;4], uses:Vec<Use>, storeys:Vec<u8>, timber:Vec<[i32;3]> }

pub fn enabled()->bool {std::env::var("VOXEL_CITY_DISTRICTS").is_ok_and(|v|v=="1")}

pub fn radius(plan:&Plan,road:usize,at:Point)->i32 {
    let Some(design)=&plan.design else {return if road==0 {2}else{1};};
    let radius=i32::from(design.widths[road])/2;
    if (1..=6).contains(&road) && plan.roads[road].first().is_some_and(|a|
        (at[0]-a[0]).abs().max((at[1]-a[1]).abs())<=2) {1} else {radius}
}

pub fn paving(plan:&Plan)->BTreeSet<Point> {
    let mut cells=BTreeSet::new();
    for (i,road) in plan.roads.iter().enumerate() {for &[x,z] in road {
        let r=radius(plan,i,[x,z]);
        for dx in -r..=r {for dz in -r..=r {cells.insert([x+dx,z+dz]);}}
    }}
    if let Some(d)=&plan.design {let [x0,z0,x1,z1]=d.plaza;
        for x in x0..=x1 {for z in z0..=z1 {cells.insert([x,z]);}}
    }
    cells
}

pub(super) fn footprints(plan:&Plan)->Result<BTreeSet<Point>,String> {
    let styles=traditions()?; let mut out=BTreeSet::from([[STOCKPILE.x,STOCKPILE.z]]);
    for (i,p) in plan.parcels.iter().enumerate() {let lot=p.lot(IVec3::ZERO,i==0);
        for (cell,_) in geometry::plan(&lot,&styles[p.tradition]) {
            let at=geometry::at(&lot,cell);out.insert([at.x,at.z]);
        }
    }
    Ok(out)
}

pub(super) fn occupied(plan:&Plan)->Result<BTreeSet<Point>,String> {
    let mut out=footprints(plan)?;
    if let Some(d)=&plan.design {out.extend(d.timber.iter().map(|p|[p[0],p[2]]));}
    Ok(out)
}

pub fn generate(seed:u32,profile:u8,style:usize,complexity:u8,
    height:&impl Fn(Point)->Option<i32>)->Result<Plan,String> {
    let content:Content=ron::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../assets/city_districts.ron")))
        .map_err(|e|e.to_string())?;
    if content.widths!=[9,5,5,5,5,5,5,3,3] || content.uses.len()!=6 || content.uses[1]!=Use::MarketHall
        || content.storeys.len()!=6 || content.storeys.iter().any(|n|!(1..=3).contains(n))
        || content.timber.len()!=8 || content.plaza!=[-7,-3,7,3] {return Err("Invalid district content".into());}
    let mut plan=super::generate(seed,profile,style,complexity,height)?;
    let courtyard=traditions()?[style].form==super::super::Roof::Courtyard;
    for (i,p) in plan.parcels.iter_mut().enumerate() {
        p.tradition=style;
        p.roof_access=courtyard;
        p.origin[1]=if p.turn {p.origin[1].min(-8)}else{p.origin[1].max(8)};
        if i>0 {p.complexity=(content.storeys[i]+(engine_core::hash::hash_u32(seed^i as u32)%2) as u8).min(3);}
    }
    // Keep a nine-metre corridor beside the existing construction stockpile.
    plan.parcels[0].origin[0]=plan.parcels[0].origin[0].max(9);
    plan.parcels[1].origin[0]=plan.parcels[1].origin[0].min(-20);
    plan.parcels[2].origin[0]=plan.parcels[2].origin[0].min(plan.parcels[1].origin[0]-16);
    let hall=plan.parcels[1].lot(IVec3::ZERO,false);
    plan.design=Some(Design {version:2,widths:content.widths,plaza:content.plaza,uses:content.uses,
        merchant:geometry::at(&hall,IVec3::new(4,1,3)).to_array(),
        merchant_feet:geometry::at(&hall,IVec3::new(4,1,2)).to_array(),timber:content.timber});
    if plan.design.as_ref().is_some_and(|d|d.timber.iter().any(|p|height([p[0],p[2]])!=Some(0))) {
        return Err("Timber yard requires level surveyed ground".into());
    }
    if let Some(d)=&plan.design {let [x0,z0,x1,z1]=d.plaza;
        if (x0..=x1).any(|x|(z0..=z1).any(|z|height([x,z])!=Some(0))) {
            return Err("Public plaza requires level surveyed ground".into());
        }
    }
    let occupied=occupied(&plan)?;
    let open=|[x,z]:Point,r:i32|height([x,z]).is_some_and(|h|
        (-r..=r).all(|dx|(-r..=r).all(|dz| {
            let q=[x+dx,z+dz];inside(q)&&!occupied.contains(&q)
                &&height(q).is_some_and(|n|(n-h).abs()<=1)
        })));
    let row=profiles()?.get(profile as usize).cloned().ok_or("Unknown district street profile")?;
    let mut roads=Vec::new(); let mut expansions=plan.expansions;
    let (mut main,a)=route::find([0,-27],[0,0],&row,height,&|q|open(q,4))?;
    let (tail,b)=route::find([0,0],[0,27],&row,height,&|q|open(q,4))?;
    main.extend(tail.into_iter().skip(1));roads.push(main);expansions+=a+b;
    let mut links:Vec<_>=plan.parcels.iter().map(|p|(p.door(),[0,0],2)).collect();
    links.extend([(plan.parcels[2].door(),plan.parcels[1].door(),1),
        (plan.parcels[5].door(),plan.parcels[3].door(),1)]);
    for (start,end,r) in links {
        let clear=|q:Point|open(q,if r==2&&(q[0]-start[0]).abs().max((q[1]-start[1]).abs())<=2 {1}else{r});
        let (road,count)=route::find(start,end,&row,height,&clear)?;
        roads.push(road);expansions+=count;
    }
    plan.version=2;plan.roads=roads;plan.expansions=expansions;
    if !plan.valid() {return Err("District connectivity or parcel clearance failed".into());}
    let paving=plan.paving();
    if paving.iter().any(|&q|height(q).is_none() || [[1,0],[-1,0],[0,1],[0,-1]].iter().any(|d| {
        let next=[q[0]+d[0],q[1]+d[1]];
        paving.contains(&next)&&height(q).zip(height(next)).is_none_or(|(a,b)|(a-b).abs()>1)
    })) {return Err("District paving exceeds the supported walking grade".into());}
    if plan.parcels.iter().any(|p| {let [x0,z0,x1,z1]=p.bounds();
        (x0..=x1).any(|x|(z0..=z1).any(|z|height([x,z])!=Some(0)))}) {
        return Err("District parcel is not level surveyed ground".into());
    }
    let styles=traditions()?;let paid=plan.parcels[0].lot(IVec3::ZERO,true);
    if geometry::plan(&paid,&styles[style]).iter().any(|&(at,_)|
        (geometry::at(&paid,at)-STOCKPILE).as_vec3().length_squared()>super::super::super::RANGE.powi(2)) {
        return Err("District project exceeds construction service radius".into());
    }
    Ok(plan)
}
