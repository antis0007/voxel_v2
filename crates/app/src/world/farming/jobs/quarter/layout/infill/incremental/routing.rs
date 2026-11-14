use super::super::*;

pub(super) fn starts(p: &Project, base: IVec3) -> Vec<Point> {
    let Some((bounds,_))=shape(p,base) else {return Vec::new();};
    p.cells.iter().filter(|o|o.block()==B::Planks && o.cell().y==base.y+1)
        .filter(|o| {
            let q=point(base,o.cell());
            (q[0]==bounds[0] || q[0]==bounds[2] || q[1]==bounds[1] || q[1]==bounds[3])
                && !p.cells.iter().any(|other|other.cell().x==o.cell().x
                    && other.cell().z==o.cell().z && other.cell().y>o.cell().y)
        }).map(|o|point(base,o.cell())).collect()
}

pub(super) fn route(w: &World, plan: &InfillPlan, parcel: &mut InfillParcel,
    survey: &local::Survey<'_>, scans: &mut Scans, remaining: u32, requests: &mut u32) -> Result<bool,String> {
    let base=IVec3::from(plan.base);
    let pile=IVec3::from(plan.stockpile);
    let radius=(jobs::RANGE/w.voxel_scale.meters()).powi(2);
    let p=parcel.building(base)?;
    let mut links=Vec::new();
    for start in starts(&p,base) {
        for &(end,anchor) in &survey.anchors {
            let distance=(start[0]-end[0]).unsigned_abs()+(start[1]-end[1]).unsigned_abs();
            links.push((distance,start,end,anchor));
        }
    }
    links.sort_unstable();
    let profiles=super::super::super::profiles()?;
    let profile=profiles.get(plan.profile as usize).ok_or("Planning Profile Invalid")?;
    for (_,start,end,anchor) in links.into_iter().take(ROUTES_PER_POLL.min(remaining) as usize) {
        *requests+=1;
        scans.path_searches=scans.path_searches.saturating_add(1);
        scans.path_node_budget=scans.path_node_budget.saturating_add(20_001);
        let clear=|q:Point|bounded(q) && (at(base,q)-pile).as_vec3().length_squared()<=radius
            && (q==end || ((q==start || !contains(parcel.footprint,q)) && survey.free(q)));
        let Ok((path,_))=super::super::super::route::find(start,end,profile,&|_|Some(1),&clear) else {continue;};
        let mut access=vec![start];
        for pair in path.windows(2) {
            if pair[0][0]!=pair[1][0] && pair[0][1]!=pair[1][1] {access.push([pair[1][0],pair[0][1]]);}
            access.push(pair[1]);
        }
        if access.len()>32 {continue;}
        parcel.access=access;parcel.anchor=anchor;
        if parcel.build(base)?.cells.len()<=jobs::construction::LIMIT {return Ok(true);}
    }
    Ok(false)
}
