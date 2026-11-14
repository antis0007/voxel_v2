//! Bounded local construction crew. Shared orders and physical cargo keep one owner.
use super::*;

pub(crate) const CAPACITY: usize = 64;
pub(super) const MAX_HELPERS: usize = CAPACITY - 1;
const CONSTRUCTORS: u32 = 10;
const CONSTRUCTOR_BASE: u32 = 0x6000_0000;

pub(super) fn service_phase(index:usize)->u64 { (index as u64+1)*super::PULSE/CAPACITY as u64 }

pub(super) fn phase_indices(phase: u64, helpers: usize) -> std::ops::Range<usize> {
    let first = (phase * CAPACITY as u64).div_ceil(super::PULSE).max(1) as usize;
    let end = ((phase + 1) * CAPACITY as u64).div_ceil(super::PULSE) as usize;
    first.min(helpers + 1) - 1..end.min(helpers + 1) - 1
}

pub(crate) fn is_constructor(id: u32) -> bool {
    (CONSTRUCTOR_BASE..CONSTRUCTOR_BASE + CONSTRUCTORS).contains(&id)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Worker {
    #[serde(default)] sowing: Option<production::Bed>,
    #[serde(default)] pub(super) routine: routine::Resident,
    worker: Option<u32>,
    worker_pos: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")] body: Option<vehicle::ResidentBody>,
    #[serde(default)] motion_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")] activity: Option<activity::Commitment>,
    #[serde(default, skip_serializing_if = "Option::is_none")] batch: Option<production::Batch>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub(super) mission: Option<communities::transit::Mission>,
    carry: Option<ItemStack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cargo: Option<supply::Cargo>,
    building_material: bool,
    #[serde(default)] shopping: Option<market::Trip>,
    #[serde(default)] delivery: Option<[i32; 3]>,
    #[serde(default)] pub(super) priorities: Option<[u8; 3]>,
    #[serde(default)] haul_scan_cursor: usize,
    #[serde(default)] task: Option<Task>,
    #[serde(skip)] path: VecDeque<Vec3>,
    #[serde(skip)] navigation_wait: bool,
    #[serde(default)] work: u8,
    #[serde(skip)] last_placement: Option<(u64, Vec3)>,
    #[serde(default)] retry_at: u64,
    #[serde(default)] next_candidate: usize,
    #[serde(skip)] status: &'static str,
    #[serde(skip)] last_walk: Option<Vec3>,
    #[serde(skip)] stalled: u8,
}

impl Worker {
    pub(super) fn swap(&mut self, j: &mut State) {
        macro_rules! fields { ($($f:ident),*) => { $(std::mem::swap(&mut self.$f, &mut j.$f);)* }; }
        fields!(routine, sowing, worker, worker_pos, body, motion_at, activity, batch, mission, carry, cargo, building_material, shopping, task, path, navigation_wait, work,
            last_placement, retry_at, next_candidate, status, last_walk, stalled, haul_scan_cursor);
    }

    pub(super) fn valid(&self) -> bool {
        self.work <= 60 && self.task.is_none_or(Task::valid) && self.routine.valid() && self.sowing.is_none_or(|b| b.valid() && self.carry == Some(ItemStack::new(crate::world::inventory::Item::Block(b.crop), 1)) && !self.building_material && self.shopping.is_none())
            && self.body.is_none_or(vehicle::ResidentBody::valid)
            && self.activity.as_ref().is_none_or(activity::Commitment::valid)
            && self.batch.as_ref().is_none_or(production::Batch::valid)
            && self.mission.as_ref().is_none_or(|m| m.valid() && self.carry.is_none()
                && self.batch.is_none() && self.shopping.is_none())
            && self.worker.is_some()
            && self.worker_pos.is_none_or(|p| Vec3::from(p).is_finite() && p.iter().all(|n| n.abs() < 1.0e7))
            && self.carry.is_none_or(|s| s.count > 0)
            && (!self.building_material || self.carry.is_some())
            && self.cargo.is_none_or(|c| c.valid(self.carry, self.building_material) && self.shopping.is_none() && self.sowing.is_none())
            && self.shopping.is_none_or(|t| t.valid(self.carry) && !self.building_material)
            && self.delivery.is_none_or(|p| p.iter().all(|n| n.unsigned_abs() < 1 << 28))
            && self.priorities.is_none_or(|p| p.into_iter().all(|n| n <= 4))
    }

    pub(super) fn reset(&mut self) {
        self.activity = None;
        self.navigation_wait = false;
        self.delivery = None;
        self.priorities = None;
        self.task = None;
        self.path.clear();
        self.work = 0;
        self.retry_at = 0;
        self.next_candidate = 0;
        self.last_walk = None;
        self.stalled = 0;
    }
}

pub(super) struct View<'a> {
    pub activity: Option<&'a activity::Commitment>,
    pub navigation_wait: bool,
    pub routine: &'a routine::Resident,
    pub path: &'a VecDeque<Vec3>,
    pub task: Option<Task>,
    pub work: u8,
    pub last_placement: Option<(u64, Vec3)>,
    pub carry: Option<ItemStack>,
    pub batch: bool,
    pub mission: bool,
    pub cargo: Option<supply::Cargo>,
    pub building_material: bool,
    pub pos: Option<[f32; 3]>,
    pub shopping: Option<market::Trip>,
    pub status: &'static str,
    pub priorities: Option<[u8; 3]>,
}

