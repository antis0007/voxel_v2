//! Cargo keeps its source across interrupted routes and save reloads.
use super::*;
use crate::world::inventory::Item;
use super::quarter::frontier::{self, HubPlan};
mod accounting;
mod transport;
mod frame;
pub(super) use frame::Frame;
pub(super) mod check;
#[cfg(test)] mod tests;
pub(super) use transport::{candidates, execute, task_valid};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Cargo {
    Build { source: [i32; 3] },
    Supply { source: [i32; 3], target: [i32; 3], project: usize },
}

impl Cargo {
    pub(super) fn valid(self, carry: Option<ItemStack>, building: bool) -> bool {
        let bounded = |p: [i32; 3]| p.iter().all(|n| n.unsigned_abs() < 1 << 28);
        carry.is_some() && match self {
            Self::Build { source } => bounded(source),
            Self::Supply { source, target, project } => !building && bounded(source)
                && bounded(target) && source != target && project < 64,
        }
    }
    fn destination(self) -> IVec3 {
        match self { Self::Build { source } => source.into(), Self::Supply { target, .. } => target.into() }
    }
    fn transfer(self, plan: HubPlan) -> bool {
        self == Self::Supply { source: plan.primary.to_array(), target: plan.stockpile.to_array(), project: plan.project }
    }
}

pub(super) fn remote(j: &State, order: Order) -> bool {
    if let Some(frame)=&j.supply_frame { return frame.remote(order); }
    frontier::hub_plan(j).and_then(|plan|j.quarter.as_ref()?.frontier.as_ref()?.projects.get(plan.project))
        .is_some_and(|p|p.cells.contains(&order))
}

pub(super) fn active(j: &State) -> Option<HubPlan> {
    if let Some(frame)=&j.supply_frame { return frame.active.then_some(frame.plan); }
    let plan = frontier::hub_plan(j)?;
    j.quarter.as_ref()?.frontier.as_ref()?.growth.as_ref()?.active_project()
        .filter(|&project|project == plan.project).map(|_|plan)
}

pub(super) fn source(w: &World, j: &State, order: Order) -> Option<IVec3> {
    if remote(j, order) {
        hub_available(w,j).then(||frontier::hub_plan(j).map(|p|p.stockpile)).flatten()
    } else { j.stockpile.map(IVec3::from) }
}

pub(super) fn matches_source(j: &State, cargo: Option<Cargo>, order: Order) -> bool {
    let remote = remote(j,order);
    let expected = if remote { frontier::hub_plan(j).map(|p|p.stockpile.to_array()) } else { j.stockpile };
    let actual = match cargo { Some(Cargo::Build { source }) => Some(source), _ => None };
    accounting::accepts(actual,matches!(cargo,Some(Cargo::Supply { .. })),expected,remote)
}

pub(super) fn destination(j: &State, pile: IVec3) -> IVec3 {
    j.cargo.map_or(pile,Cargo::destination)
}

pub(super) fn store_valid(w: &World, j: &State, pile: IVec3, at: IVec3) -> bool {
    at == destination(j,pile) && known(w,at).is_some_and(|b|b.chest_tier().is_some())
        && j.shopping.is_none()
}

fn carried(j: &State, item: Item, accept: impl Fn(Cargo) -> bool) -> u32 {
    crew::loads(j).filter_map(|a|a.carry.zip(a.cargo))
        .filter(|(s,c)|s.item == item && accept(*c)).fold(0u32,|n,(s,_)|n.saturating_add(s.count))
}

fn hub_available(w: &World, j: &State) -> bool {
    j.hub_access.unwrap_or_else(||frontier::hub_available(w,j))
}

pub(super) fn valid(j: &State) -> bool {
    crew::loads(j)
        .filter(|a|matches!(a.cargo,Some(Cargo::Supply { .. }))).count() <= 1
}

pub(super) fn required(w: &World, j: &State, item: Item) -> u32 {
    if let Some(frame)=&j.supply_frame { return frame.required(item); }
    let Some(plan) = active(j) else { return 0; };
    let missing = j.quarter.as_ref().and_then(|q|q.frontier.as_ref())
        .and_then(|s|s.projects.get(plan.project)).map_or(0,|p|p.cells.iter()
            .filter(|o|o.material() == Some(item) && known(w,o.cell()) != Some(o.block())).count() as u32);
    missing.saturating_sub(carried(j,item,|c|c == Cargo::Build { source: plan.stockpile.to_array() }))
}

pub(super) fn transfer_needed(w: &World, j: &State, item: Item) -> u32 {
    let Some(plan) = active(j) else { return 0; };
    let transit=j.supply_frame.as_ref().map_or_else(||carried(j,item,|c|c.transfer(plan)),|frame|frame.transit(item));
    accounting::remaining(required(w,j,item), production::count(&w.block_data.chest(plan.stockpile),item),
        transit)
}

pub(super) fn begin_frame(w: &World, j: &mut State) {
    clear_frame(j);
    j.hub_access=frontier::hub_plan(j).map(|_|frontier::hub_available(w,j));
    j.supply_frame=Frame::new(w,j);
}

pub(super) fn prepare_frame(w: &World, j: &mut State) -> bool {
    // Completed idle stages need no index; queued repairs still use membership.
    if j.supply_frame.is_some() || (!j.growth_active() && j.build.is_empty() && j.claims.is_empty()) { return false; }
    begin_frame(w,j);
    j.supply_frame.is_some()
}

pub(super) fn clear_frame(j: &mut State) { j.hub_access=None; j.supply_frame=None; }

pub(super) fn funded(j: &State, order: Order) -> Option<bool> {
    j.supply_frame.as_ref().map(|frame|frame.funded(order))
}

pub(super) fn reserved(w: &World, j: &State, pile: IVec3, item: Item) -> Option<u32> {
    let plan = active(j)?;
    if pile == plan.primary { Some(transfer_needed(w,j,item)) }
    else if pile == plan.stockpile { Some(required(w,j,item)) }
    else { None }
}

pub(super) fn in_range(w: &World, j: &State, feet: Vec3, pile: IVec3, task: Task) -> bool {
    let v = w.voxel_scale.meters();
    match task {
        Task::Build(order) | Task::Fetch(_,order) => source(w,j,order).is_some_and(|from|
            within_job_range(j,feet,Task::Store(from).point(v),order.point(v))),
        Task::Store(at) if j.cargo.is_some() => at == destination(j,pile),
        Task::Supply(at,_) => active(j).is_some_and(|p|at == p.primary),
        Task::Recover(at,_) => active(j).is_some_and(|p|at.distance_squared(Task::Store(p.stockpile).point(v)) <= RANGE*RANGE),
        _ => within_job_range(j,feet,Task::Store(pile).point(v),task.point(v)),
    }
}

#[cfg(test)] pub(crate) mod throughput_tests;
