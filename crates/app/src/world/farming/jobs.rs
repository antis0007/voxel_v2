use super::{World, resident};
use crate::world::{BlockId as B, adventure::wildlife::vehicle, block_data, inventory::ItemStack};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, VecDeque},
};

mod construction;
pub(crate) mod onboarding;
pub(crate) mod production;
pub(crate) mod crew;
pub(crate) mod communities;
pub(crate) mod activity;
mod reservations;
mod supply;
mod level_walk;
mod residency;
pub(crate) mod routine;
pub(crate) mod groups;
mod drop_scan;
mod market;
pub(crate) mod quarter;
pub(crate) mod review;
use construction::Order;
pub(crate) use construction::queue_blueprint;
pub(crate) use quarter::frontier::{enable_housing_growth, check_housing_growth_funding, check_housing_growth_sites};

const RANGE: f32 = 24.0;
const LIMIT: usize = 64;
const PULSE: u64 = (0.25 / engine_core::time::PLAYER_STEP_SECONDS + 0.5) as u64;

// Agent decisions never generate or synchronously read terrain. Source admission
// belongs to residency; an unknown cell is a dependency, not an empty voxel.
fn known(world: &World, at: IVec3) -> Option<B> { world.resident_block(at) }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct State {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    homes: std::collections::BTreeMap<u32, communities::Home>,
    #[serde(default)]
    territory: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) quarter: Option<quarter::State>,
    #[serde(default)]
    helpers: Vec<crew::Worker>,
    #[serde(default)]
    routine: routine::Resident,
    #[serde(default)]
    meetings: Vec<routine::Meeting>,
    #[serde(default, skip_serializing_if = "groups::State::is_empty")]
    groups: groups::State,
    #[serde(default)]
    claims: Vec<reservations::Claim>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    market: Option<market::Market>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shopping: Option<market::Trip>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    production: Option<production::State>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sowing: Option<production::Bed>,
    worker: Option<u32>,
    worker_pos: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    body: Option<vehicle::ResidentBody>,
    #[serde(default)]
    motion_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    activity: Option<activity::Commitment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    batch: Option<production::Batch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mission: Option<communities::transit::Mission>,
    stockpile: Option<[i32; 3]>,
    mine: Vec<([i32; 3], u16)>,
    carry: Option<ItemStack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cargo: Option<supply::Cargo>,
    #[serde(default)]
    build: Vec<Order>,
    #[serde(default)]
    building_material: bool,
    #[serde(default)]
    priorities: Option<[u8; 3]>,
    #[serde(default)]
    haul_scan_cursor: usize,
    #[serde(default)]
    task: Option<Task>,
    #[serde(skip)]
    path: VecDeque<Vec3>,
    #[serde(skip)]
    navigation_wait: bool,
    #[serde(default)]
    work: u8,
    #[serde(skip)]
    last_placement: Option<(u64, Vec3)>,
    #[serde(default)]
    retry_at: u64,
    #[serde(default)]
    next_candidate: usize,
    #[serde(skip)]
    status: &'static str,
    #[serde(skip)]
    last_walk: Option<Vec3>,
    #[serde(skip)]
    stalled: u8,
    #[serde(skip)]
    hub_access: Option<bool>,
    #[serde(skip)]
    supply_frame: Option<supply::Frame>,
    #[serde(skip)]
    sources: residency::Sources,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum Task {
    Routine(IVec3),
    Produce(production::Task),
    Store(IVec3),
    Mine(IVec3, u16),
    Haul(Vec3),
    Fetch(IVec3, Order),
    Supply(IVec3, crate::world::inventory::Item),
    Recover(Vec3, crate::world::inventory::Item),
    Fund(IVec3, Order),
    Buy(IVec3),
    Build(Order),
}

impl Task {
    fn valid(self) -> bool {
        let cell = |p: IVec3| p.to_array().iter().all(|n| n.unsigned_abs() < 1 << 28);
        let order = |o: Order| cell(o.cell()) && o.material().is_some();
        let point = |p: Vec3| p.is_finite() && p.abs().max_element() < 1.0e7;
        match self {
            Self::Routine(p) | Self::Store(p) | Self::Buy(p) => cell(p),
            Self::Mine(p, b) => cell(p) && mineable(B::from_u32(u32::from(b))),
            Self::Fetch(p, o) | Self::Fund(p, o) => cell(p) && order(o),
            Self::Build(o) => order(o),
            Self::Haul(p) | Self::Recover(p, _) => point(p),
            Self::Supply(p, _) => cell(p),
            Self::Produce(t) => t.valid(),
        }
    }

    fn point(self, v: f32) -> Vec3 {
        match self {
            Self::Store(p) | Self::Mine(p, _) | Self::Fetch(p, _) | Self::Supply(p, _) | Self::Fund(p, _) | Self::Buy(p) => {
                (p.as_vec3() + Vec3::splat(0.5)) * v
            }
            Self::Routine(p) => metres(p, v),
            Self::Produce(t) => t.point(v),
            Self::Haul(p) => p,
            Self::Recover(p, _) => p + Vec3::Y * 0.1,
            Self::Build(order) => order.point(v),
        }
    }

    fn priority(self, p: [u8; 3]) -> u8 {
        match self {
            Self::Produce(t) => t.priority(p),
            Self::Haul(_) => p[0],
            Self::Mine(..) => p[1],
            Self::Build(_) | Self::Fetch(..) | Self::Supply(..) | Self::Recover(..) | Self::Fund(..) | Self::Buy(_) => p[2],
            Self::Store(_) | Self::Routine(_) => 1,
        }
    }
}

impl State {
    pub(super) fn checkpoint(&self, actors: &[crate::world::adventure::Enemy], now: u64) -> Self {
        let mut saved = self.clone();
        crew::checkpoint(&mut saved, actors, now);
        saved
    }

    pub(super) fn used(&self) -> bool {
        !self.groups.is_empty() || self.production.is_some() || self.sowing.is_some() || self.quarter.is_some() || self.market.is_some() || self.worker.is_some() || !self.helpers.is_empty()
            || self.stockpile.is_some()
            || !self.mine.is_empty()
            || !self.build.is_empty()
            || self.priorities.is_some()
            || self.carry.is_some()
    }