pub(super) fn view(j: &State, id: u32) -> Option<View<'_>> {
    if j.worker == Some(id) {
        Some(View { activity: j.activity.as_ref(), navigation_wait: j.navigation_wait, routine: &j.routine, path: &j.path, task: j.task, work: j.work,
            last_placement: j.last_placement, carry: j.carry, batch: j.batch.is_some(), cargo: j.cargo, building_material: j.building_material, pos: j.worker_pos, shopping: j.shopping,
            status: j.status, priorities: j.priorities, mission: j.mission.is_some() })
    } else {
        j.helpers.iter().find(|a| a.worker == Some(id)).map(|a| View { activity: a.activity.as_ref(), navigation_wait: a.navigation_wait,
            routine: &a.routine, path: &a.path, task: a.task, work: a.work, last_placement: a.last_placement,
            carry: a.carry, batch: a.batch.is_some(), cargo: a.cargo, building_material: a.building_material, pos: a.worker_pos, shopping: a.shopping,
            status: a.status, priorities: a.priorities, mission: a.mission.is_some() })
    }
}

#[derive(Clone, Copy)]
pub(super) struct Load {
    pub id: Option<u32>,
    pub carry: Option<ItemStack>,
    pub cargo: Option<supply::Cargo>,
    pub task: Option<Task>,
}

pub(super) fn loads(j: &State) -> impl Iterator<Item=Load> + '_ {
    std::iter::once(Load {id:j.worker,carry:j.carry,cargo:j.cargo,task:j.task})
        .chain(j.helpers.iter().map(|a|Load {id:a.worker,carry:a.carry,cargo:a.cargo,task:a.task}))
        .filter(|a|a.id.is_some())
}

pub(crate) fn add(world: &mut World, id: u32) -> bool {
    if !enabled(world) || assigned(world, id) || world.farming.jobs.helpers.len() >= MAX_HELPERS
        || world.travel.guard_posts.contains_key(&id) {
        return false;
    }
    let Some(pos) = vehicle::villager_feet(world, id) else { return false };
    match communities::enlist(world, id) {
        Ok(true) => {
            if world.travel.following == Some(id) { world.travel.following = None; }
            return true;
        }
        Err(_) => return false,
        Ok(false) => {}
    }
    world.farming.jobs.helpers.push(Worker { worker: Some(id), worker_pos: Some(pos.to_array()),
        motion_at: world.farming.clock, ..Worker::default() });
    if world.travel.following == Some(id) { world.travel.following = None; }
    true
}

pub(crate) fn count(world: &World) -> usize {
    let jobs = &world.farming.jobs;
    usize::from(jobs.worker.is_some()) + jobs.helpers.len()
}

