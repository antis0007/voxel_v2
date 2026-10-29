//! Brewing stand: a timed brew in fixed steps, stored in the block-data sidecar.
use super::table::{BREWING, Brew};
use crate::world::block_data::Stand;
use crate::world::inventory::{Item, ItemStack, SlotAddr};
use crate::world::{BlockId, World};
use glam::IVec3;

/// Which stand slot an item goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Base,
    Modifier,
}

pub fn resolve(base: Item, modifier: Item) -> Option<&'static Brew> {
    BREWING.recipes.iter().find(|r| r.base == base && r.modifier == modifier)
}

pub fn role(item: Item) -> Option<Role> {
    if BREWING.recipes.iter().any(|r| r.base == item) {
        Some(Role::Base)
    } else if BREWING.recipes.iter().any(|r| r.modifier == item) {
        Some(Role::Modifier)
    } else {
        None
    }
}

/// Fixed steps one brew takes.
pub fn brew_steps() -> u16 {
    (BREWING.seconds / crate::simulation::STEP_SECONDS).round().clamp(1.0, f32::from(u16::MAX)) as u16
}

/// A water block on any face of the stand.
pub fn has_water(world: &World, p: IVec3) -> bool {
    [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z]
        .into_iter()
        .any(|d| world.get(p + d) == BlockId::Water)
}

/// One fixed step of a stand. Returns whether the record changed.
pub fn advance(stand: &mut Stand, water: bool) -> bool {
    let recipe = stand.base.zip(stand.modifier).and_then(|(b, m)| resolve(b.item, m.item));
    let Some(recipe) = recipe else {
        let changed = stand.steps != 0;
        stand.steps = 0;
        return changed;
    };
    let out = Item::Food(recipe.output);
    let room = stand.output.is_none_or(|o| o.item == out && o.count + recipe.count <= out.max_stack());
    if !water || !room {
        return false;
    }
    stand.steps += 1;
    if stand.steps < brew_steps() {
        return true;
    }
    stand.steps = 0;
    for slot in [&mut stand.base, &mut stand.modifier] {
        if let Some(s) = slot.as_mut() {
            s.count -= 1;
            if s.count == 0 {
                *slot = None;
            }
        }
    }
    let count = stand.output.map_or(0, |o| o.count) + recipe.count;
    stand.output = Some(ItemStack::new(out, count));
    true
}

/// Advances every brewing stand by one fixed step.
pub fn tick(world: &mut World) {
    let keys: Vec<_> = world.block_data.stands.iter().copied().collect();
    for (x, y, z) in keys {
        let p = IVec3::new(x, y, z);
        if world.get(p) != BlockId::BrewingStand {
            continue;
        }
        let water = has_water(world, p);
        let mut stand = world.block_data.stand(p);
        if advance(&mut stand, water) {
            world.block_data.set_stand(p, stand);
        }
    }
}

/// Moves the player's stack at `addr` into the stand slot its role names, merging.
pub fn store(world: &mut World, p: IVec3, addr: SlotAddr) {
    if world.get(p) != BlockId::BrewingStand {
        return;
    }
    let Some(mut stack) = world.inventory.slot(addr) else { return };
    let Some(role) = role(stack.item) else { return };
    let mut stand = world.block_data.stand(p);
    let slot = match role {
        Role::Base => &mut stand.base,
        Role::Modifier => &mut stand.modifier,
    };
    let max = stack.item.max_stack();
    match slot {
        Some(s) if s.item == stack.item => {
            let add = stack.count.min(max.saturating_sub(s.count));
            s.count += add;
            stack.count -= add;
        }
        Some(_) => return,
        None => {
            *slot = Some(stack);
            stack.count = 0;
        }
    }
    if let Some(cell) = world.inventory.slot_mut(addr) {
        *cell = (stack.count > 0).then_some(stack);
    }
    world.block_data.set_stand(p, stand);
    world.sync_from_inventory();
}

/// Moves stand slot `index` (base, modifier, output) into the player's inventory.
pub fn take(world: &mut World, p: IVec3, index: usize) {
    if world.get(p) != BlockId::BrewingStand {
        return;
    }
    let mut stand = world.block_data.stand(p);
    let slot = match index {
        0 => &mut stand.base,
        1 => &mut stand.modifier,
        _ => &mut stand.output,
    };
    let Some(stack) = *slot else { return };
    let (_, left) = world.inventory.try_insert(stack);
    *slot = (left > 0).then_some(ItemStack::new(stack.item, left));
    world.block_data.set_stand(p, stand);
    world.sync_from_inventory();
}