    pub(super) fn valid(&self) -> bool {
        let bounded = |p: &[i32; 3]| p.iter().all(|n| n.unsigned_abs() < (1 << 28));
        self.work <= 60 && self.task.is_none_or(Task::valid)
            && self.activity.as_ref().is_none_or(activity::Commitment::valid)
            && self.homes.len() <= crew::CAPACITY && self.homes.values().all(communities::Home::valid)
            && self.territory.is_finite() && (0.0..=128.0).contains(&self.territory)
            && self.body.is_none_or(vehicle::ResidentBody::valid)
            && self.batch.as_ref().is_none_or(|batch| self.worker.is_some() && batch.valid())
            && self.mission.as_ref().is_none_or(|m| m.valid() && self.worker.is_some()
                && self.carry.is_none() && self.batch.is_none() && self.shopping.is_none())
            && self.groups.valid() && self.routine.valid() && routine::meetings_valid(&self.meetings)
            && self.production.as_ref().is_none_or(production::State::valid)
            && self.sowing.is_none_or(|b| b.valid() && self.carry == Some(ItemStack::new(crate::world::inventory::Item::Block(b.crop), 1)) && !self.building_material && self.shopping.is_none())
            && self.helpers.len() <= crew::MAX_HELPERS
            && self.helpers.iter().all(crew::Worker::valid)
            && crew::identities_valid(self)
            && self.claims_valid()
            && supply::valid(self)
            && self.market.as_ref().is_none_or(market::Market::valid)
            && self.shopping.is_none_or(|t| t.valid(self.carry) && !self.building_material)
            && self.mine.len() <= LIMIT
            && self.quarter.as_ref().is_none_or(quarter::State::valid)
            && self.stockpile.as_ref().is_none_or(bounded)
            && self.mine.iter().all(|(p, _)| bounded(p))
            && self.build.len() <= construction::LIMIT
            && self
                .build
                .iter()
                .all(|o| bounded(&o.at) && o.material().is_some())
            && self
                .priorities
                .is_none_or(|p| p.into_iter().all(|n| n <= 4))
            && (!self.building_material || self.carry.is_some())
            && self.cargo.is_none_or(|c| c.valid(self.carry,self.building_material) && self.shopping.is_none() && self.sowing.is_none())
            && self
                .worker_pos
                .is_none_or(|p| Vec3::from(p).is_finite() && p.iter().all(|v| v.abs() < 1.0e7))
            && self
                .carry
                .is_none_or(|s| s.count > 0 && self.worker.is_some())
    }

    fn reset_task(&mut self) {
        self.activity = None;
        self.navigation_wait = false;
        self.task = None;
        self.path.clear();
        self.work = 0;
        self.retry_at = 0;
        self.last_walk = None;
        self.stalled = 0;
    }

    fn priorities(&self) -> [u8; 3] {
        self.priorities.unwrap_or([2; 3])
    }
}

fn enabled(_world: &World) -> bool {
    #[cfg(feature = "multiplayer")]
    if _world.multiplayer.is_some() {
        return false;
    }
    _world.sample_config().civilization_enabled() && _world.ca_view.is_none()
}

pub(crate) fn colony_resident(world:&World,id:u32)->bool {
    let Some(j) = resident_state(world, id) else { return false; };
    j.quarter.as_ref().and_then(|q|q.frontier.as_ref()).is_some_and(|s|!s.project_layers.is_empty())
        && j.production.as_ref().is_some_and(|p|p.workers.contains(&id))
}

fn resident_state(world: &World, id: u32) -> Option<&State> {
    if crew::view(&world.farming.jobs, id).is_some() { Some(&world.farming.jobs) }
    else { communities::state_for(world, id) }
}

pub(crate) fn assigned(world: &World, id: u32) -> bool {
    crew::view(&world.farming.jobs, id).is_some()
}

pub(crate) fn priorities(world: &World) -> [u8; 3] {
    world.farming.jobs.priorities()
}

pub(crate) fn priorities_for(world: &World, id: u32) -> [u8; 3] {
    resident_state(world, id).map_or([2; 3], |j|
        crew::view(j, id).and_then(|worker| worker.priorities).unwrap_or_else(|| j.priorities()))
}

pub(crate) fn set_priorities(world: &mut World, id: u32, priorities: [u8; 3]) {
    if enabled(world) && assigned(world, id) && priorities.into_iter().all(|n| n <= 4) {
        let jobs = &mut world.farming.jobs;
        if let Some(helper) = crew::member(jobs, id) {
            helper.priorities = Some(priorities);
            helper.reset();
            return;
        }
        jobs.priorities = Some(priorities);
        jobs.reset_task();
        jobs.next_candidate = 0;
        for helper in &mut jobs.helpers {
            if helper.priorities.is_none() { helper.reset(); }
        }
    }
}

pub(crate) fn clear_builds(world: &mut World, id: u32) {
    if enabled(world) && assigned(world, id) {
        world.farming.jobs.build.clear();
        world.farming.jobs.claims.clear();
        world.farming.jobs.reset_task();
        world.farming.jobs.next_candidate = 0;
        for helper in &mut world.farming.jobs.helpers { helper.reset(); }
    }
}

pub(crate) fn assign(world: &mut World, id: u32) {
    if !enabled(world) { return; }
    if let Some(worker) = crew::view(&world.farming.jobs, id) {
        if worker.carry.is_some() || worker.shopping.is_some() || worker.mission {
            world.expedition.say("Delivery Active");
            return;
        }
        crew::interrupt_activity(&mut world.farming.jobs,id,world.farming.clock);
        activity::forget(world,id);
        if let Some(batch) = crew::take_batch(&mut world.farming.jobs, id) {
            recover_batch(world, batch);
        }
        if communities::return_home(world, id) {
            world.expedition.say("Resident Released");
            return;
        }
        let jobs = &mut world.farming.jobs;
        if jobs.worker == Some(id) {
            jobs.release_claims();
            jobs.worker = None;
            jobs.worker_pos = None;
            jobs.body = None;
            jobs.motion_at = 0;
            jobs.reset_task();
            jobs.last_placement = None;
            crew::promote(jobs);
        } else {
            crew::remove(jobs, id);
        }
        world.expedition.say("Settler released");
        return;
    }
    if world.travel.guard_posts.contains_key(&id) {
        world.expedition.say("Release Guard First");
        return;
    }
    let Some(feet) = vehicle::villager_feet(world, id) else {
        return;
    };
    match communities::enlist(world, id) {
        Ok(true) => {
            if world.travel.following == Some(id) { world.travel.following = None; }
            world.expedition.say("Settler assigned");
            return;
        }
        Err(message) => { world.expedition.say(message); return; }
        Ok(false) => {}
    }
    if world.farming.jobs.worker.is_some() {
        let message = if crew::add(world, id) { "Settler assigned" } else { "Construction crew full" };
        world.expedition.say(message);
        return;
    }
    let now = world.farming.clock;
    let jobs = &mut world.farming.jobs;
    jobs.routine = routine::Resident::default();
    jobs.worker = Some(id);
    jobs.worker_pos = Some(feet.to_array());
    jobs.body = None;
    jobs.motion_at = now;
    jobs.reset_task();
    jobs.last_placement = None;
    jobs.next_candidate = 0;
    if world.travel.following == Some(id) {
        world.travel.following = None;
    }
    world.expedition.say("Settler assigned");
}


fn mineable(b: B) -> bool {
    matches!(
        b,
        B::Stone
            | B::Dirt
            | B::Grass
            | B::Sand
            | B::Gravel
            | B::Clay
            | B::Sandstone
            | B::Basalt
            | B::Cobblestone
            | B::Limestone
            | B::Log
            | B::CoalOre | B::IronOre | B::CopperOre | B::TinOre
            | B::Chalcopyrite | B::Malachite | B::Hematite | B::Magnetite | B::Cassiterite
            | B::GoldQuartz
    )
}

