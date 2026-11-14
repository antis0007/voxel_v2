//! One worker call indexes project obligations, never chest inventory.
use super::*;
mod keys;

#[derive(Clone, Debug)]
pub(in super::super) struct Frame {
    pub(super) plan: HubPlan,
    pub(super) active: bool,
    remote: keys::Keys,
    funded: Option<keys::Keys>,
    needed: HashMap<Item,u32>,
    moving: HashMap<Item,u32>,
    pub(super) materials: Vec<Item>,
    pub(super) other_courier: bool,
    pub(super) project_rows: usize,
    pub(super) worker_rows: usize,
}

impl Frame {
    pub(super) fn new(w: &World, j: &State) -> Option<Self> {
        let plan=frontier::hub_plan(j)?;
        let site=j.quarter.as_ref()?.frontier.as_ref()?;
        let project=site.projects.get(plan.project)?;
        let funded=site.growth.as_ref()?.active_project();
        let active=funded==Some(plan.project);
        let mut needed=HashMap::<Item,u32>::new();
        let mut materials=Vec::new();
        let mut remote=Vec::with_capacity(project.cells.len());
        for &order in &project.cells {
            remote.push(key(order));
            if let Some(item)=order.material() {
                if !materials.contains(&item) { materials.push(item); }
                if active && known(w,order.cell())!=Some(order.block()) { *needed.entry(item).or_default()+=1; }
            }
        }
        let mut project_rows=project.cells.len();
        let funded=if active {None} else {funded.and_then(|i|site.projects.get(i)).map(|p| {
            project_rows+=p.cells.len();
            keys::Keys::new(p.cells.iter().copied().map(key))
        })};
        let mut moving=HashMap::<Item,u32>::new();
        let mut other_courier=false;
        let mut worker_rows=0;
        for worker in crew::loads(j) {
            worker_rows+=1;
            other_courier |= worker.id!=j.worker && (matches!(worker.cargo,Some(Cargo::Supply { .. }))
                || matches!(worker.task,Some(Task::Supply(..)|Task::Recover(..))));
            let Some((stack,cargo))=worker.carry.zip(worker.cargo) else { continue; };
            if cargo== (Cargo::Build {source:plan.stockpile.to_array()}) {
                let count=needed.entry(stack.item).or_default();
                *count=count.saturating_sub(stack.count);
            } else if cargo.transfer(plan) {
                let count=moving.entry(stack.item).or_default();
                *count=count.saturating_add(stack.count);
            }
        }
        Some(Self {plan,active,remote:keys::Keys::new(remote),funded,needed,moving,materials,
            other_courier,project_rows,worker_rows})
    }

    pub(super) fn remote(&self, order: Order) -> bool { self.remote.contains(key(order)) }
    pub(super) fn funded(&self, order: Order) -> bool {
        if self.active {self.remote(order)} else {self.funded.as_ref().is_some_and(|keys|keys.contains(key(order)))}
    }
    pub(super) fn required(&self, item: Item) -> u32 { self.needed.get(&item).copied().unwrap_or(0) }
    pub(super) fn transit(&self, item: Item) -> u32 { self.moving.get(&item).copied().unwrap_or(0) }
}

fn key(order: Order) -> ([i32;3],u16) { (order.at,order.block() as u16) }
