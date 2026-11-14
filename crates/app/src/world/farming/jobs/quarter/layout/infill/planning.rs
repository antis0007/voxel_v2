use super::*;

pub(crate) fn generate(w: &World, s: &Site, plan: &mut InfillPlan, scans: &mut Scans) -> Result<(),String> {
    plan.parcels.retain(|p|p.project.is_some());
    if plan.parcels.len()>=2 {return Err("Infill Limit: 2 Homes".into());}
    plan.surveyed=true;
    plan.attempts=plan.attempts.saturating_add(1);
    plan.retry_at=w.farming.clock.saturating_add(RETRY);
    let survey=survey::Survey::new(w,s,plan,scans)?;
    let profiles=super::super::profiles()?;
    let profile=profiles.get(plan.profile as usize).ok_or("Planning Profile Invalid")?;
    let base=IVec3::from(plan.base);
    let pile=IVec3::from(plan.stockpile);
    let range_squared=(jobs::RANGE/w.voxel_scale.meters()).powi(2);
    let mut candidates=Vec::new();
    for turn in [false,true] { for stairs_right in [false,true] {
        let template=scene::infill([0,1,0],turn,stairs_right,Vec::new(),base)?;
        let (bounds,entry)=shape(&template,base).ok_or("House Geometry Invalid")?;
        for x in -GRID..=GRID {for z in -GRID..=GRID {
            scans.candidates=scans.candidates.saturating_add(1);
            let footprint=[bounds[0]+x,bounds[1]+z,bounds[2]+x,bounds[3]+z];
            if !bounded([footprint[0],footprint[1]]) || !bounded([footprint[2],footprint[3]])
                || !(footprint[0]..=footprint[2]).all(|a|(footprint[1]..=footprint[3]).all(|b|survey.free.contains(&[a,b])))
                || template.cells.iter().any(|o|(o.cell()+IVec3::new(x,0,z)-pile).as_vec3().length_squared()>range_squared) {continue;}
            let parcel=InfillParcel {origin:[x,z],turn,stairs_right,footprint,
                entrance:[entry[0]+x,entry[1]+z],access:Vec::new(),anchor:[0,0],
                provenance:provenance(plan.seed,[x,z],turn,stairs_right),project:None};
            if plan.parcels.iter().all(|old|compatible(old,&parcel)) {candidates.push(parcel);}
        }}
    }}
    candidates.sort_by_key(|p|(p.provenance,p.origin,p.turn,p.stairs_right));
    let mut shortlist=Vec::new();
    for (i,p) in candidates.iter().enumerate().take(12) {
        if !shortlist.contains(&i) {shortlist.push(i);}
        if let Some((j,_))=candidates.iter().enumerate().find(|(_,q)|compatible(p,q)) {
            if !shortlist.contains(&j) {shortlist.push(j);}
        }
    }
    let mut routed=Vec::new();
    let mut requests=0;
    for i in shortlist {
        if requests>=ROUTES {break;}
        let mut parcel=candidates[i].clone();
        if !route(&mut parcel,&survey,base,pile,w.voxel_scale.meters(),profile,scans,&mut requests)? {continue;}
        if !plan.parcels.iter().all(|old|compatible(old,&parcel)) {continue;}
        if plan.parcels.len()==1 {plan.parcels.push(parcel);return Ok(());}
        if let Some(first)=routed.iter().find(|p|compatible(p,&parcel)).cloned() {
            plan.parcels.extend([first,parcel]);
            return Ok(());
        }
        routed.push(parcel);
    }
    if let Some(parcel)=routed.into_iter().next() {plan.parcels.push(parcel);return Ok(());}
    Err(if candidates.is_empty() {"No Space: Level Unreserved Ground"} else {"Street Search Limit: No Connected Parcel"}.into())
}

fn route(parcel: &mut InfillParcel, survey: &survey::Survey, base: IVec3, pile: IVec3,
    scale: f32, profile: &Profile, scans: &mut Scans, requests: &mut u32) -> Result<bool,String> {
    let p=parcel.building(base)?;
    let range_squared=(jobs::RANGE/scale).powi(2);
    let mut links=Vec::new();
    for o in p.cells.iter().filter(|o|o.cell().y==base.y+1) {
        let start=point(base,o.cell());
        if start[0]!=parcel.footprint[0] && start[0]!=parcel.footprint[2]
            && start[1]!=parcel.footprint[1] && start[1]!=parcel.footprint[3] {continue;}
        if p.cells.iter().any(|other|other.cell().x==o.cell().x && other.cell().z==o.cell().z && other.cell().y>o.cell().y) {continue;}
        for &(end,anchor) in &survey.anchors {
            let distance=(start[0]-end[0]).abs()+(start[1]-end[1]).abs();
            links.push((distance,start,end,anchor));
        }
    }
    links.sort();
    for (_,start,end,anchor) in links.into_iter().take(2) {
        if *requests>=ROUTES {break;}
        *requests+=1;
        scans.path_searches=scans.path_searches.saturating_add(1);
        scans.path_node_budget=scans.path_node_budget.saturating_add(20_000);
        let clear=|q:Point| bounded(q) && (at(base,q)-pile).as_vec3().length_squared()<=range_squared
            && (q==end || (survey.free.contains(&q) && (q==start || !contains(parcel.footprint,q))));
        let Ok((path,_))=super::super::route::find(start,end,profile,&|_|Some(1),&clear) else {continue;};
        let mut access=vec![start];
        for pair in path.windows(2) {
            if pair[0][0]!=pair[1][0] && pair[0][1]!=pair[1][1] {access.push([pair[1][0],pair[0][1]]);}
            access.push(pair[1]);
        }
        if access.len()>32 {continue;}
        parcel.access=access;parcel.anchor=anchor;
        let paid=parcel.build(base)?;
        if paid.cells.len()<=jobs::construction::LIMIT {return Ok(true);}
    }
    Ok(false)
}