pub(crate) fn recruit(world: &mut World, player: Vec3) -> Result<usize, &'static str> {
    if !enabled(world) { return Err("Construction crew unavailable"); }
    let pile = world.farming.jobs.stockpile.map(IVec3::from).ok_or("No Stockpile")?;
    if !known(world, pile).is_some_and(|block| block.chest_tier().is_some()) {
        return Err("Stockpile Unavailable");
    }
    let v = world.voxel_scale.meters();
    if metres(pile, v).distance_squared(player) > RANGE * RANGE { return Err("Stockpile Out of Range"); }
    let spacing = (1.0 / v).ceil() as i32;
    let mut spots = (-5..=5).flat_map(|x| (-5..=5).map(move |z| (x, z)))
        .filter(|&(x, z)| x * x + z * z <= 25).collect::<Vec<_>>();
    spots.sort_by_key(|&(x, z)| (x * x + z * z, x, z));
    let mut recruits = 0;
    for id in CONSTRUCTOR_BASE..CONSTRUCTOR_BASE + CONSTRUCTORS {
        if count(world) >= CAPACITY || assigned(world, id) { continue; }
        if vehicle::villager_actor(world, id).is_none() {
            if world.travel.homes.contains_key(&id) || world.travel.homes.len() >= 64 { continue; }
            let Some(pos) = spots.iter().find_map(|&(x, z)| {
                [0, 1, -1, 2, -2].into_iter().map(|y| pile + IVec3::new(x * spacing, y, z * spacing))
                    .filter(|&at| stand(world, at, v)).map(|at| metres(at, v)).find(|&feet|
                        ((feet - player) * Vec3::new(1.0, 0.0, 1.0)).length_squared() > 2.25 && world.adventure.enemies.iter()
                            .all(|actor| actor.health <= 0.0 || actor.pos.distance_squared(feet) >= 0.95 * 0.95))
            }) else { break };
            vehicle::spawn_resident(world, id, pos);
            world.travel.homes.insert(id, pos);
        }
        if world.farming.jobs.worker.is_none() {
            super::assign(world, id);
            if world.farming.jobs.worker != Some(id) { continue; }
            world.farming.jobs.priorities = Some([0, 0, 1]);
        } else if add(world, id) {
            if let Some(worker) = member(&mut world.farming.jobs, id) { worker.priorities = Some([0, 0, 1]); }
        } else {
            continue;
        }
        recruits += 1;
    }
    if recruits == 0 { Err("No Constructor Slots") } else { Ok(recruits) }
}

pub(super) fn member(j: &mut State, id: u32) -> Option<&mut Worker> {
    j.helpers.iter_mut().find(|helper| helper.worker == Some(id))
}

pub(super) fn take_batch(j: &mut State, id: u32) -> Option<production::Batch> {
    if j.worker == Some(id) { j.batch.take() }
    else { member(j, id).and_then(|worker| worker.batch.take()) }
}

pub(super) fn remove(j: &mut State, id: u32) {
    j.helpers.retain(|helper| helper.worker != Some(id));
    j.meetings.retain(|m|!m.ids.contains(&id));
    j.claims.retain(|claim| claim.worker != id);
}

pub(super) fn promote(j: &mut State) {
    if let Some(index) = j.helpers.iter().position(|helper| helper.delivery.is_none()) {
        let mut next = j.helpers.remove(index);
        next.swap(j);
        if let Some(role) = next.priorities { j.priorities = Some(role); }
    }
}

pub(super) fn new_worker(id: u32, position: Vec3, now: u64) -> Worker {
    Worker { worker: Some(id), worker_pos: Some(position.to_array()), motion_at: now,
        routine: routine::Resident { needs_at: now, ..routine::Resident::default() }, ..Worker::default() }
}

pub(super) fn take_worker(j: &mut State, id: u32) -> Option<Worker> {
    j.claims.retain(|c| c.worker != id);
    j.meetings.retain(|m| !m.ids.contains(&id));
    if j.worker == Some(id) {
        let mut worker = Worker { priorities: j.priorities, ..Worker::default() };
        worker.swap(j);
        promote(j);
        Some(worker)
    } else {
        let index = j.helpers.iter().position(|w| w.worker == Some(id))?;
        Some(j.helpers.remove(index))
    }
}

