//! Resident work uses existing crops, recipes, navigation and physical chests.
use super::{B, IVec3, ItemStack, State as Jobs, Task as Job, Vec3, World, known};
use crate::world::{block_data::Slots, farming::{self, table}, inventory::{Item, all_recipes}};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod actions;
mod batch;
mod smelting;
pub(super) use batch::Batch;
pub(crate) mod check;
pub(crate) mod scene;
mod woodlot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Bed { pub at: [i32; 3], pub crop: B }

impl Bed {
    pub fn valid(self) -> bool {
        self.at.iter().all(|n| n.unsigned_abs() < 1 << 28)
            && self.row().is_some()
    }
    pub fn row(self) -> Option<(u8, &'static table::Crop)> {
        table::by_seed(self.crop).filter(|(_, c)| c.planting == table::Planting::Soil
            && (c.tree.is_none() || c.crop == B::Sapling) && c.stalk.is_none()
            && c.produce.is_none() && !c.gourd && !c.regrow)
    }
    fn key(self) -> (i32, i32, i32) { (self.at[0], self.at[1], self.at[2]) }
    pub(super) fn open(self, w: &World) -> bool {
        let p = IVec3::from(self.at);
        known(w, p) == Some(B::Air) && !w.farming.plots.contains_key(&self.key())
            && self.row().is_some_and(|(_, c)| known(w, p - IVec3::Y).is_some_and(|b| b == farming::BED || c.soil.contains(&b)))
            && !farming::base::overlaps(w, p, p)
    }
    fn ripe(self, w: &World) -> bool {
        self.row().is_some_and(|(index, c)| c.tree.is_none() && w.farming.plots.get(&self.key())
            .is_some_and(|p| p.crop == index && p.stage == c.mature()))
            && known(w, self.at.into()) == Some(self.crop)
            && !farming::base::overlaps(w, self.at.into(), self.at.into())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Bill {
    pub recipe: String,
    pub target: u32,
    pub station: [i32; 3],
    pub worker: u32,
    #[serde(default)] pub once: bool,
    #[serde(default)] pub per_resident: u16,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Counters {
    pub planted: u32, pub harvested: u32, pub batches: u32, pub meals: u32, pub eaten: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct State {
    pub workers: Vec<u32>,
    pub beds: Vec<Bed>,
    pub bills: Vec<Bill>,
    pub meal: Item,
    pub meal_ticks: u64,
    #[serde(default = "normal_speed")] pub speed: u8,
    #[serde(default)] pub next_meal: BTreeMap<u32, u64>,
    #[serde(default)] pub served: BTreeMap<u32, u32>,
    #[serde(default)] pub counts: Counters,
    #[serde(default)] pub made: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub woodlot: Option<woodlot::State>,
}

fn normal_speed() -> u8 { 1 }

pub(super) fn enable_forestry(state: &mut State) {
    state.woodlot.get_or_insert(woodlot::State { target: 96, planted: 0, felled: 0 });
}

pub(super) fn woodlot_target(state: &State) -> Option<u32> {
    state.woodlot.as_ref().map(|woodlot|woodlot.target)
}

pub(super) fn set_woodlot_target(state: &mut State, target: u32) -> Result<(), String> {
    if !(1..=256).contains(&target) { return Err("Invalid woodlot target".into()); }
    let woodlot = state.woodlot.as_mut().ok_or("Woodlot unavailable")?;
    woodlot.target = target;
    Ok(())
}

impl State {
    pub fn valid(&self) -> bool {
        !self.workers.is_empty() && self.workers.len() <= 1024
            && self.workers.iter().enumerate().all(|(i,id)| !self.workers[..i].contains(id))
            && self.beds.len() <= 256 && self.beds.iter().all(|b| b.valid())
            && self.beds.iter().enumerate().all(|(i,b)| !self.beds[..i].iter().any(|x| x.at == b.at))
            && self.bills.len() <= 64 && self.bills.iter().all(|b| b.target > 0 && b.target <= 4096 && b.per_resident <= 8
                && b.recipe.len() <= 96 && self.workers.contains(&b.worker)
                && b.station.iter().all(|n| n.unsigned_abs() < 1 << 28))
            && matches!(self.meal, Item::Food(_)) && self.meal_ticks >= 120
            && matches!(self.speed, 1 | 2 | 4 | 8)
            // Departed residents retain meal timing and history if they return.
            && self.next_meal.len() <= 1024 && self.served.len() <= 1024
            && self.made.len() <= 68 && self.made.keys().all(|name|
                smelting::is_receipt(name) || self.bills.iter().any(|b| &b.recipe == name))
            && self.woodlot.as_ref().is_none_or(woodlot::State::valid)
            && (self.woodlot.is_some() || self.beds.iter().all(|b| b.crop != B::Sapling))
    }
    pub(super) fn owns(&self, worker: u32, bed: Bed) -> bool {
        self.beds.iter().position(|b| *b == bed)
            .is_some_and(|i| self.workers.get(i % self.workers.len()) == Some(&worker))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Task {
    Seed(IVec3, Bed), Sow(Bed), Harvest(Bed), Fell(Bed), Craft(IVec3, u8), Smelt(IVec3, u8), Eat(IVec3),
}

impl Task {
    pub fn valid(self) -> bool {
        self.cell().to_array().iter().all(|n| n.unsigned_abs() < 1 << 28)
            && match self {
                Self::Seed(_, bed) | Self::Sow(bed) | Self::Harvest(bed) | Self::Fell(bed) => bed.valid(),
                Self::Craft(_, index) => index < 64,
                Self::Smelt(_, index) => index < 4,
                Self::Eat(_) => true,
            }
    }

    pub fn cell(self) -> IVec3 {
        match self { Self::Seed(p, _) | Self::Craft(p, _) | Self::Smelt(p, _) | Self::Eat(p) => p,
            Self::Sow(b) | Self::Harvest(b) | Self::Fell(b) => b.at.into() }
    }
    pub fn point(self, v: f32) -> Vec3 { (self.cell().as_vec3() + Vec3::splat(0.5)) * v }
    pub fn priority(self, p: [u8; 3]) -> u8 {
        match self { Self::Eat(_) => 1, Self::Craft(..) | Self::Smelt(..) => p[2], _ => p[1] }
    }
    pub fn label(self) -> &'static str {
        match self { Self::Seed(..) => "Collecting seed", Self::Sow(_) => "Sowing",
            Self::Harvest(_) => "Harvesting", Self::Fell(_) => "Felling tree",
            Self::Craft(..) => "Crafting", Self::Smelt(..) => "Smelting", Self::Eat(_) => "Eating" }
    }
}

pub(super) fn count(slots: &[Option<ItemStack>], item: Item) -> u32 {
    slots.iter().flatten().filter(|s| s.item == item).map(|s| s.count).sum()
}

fn recipe(b: &Bill) -> Option<&'static crate::world::inventory::CraftingRecipe> {
    all_recipes().iter().find(|r| r.name == b.recipe)
}

pub(super) fn sowing(w: &World, j: &Jobs) -> Option<Task> {
    let bed = j.sowing?;
    let state = j.production.as_ref()?;
    (bed.open(w) && j.worker.is_some_and(|id| state.owns(id, bed))
        && j.carry == Some(ItemStack::new(Item::Block(bed.crop), 1)))
        .then_some(Task::Sow(bed))
}

pub(super) fn candidates(w: &World, j: &Jobs, pile: IVec3) -> Vec<Job> {
    let accepted = || j.batch.as_ref().filter(|batch| batch.ready(w))
        .map(|batch| vec![Job::Produce(batch.task())]).unwrap_or_default();
    if j.production.is_none() { return accepted(); }
    let (Some(s), Some(id)) = (&j.production, j.worker) else { return Vec::new() };
    let slots = w.block_data.chest(pile);
    if w.farming.clock >= s.next_meal.get(&id).copied().unwrap_or(0) && count(&slots, s.meal) > 0 {
        return vec![Job::Produce(Task::Eat(pile))];
    }
    if j.batch.is_some() { return accepted(); }
    let Some(worker_index) = s.workers.iter().position(|&worker| worker == id) else { return Vec::new(); };
    let mut tasks = Vec::new();
    for &b in s.beds.iter().skip(worker_index).step_by(s.workers.len()) {
        if woodlot::ready(w, j, pile, b) { tasks.push(Job::Produce(Task::Fell(b))); }
        else if b.ripe(w) && count(&slots, Item::Block(b.crop)) + super::crew::cargo_item(j, Item::Block(b.crop))
            < (s.workers.len() as u32 * 4).clamp(32, 128) { tasks.push(Job::Produce(Task::Harvest(b))); }
        else if b.open(w) && count(&slots, Item::Block(b.crop)) > 0 {
            tasks.push(Job::Produce(Task::Seed(pile, b)));
        }
    }
    for (i, b) in s.bills.iter().enumerate() {
        if b.worker == id && craft_ready(w, j, pile, b).is_some() {
            tasks.push(Job::Produce(Task::Craft(b.station.into(), i as u8)));
        }
    }
    let hearth = pile - IVec3::Z;
    for index in 0..4 {
        if smelting::plan(w,j,pile,hearth,index).is_some() {
            tasks.push(Job::Produce(Task::Smelt(hearth,index)));
        }
    }
    tasks
}

pub(super) fn craft_ready(w: &World, j: &Jobs, pile: IVec3, bill: &Bill) -> Option<Slots> {
    if j.batch.is_some() { return None; }
    let station = IVec3::from(bill.station);
    if !super::crew::station_available(j, station) { return None; }
    let r = recipe(bill)?;
    if bill.once && j.production.as_ref()?.made.get(&bill.recipe).copied().unwrap_or(0) >= bill.target { return None; }
    if known(w, station) != Some(B::Workbench) || !super::resident(w, station)
        || (station - pile).as_vec3().length() * w.voxel_scale.meters() > 1.5
        || farming::base::overlaps(w, station, station) { return None; }
    let slots = w.block_data.chest(pile);
    let target = bill.target.max(u32::from(bill.per_resident) * j.production.as_ref()?.workers.len() as u32).min(4096);
    if count(&slots, r.output.item) + super::crew::cargo_item(j, r.output.item) >= target { return None; }
    let next = actions::transformed(w, &slots, r)?;
    // Empty beds keep their planting stock ahead of recipe consumption.
    for &(item, _) in r.ingredients {
        let reserve = j.production.as_ref()?.beds.iter().filter(|b| Item::Block(b.crop) == item && b.open(w)).count() as u32;
        if count(&next, item) + super::crew::cargo_item(j, item) < reserve { return None; }
    }
    j.ingredients_available_at(w,pile,&next,r).then_some(next)
}

/// Validate the accepted task directly; don't regenerate every possible job each pulse.
pub(super) fn task_valid(w: &World, j: &Jobs, pile: IVec3, task: Task) -> bool {
    let Some(id) = j.worker else { return false; };
    if j.carry.is_some() { return sowing(w, j) == Some(task); }
    let hungry = j.production.as_ref().is_some_and(|s|
        w.farming.clock >= s.next_meal.get(&id).copied().unwrap_or(0)
            && count(&w.block_data.chest(pile), s.meal) > 0);
    if hungry { return task == Task::Eat(pile); }
    if let Some(batch) = &j.batch { return batch.task() == task && batch.ready(w); }
    let Some(s) = &j.production else { return false; };
    if !s.workers.contains(&id) { return false; }
    match task {
        Task::Seed(from, bed) => from == pile && s.owns(id, bed) && bed.open(w)
            && count(&w.block_data.chest(pile), Item::Block(bed.crop)) > 0,
        Task::Sow(_) | Task::Eat(_) => false,
        Task::Harvest(bed) => s.owns(id, bed) && bed.ripe(w)
            && count(&w.block_data.chest(pile), Item::Block(bed.crop)) + super::crew::cargo_item(j, Item::Block(bed.crop))
                < (s.workers.len() as u32 * 4).clamp(32, 128),
        Task::Fell(bed) => s.owns(id, bed) && woodlot::ready(w, j, pile, bed),
        Task::Craft(station, index) => s.bills.get(index as usize)
            .is_some_and(|bill| bill.worker == id && bill.station == station.to_array() && craft_ready(w, j, pile, bill).is_some()),
        Task::Smelt(station,index) => smelting::plan(w,j,pile,station,index).is_some(),
    }
}

pub(super) fn food_scarce(w: &World, j: &Jobs) -> bool {
    let (Some(s), Some(pile)) = (&j.production, j.stockpile) else { return false };
    count(&w.block_data.chest(pile.into()), s.meal) < s.workers.len() as u32
}

pub(super) fn work_priority(j: &Jobs, task: Job, priorities: [u8; 3], scarce: bool) -> u8 {
    let normal = task.priority(priorities);
    let Some(s) = &j.production else { return normal };
    if matches!(task, Job::Produce(Task::Eat(_))) { return 0; }
    if scarce && matches!(task, Job::Produce(Task::Seed(_, b) | Task::Sow(b) | Task::Harvest(b)) if b.crop != B::Sapling) { return 1; }
    if scarce && let Job::Produce(Task::Craft(_, i)) = task
        && s.bills.get(i as usize).and_then(recipe).is_some_and(|r| r.output.item == s.meal) { return 1; }
    normal.saturating_add(2)
}

pub(super) fn execute(w: &mut World, j: &mut Jobs, t: Task) -> bool { actions::execute(w, j, t) }

pub(crate) fn summary(w: &World) -> Option<String> {
    let s = w.farming.jobs.production.as_ref()?;
    Some(format!("Sown {} · Harvested {} · Batches {} · Meals {} · Eaten {}",
        s.counts.planted, s.counts.harvested, s.counts.batches, s.counts.meals, s.counts.eaten))
}

pub(super) fn woodlot_counts(w:&World)->Option<(u32,u32)> {
    w.farming.jobs.production.as_ref()?.woodlot.as_ref().map(|f|(f.planted,f.felled))
}
