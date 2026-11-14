//! Chest transfers share machine time credit and physical storage policies.
use super::{CAPACITY, Kind, Node, RESOURCE_COUNT, assembler::{block_needs, recipe}, recipes, stock_order};
use crate::simulation::Budget;
use crate::world::{World, block_data, factory::{DIRS, sides::{self, Role}}, inventory::{Item, ItemStack}, scanner::{Reach, policy}};
use glam::IVec3;

pub(super) const INTERVAL: f32 = 0.25;
const TRANSFER_LIMIT: u32 = 16;
const RESOURCE_UNITS: u32 = 64;

pub(super) fn has_output(n: &Node) -> bool {
    if n.kind.processor() {
        return stock_order::read(n.kind, n.mode, n.target).is_none()
            && n.output.iter().any(|&units| units >= RESOURCE_UNITS);
    }
    n.kind == Kind::Assembler && recipe(n).is_some_and(|r| match r.output.item {
        Item::Resource(i) => n.output[i as usize] >= RESOURCE_UNITS,
        _ => n.stock > 0,
    })
}

pub(super) fn transfer(world: &mut World, pos: IVec3, n: &mut Node, size: i32, budget: Budget) {
    if n.work_clock < INTERVAL || !budget.has_time() { return; }
    let recipe = (n.kind == Kind::Assembler).then(|| recipe(n)).flatten();
    if n.kind == Kind::Assembler && recipe.is_none() { return; }
    if !(n.kind == Kind::Assembler || n.kind.processor())
        || stock_order::read(n.kind, n.mode, n.target).is_some() { return; }
    if !n.enabled && !has_output(n) { return; }
    let inputs = resource_needs(n);
    let block_inputs: Vec<_> = if n.kind == Kind::Assembler {
        recipe.into_iter().flat_map(block_needs).collect()
    } else if n.kind == Kind::Cultivator && super::biomatter::recipe(n.mode).is_some() {
        vec![(super::blocks::Material::Grass, super::biomatter::recipe(n.mode).unwrap().grass)]
    } else {
        n.recipe_candidates().filter_map(|i| recipes::process(n.kind, i)?.block_cost(size)).collect()
    };
    n.work_clock -= INTERVAL;
    let (mut input_left, mut output_left) = (TRANSFER_LIMIT, TRANSFER_LIMIT);
    for (face, dir) in DIRS.into_iter().enumerate() {
        let role = sides::role(n, face);
        if role == Role::Off { continue; }
        let edge = if dir.min_element() < 0 { -1 } else { size };
        for a in 0..size {
            for b in 0..size {
                if !budget.has_time() { return; }
                let p = pos + match face / 2 {
                    0 => IVec3::new(edge, a, b),
                    1 => IVec3::new(a, edge, b),
                    _ => IVec3::new(a, b, edge),
                };
                if !matches!(world.block_data.entries.get(&block_data::key(p)), Some(block_data::Entry::Chest(_)))
                    || world.get(p).chest_tier().is_none() { continue; }
                let rules = [policy::get(world, p)];
                let mut slots = [world.block_data.chest(p)];
                let mut changed = false;
                if n.enabled && matches!(role, Role::In | Role::Both) {
                    for &(i, count) in inputs.iter().filter(|(_, count)| *count > 0) {
                        let item = Item::Resource(i as u8);
                        let input = &mut n.input[i];
                        let funded = input.saturating_add(world.factory.input_reserved(pos, i));
                        let needed = count.saturating_sub(funded);
                        // Piped fractions stay in the buffer; only whole chest items cross.
                        let amount = needed.div_ceil(RESOURCE_UNITS)
                            .min(CAPACITY.saturating_sub(funded) / RESOURCE_UNITS).min(input_left);
                        let taken = take(world, p, &mut slots, &rules, item, amount);
                        *input += taken * RESOURCE_UNITS;
                        input_left -= taken;
                        changed |= taken > 0;
                    }
                    if n.kind == Kind::Crusher {
                        let volume = super::placer::cell_volume(size);
                        for (material, i) in super::blocks::raw_ores() {
                            let funded = n.input[i].saturating_add(world.factory.input_reserved(pos, i));
                            let amount = inputs[i].1.saturating_sub(funded).div_ceil(volume)
                                .min(CAPACITY.saturating_sub(funded) / volume).min(input_left);
                            let taken = take(world, p, &mut slots, &rules, material.item(), amount);
                            n.input[i] += taken * volume;
                            input_left -= taken;
                            changed |= taken > 0;
                        }
                    }
                    for &(material, count) in &block_inputs {
                        let input = &mut n.blocks.input[material.index()];
                        let amount = count.saturating_sub(*input).min(CAPACITY.saturating_sub(*input)).min(input_left);
                        let taken = take(world, p, &mut slots, &rules, material.item(), amount);
                        *input += taken;
                        input_left -= taken;
                        changed |= taken > 0;
                    }
                }
                if output_left > 0 && matches!(role, Role::Out | Role::Both) {
                    for i in 0..RESOURCE_COUNT {
                        if n.kind == Kind::Assembler && recipe.is_none_or(|r| r.output.item != Item::Resource(i as u8)) { continue; }
                        let moved = deposit(world, &mut slots, &rules, Item::Resource(i as u8),
                            (n.output[i] / RESOURCE_UNITS).min(output_left));
                        n.output[i] -= moved * RESOURCE_UNITS;
                        output_left -= moved;
                        changed |= moved > 0;
                    }
                    if let Some(item) = recipe.map(|r| r.output.item).filter(|item| !matches!(item, Item::Resource(_))) {
                        let moved = deposit(world, &mut slots, &rules, item, n.stock.min(output_left));
                        n.stock -= moved;
                        output_left -= moved;
                        changed |= moved > 0;
                    }
                }
                if changed {
                    let [slots] = slots;
                    world.block_data.set_chest(p, slots);
                }
                if input_left == 0 && output_left == 0 { return; }
            }
        }
    }
}

fn take(world: &World, p: IVec3, slots: &mut [block_data::Slots; 1], rules: &[policy::Policy; 1], item: Item, amount: u32) -> u32 {
    if amount == 0 { return 0; }
    let allowed = crate::world::farming::jobs::unreserved_chest_stock(world, p, item);
    policy::take(slots, rules, item, amount.min(allowed))
}

fn resource_needs(n: &Node) -> [(usize, u32); RESOURCE_COUNT] {
    let mut inputs = [(0, 0); RESOURCE_COUNT];
    if n.kind == Kind::Assembler {
        if let Some(recipe) = recipe(n) {
            for (slot, &(item, count)) in inputs.iter_mut().zip(recipe.ingredients.iter().filter(|(item, _)| matches!(item, Item::Resource(_)))) {
                if let Item::Resource(i) = item { *slot = (i as usize, count * RESOURCE_UNITS); }
            }
        }
    } else if n.kind.processor() {
        for index in n.recipe_candidates() {
            if let Some(process) = recipes::process(n.kind, index) {
                for (item, count) in process.inputs {
                    if count > 0 && n.accepts(item) { inputs[item] = (item, inputs[item].1.max(count)); }
                }
            }
        }
    }
    inputs
}

fn deposit(world: &World, slots: &mut [block_data::Slots; 1], rules: &[policy::Policy; 1], item: Item, amount: u32) -> u32 {
    if amount == 0 { return 0; }
    let mut stack = ItemStack::new(item, amount);
    policy::deposit(slots, rules, &mut stack, world.inventory.stack_limit(item), Reach::Smart);
    amount - stack.count
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod crusher_tests;
