//! Fixed-tick daily life shares worker routes, cargo and physical actors.
use super::*;
mod social;
mod policy;
pub(crate) mod check;
pub(super) use policy::Meeting;
pub(super) use policy::{Phase, Resident};

pub(super) const REST_TICKS: u64 = 16 * PULSE;
pub(super) const SOCIAL_TICKS: u64 = 12 * PULSE;
const WORK_TICKS: u64 = 240 * PULSE;
const TIMEOUT: u64 = 960 * PULSE;

pub(super) fn arrived(feet: Vec3, goal: Vec3, v: f32) -> bool {
    (feet.y-goal.y).abs() < v.max(0.25)*0.25
        && ((feet-goal)*Vec3::new(1.0,0.0,1.0)).length_squared() < 0.23*0.23
}

pub(super) fn meetings_valid(m: &[Meeting]) -> bool { policy::meetings_valid(m,crew::CAPACITY) }
pub(super) fn ate(r: &mut Resident, now: u64) {
    r.meal_committed(now);
}

pub(super) fn source_cells(j: &State, id: u32) -> Option<[IVec3; 2]> {
    if let Some(home) = j.homes.get(&id) { return Some([home.destination.into(), home.entry.into()]); }
    let site = j.quarter.as_ref()?.frontier.as_ref()?;
    let household = site.households.iter().find(|h| h.members.contains(&id))?;
    let home = site.projects.get(household.project)?.home.as_ref()?;
    Some([home.destination.into(), home.entry.into()])
}

fn home(w: &World, j: &State, id: u32) -> Option<(IVec3, IVec3, Vec<u32>)> {
    if w.travel.following == Some(id) || w.travel.guard_posts.contains_key(&id) { return None; }
    if let Some(home) = j.homes.get(&id) {
        let (destination, entry) = (IVec3::from(home.destination), IVec3::from(home.entry));
        let v = w.voxel_scale.meters();
        if !stand(w, destination, v) || !stand(w, entry, v) { return None; }
        let mut query = navigation::Query::new(w, v);
        return clear_level_walk(&mut query, metres(entry, v), metres(destination, v), v)
            .then(|| (destination, entry, crew::ids(j).collect()));
    }
    let s = j.quarter.as_ref()?.frontier.as_ref()?;
    if s.project_layers.is_empty() || s.housing_overrides.contains(&id)
        || (s.phase != s.projects.len() && s.growth.as_ref().is_none_or(|g|!g.households_live(s))) { return None; }
    let h = s.households.iter().find(|h| h.members.contains(&id))?;
    let project = s.projects.get(h.project)?;
    if !s.home_complete(w,h.project) { return None; }
    let home = project.home.as_ref()?;
    let destination = IVec3::from(home.destination);
    let entry = IVec3::from(home.entry);
    let v = w.voxel_scale.meters();
    if w.travel.homes.get(&id) != Some(&metres(destination,v)) || !stand(w, destination,v) || !stand(w,entry,v) { return None; }
    let mut query = navigation::Query::new(w,v);
    clear_level_walk(&mut query,metres(entry,v),metres(destination,v),v)
        .then(|| (destination,entry,h.members.clone()))
}

pub(super) fn interrupt(j: &mut State, now: u64) {
    if j.routine.phase != Phase::Work {j.routine.interrupted=j.routine.interrupted.saturating_add(1);}
    if matches!(j.task,Some(Task::Routine(_))) { j.reset_task(); }
    j.routine.enter(Phase::Work,now);
    j.routine.due = now.saturating_add(WORK_TICKS);
    if let Some(id) = j.worker { j.meetings.retain(|m| !m.ids.contains(&id)); }
}

