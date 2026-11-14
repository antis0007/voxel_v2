use super::*;

const LOAD: u32 = 16;

fn occupied(j: &State) -> bool {
    if let Some(frame)=&j.supply_frame { return frame.other_courier; }
    crew::loads(j).filter(|a|a.id != j.worker)
        .any(|a|matches!(a.cargo,Some(Cargo::Supply { .. }))
        || matches!(a.task,Some(Task::Supply(..)|Task::Recover(..))))
}

fn room(w: &World, at: IVec3, item: Item) -> u32 {
    if !w.block_data.can_store_chest(at) { return 0; }
    let limit = w.inventory.stack_limit(item);
    w.block_data.chest(at).iter().map(|slot|match slot {
        None => limit, Some(s) if s.item == item => limit.saturating_sub(s.count), _ => 0,
    }).sum()
}

fn admitted(w: &World, j: &State) -> Option<HubPlan> {
    let plan = active(j)?;
    (j.carry.is_none() && !occupied(j) && hub_available(w,j)
        && known(w,plan.primary).is_some_and(|b|b.chest_tier().is_some())).then_some(plan)
}

pub(in super::super) fn candidates(w: &World, j: &State) -> Vec<Task> {
    let Some(plan) = admitted(w,j) else { return Vec::new(); };
    let Some(project) = j.quarter.as_ref().and_then(|q|q.frontier.as_ref()).and_then(|s|s.projects.get(plan.project))
        else { return Vec::new(); };
    let mut items = j.supply_frame.as_ref().map_or_else(||accounting::unique(project.cells.iter().filter_map(|o|o.material())),
        |frame|frame.materials.clone());
    items.retain(|&item|transfer_needed(w,j,item) > 0 && room(w,plan.stockpile,item) > 0);
    let mut cursor = j.haul_scan_cursor;
    let recovery: Vec<_> = drop_scan::window(&mut cursor,w.inventory.dropped.len())
        .map(|i|&w.inventory.dropped[i]).filter(|d|d.stack.count > 0 && items.contains(&d.stack.item)
            && d.pos.distance_squared(Task::Store(plan.stockpile).point(w.voxel_scale.meters())) <= RANGE*RANGE)
        .map(|d|Task::Recover(d.pos,d.stack.item)).collect();
    if !recovery.is_empty() { return recovery; }
    items.into_iter().filter(|&item|production::count(&w.block_data.chest(plan.primary),item) > 0)
        .map(|item|Task::Supply(plan.primary,item)).collect()
}

pub(in super::super) fn task_valid(w: &World, j: &State, task: Task) -> bool {
    let Some(plan) = admitted(w,j) else { return false; };
    match task {
        Task::Supply(at,item) => at == plan.primary && transfer_needed(w,j,item) > 0
            && room(w,plan.stockpile,item) > 0 && production::count(&w.block_data.chest(at),item) > 0,
        Task::Recover(at,item) => transfer_needed(w,j,item) > 0 && room(w,plan.stockpile,item) > 0
            && at.distance_squared(Task::Store(plan.stockpile).point(w.voxel_scale.meters())) <= RANGE*RANGE
            && w.inventory.dropped.iter().any(|d|d.pos == at && d.stack.item == item && d.stack.count > 0),
        _ => false,
    }
}

pub(in super::super) fn execute(w: &mut World, j: &mut State, task: Task) {
    if !task_valid(w,j,task) { return; }
    let Some(feet) = j.worker.and_then(|id|vehicle::villager_feet(w,id)) else { return; };
    if !reachable(w,feet,task,w.voxel_scale.meters()) { return; }
    let Some(plan) = active(j) else { return; };
    let item = match task { Task::Supply(_,item)|Task::Recover(_,item) => item, _ => return };
    let allowed = transfer_needed(w,j,item).min(LOAD).min(room(w,plan.stockpile,item));
    let count = match task {
        Task::Supply(at,_) => block_data::take_hauled(w,at,item,allowed),
        Task::Recover(at,_) => {
            let Some(index) = w.inventory.dropped.iter().position(|d|d.pos == at && d.stack.item == item && d.stack.count > 0)
                else { return; };
            let count = allowed.min(w.inventory.dropped[index].stack.count);
            w.inventory.dropped[index].stack.count -= count;
            if w.inventory.dropped[index].stack.count == 0 { w.inventory.dropped.remove(index); }
            count
        }
        _ => 0,
    };
    if count == 0 { return; }
    j.release_claims();
    j.building_material = false;
    j.carry = Some(ItemStack::new(item,count));
    j.cargo = Some(Cargo::Supply { source: plan.primary.to_array(), target: plan.stockpile.to_array(), project: plan.project });
    j.status = "Carrying hub materials";
}