pub(super) fn put_worker(j: &mut State, mut worker: Worker) {
    debug_assert!(ids(j).count() < CAPACITY);
    if j.worker.is_none() {
        let priorities = worker.priorities;
        worker.swap(j);
        if priorities.is_some() { j.priorities = priorities; }
    } else { j.helpers.push(worker); }
}

pub(super) fn service_person(w: &mut World, player: Vec3, id: u32) {
    if w.farming.jobs.worker == Some(id) {
        super::service_worker(w, player);
        if w.farming.jobs.worker.is_none() { promote(&mut w.farming.jobs); }
    } else if let Some(index) = w.farming.jobs.helpers.iter().position(|a| a.worker == Some(id)) {
        service(w, player, index);
    }
}

pub(super) fn interrupt_activity(j: &mut State, id: u32, now: u64) -> bool {
    if j.worker == Some(id) {
        activity::interrupt(j, now);
        return true;
    }
    let Some(index) = j.helpers.iter().position(|a|a.worker == Some(id)) else { return false; };
    // Only the selected person's state moves; the roster and inventories retain ownership.
    let mut worker = std::mem::take(&mut j.helpers[index]);
    worker.swap(j);
    activity::interrupt(j, now);
    worker.swap(j);
    j.helpers[index] = worker;
    true
}

/// Retire old certificates once at the serialized checkpoint, preserving custody.
pub(in crate::world::farming) fn migrate_legacy_commitments(j: &mut State, clock: u64) -> Vec<u32> {
    let mut migrated = Vec::new();
    if activity::migrate_legacy_commitment(j, clock) {
        migrated.extend(j.worker);
    }
    for index in 0..j.helpers.len() {
        let mut worker = std::mem::take(&mut j.helpers[index]);
        worker.swap(j);
        if activity::migrate_legacy_commitment(j, clock) {
            migrated.extend(j.worker);
        }
        worker.swap(j);
        j.helpers[index] = worker;
    }
    migrated
}

pub(super) fn next_wake(j: &State, id: u32, now: u64) -> u64 {
    let earliest = now.saturating_add(PULSE);
    let Some(view) = view(j, id) else { return earliest; };
    if view.status == "Awaiting terrain" { return earliest; }
    if let Some(activity) = view.activity { return activity.wake_at(j,id).max(now.saturating_add(1)); }
    if view.navigation_wait { return now.saturating_add(600); }
    let retry = if j.worker == Some(id) { j.retry_at }
        else { j.helpers.iter().find(|w| w.worker == Some(id)).map_or(0, |w| w.retry_at) };
    let motion_at = if j.worker == Some(id) { j.motion_at }
        else { j.helpers.iter().find(|w|w.worker == Some(id)).map_or(now,|w|w.motion_at) };
    if !view.path.is_empty() && motion_at < now { return now.saturating_add(1); }
    let body = if j.worker == Some(id) { j.body }
        else { j.helpers.iter().find(|w| w.worker == Some(id)).and_then(|w| w.body) };
    if body.is_some_and(vehicle::ResidentBody::needs_tick) { return earliest; }
    if view.task.is_some() || !view.path.is_empty() || view.carry.is_some() || view.batch || view.mission
        || view.routine.phase != routine::Phase::Work { return earliest; }
    let mut due = if retry > now { retry.max(earliest) }
        else { now.saturating_add(if j.homes.is_empty() { PULSE } else { 6000 }) };
    if view.routine.due > now { due = due.min(view.routine.due.max(earliest)); }
    if let Some(meal) = j.production.as_ref().and_then(|p| p.next_meal.get(&id)).copied().filter(|&t| t > now) {
        due = due.min(meal.max(earliest));
    }
    due
}

pub(super) fn set_health(j: &mut State, id: u32, health: f32) {
    if j.worker == Some(id) { j.body = j.body.map(|body| body.with_health(health)); }
    else if let Some(worker) = member(j, id) { worker.body = worker.body.map(|body| body.with_health(health)); }
}

