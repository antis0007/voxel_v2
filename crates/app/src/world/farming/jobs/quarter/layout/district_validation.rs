use super::*;

pub(in super::super) fn valid(plan:&Plan)->bool {
    let Some(d)=&plan.design else {return false;};
    if plan.version!=2 || !(1..=2).contains(&d.version) || d.widths!=[9,5,5,5,5,5,5,3,3]
        || d.uses.len()!=6 || d.uses[1]!=Use::MarketHall || d.timber.len()!=8
        || d.plaza!=[-7,-3,7,3] || plan.parcels.iter().any(|p|p.tradition!=plan.parcels[0].tradition)
        || d.timber.iter().any(|p|p[1]!=1||!inside([p[0],p[2]]))
        || d.timber.iter().copied().collect::<BTreeSet<_>>().len()!=8 {return false;}
    for (i,p) in plan.parcels.iter().enumerate() {
        let b=p.bounds();
        if !inside([b[0],b[1]])||!inside([b[2],b[3]])
            || plan.parcels[..i].iter().any(|q|super::super::overlaps(b,q.bounds())) {return false;}
    }
    if !plan.roads[0].contains(&[0,0]) || plan.roads[0].first()!=Some(&[0,-27])
        || plan.roads[0].last()!=Some(&[0,27]) {return false;}
    if plan.roads.iter().skip(1).take(6).zip(&plan.parcels)
        .any(|(r,p)|r.first()!=Some(&p.door())||r.last()!=Some(&[0,0])) {return false;}
    for (road,a,b) in [(7,2,1),(8,5,3)] {
        if plan.roads[road].first()!=Some(&plan.parcels[a].door())
            || plan.roads[road].last()!=Some(&plan.parcels[b].door()) {return false;}
    }
    let Ok(footprints)=footprints(plan) else{return false;};
    if d.timber.iter().any(|at|footprints.contains(&[at[0],at[2]])) {return false;}
    let Ok(occupied)=occupied(plan) else{return false;};
    if plan.paving().iter().any(|p|!inside(*p)||occupied.contains(p)) {return false;}
    let hall=plan.parcels[1].lot(IVec3::ZERO,false);
    if d.merchant!=geometry::at(&hall,IVec3::new(4,1,3)).to_array()
        ||d.merchant_feet!=geometry::at(&hall,IVec3::new(4,1,2)).to_array() {return false;}
    let Ok(styles)=traditions() else{return false;};
    if plan.parcels.iter().any(|p|p.roof_access!=(d.version==2 && styles[p.tradition].form==super::super::super::Roof::Courtyard)) {return false;}
    let paid=plan.parcels[0].lot(IVec3::ZERO,true);
    if geometry::plan(&paid,&styles[paid.tradition]).iter().any(|&(at,_)|
        (geometry::at(&paid,at)-STOCKPILE).as_vec3().length_squared()>super::super::super::super::RANGE.powi(2)) {return false;}
    d.timber.iter().all(|at|(IVec3::from(*at)-IVec3::from(d.merchant)).as_vec3().length()<24.0)
}
