use super::*;

pub(super) fn valid(plan: &InfillPlan, s: &Site, count: usize) -> bool {
    let serial=plan.schema==2;
    if !matches!(plan.schema,1|2) || plan.profile>=3 || plan.parcels.len()>if serial {PROJECT_LIMIT-6}else{2}
        || plan.base.iter().any(|n|n.unsigned_abs()>=1<<27)
        || plan.stockpile!=(IVec3::from(plan.base)+IVec3::Y).to_array()
        || (!serial && (plan.search.is_some()
            || (!plan.surveyed && (!plan.parcels.is_empty() || plan.attempts!=0 || plan.retry_at!=0))))
        || (serial && (!incremental::valid(plan) || plan.parcels.iter().filter(|p|p.project.is_none()).count()>1
            || (!plan.surveyed && (plan.search.is_some() || plan.pending().is_some() || plan.retry_at!=0))))
        || (plan.surveyed && plan.attempts==0) {return false;}
    let base=IVec3::from(plan.base);
    let mut funded=0;
    let mut cells:BTreeSet<_>=s.projects.iter().take(6).flat_map(|p|p.cells.iter().map(|o|o.at)).collect();
    for (i,parcel) in plan.parcels.iter().enumerate() {
        if !bounded(parcel.origin) || !(2..=32).contains(&parcel.access.len())
            || parcel.provenance!=provenance(plan.seed,parcel.origin,parcel.turn,parcel.stairs_right)
            || parcel.access.iter().any(|p|!bounded(*p))
            || parcel.access.windows(2).any(|p|(p[0][0]-p[1][0]).abs()+(p[0][1]-p[1][1]).abs()!=1)
            || parcel.access.iter().copied().collect::<BTreeSet<_>>().len()!=parcel.access.len()
            || plan.parcels[..i].iter().any(|p|!compatible(p,parcel)) {return false;}
        let Ok(building)=parcel.building(base) else {return false;};
        if shape(&building,base)!=Some((parcel.footprint,parcel.entrance)) {return false;}
        let start=at(base,parcel.access[0]);
        if !building.cells.iter().any(|o|o.cell()==start && o.block()==B::Planks)
            || building.cells.iter().any(|o|o.cell().x==start.x && o.cell().z==start.z && o.cell().y>start.y)
            || parcel.access.iter().skip(1).any(|p|contains(parcel.footprint,*p)) {return false;}
        let [pi,oi]=parcel.anchor;
        let Some(anchor_project)=s.projects.get(pi).filter(|_|pi<6+i) else {return false;};
        let Some(anchor)=anchor_project.cells.get(oi) else {return false;};
        let high=anchor_project.cells.iter().map(|o|anchor_project.work_layer(*o)).max().unwrap_or(0);
        if anchor.block()!=B::Planks || anchor.cell()!=at(base,parcel.access[parcel.access.len()-1])
            || !s.project_layers.get(pi).is_some_and(|l|*l>high+1) {return false;}
        let Ok(p)=parcel.build(base) else {return false;};
        if p.cells.len()>jobs::construction::LIMIT || p.cells.iter().any(|o|
            (o.cell()-IVec3::from(plan.stockpile)).as_vec3().length()>jobs::RANGE || !cells.insert(o.at)) {return false;}
        if let Some(index)=parcel.project {
            if i!=funded || index!=6+i || !s.projects.get(index).is_some_and(|old|same(old,&p)) {return false;}
            funded+=1;
        }
    }
    count<=s.projects.len() && count==6+funded
}
