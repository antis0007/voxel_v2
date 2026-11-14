use super::*;

fn grounded(w: &World, p: &mut crate::player::Player, target: Vec3) -> Result<f32> {
    let mut distance = 0.0;
    for _ in 0..2400 {
        let delta = (target-p.pos)*Vec3::new(1.0,0.0,1.0);
        if delta.length()<0.05 && p.on_ground && (p.pos.y-target.y).abs()<0.15 {
            ensure!(!p.flying,"Infill route used flight"); return Ok(distance);
        }
        let mut input = Input::default();
        if delta.length()>=0.04 {p.yaw=delta.x.atan2(-delta.z);input.forward=true;}
        let before=p.pos;crate::player::update_player(p,&mut input,w,crate::simulation::STEP_SECONDS);
        distance+=p.pos.distance(before);
    }
    anyhow::bail!("Infill grounded waypoint blocked: feet={:?}, target={target:?}",p.pos)
}

pub(super) fn check(w: &World, p: &mut crate::player::Player, index: usize) -> Result<serde_json::Value> {
    let home = site(w)?.projects.get(index).and_then(|p|p.home.as_ref()).context("Missing infill home")?;
    let pile = IVec3::from(w.farming.jobs.stockpile.context("Missing stockpile")?);
    let v = w.voxel_scale.meters();ensure!((v-1.0).abs()<0.001,"Infill acceptance expects one-metre scene");
    let start = [IVec3::X,IVec3::NEG_X,IVec3::Z,IVec3::NEG_Z].into_iter().map(|d|pile+d)
        .find(|at|super::super::super::super::stand(w,*at,v)).context("Stockpile has no standing space")?;
    let start = super::super::super::super::metres(start,v);
    p.pos=start;p.precise_pos=start.as_dvec3();p.vel=Vec3::ZERO;p.flying=false;p.on_ground=false;
    let entry = IVec3::from(home.entry); let destination = IVec3::from(home.destination);
    let task = super::super::super::super::Task::Routine(entry);
    let path = super::super::super::super::path_in_range(w,start,task,4096,52.0);
    ensure!(!path.is_empty() || super::super::super::super::reachable(w,start,task,v),"Generated home has no stockpile approach");
    let mut distance=0.0;for at in path {distance+=grounded(w,p,at)?;}
    distance+=grounded(w,p,super::super::super::super::metres(entry,v))?;
    distance+=grounded(w,p,super::super::super::super::metres(destination,v))?;
    let return_task=super::super::super::super::Task::Store(pile);
    let path=super::super::super::super::path_in_range(w,p.pos,return_task,4096,52.0);
    ensure!(!path.is_empty() || super::super::super::super::reachable(w,p.pos,return_task,v),"Generated home has no stockpile return");
    for at in path {distance+=grounded(w,p,at)?;}
    ensure!(p.on_ground&&!p.flying&&super::super::super::super::reachable(w,p.pos,return_task,v),"Generated home return did not reach stockpile");
    Ok(serde_json::json!({"project":index,"grounded_round_trip_m":distance,"start":start.to_array(),"end":p.pos.to_array(),"entry":entry.to_array(),"interior":destination.to_array()}))
}