pub(super) fn set_position(j: &mut State, id: u32, at: Vec3, now: u64) {
    if j.worker == Some(id) {
        j.worker_pos=Some(at.to_array()); j.motion_at=now; j.reset_task();
    } else if let Some(worker)=member(j,id) {
        worker.worker_pos=Some(at.to_array()); worker.motion_at=now; worker.reset();
    }
}

pub(super) fn mission(j: &State, id: u32) -> Option<&communities::transit::Mission> {
    if j.worker == Some(id) { j.mission.as_ref() }
    else { j.helpers.iter().find(|w| w.worker == Some(id)).and_then(|w| w.mission.as_ref()) }
}

pub(super) fn set_mission(j: &mut State, id: u32, mission: communities::transit::Mission) {
    j.claims.retain(|c| c.worker != id);
    if j.worker == Some(id) { j.reset_task(); j.mission = Some(mission); }
    else if let Some(w) = member(j,id) { w.reset(); w.mission = Some(mission); }
}

pub(super) fn end_mission(j: &mut State, id: u32) {
    if j.worker == Some(id) { j.mission = None; j.reset_task(); }
    else if let Some(w) = member(j,id) { w.mission = None; w.reset(); }
}

pub(super) fn service(world: &mut World, player: Vec3, index: usize) {
    let jobs = &mut world.farming.jobs;
    if index >= jobs.helpers.len() { return; }
    let mut active = std::mem::take(&mut jobs.helpers[index]);
    let stockpile = jobs.stockpile;
    let priorities = jobs.priorities;
    if let Some(delivery) = active.delivery { jobs.stockpile = Some(delivery); }
    if let Some(role) = active.priorities { jobs.priorities = Some(role); }
    active.swap(jobs);
    // Keep the other workers' cargo and plans visible during assignment.
    jobs.helpers[index] = active;
    super::service_worker(world, player);
    let jobs = &mut world.farming.jobs;
    let mut active = std::mem::take(&mut jobs.helpers[index]);
    active.swap(jobs);
    if active.worker.is_some() { jobs.helpers[index] = active; }
    else { jobs.helpers.remove(index); }
    jobs.stockpile = stockpile;
    jobs.priorities = priorities;
}

pub(super) fn planned(j: &State, task: Task) -> bool {
    j.helpers.iter().filter_map(|worker| worker.task).any(|other| match (task, other) {
        (Task::Fetch(_, a) | Task::Fund(_, a), Task::Fetch(_, b) | Task::Fund(_, b)) => a == b,
        (Task::Mine(a, _), Task::Mine(b, _)) => a == b,
        (Task::Produce(production::Task::Craft(a, _) | production::Task::Smelt(a,_)),
         Task::Produce(production::Task::Craft(b, _) | production::Task::Smelt(b,_))) => a == b,
        (Task::Haul(a), Task::Haul(b)) => a == b,
        (Task::Recover(a, _), Task::Recover(b, _) | Task::Haul(b))
            | (Task::Haul(a), Task::Recover(b, _)) => a == b,
        (Task::Supply(..), Task::Supply(..) | Task::Recover(..))
            | (Task::Recover(..), Task::Supply(..)) => true,
        _ => false,
    })
}

pub(super) fn station_available(j: &State, station: IVec3) -> bool {
    j.helpers.iter().all(|worker| worker.batch.as_ref()
        .is_none_or(|batch| batch.station != station.to_array()))
        && !planned(j, Task::Produce(production::Task::Craft(station, 0)))
}

pub(super) fn fell(world: &mut World, id: u32, feet: Vec3) -> bool {
    let Some(index) = world.farming.jobs.helpers.iter().position(|a| a.worker == Some(id)) else { return false };
    let mut helper = world.farming.jobs.helpers.remove(index);
    helper.swap(&mut world.farming.jobs);
    super::clear_worker(world, feet);
    helper.swap(&mut world.farming.jobs);
    true
}

pub(crate) fn saved_workers(world: &World) -> Vec<(u32, Vec3)> {
    saved_workers_in(world,None)
}

