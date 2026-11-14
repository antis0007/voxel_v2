//! Accepted work owns its inputs until completion or explicit recovery.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in super::super) struct Batch {
    pub source: [i32; 3],
    pub station: [i32; 3],
    pub bill: u8,
    recipe: String,
    inputs: Vec<ItemStack>,
    output: ItemStack,
    progress: u8,
    #[serde(default)] smelt: bool,
}

const WORK: u8 = 12;

impl Batch {
    pub(in super::super) fn retain_progress(&mut self, progress: u8) {
        self.progress = self.progress.max(progress.min(WORK));
    }
    pub fn valid(&self) -> bool {
        self.source.iter().chain(&self.station).all(|n| n.unsigned_abs() < 1 << 28)
            && self.bill < 64 && !self.recipe.is_empty() && self.recipe.len() <= 96
            && !self.inputs.is_empty() && self.inputs.len() <= 16
            && self.inputs.iter().chain([&self.output]).all(|s| (1..=4096).contains(&s.count))
            && self.progress <= WORK
            && (!self.smelt || (self.bill < 4 && super::smelting::is_receipt(&self.recipe)))
    }

    pub fn task(&self) -> Task {
        if self.smelt { Task::Smelt(self.station.into(),self.bill) }
        else { Task::Craft(self.station.into(),self.bill) }
    }

    pub fn recovery(self) -> Vec<ItemStack> {
        if self.progress == WORK { vec![self.output] } else { self.inputs }
    }

    pub fn ready(&self, w: &World) -> bool {
        super::super::resident(w, self.source.into()) && super::super::resident(w, self.station.into())
            && known(w, self.source.into()).is_some_and(|b| b.chest_tier().is_some())
            && known(w, self.station.into()) == Some(if self.smelt { B::Smelter } else { B::Workbench })
            && !farming::base::overlaps(w, self.station.into(), self.station.into())
    }
}

fn debit(slots: &mut Slots, item: Item, mut amount: u32) {
    for slot in slots.iter_mut() {
        if let Some(stack) = slot.as_mut().filter(|s| s.item == item) {
            let n = amount.min(stack.count);
            amount -= n;
            stack.count -= n;
            if stack.count == 0 { *slot = None; }
        }
    }
    debug_assert_eq!(amount, 0);
}

fn credit(w: &World, slots: &mut Slots, mut output: ItemStack) -> bool {
    let limit = w.inventory.stack_limit(output.item);
    for slot in slots.iter_mut() {
        if let Some(stack) = slot.as_mut().filter(|s| s.item == output.item) {
            let n = output.count.min(limit.saturating_sub(stack.count));
            stack.count += n;
            output.count -= n;
        }
    }
    for slot in slots.iter_mut().filter(|s| s.is_none()) {
        let n = output.count.min(limit);
        if n != 0 { *slot = Some(ItemStack::new(output.item, n)); output.count -= n; }
    }
    output.count == 0
}

pub(super) fn execute(w: &mut World, j: &mut Jobs, station: IVec3, index: u8) -> bool {
    if j.batch.is_none() {
        let (Some(pile), Some(id), Some(production)) = (j.stockpile.map(IVec3::from), j.worker, &j.production) else { return true; };
        let Some(bill) = production.bills.get(index as usize) else { return true; };
        if bill.worker != id || bill.station != station.to_array() || j.carry.is_some()
            || craft_ready(w, j, pile, bill).is_none() { return true; }
        let Some(recipe) = recipe(bill) else { return true; };
        let batch = Batch { source: pile.to_array(), station: station.to_array(), bill: index,
            recipe: recipe.name.into(), inputs: recipe.ingredients.iter().map(|&(item, n)| ItemStack::new(item, n)).collect(),
            output: recipe.output, progress: 0, smelt: false };
        if !batch.valid() || !w.block_data.can_store_chest(pile) { return true; }
        let mut stock = w.block_data.chest(pile);
        for input in &batch.inputs { debit(&mut stock, input.item, input.count); }
        w.block_data.set_chest(pile, stock);
        j.batch = Some(batch);
    }
    advance(w,j,station,index)
}

pub(super) fn smelt(w: &mut World, j: &mut Jobs, station: IVec3, index: u8) -> bool {
    if j.batch.is_none() {
        let Some(pile) = j.stockpile.map(IVec3::from) else { return true; };
        let Some((inputs,output)) = super::smelting::plan(w,j,pile,station,index) else { return true; };
        if !w.block_data.can_store_chest(pile) { return true; }
        let batch = Batch { source: pile.to_array(), station: station.to_array(), bill: index,
            recipe: super::smelting::receipt(index).into(), inputs, output, progress: 0, smelt: true };
        if !batch.valid() { return true; }
        let mut stock = w.block_data.chest(pile);
        for input in &batch.inputs { debit(&mut stock,input.item,input.count); }
        w.block_data.set_chest(pile,stock);
        j.batch = Some(batch);
    }
    advance(w,j,station,index)
}

fn advance(w: &mut World, j: &mut Jobs, station: IVec3, index: u8) -> bool {
    let Some(batch) = j.batch.as_ref() else { return true; };
    if batch.station != station.to_array() || batch.bill != index || !batch.ready(w) { return false; }
    j.status = if batch.smelt { "Smelting" } else { "Crafting" };
    j.work = j.work.max(batch.progress);
    let complete = super::super::activity::work(w,j,WORK,station);
    let batch = j.batch.as_mut().expect("accepted batch");
    batch.retain_progress(j.work);
    if !complete { return false; }
    let pile = IVec3::from(batch.source);
    if !w.block_data.can_store_chest(pile) {
        j.status = "Awaiting storage";
        super::super::activity::wait(w,j,pile,w.farming.clock.saturating_add(1200));
        return false;
    }
    let mut stock = w.block_data.chest(pile);
    if !credit(w, &mut stock, batch.output) {
        j.status = "Awaiting storage";
        super::super::activity::wait(w,j,pile,w.farming.clock.saturating_add(1200));
        return false;
    }
    w.block_data.set_chest(pile, stock);
    let batch = j.batch.take().expect("accepted batch");
    if let Some(s) = &mut j.production {
        if batch.smelt || s.bills.iter().any(|bill| bill.recipe == batch.recipe) {
            let made = s.made.entry(batch.recipe).or_default();
            *made = made.saturating_add(batch.output.count);
        }
        s.counts.batches = s.counts.batches.saturating_add(1);
        if batch.output.item == s.meal { s.counts.meals = s.counts.meals.saturating_add(batch.output.count); }
    }
    true
}