pub(crate) fn designate(world: &mut World, p: IVec3) -> bool {
    if !enabled(world) || world.farming.jobs.worker.is_none() {
        return false;
    }
    let b = world.get(p);
    let jobs = &mut world.farming.jobs;
    let key = p.to_array();
    let message = if b.chest_tier().is_some() {
        jobs.stockpile = (jobs.stockpile != Some(key) || jobs.carry.is_some()).then_some(key);
        if jobs.stockpile.is_some() {
            "Stockpile marked"
        } else {
            "Stockpile cleared"
        }
    } else if let Some(i) = jobs.mine.iter().position(|&(at, _)| at == key) {
        jobs.mine.remove(i);
        "Work designation removed"
    } else if mineable(b) {
        if jobs.mine.len() >= LIMIT {
            "Work queue full"
        } else {
            jobs.mine.push((key, b as u16));
            if b == B::Log {
                "Marked for cutting"
            } else {
                "Marked for mining"
            }
        }
    } else {
        return false;
    };
    jobs.reset_task();
    jobs.next_candidate = 0;
    world.expedition.say(message);
    true
}

pub(crate) fn status(world: &World) -> String {
    status_for(world, world.farming.jobs.worker)
}

pub(crate) fn status_for(world: &World, id: Option<u32>) -> String {
    let j = id.and_then(|id| resident_state(world, id)).unwrap_or(&world.farming.jobs);
    let worker = id.and_then(|id| crew::view(j, id));
    let status = worker.as_ref().map(|worker| worker.status).filter(|status| !status.is_empty()).unwrap_or("Waiting");
    let pile = if j.stockpile.is_some() {
        "Chest marked"
    } else {
        "Mark a chest"
    };
    let orders = format!("{status} · {pile} · Mine {} · Build {}", j.mine.len(), j.build.len());
    match worker.and_then(|worker| worker.carry) {
        Some(stack) => format!("{orders} · Carrying {}", stack.count),
        None => orders,
    }
}

pub(crate) fn markers(world: &World) -> impl Iterator<Item = (IVec3, [f32; 4])> + '_ {
    let jobs = &world.farming.jobs;
    jobs.stockpile
        .iter()
        .map(|&p| (IVec3::from(p), [0.30, 0.85, 0.68, 0.9]))
        .chain(
            jobs.mine
                .iter()
                .map(|&(p, _)| (IVec3::from(p), [1.0, 0.68, 0.23, 0.85])),
        )
        .chain(
            jobs.build
                .iter()
                .map(|o| (o.cell(), [0.34, 0.72, 1.0, 0.75])),
        )
}

pub(crate) fn worker_fell(world: &mut World, id: u32, feet: Vec3) {
    if crew::view(&world.farming.jobs, id).is_none() {
        communities::fell(world, id, feet);
        return;
    }
    communities::mark_dead(world, id);
    if world.farming.jobs.worker != Some(id) && crew::fell(world, id, feet) { return; }
    if world.farming.jobs.worker != Some(id) { return; }
    clear_worker(world, feet);
    crew::promote(&mut world.farming.jobs);
}

pub(crate) fn remember_worker(world: &mut World, actor: &crate::world::adventure::Enemy) {
    crew::checkpoint(&mut world.farming.jobs, std::slice::from_ref(actor), world.farming.clock);
    communities::remember(world, actor);
}

fn recover_batch(world: &mut World, batch: production::Batch) {
    let pos = (IVec3::from(batch.station).as_vec3() + Vec3::new(0.5, 1.0, 0.5)) * world.voxel_scale.meters();
    for stack in batch.recovery() {
        if world.inventory.dropped.len() < 1024 {
            world.inventory.dropped.push(crate::world::inventory::DroppedStack { pos, stack });
        } else { world.inventory.overflow.push(stack); }
    }
}

fn clear_worker(world: &mut World, feet: Vec3) {
    let at = world.farming.jobs.worker.and_then(|id|world.farming.activities.invalidated_at(id))
        .unwrap_or(world.farming.clock);
    activity::interrupt(&mut world.farming.jobs,at);
    if let Some(id)=world.farming.jobs.worker {activity::forget(world,id);}
    if let Some(mission) = world.farming.jobs.mission.take() {
        for stack in mission.cargo {
            if world.inventory.dropped.len() < 1024 {
                world.inventory.dropped.push(crate::world::inventory::DroppedStack { pos: feet, stack });
            } else { world.inventory.overflow.push(stack); }
        }
    }
    if let Some(batch) = world.farming.jobs.batch.take() { recover_batch(world, batch); }
    let jobs = &mut world.farming.jobs;
    jobs.release_claims();
    if let Some(stack) = jobs.carry.take() {
        if world.inventory.dropped.len() < 1024 {
            world
                .inventory
                .dropped
                .push(crate::world::inventory::DroppedStack { pos: feet, stack });
        } else {
            world.inventory.overflow.push(stack);
        }
    }
    if let Some(id)=jobs.worker { jobs.meetings.retain(|m|!m.ids.contains(&id)); }
    jobs.routine = routine::Resident::default();
    jobs.worker = None;
    jobs.shopping = None;
    jobs.sowing = None;
    jobs.building_material = false;
    jobs.cargo = None;
    jobs.worker_pos = None;
    jobs.body = None;
    jobs.motion_at = 0;
    jobs.reset_task();
    jobs.last_placement = None;
}

pub(crate) fn goal(world: &World, id: u32, feet: Vec3) -> Option<Vec3> {
    crew::view(resident_state(world, id)?, id).map(|a| a.path.front().copied().unwrap_or(feet))
}

pub(crate) fn route_speed_multiplier(world: &World, id: u32) -> Option<f32> {
    let worker = crew::view(resident_state(world, id)?, id)?;
    if worker.path.is_empty() {
        return None;
    }
    Some(task_route_speed(worker.task))
}

fn task_route_speed(task: Option<Task>) -> f32 {
    match task {
        Some(Task::Build(_) | Task::Fetch(..) | Task::Fund(..) | Task::Buy(_)) => 2.0,
        _ => 1.0,
    }
}

#[derive(Clone, Copy)]
pub(crate) enum WorkPose {
    Mine { target: Vec3, phase: f32, timber: bool },
    Place { target: Vec3, elapsed: f32 },
}

impl WorkPose {
    pub(crate) fn target(self) -> Vec3 {
        match self {
            Self::Mine { target, .. } | Self::Place { target, .. } => target,
        }
    }
}

