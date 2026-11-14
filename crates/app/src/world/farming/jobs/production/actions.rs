use super::*;
use crate::world::{block_data, effects, farming::Plot};

pub(super) fn transformed(w: &World, slots: &[Option<ItemStack>], r: &crate::world::inventory::CraftingRecipe) -> Option<Slots> {
    let mut next = slots.to_vec();
    for &(item, amount) in r.ingredients {
        if count(&next, item) < amount { return None; }
        let mut remaining = amount;
        for slot in &mut next {
            if let Some(s) = slot.as_mut().filter(|s| s.item == item) {
                let n = remaining.min(s.count);
                s.count -= n; remaining -= n;
                if s.count == 0 { *slot = None; }
            }
        }
    }
    let mut output = r.output;
    let limit = w.inventory.stack_limit(output.item);
    for slot in next.iter_mut().filter(|s| s.is_some_and(|x| x.item == output.item)) {
        if let Some(s) = slot {
            let n = output.count.min(limit.saturating_sub(s.count));
            s.count += n; output.count -= n;
        }
    }
    for slot in next.iter_mut().filter(|s| s.is_none()) {
        let n = output.count.min(limit);
        if n > 0 { *slot = Some(ItemStack::new(output.item, n)); output.count -= n; }
    }
    (output.count == 0).then_some(next)
}

pub(super) fn execute(w: &mut World, j: &mut Jobs, t: Task) -> bool {
    if let Task::Craft(station, index) = t { return super::batch::execute(w, j, station, index); }
    if let Task::Smelt(station, index) = t { return super::batch::smelt(w,j,station,index); }
    let (Some(pile), Some(id), Some(s)) = (j.stockpile.map(IVec3::from), j.worker, &j.production) else { return true };
    if !s.workers.contains(&id) || !super::super::resident(w, t.cell())
        || known(w, pile).is_none_or(|b| b.chest_tier().is_none()) { return true; }
    j.status = t.label();
    match t {
        Task::Seed(from, b) => {
            if from != pile || !s.owns(id, b) || !b.open(w) || j.carry.is_some() { return true; }
            if block_data::take_hauled(w, pile, Item::Block(b.crop), 1) == 1 {
                j.carry = Some(ItemStack::new(Item::Block(b.crop), 1));
                j.sowing = Some(b);
            }
        }
        Task::Sow(b) => {
            if super::sowing(w, j) != Some(t) || !b.open(w) || w.farming.plots.len() >= farming::MAX_PLOTS { return true; }
            if !super::super::activity::work(w,j,4,b.at.into()) { return false; }
            let Some((index, c)) = b.row() else { return true };
            let Ok(commit) = effects::replace(w, b.at.into(), B::Air, c.sprout) else { return true };
            let mut changed = Vec::new(); commit.publish_voxel(w, &mut changed);
            w.adventure.dirty_voxels.extend(changed);
            w.farming.plots.insert(b.key(), Plot::new(index, w.farming.clock));
            j.carry = None; j.sowing = None;
            if let Some(s) = &mut j.production {
                if c.tree.is_some() {
                    if let Some(f) = &mut s.woodlot { f.planted += 1; }
                } else { s.counts.planted += 1; }
            }
        }
        Task::Harvest(b) => {
            if !s.owns(id, b) || !b.ripe(w) || j.carry.is_some() { return true; }
            if !super::super::activity::work(w,j,6,b.at.into()) { return false; }
            let Some((index, c)) = b.row() else { return true };
            let Some(plot) = w.farming.plots.get(&b.key()).copied() else { return true };
            let yield_count = farming::harvest_count(w.seed, b.at.into(), &plot, c);
            if yield_count == 0 { return true; }
            let Ok(commit) = effects::replace(w, b.at.into(), b.crop, c.sprout) else { return true };
            let mut changed = Vec::new(); commit.publish_voxel(w, &mut changed);
            w.adventure.dirty_voxels.extend(changed);
            // One harvested seed immediately returns to the same bed; only surplus leaves it.
            w.farming.plots.insert(b.key(), Plot::new(index, w.farming.clock));
            j.carry = (yield_count > 1).then_some(ItemStack::new(Item::Block(b.crop), yield_count - 1));
            if let Some(s) = &mut j.production { s.counts.harvested += yield_count; s.counts.planted += 1; }
        }
        Task::Fell(b) => { return super::woodlot::execute(w, j, b); }
        Task::Craft(..) | Task::Smelt(..) => unreachable!("production batches use their persistent owner"),
        Task::Eat(from) => {
            if from != pile || j.carry.is_some() || w.farming.clock < s.next_meal.get(&id).copied().unwrap_or(0) { return true; }
            let meal = s.meal;
            let ticks = s.meal_ticks;
            if block_data::take_hauled(w, pile, meal, 1) == 1 {
                super::super::routine::ate(&mut j.routine, w.farming.clock);
                if let Some(s) = &mut j.production {
                    s.next_meal.insert(id, w.farming.clock.saturating_add(ticks));
                    *s.served.entry(id).or_default() += 1;
                    s.counts.eaten += 1;
                }
            }
        }
    }
    true
}