/// Returns true only while this worker is owned by rest/social routing.
pub(super) fn service(w: &mut World, feet: Vec3, j: &mut State, pile: IVec3, limit: usize, range: f32) -> bool {
    let Some(id) = j.worker else { return false; };
    let now = w.farming.clock;
    let Some((destination,entry,members)) = home(w,j,id) else {
        if j.routine.phase != Phase::Work { interrupt(j,now); }
        return false;
    };
    let elapsed=if j.routine.needs_at == 0 { PULSE } else { now.saturating_sub(j.routine.needs_at) };
    j.routine.fatigue=j.routine.fatigue.saturating_add(elapsed);
    j.routine.social_need=j.routine.social_need.saturating_add(elapsed);
    j.routine.needs_at=now;
    if j.routine.due == 0 {
        j.routine.due = now.saturating_add(60*PULSE + u64::from(id%24)*8*PULSE);
        j.routine.since = now;
    }
    let hungry=j.production.as_ref().is_some_and(|p|now>=p.next_meal.get(&id).copied().unwrap_or(0));
    // Existing job validation may commit a due meal without an intervening idle pulse.
    j.routine.request_meal(now,hungry,60*PULSE);
    if j.routine.phase == Phase::Work { return false; }
    // Commit an existing delivery first; subsequent cargo goes through Store.
    if j.carry.is_some() {
        if j.task.is_none() || matches!(j.task,Some(Task::Routine(_))) {
            j.release_claims(); j.building_material=false; j.shopping=None; j.sowing=None;
            j.reset_task(); j.task=Some(Task::Store(supply::destination(j,pile)));
        }
        return false;
    }
    if j.routine.phase == Phase::Meal { return false; }
    if j.production.as_ref().is_some_and(|p|now>=p.next_meal.get(&id).copied().unwrap_or(0)) {
        interrupt(j,now);j.routine.enter(Phase::Meal,now);return false;
    }
    if now.saturating_sub(j.routine.since) > TIMEOUT {
        interrupt(j,now); return false;
    }
    if !matches!(j.task,None|Some(Task::Routine(_))) { j.reset_task(); j.release_claims(); }
    let goal = if j.routine.phase == Phase::Rest {
        // One resident occupies the narrow interior; the rest wait outside work ownership.
        if j.routine.target.is_none() && j.helpers.iter().any(|a|
            a.routine.phase==Phase::Rest && a.routine.target==Some(destination.to_array())) {
            j.status="Home Occupied"; return true;
        }
        if j.routine.target.is_none() && members.iter().any(|&other|other!=id
            && crew::physical(w,j,other).is_some_and(|at|at.distance(metres(destination,w.voxel_scale.meters()))<1.8)) {
            j.status="Home Occupied"; return true;
        }
        destination
    } else {
        let Some(goal)=social::goal(w,j,id,entry,destination,&members) else {
            j.reset_task();j.status="Social Waiting"; return true;
        };
        if j.routine.phase == Phase::Work { j.reset_task(); return false; }
        goal
    };
    j.routine.target=Some(goal.to_array());
    if !stand(w,goal,w.voxel_scale.meters()) {
        j.routine.dwell=0; j.routine.last=now; j.status="Destination Blocked"; j.reset_task(); return true;
    }
    if arrived(feet,metres(goal,w.voxel_scale.meters()),w.voxel_scale.meters()) {
        j.path.clear(); j.task=Some(Task::Routine(goal));
        if j.routine.phase==Phase::Rest {
            j.routine.observe(now,true,PULSE);
            j.status="Resting";
            super::activity::rest(w,j,feet,REST_TICKS.saturating_sub(j.routine.dwell));
            if j.routine.dwell>=REST_TICKS {
                j.routine.rests=j.routine.rests.saturating_add(1);
                j.routine.fatigue=0;
                j.routine.rested_at=now;
                j.routine.enter(Phase::Social,now); j.reset_task();
            }
        } else { j.status="Social Visit"; }
        return true;
    }
    j.routine.observe(now,false,PULSE);
    j.status=if j.routine.phase==Phase::Rest {"Homeward"} else {"Social Travel"};
    while j.path.front().is_some_and(|p|
        policy::waypoint_reached(((feet-*p)*Vec3::new(1.0,0.0,1.0)).length_squared(),
            (feet.y-p.y).abs(),w.voxel_scale.meters())) { j.path.pop_front(); }
    let stalled=j.last_walk.is_some_and(|p| feet.distance_squared(p)<0.0025);
    j.stalled=if stalled {j.stalled.saturating_add(1)} else {0}; j.last_walk=Some(feet);
    if j.stalled>=8 {
        j.reset_task(); j.retry_at=now.saturating_add(4*PULSE); j.routine.dwell=0;
    }
    if now<j.retry_at { return true; }
    let task=Task::Routine(goal);
    let obstructed=j.path.front().is_some_and(|p|!stand(w,(*p/w.voxel_scale.meters()).floor().as_ivec3(),w.voxel_scale.meters()));
    if j.task!=Some(task)||j.path.is_empty()||obstructed {
        j.task=Some(task);
        match navigation::route(w,j,feet,task,limit,range) {
            navigation::Route::Ready(path)=>j.path=path,
            navigation::Route::Pending=>{j.status="Finding route";return true;}
            navigation::Route::Blocked=>j.path.clear(),
        }
        if j.path.is_empty() { j.retry_at=now.saturating_add(8*PULSE); }
    }
    true
}

pub(crate) fn summary(w: &World) -> String {
    let j=&w.farming.jobs;
    let rows=j.production.as_ref().into_iter().flat_map(|p|p.workers.iter()).filter_map(|&id|crew::view(j,id));
    let mut rests=0u32; let mut visits=0u32; let mut resting=0; let mut social=0;
    for a in rows { rests=rests.saturating_add(a.routine.rests); visits=visits.saturating_add(a.routine.socials);
        resting+=usize::from(a.routine.phase==Phase::Rest); social+=usize::from(a.routine.phase==Phase::Social); }
    format!("Rest {resting} | Visits {social} | Rested {rests} | Socialized {visits}")
}