/// Read the same work and cargo state that owns world edits and inventory transfers.
pub(crate) fn presentation(world: &World, id: u32) -> (Option<WorkPose>, bool) {
    let Some(j) = resident_state(world, id).and_then(|j| crew::view(j, id)) else { return (None, false) };
    let clock = world.farming.clock;
    let progress = j.activity.and_then(|a|a.progress(clock)).unwrap_or(j.work);
    let pose = if let Some(Task::Mine(p, expected)) = j.task
        && progress > 0 && known(world,p).is_some_and(|b|b as u16 == expected)
    {
        Some(WorkPose::Mine {
            target: Task::Mine(p, expected).point(world.voxel_scale.meters()),
            phase: (clock % PULSE) as f32 / PULSE as f32,
            timber: expected == B::Log as u16,
        })
    } else if let Some(Task::Produce(production::Task::Fell(bed))) = j.task
        && progress > 0 && known(world,bed.at.into()) == Some(B::Log) {
        Some(WorkPose::Mine {
            target: (IVec3::from(bed.at).as_vec3()+Vec3::splat(0.5))*world.voxel_scale.meters(),
            phase: (clock % PULSE) as f32 / PULSE as f32,
            timber: true,
        })
    } else {
        j.last_placement.and_then(|(at, target)| {
            let elapsed = clock.saturating_sub(at);
            (elapsed < PULSE).then_some(WorkPose::Place { target, elapsed: elapsed as f32 / PULSE as f32 })
        })
    };
    (pose, j.carry.is_some())
}

fn stand(world: &World, p: IVec3, v: f32) -> bool {
    known(world, p - IVec3::Y).is_some_and(B::is_solid)
        && (0..(1.8 / v).ceil() as i32).all(|dy| known(world, p + IVec3::Y * dy) == Some(B::Air))
}

fn metres(p: IVec3, v: f32) -> Vec3 {
    (p.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * v
}

fn in_reach(feet: Vec3, target: Vec3, task: Task, v: f32) -> bool {
    if matches!(task, Task::Routine(_)) { return routine::arrived(feet, target, v); }
    let delta = target - feet;
    let flat = delta.x.hypot(delta.z);
    let overhead = matches!(task, Task::Build(_)) && delta.y - v * 0.5 >= 1.8;
    let reach = if overhead { 2.25 } else { 1.65 };
    (feet + Vec3::Y * 0.7).distance(target) <= reach
        && (!matches!(task, Task::Mine(..)) || flat >= v * 0.45)
        && (!matches!(task, Task::Build(_))
            || delta.y + v * 0.5 <= 0.0
            || delta.y - v * 0.5 >= 1.8
            // Millimetre tolerance keeps equivalent cell-edge work positions
            // reachable after translating a parcel across float boundaries.
            || delta.x.abs().max(delta.z.abs()) + 0.001 >= v * 0.5 + 0.4)
}

fn reachable(world: &World, feet: Vec3, task: Task, v: f32) -> bool {
    let target = task.point(v);
    if let Task::Routine(at) = task { return stand(world, at, v) && routine::arrived(feet, target, v); }
    in_reach(feet, target, task, v)
        && {
            // The normal trace is safe only after every crossed source page is
            // resident; path planning must not generate terrain synchronously.
            let from=feet+Vec3::Y*0.7;
            let lo=crate::world::chunk_coord_from_world_block((from.min(target)/v).floor().as_ivec3());
            let hi=crate::world::chunk_coord_from_world_block((from.max(target)/v).floor().as_ivec3());
            (lo.x..=hi.x).all(|x|(lo.y..=hi.y).all(|y|(lo.z..=hi.z)
                .all(|z|world.chunks.contains_key(&IVec3::new(x,y,z)))))
        }
        && match crate::world::trace::solid(world, feet + Vec3::Y * 0.7, target) {
            None => true,
            Some((_, hit)) => match task {
                Task::Mine(p, _) | Task::Store(p) | Task::Fetch(p, _) | Task::Supply(p, _) | Task::Fund(p, _) | Task::Buy(p) => hit == p,
                Task::Produce(t) => hit == t.cell(),
                Task::Haul(_) | Task::Recover(..) | Task::Build(_) | Task::Routine(_) => false,
            },
        }
}

mod navigation;

fn path(world: &World, feet: Vec3, task: Task) -> VecDeque<Vec3> {
    path_with_limit(world, feet, task, 1100)
}

fn paved(world: &World, cell: IVec3) -> bool {
    known(world, cell - IVec3::Y) == Some(B::Cobblestone)
}

fn clear_level_walk(query: &mut navigation::Query<'_>, from: Vec3, to: Vec3, v: f32) -> bool {
    if from.y != to.y { return false; }
    let steps = (from.distance(to) / 0.1).ceil().max(1.0) as u32;
    (0..=steps).all(|i| {
        let point = level_walk::point(from,to,i as f32 / steps as f32);
        // Include half a sample step plus the worker's clearance on each side.
        let margin = Vec3::new(0.45, 0.0, 0.45);
        let low = ((point - margin) / v).floor().as_ivec3();
        let high = ((point + margin) / v).floor().as_ivec3();
        (low.x..=high.x).all(|x| (low.z..=high.z).all(|z| query.stand(IVec3::new(x, low.y, z))))
    })
}

fn path_with_limit(world: &World, feet: Vec3, task: Task, limit: usize) -> VecDeque<Vec3> {
    path_in_range(world, feet, task, limit, RANGE)
}

fn path_in_range(world: &World, feet: Vec3, task: Task, limit: usize, range: f32) -> VecDeque<Vec3> {
    let v = world.voxel_scale.meters();
    let mut query = navigation::Query::new(world, v);
    let target = task.point(v);
    let start = ((feet + Vec3::Y * 0.15) / v).floor().as_ivec3();
    // Estimate distance to the work radius at the cheapest possible surface rate.
    let reach = if matches!(task, Task::Build(_)) { 2.25 } else { 1.65 };
    let priority = |p: IVec3, cost| {
        let distance = (metres(p, v) + Vec3::Y * 0.7).distance(target);
        cost + (((distance - reach).max(0.0) / v) * 900.0).floor() as u32
    };
    let mut queue = BinaryHeap::from([Reverse((priority(start, 0), Reverse(0), start.to_array()))]);
    let mut prev = HashMap::from([(start, (start, 0))]);
    while let Some(Reverse((_, Reverse(cost), at))) = queue.pop() {
        let at = IVec3::from(at);
        if prev[&at].1 != cost {
            continue;
        }
        // Arrival stops within 0.1 m. A ray that only clears a voxel corner at
        // the exact cell centre is not a usable work position.
        if let Some(point) = navigation::work_point(world, at, v)
            && reachable(world, point, task, v)
            && [-0.1, 0.1].into_iter().all(|x| [-0.1, 0.1].into_iter()
                .all(|z| reachable(world, point + Vec3::new(x, 0.0, z), task, v))) {
            let mut route = VecDeque::new();
            let mut cell = at;
            let mut follows_paving = false;
            let mut level = true;
            while cell != start {
                route.push_front(metres(cell, v));
                let parent = prev[&cell].0;
                follows_paving |= query.paved(cell) && query.paved(parent);
                level &= cell.y == parent.y;
                cell = parent;
            }
            if let Some(last) = route.back_mut() { *last = point; }
            // On open ground, walk straight to the work position at any angle.
            if route.len() > 1 && level && !follows_paving
                && (feet.y - point.y).abs() < 0.01
                && clear_level_walk(&mut query, Vec3::new(feet.x, point.y, feet.z), point, v)
            {
                return VecDeque::from([point]);
            }
            if route.is_empty() && !reachable(world, feet, task, v) {
                route.push_back(point);
            }
            return route;
        }
        if prev.len() >= limit {
            break;
        }
        for (x, z) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let horizontal = IVec3::new(x, 0, z);
            let n = at + horizontal;
            let diagonal = x != 0 && z != 0;
            // Only cut across a corner when both bordering cells have clear,
            // level footing. Changes of elevation use the normal step edges.
            if diagonal && (!query.stand(at + IVec3::X * x) || !query.stand(at + IVec3::Z * z)) {
                continue;
            }
            let max = if diagonal { 0 } else { (1.0 / v).floor() as i32 };
            let next = std::iter::once(0)
                .chain((1..=max).flat_map(|dy| [dy, -dy]))
                .map(|dy| n + IVec3::Y * dy)
                .find(|&p| query.stand(p));
            let Some(next) = next else { continue };
            if diagonal && !query.diagonal_clear(at, next) { continue; }
            if next.y != at.y {
                let high = at.y.max(next.y);
                let height = (1.8 / v).ceil() as i32;
                // A standable lower cell can still have a roof lip that blocks
                // the body before gravity lowers it across the step edge.
                if [at, next].into_iter().any(|p| (high..high + height)
                    .any(|y| known(world, IVec3::new(p.x, y, p.z)) != Some(B::Air))) {
                    continue;
                }
            }
            let rate = if query.paved(at) && query.paved(next) { 900.0 } else { 1000.0 };
            let step = cost + ((next - at).as_vec3().length() * rate).ceil() as u32;
            if metres(next, v).distance_squared(feet) <= range * range
                && prev.get(&next).is_none_or(|&(_, best)| step < best)
            {
                prev.insert(next, (at, step));
                queue.push(Reverse((priority(next, step), Reverse(step), next.to_array())));
            }
        }
    }
    VecDeque::new()
}