pub(crate) fn saved_workers_near(world:&World,center:Vec3,radius:f32)->Vec<(u32,Vec3)> {
    saved_workers_in(world,Some((center,radius*radius)))
}

fn saved_workers_in(world:&World,near:Option<(Vec3,f32)>)->Vec<(u32,Vec3)> {
    let mut out=Vec::new();
    for j in std::iter::once(&world.farming.jobs).chain(communities::all_jobs(world)) {
        let workers=j.worker.zip(j.worker_pos).map(|(id,at)|(id,at,j.activity.as_ref())).into_iter()
            .chain(j.helpers.iter().filter_map(|a|a.worker.zip(a.worker_pos).map(|(id,at)|(id,at,a.activity.as_ref()))));
        for (id,saved,activity) in workers {
            let at=activity.and_then(|a|a.position(world.farming.clock)).unwrap_or_else(||saved.into());
            if near.is_some_and(|(center,r2)|at.distance_squared(center)>r2) {continue;}
            if world.travel.following==Some(id)
                || (world.travel.politics.mobilized(id) && !world.travel.politics.physical_march(id)) {continue;}
            out.push((id,at));
        }
    }
    out
}

pub(crate) fn body(world: &World, id: u32) -> Option<vehicle::ResidentBody> {
    let j = super::resident_state(world, id)?;
    if j.worker == Some(id) { j.body }
    else { j.helpers.iter().find(|a| a.worker == Some(id)).and_then(|a| a.body) }
}

pub(super) fn cargo(j: &State) -> u32 {
    j.carry.map_or(0, |s| s.count) + j.helpers.iter().filter_map(|a| a.carry).map(|s| s.count).sum::<u32>()
}

pub(super) fn identities_valid(j: &State) -> bool {
    let mut ids = std::collections::HashSet::new();
    j.worker.into_iter().chain(j.helpers.iter().filter_map(|a| a.worker)).all(|id| ids.insert(id))
}

pub(super) fn supplier(world: &mut World, id: u32, delivery: IVec3) -> bool {
    if world.farming.jobs.worker.is_none() { return false; }
    if !add(world, id) { return false; }
    let Some(worker) = world.farming.jobs.helpers.iter_mut().find(|a| a.worker == Some(id)) else { return false };
    worker.delivery = Some(delivery.to_array());
    worker.priorities = Some([0, 1, 0]);
    true
}

pub(super) fn cargo_item(j: &State, item: crate::world::inventory::Item) -> u32 {
    j.carry.into_iter().chain(j.helpers.iter().filter_map(|a| a.carry))
        .filter(|s| s.item == item).map(|s| s.count).sum()
}

pub(super) fn ids(j:&State) -> impl Iterator<Item=u32> + '_ {
    j.worker.into_iter().chain(j.helpers.iter().filter_map(|a|a.worker))
}

/// Interaction uses persistent positions even when no visible actor is needed.
pub(super) fn physical(w: &World, j: &State, id: u32) -> Option<Vec3> {
    if let Some(at) = view(j,id).and_then(|v|v.activity).and_then(|a|a.position(w.farming.clock)) { return Some(at); }
    if let Some(actor) = vehicle::villager_actor(w, id) {
        return (!vehicle::seeking_safety(actor)).then_some(actor.pos);
    }
    if w.travel.following == Some(id) || w.travel.guard_posts.contains_key(&id) { return None; }
    view(j, id).and_then(|worker| worker.pos).map(Vec3::from)
}

pub(super) fn checkpoint(j: &mut State, actors: &[crate::world::adventure::Enemy], now: u64) {
    for actor in actors {
        let Some(id) = vehicle::villager_id(actor).filter(|_| actor.health > 0.0) else { continue; };
        if j.worker == Some(id) {
            j.worker_pos = Some(actor.pos.to_array());
            j.body = Some(vehicle::ResidentBody::capture(actor));
            j.motion_at = now;
        } else if let Some(worker) = j.helpers.iter_mut().find(|worker| worker.worker == Some(id)) {
            worker.worker_pos = Some(actor.pos.to_array());
            worker.body = Some(vehicle::ResidentBody::capture(actor));
            worker.motion_at = now;
        }
    }
}
