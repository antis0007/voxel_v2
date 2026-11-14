//! Voluntary work circles share goals, never cargo or construction authority.
use super::{crew, known, vehicle, construction::Order, B, IVec3, State as Jobs, Task, Vec3, World};
mod model;
mod decision;
pub(crate) mod check;
pub(super) use model::State;
use model::{Goal, PERIOD};

fn activate(j:&mut Jobs) -> bool {
    // Voluntary cooperation is ordinary resident behavior in a living world.
    j.groups.enabled = true;
    j.groups.enabled
}

pub(super) fn service(w:&World,feet:Vec3,j:&mut Jobs,pile:IVec3) {
    if !activate(j) || j.routine.phase != super::routine::Phase::Work { return; }
    let Some(id) = j.worker else { return; };
    let now = w.farming.clock;
    if j.groups.people.get(&id).is_some_and(|p|now < p.due) { return; }
    let ids:Vec<_> = crew::ids(j).collect();
    j.groups.retain(&ids,now);
    for &member in &ids { j.groups.person(w.seed,member); }
    if j.routine.productive > 0 {
        let peer = ids.iter().copied().filter(|&other|other != id
            && w.travel.following != Some(other) && !w.travel.guard_posts.contains_key(&other)
            && crew::view(j,other).is_some_and(|a|a.routine.productive > 0))
            .filter_map(|other|crew::physical(w,j,other).filter(|at|at.distance_squared(feet) < 16.0)
                .map(|at|(other,at.distance_squared(feet))))
            .min_by(|a,b|a.1.total_cmp(&b.1).then(a.0.cmp(&b.0))).map(|(id,_)|id);
        if let Some(other) = peer
            && crew::physical(w,j,other).is_some_and(|at|crate::world::trace::solid(w,
                feet+Vec3::Y*0.7,at+Vec3::Y*0.7).is_none()) {
            j.groups.contact(id,other,now);
        }
    }
    let proposed = j.groups.proposed_goal(id);
    let preferred = j.groups.offers.iter().find(|o|o.to == id).map(|o|o.goal).or(proposed);
    let cursor = j.groups.people.get(&id).map_or(0,|p|p.cursor);
    let count = j.build.len();
    if count > 0 { super::supply::prepare_frame(w,j); }
    let slots = w.block_data.chest(pile);
    let v = w.voxel_scale.meters();
    let mut candidates = Vec::new();
    let mut live = false;
    for offset in 0..count.min(16) {
        let order = j.build[(cursor+offset)%count];
        j.groups.scanned_orders = j.groups.scanned_orders.saturating_add(1);
        if order.ready(w) && j.build_stock_from(w,pile,order,&slots) > 0
            && (order.point(v)-super::Task::Store(pile).point(v)).length_squared() <= super::RANGE*super::RANGE {
            let goal = Goal::at(order.at);
            live |= proposed == Some(goal);
            candidates.push((goal,feet.distance_squared(order.point(v))));
        }
    }
    if let Some(p) = j.groups.people.get_mut(&id) { p.cursor = if count == 0 {0} else {(cursor+16)%count}; }
    candidates.sort_by(|a,b|(preferred != Some(a.0)).cmp(&(preferred != Some(b.0)))
        .then_with(||a.1.total_cmp(&b.1)));
    // A partial scan cannot prove an existing goal dead.
    if !live && let Some(goal) = proposed {
        for order in &j.build {
            j.groups.goal_checks = j.groups.goal_checks.saturating_add(1);
            if goal.contains(order.at) { live=true; break; }
        }
    }
    j.groups.decide(id,now,candidates.first().map(|(goal,_)|*goal),live);
}

pub(super) fn met(j:&mut Jobs,seed:u32,ids:[u32;2],now:u64) {
    if !activate(j) { return; }
    for id in ids { j.groups.person(seed,id); }
    j.groups.contact(ids[0],ids[1],now);
}
pub(super) fn goal(j:&Jobs) -> Option<Goal> { j.groups.goal(j.worker?) }
pub(super) fn preference(goal:Option<Goal>,task:Task) -> bool {
    matches!(task,Task::Build(o)|Task::Fetch(_,o)|Task::Fund(_,o) if goal.is_some_and(|g|g.contains(o.at)))
}
pub(super) fn selected(j:&mut Jobs,task:Task) {
    if preference(goal(j),task) { j.groups.selections = j.groups.selections.saturating_add(1); }
}
pub(super) fn committed(j:&mut Jobs,order:Order,now:u64) {
    if let Some(id) = j.worker { j.groups.committed(id,order.at,now); }
}
pub(crate) fn report(w:&World) -> serde_json::Value {
    let s = &w.farming.jobs.groups;
    serde_json::json!({"kind":"voluntary_work_circles","enabled":s.enabled,"circles":s.circles,"offers":s.offers,
        "people":s.people,"bonds":s.bonds,"events":s.events,"reviews":s.reviews,
        "scanned_orders":s.scanned_orders,"group_task_selections":s.selections,"accepted":s.accepted,"exits":s.exits,"splits":s.splits,
        "decision_period_ticks":PERIOD,"order_candidate_checks_per_review":16,
        "goal_membership_checks":s.goal_checks,"max_goal_membership_checks_per_review":256,"max_people":model::PEOPLE})
}