pub(crate) fn service(world: &mut World, player: Vec3) {
    let phase = world.farming.clock % PULSE;
    if !enabled(world) { return; }
    activity::rebuild(world);
    activity::wake_disrupted(world);
    navigation::service(world);
    if phase == 0 { quarter::service(world); }
    if world.farming.jobs.quarter.as_ref().is_some_and(|q| q.paused) {
        communities::service(world, player);
        return;
    }
    if phase == 0 {
        let mut jobs = std::mem::take(&mut world.farming.jobs);
        jobs.mine.retain(|&(p, expected)| {
            known(world, IVec3::from(p)).is_none_or(|b| mineable(b) && b as u16 == expected)
        });
        jobs.build.retain(|&order| known(world, order.cell()) != Some(order.block()));
        jobs.claims.retain(|claim| jobs.build.contains(&claim.order));
        world.farming.jobs = jobs;
        service_worker(world, player);
        if world.farming.jobs.worker.is_none() { crew::promote(&mut world.farming.jobs); }
    }
    // Visit only the phase's contiguous roster slice, independent of roster size.
    for index in crew::phase_indices(phase, world.farming.jobs.helpers.len()) {
        crew::service(world, player, index);
    }
    activity::flush(world);
    communities::service(world, player);
}

fn service_worker(world: &mut World, player: Vec3) {
    supply::clear_frame(&mut world.farming.jobs);
    let Some(id) = world.farming.jobs.worker else { return };
    if world.travel.following == Some(id) || world.travel.guard_posts.contains_key(&id)
        || (world.travel.politics.mobilized(id) && world.farming.jobs.mission.is_none()) {
        routine::interrupt(&mut world.farming.jobs, world.farming.clock);
        world.farming.jobs.release_claims();
        world.farming.jobs.building_material = false;
        world.farming.jobs.reset_task();
        world.farming.jobs.last_placement = None;
        world.farming.jobs.status = if world.travel.politics.mobilized(id) {"On expedition"}
            else if world.travel.following == Some(id) {"Following"} else {"Guarding"};
        return;
    }
    if world.farming.jobs.activity.is_some() {
        let mut jobs = std::mem::take(&mut world.farming.jobs);
        let sleeping = activity::resume(world, &mut jobs);
        activity::register(world, &jobs);
        world.farming.jobs = jobs;
        if sleeping { return; }
    }
    let worker = vehicle::villager_actor(world, id);
    let active = worker.map(|actor| actor.pos);
    let body = worker.map(vehicle::ResidentBody::capture);
    if worker.is_some_and(vehicle::seeking_safety) {
        routine::interrupt(&mut world.farming.jobs, world.farming.clock);
        let jobs = &mut world.farming.jobs;
        jobs.worker_pos = active.map(|position| position.to_array());
        jobs.body = body;
        jobs.motion_at = world.farming.clock;
        jobs.release_claims();
        jobs.building_material = false;
        jobs.reset_task();
        jobs.last_placement = None;
        jobs.status = "Seeking safety";
        return;
    }
    let saved = world.farming.jobs.worker_pos.map(Vec3::from);
    if body.is_some() { world.farming.jobs.body = body; }
    let Some(mut feet) = active.or(saved) else {
        world.farming.jobs.status = "Awaiting resident";
        return;
    };
    let elapsed = world.farming.clock.saturating_sub(world.farming.jobs.motion_at).min(PULSE);
    world.farming.jobs.worker_pos = Some(feet.to_array());
    if !residency::prepare(world, feet) {
        world.farming.jobs.status = "Awaiting terrain";
        // Neither lack of a render actor nor delayed I/O cancels accepted work.
        world.farming.jobs.last_walk = None;
        world.farming.jobs.stalled = 0;
        return;
    }
    if active.is_none() {
        let goal = world.farming.jobs.path.front().copied().unwrap_or(feet);
        let (at, body) = vehicle::advance_worker(world, id, feet, goal, elapsed, world.farming.jobs.body);
        feet = at;
        if !body.alive() || feet.y <= -256.0 {
            communities::mark_dead(world, id);
            clear_worker(world, feet);
            let today = crate::world::travel::day(world);
            world.travel.away.insert(id, today);
            return;
        }
        world.farming.jobs.body = Some(body);
        // Keep every unprocessed physical step. Scheduling pressure must never
        // erase time; certified ordinary travel avoids this substep path entirely.
        world.farming.jobs.motion_at = world.farming.jobs.motion_at.saturating_add(elapsed);
        if body.can_schedule() && feet.distance_squared(goal)<0.0064 {
            // A quiet stationary interval has no remaining physical work.
            world.farming.jobs.motion_at=world.farming.clock;
            world.farming.jobs.body=Some(body.parked());
        }
        if body.seeking_safety() {
            routine::interrupt(&mut world.farming.jobs, world.farming.clock);
            let jobs = &mut world.farming.jobs;
            jobs.worker_pos = Some(feet.to_array());
            jobs.release_claims();
            jobs.building_material = false;
            jobs.reset_task();
            jobs.status = "Seeking safety";
            return;
        }
    }
    if active.is_some() { world.farming.jobs.motion_at = world.farming.clock; }
    world.farming.jobs.worker_pos = Some(feet.to_array());
    if active.is_none() && world.farming.jobs.motion_at<world.farming.clock
        && !world.farming.jobs.path.is_empty() { return; }
    let mut j = std::mem::take(&mut world.farming.jobs);
    work(world, feet, player, &mut j);
    activity::travel(world, &mut j);
    if j.activity.is_some() { j.motion_at = world.farming.clock; }
    activity::register(world, &j);
    supply::clear_frame(&mut j);
    world.farming.jobs = j;
}

