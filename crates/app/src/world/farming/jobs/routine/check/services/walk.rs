use super::*;

fn grounded(w: &World, p: &mut crate::player::Player, target: Vec3) -> Result<f32> {
    let mut distance = 0.0;
    for _ in 0..2400 {
        let delta = (target-p.pos)*Vec3::new(1.0,0.0,1.0);
        if delta.length()<0.05 && p.on_ground && (p.pos.y-target.y).abs()<0.15 {
            ensure!(!p.flying,"Service route used flight"); return Ok(distance);
        }
        let mut input = Input::default();
        if delta.length()>=0.04 {p.yaw=delta.x.atan2(-delta.z);input.forward=true;}
        let before=p.pos;crate::player::update_player(p,&mut input,w,crate::simulation::STEP_SECONDS);
        distance+=p.pos.distance(before);
    }
    anyhow::bail!("Service grounded waypoint blocked: feet={:?}, target={target:?}",p.pos)
}

pub(super) fn check(w: &World, p: &mut crate::player::Player, index: usize) -> Result<serde_json::Value> {
    let home = site(w)?.projects.get(index).and_then(|p|p.home.as_ref()).context("Missing service home")?;
    let pile = IVec3::from(w.farming.jobs.stockpile.context("Missing stockpile")?);
    let v = w.voxel_scale.meters();ensure!((v-1.0).abs()<0.001,"Service acceptance expects one-metre scene");
    let start = [IVec3::X,IVec3::NEG_X,IVec3::Z,IVec3::NEG_Z].into_iter().map(|d|pile+d)
        .find(|at|jobs::stand(w,*at,v)).context("Stockpile has no standing space")?;
    let start = jobs::metres(start,v);
    p.pos=start;p.precise_pos=start.as_dvec3();p.vel=Vec3::ZERO;p.flying=false;p.on_ground=false;
    let entry = IVec3::from(home.entry); let destination = IVec3::from(home.destination);
    let hub=frontier::hub_plan(&w.farming.jobs).context("Missing paid service hub")?;
    let supply_task=jobs::Task::Store(hub.stockpile);
    let supply_path=jobs::path_in_range(w,start,supply_task,4096,52.0);
    ensure!(!supply_path.is_empty()||jobs::reachable(w,start,supply_task,v),"No primary-to-hub route");
    let mut supply_distance=0.0;
    for at in supply_path {supply_distance+=grounded(w,p,at)?;}
    ensure!(p.on_ground&&jobs::reachable(w,p.pos,supply_task,v),"Paid hub interaction unreachable");
    let task = jobs::Task::Routine(entry);
    let path = jobs::path_in_range(w,p.pos,task,4096,52.0);
    ensure!(!path.is_empty() || jobs::reachable(w,p.pos,task,v),"Generated home has no stockpile approach");
    let mut distance=supply_distance;for at in path {distance+=grounded(w,p,at)?;}
    distance+=grounded(w,p,jobs::metres(entry,v))?;
    distance+=grounded(w,p,jobs::metres(destination,v))?;
    let return_task=jobs::Task::Store(pile);
    let path=jobs::path_in_range(w,p.pos,return_task,4096,52.0);
    ensure!(!path.is_empty() || jobs::reachable(w,p.pos,return_task,v),"Generated home has no stockpile return");
    for at in path {distance+=grounded(w,p,at)?;}
    ensure!(p.on_ground&&!p.flying&&jobs::reachable(w,p.pos,return_task,v),"Generated home return did not reach stockpile");
    Ok(serde_json::json!({"project":index,"grounded_round_trip_m":distance,"primary_to_hub_m":supply_distance,"hub":hub.stockpile.to_array(),"start":start.to_array(),"end":p.pos.to_array(),"entry":entry.to_array(),"interior":destination.to_array()}))
}