fn work(world: &mut World, feet: Vec3, player: Vec3, j: &mut State) {
    if communities::transit::walk(world, feet, j) { return; }
    if j.carry.is_none() { j.cargo = None; }
    let v = world.voxel_scale.meters();
    let colony=!j.homes.is_empty() || j.quarter.as_ref().and_then(|q|q.frontier.as_ref()).is_some_and(|s|!s.project_layers.is_empty());
    // Routine local work gets a small deterministic search budget. The larger
    // search rotates across identities; its seven-pulse period also avoids
    // permanently missing residents that retry every two or eight pulses.
    let broad=(world.farming.clock/PULSE+u64::from(j.worker.unwrap_or(0))).is_multiple_of(7);
    let path_range = if colony { work_range(j) + 28.0 } else {RANGE};
    let path_limit = if colony&&!broad {768} else if j.quarter.is_some() { 4096 } else { 1100 };
    let Some(pile) = j.stockpile.map(IVec3::from) else {
        j.status = "Mark a stockpile chest";
        return;
    };
    let Some(stockpile_block) = known(world, pile) else {
        j.status = "Stockpile unloaded";
        return;
    };
    if stockpile_block.chest_tier().is_none() {
        j.status = "Stockpile missing";
        j.reset_task();
        return;
    }
    if !j.claims.is_empty() || j.building_material { supply::prepare_frame(world,j); }
    j.reconcile_claims();
    if routine::service(world, feet, j, pile, path_limit, path_range) { return; }
    groups::service(world, feet, j, pile);
    let priorities = j.priorities();
    if j.task.is_none() && world.farming.clock < j.retry_at {
        return;
    }
    if j.shopping.is_some_and(|trip| !market::wanted(j, trip) || priorities[2] == 0) {
        j.shopping = None;
    }
    if !matches!(j.task,Some(Task::Store(_))) { supply::prepare_frame(world,j); }
    if !j.task.is_some_and(|task| task_still_valid(world, feet, player, j, pile, task)) {
        supply::prepare_frame(world,j);
        select_task(world, feet, player, j, pile, path_limit, path_range);
    }
    let Some(task) = j.task else { return };
    // Follow cell centres around corners before advancing to the next waypoint.
    while j.path.front().is_some_and(|p| {
        ((feet - *p) * Vec3::new(1.0, 0.0, 1.0)).length_squared() < 0.01
            && (feet.y - p.y).abs() < v.max(0.25) * 0.25
    }) {
        j.path.pop_front();
    }
    if !reachable(world, feet, task, v) {
        j.status = match task {
            Task::Produce(t) => t.label(),
            Task::Mine(..) => "Walking to mine",
            Task::Fetch(..) => "Fetching building materials",
            Task::Fund(..) => "Collecting market payment",
            Task::Buy(_) => "Walking to timber merchant",
            Task::Build(_) => "Walking to build",
            _ => "Hauling",
        };
        j.stalled = if !j.path.is_empty() && j.last_walk.is_some_and(|p| j.path.front().map_or(
            feet.distance_squared(p)<0.0025,|goal|p.distance(*goal)-feet.distance(*goal)<0.025)) {
            j.stalled.saturating_add(1)
        } else { 0 };
        j.last_walk = Some(feet);
        // A blocked carrier must not reserve a roof or doorway forever. Keep
        // the actual cargo, return it to storage, and let another worker try.
        // Repeating the same failed route also wastes the full search budget.
        if j.stalled >= 8 {
            j.release_claims();
            j.building_material = false;
            j.sowing = None;
            j.shopping = None;
            j.reset_task();
            j.retry_at = world.farming.clock.saturating_add(2 * PULSE);
            j.status = "Blocked route: returning cargo";
            return;
        }
        let obstructed = j.path.front().is_some_and(|p| !stand(world, (*p / v).floor().as_ivec3(), v));
        // Preserve a valid descent until the actor reaches it. Rebuilding it
        // every second can send a slow walker back to the same ledge forever.
        if j.path.is_empty() || obstructed {
            match navigation::route(world,j,feet,task,path_limit,path_range) {
                navigation::Route::Ready(path) => j.path=path,
                navigation::Route::Pending => { j.status="Finding route"; return; }
                navigation::Route::Blocked => j.path.clear(),
            }
            // Replanning is not movement. Only an actual position change resets
            // the stall count, otherwise an obstructed waypoint pins its claim.
            if j.path.is_empty() {
                j.release_claims();
                j.building_material = false;
                j.reset_task();
            }
        }
        j.work = 0;
        return;
    }
    j.path.clear();
    let before_carry=j.carry;
    let before_build=j.build.len();
    let before_production=j.production.as_ref().map(|p|(p.counts.planted,p.counts.harvested,p.counts.meals,p.counts.batches));
    match task {
        Task::Routine(_) => {},
        Task::Produce(t) => { if !production::execute(world, j, t) { return; } },
        Task::Store(p) => {
            let Some(mut stack) = j.carry.take() else {
                j.reset_task();
                return;
            };
            block_data::store_hauled(world, p, &mut stack);
            j.carry = (stack.count != 0).then_some(stack);
            j.building_material &= j.carry.is_some();
            if j.carry.is_none() { j.shopping = None; j.sowing = None; j.cargo = None; }
            j.release_claims();
            j.status = if j.carry.is_some() {
                "Stockpile full"
            } else {
                "Delivered"
            };
        }
        Task::Haul(pos) => {
            if let Some(i) = world
                .inventory
                .dropped
                .iter()
                .position(|d| d.pos == pos && d.stack.count != 0)
            {
                j.carry = Some(world.inventory.dropped.remove(i).stack);
                j.status = "Hauling";
            }
        }
        Task::Mine(p, expected) => {
            let b = world.get(p);
            j.status = if b == B::Log {
                "Cutting timber"
            } else {
                "Mining"
            };
            let hardness = b.material().physics.hardness;
            let pulses = (3.0 + 4.0 * hardness).ceil().clamp(3.0, 60.0) as u8;
            if !activity::work(world, j, pulses, p) {
                return;
            }
            if resident(world, p)
                && mineable(b)
                && b as u16 == expected
                && let Some(item) = block_data::blast_drop(b)
            {
                if crate::world::farming::base::overlaps(world, p, p) { j.status = "Protected Base"; j.reset_task(); return; }
                if let Ok(commit) = crate::world::effects::replace(world, p, b, B::Air) {
                    let mut changed = Vec::new();
                    commit.publish_voxel(world, &mut changed);
                    world.adventure.dirty_voxels.extend(changed);
                    j.carry = Some(ItemStack::new(item, 1));
                    market::mined(j, p);
                    j.mine.retain(|&(at, _)| at != p.to_array());
                }
            }
        }
        Task::Fetch(pile, order) => construction::fetch(world, j, pile, order),
        Task::Supply(..) | Task::Recover(..) => supply::execute(world,j,task),
        Task::Fund(pile, order) => market::fund(world, j, pile, order),
        Task::Buy(seller) => market::buy(world, j, seller),
        Task::Build(order) => construction::place(world, j, order, player),
    }
    let changed = before_build != j.build.len() || before_production != j.production.as_ref().map(|p|(p.counts.planted,p.counts.harvested,p.counts.meals,p.counts.batches))
        || (matches!(task,Task::Store(_)|Task::Mine(..)) && before_carry!=j.carry);
    if changed {
        j.routine.productive=j.routine.productive.saturating_add(1);
        if j.routine.phase==routine::Phase::Work && j.routine.socials>0 { j.routine.resumed=j.routine.resumed.saturating_add(1); }
    }
    j.reset_task();
}

fn within_job_range(jobs: &State, feet: Vec3, pile: Vec3, target: Vec3) -> bool {
    let colony=!jobs.homes.is_empty() || jobs.quarter.as_ref().and_then(|q|q.frontier.as_ref())
        .is_some_and(|s|!s.project_layers.is_empty());
    pile.distance_squared(target) <= work_range(jobs).powi(2)
        && (colony || feet.distance_squared(target) <= RANGE * RANGE)
}

fn work_range(jobs: &State) -> f32 {
    if !jobs.homes.is_empty() { jobs.territory.max(48.0) } else { RANGE }
}

fn task_still_valid(world: &World, feet: Vec3, player: Vec3, jobs: &State, pile: IVec3, task: Task) -> bool {
    if task.priority(jobs.priorities()) == 0 || crew::planned(jobs, task)
        || !supply::in_range(world,jobs,feet,pile,task)
    {
        return false;
    }
    let accepts = |stack: ItemStack| {
        let limit = world.inventory.stack_limit(stack.item);
        world.block_data.can_store_chest(pile) && world.block_data.chest(pile).iter().map(|slot| match slot {
            None => limit,
            Some(s) if s.item == stack.item => limit.saturating_sub(s.count),
            _ => 0,
        }).sum::<u32>() >= stack.count
    };
    match task {
        Task::Routine(_) => false,
        Task::Store(at) => supply::store_valid(world,jobs,pile,at),
        Task::Supply(..) | Task::Recover(..) => supply::task_valid(world,jobs,task),
        Task::Mine(at, block) => jobs.carry.is_none() && jobs.mine.contains(&(at.to_array(), block))
            && !crate::world::farming::base::overlaps(world, at, at)
            && known(world, at).filter(|b| *b as u16 == block).and_then(block_data::blast_drop)
                .is_some_and(|item| accepts(ItemStack::new(item, 1))),
        Task::Haul(at) => jobs.carry.is_none() && world.inventory.dropped.iter()
            .any(|drop| drop.pos == at && drop.stack.count > 0 && accepts(drop.stack)),
        Task::Fetch(at, order) | Task::Fund(at, order) => {
            (if matches!(task,Task::Fetch(..)) { supply::source(world,jobs,order) == Some(at) }
                else { at == pile && !supply::remote(jobs,order) })
                && jobs.carry.is_none() && jobs.available(order) && jobs.build.contains(&order)
                && order.ready(world) && construction::unoccupied(world, order, player)
                && {
                    let slots = world.block_data.chest(at);
                    if matches!(task, Task::Fetch(..)) {
                        jobs.build_stock_from(world,at,order,&slots) > 0
                    } else {
                        market::candidate(world,jobs,pile,order,&slots)
                    }
                }
        }
        Task::Build(order) => jobs.building_material && jobs.owns(order) && jobs.build.contains(&order)
            && supply::matches_source(jobs,jobs.cargo,order)
            && order.ready(world) && construction::unoccupied(world, order, player)
            && construction::workers_clear(jobs, order, world.voxel_scale.meters()),
        Task::Buy(at) => jobs.shopping.is_some_and(|trip| trip.seller == at.to_array()),
        Task::Produce(production_task) => production::task_valid(world, jobs, pile, production_task),
    }
}

fn select_task(world: &mut World, feet: Vec3, player: Vec3, j: &mut State, pile: IVec3, path_limit: usize, path_range: f32) {
    let mut haul_scan_cursor = j.haul_scan_cursor;
    let drop_indices = drop_scan::window(&mut haul_scan_cursor, world.inventory.dropped.len());
    let mut scanned_drops = false;
    let v = world.voxel_scale.meters();
    let priorities = j.priorities();
    let pile_m = Task::Store(pile).point(v);
    let near = |point: Vec3| within_job_range(j, feet, pile_m, point);
    let mut tasks = Vec::new();
    let mut blocked_by_storage = false;
    if let Some(stack) = j.carry {
        if let Some(task) = production::sowing(world, j) {
            tasks.push(Task::Produce(task));
        } else if let Some(trip) = j.shopping {
            tasks.push(Task::Buy(trip.seller.into()));
        } else if j.building_material && priorities[2] > 0 {
            tasks.extend(
                j.build
                    .iter()
                    .copied()
                    .filter(|&o| {
                        o.material() == Some(stack.item) && j.owns(o)
                            && supply::in_range(world,j,feet,pile,Task::Build(o))
                            && supply::matches_source(j,j.cargo,o) && o.ready(world)
                            && construction::unoccupied(world, o, player)
                            && construction::workers_clear(j, o, v)
                    })
                    .map(Task::Build),
            );
        }
        let target = supply::destination(j,pile);
        if supply::store_valid(world,j,pile,target) { tasks.push(Task::Store(target)); }
    } else {
        let slots = world.block_data.chest(pile);
        let storage_available = world.block_data.can_store_chest(pile);
        let mut accepts = |stack: ItemStack| {
            let limit = world.inventory.stack_limit(stack.item);
            let room: u32 = slots
                .iter()
                .map(|slot| match slot {
                    None => limit,
                    Some(s) if s.item == stack.item => limit.saturating_sub(s.count),
                    Some(_) => 0,
                })
                .sum();
            let fits = storage_available && room >= stack.count;
            blocked_by_storage |= !fits;
            fits
        };
        if near(pile_m) {
            if priorities[2] > 0 {
                tasks.extend(j.build.iter().copied().filter_map(|o| {
                    let from = supply::source(world,j,o)?;
                    if !supply::in_range(world,j,feet,pile,Task::Build(o)) || !j.available(o)
                        || crew::planned(j,Task::Fetch(from,o)) || !construction::unoccupied(world,o,player)
                        || !o.ready(world) { return None; }
                    if j.build_stock(world,from,o) > 0 { Some(Task::Fetch(from,o)) }
                    else if from == pile && !supply::remote(j,o) && market::candidate(world,j,pile,o,&slots) {
                        Some(Task::Fund(pile,o))
                    } else { None }
                }));
            }
            tasks.extend(j.mine.iter().filter_map(|&(p, expected)| {
                let p = IVec3::from(p);
                if crate::world::farming::base::overlaps(world, p, p) || !near(Task::Mine(p, expected).point(v)) {
                    return None;
                }
                let b = known(world, p)?;
                block_data::blast_drop(b)
                    .filter(|&item| accepts(ItemStack::new(item, 1)))
                    .map(|_| Task::Mine(p, expected))
            }));
            scanned_drops = true;
            tasks.extend(
                drop_indices.map(|i| &world.inventory.dropped[i])
                    .filter(|d| d.stack.count > 0 && near(d.pos) && accepts(d.stack))
                    .map(|d| Task::Haul(d.pos)),
            );
        }
    }
    if j.carry.is_none() {
        tasks.extend(supply::candidates(world,j));
        tasks.extend(production::candidates(world, j, pile));
    }
    // Finish leaving a work area even if other workers own the remaining cells.
    if j.carry.is_none() && j.task == Some(Task::Store(pile)) {
        tasks.push(Task::Store(pile));
    }
    tasks.retain(|&task| task.priority(priorities) > 0 && supply::in_range(world,j,feet,pile,task) && !crew::planned(j, task));
    if scanned_drops { j.haul_scan_cursor = haul_scan_cursor; }
    {
        j.reset_task();
        let scarce = production::food_scarce(world, j);
        let prefer_delivery = j.building_material || production::sowing(world, j).is_some();
        let group_goal = groups::goal(j);
        let key = |task: Task| {
            let priority = if prefer_delivery && matches!(task, Task::Store(_)) {
                u8::MAX
            } else {
                production::work_priority(j, task, priorities, scarce)
            };
            let distance = match task {
                Task::Fetch(from, order) | Task::Fund(from, order) => Task::Store(from).point(v).distance_squared(order.point(v)),
                _ => feet.distance_squared(task.point(v)),
            };
            (priority, !groups::preference(group_goal, task), distance)
        };
        tasks.sort_by(|&a, &b| {
            let (ap, ag, ad) = key(a);
            let (bp, bg, bd) = key(b);
            ap.cmp(&bp).then(ag.cmp(&bg)).then_with(|| ad.total_cmp(&bd))
        });
        let count = tasks.len();
        let start = j.next_candidate.min(count.saturating_sub(1));
        let attempts = count.min(if j.helpers.is_empty() { 4 } else { 2 });
        for offset in 0..attempts {
            let task = tasks[(start + offset) % count];
            let at_work = reachable(world, feet, task, v);
            let route = if at_work {
                VecDeque::new()
            } else {
                match navigation::route(world,j,feet,task,path_limit,path_range) {
                    navigation::Route::Ready(path) => path,
                    navigation::Route::Pending => { j.task=Some(task); j.status="Finding route"; break; }
                    navigation::Route::Blocked => continue,
                }
            };
            if at_work || !route.is_empty() {
                j.task = Some(task);
                j.path = route;
                groups::selected(j,task);
                break;
            }
        }
        j.next_candidate = if j.task.is_some() || count == 0 {
            0
        } else {
            (start + attempts) % count
        };
    }
    if j.task.is_none() {
        j.release_claims();
        j.building_material = false;
        // An idle resident cannot remain inside somebody else's next wall cell.
        // Use the same bounded navigation and collision mover to leave the site.
        let park = Task::Store(pile);
        if j.carry.is_none() && !j.build.is_empty() && !reachable(world, feet, park, v) {
            let route = match navigation::route(world,j,feet,park,path_limit,path_range) {
                navigation::Route::Ready(path) => path,
                navigation::Route::Pending => { j.task=Some(park); return; }
                navigation::Route::Blocked => VecDeque::new(),
            };
            if !route.is_empty() {
                j.task = Some(park);
                j.path = route;
                j.status = "Clearing build site";
                return;
            }
        }
        let delay = if tasks.is_empty() { 8 } else { 2 };
        j.retry_at = world.farming.clock.saturating_add(delay * PULSE);
        j.status = if j.carry.is_some() {
            "Stockpile out of reach"
        } else if !j.build.is_empty() && priorities[2] > 0 {
            "Waiting for build materials or access"
        } else if blocked_by_storage && tasks.is_empty() {
            "Stockpile has no room for orders"
        } else {
            "No reachable jobs"
        };
    }
}

/// Factory draws honor the resident stock reservation at debit time.
pub(crate) fn unreserved_chest_stock(w: &World, pile: IVec3, item: crate::world::inventory::Item) -> u32 {
    w.farming.jobs.unreserved_stock(w,pile,item)
}

/// Read-only evidence for the development panel; capacity is not a benchmark result.
pub(crate) fn diagnostics(w: &World) -> String {
    let j = &w.farming.jobs;
    let ids: Vec<_> = crew::ids(j).collect();
    let embodied = ids.iter().filter(|&&id| vehicle::villager_actor(w, id).is_some()).count();
    let commitments = ids.iter().filter(|&&id| crew::view(j, id).is_some_and(|a| a.task.is_some())).count();
    let batches = ids.iter().filter(|&&id| crew::view(j, id).is_some_and(|a| a.batch)).count();
    let waiting = ids.iter().filter(|&&id| crew::view(j, id).is_some_and(|a| a.status == "Awaiting terrain")).count();
    let (pending, source_bytes) = residency::snapshot(j);
    let (hits, misses) = navigation::cache_counts();
    serde_json::json!({
        "civilization": w.sample_config().civilization_enabled(),
        "tick": w.farming.clock,
        "workers": ids.len(), "worker_capacity": crew::CAPACITY,
        "embodied_workers": embodied, "unembodied_workers": ids.len() - embodied,
        "commitments": commitments, "work_in_progress_batches": batches,
        "building_reservations": j.claims.len(), "waiting_for_terrain": waiting,
        "terrain_requests": pending, "terrain_reserved_bytes": source_bytes,
        "walkability_cache_hits": hits, "walkability_cache_misses": misses,
        "work_circles": groups::report(w)
        ,"communities": communities::report(w)
    }).to_string()
}
