//! Physical container admission and deterministic destination ordering.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Policy {
    pub enabled: bool,
    pub assigned: Option<Item>,
    pub category: Option<stacks::StackClass>,
    pub priority: i16,
    /// Never withdraw below this amount of an item.
    pub reserve: u32,
    /// Deposit ceiling per item; zero means the physical capacity.
    pub target: u32,
}

impl Default for Policy {
    fn default() -> Self {
        Self { enabled: true, assigned: None, category: None, priority: 0, reserve: 0, target: 0 }
    }
}

impl Policy {
    pub fn accepts(&self, item: Item) -> bool {
        self.enabled && self.assigned.is_none_or(|i| i == item)
            && self.category.is_none_or(|c| stacks::class(item) == c)
    }
}

pub fn get(world: &World, pos: IVec3) -> Policy {
    let mut policy = world.block_data.storage_policies.get(&block_data::key(pos)).cloned().unwrap_or_default();
    // A scanner cannot bypass a storage machine's existing resource filter.
    if let Some(node) = world.factory.nodes.get(&pos).filter(|n| n.kind == factory::Kind::Storage) {
        let filter = node.filter.or_else(|| (node.mode == 2).then(|| node.input.iter().position(|&n| n > 0)).flatten());
        if let Some(id) = filter {
            let item = Item::Resource(id as u8);
            if policy.assigned.is_some_and(|assigned| assigned != item) { policy.enabled = false; }
            policy.assigned = Some(item);
        }
    }
    policy
}

pub fn set(world: &mut World, pos: IVec3, policy: Policy) -> bool {
    if !world.block_data.storage_policies.contains_key(&block_data::key(pos))
        && world.block_data.storage_policies.len() >= block_data::MAX_ENTRIES { return false; }
    world.block_data.storage_policies.insert(block_data::key(pos), policy);
    world.block_data.storage_revision = world.block_data.storage_revision.wrapping_add(1);
    true
}

pub fn deposit<S: AsRef<[Option<ItemStack>]> + AsMut<[Option<ItemStack>]>>(slots: &mut [S], policies: &[Policy], stack: &mut ItemStack, cap: u32, reach: Reach) {
    let item = stack.item;
    let mut order: Vec<_> = slots.as_ref().iter().enumerate().filter_map(|(i, s)| {
        let p = &policies[i];
        let holds = s.as_ref().iter().flatten().any(|s| s.item == item);
        if !p.accepts(item) || (reach == Reach::Existing && !holds) { return None; }
        let tier = if reach == Reach::Smart && p.assigned == Some(item) { 0 }
            else if s.as_ref().iter().flatten().any(|s| s.item == item && s.count < cap) { 1 }
            else if reach == Reach::Smart && p.category.is_some() { 2 } else { 3 };
        Some((tier, std::cmp::Reverse(p.priority), i))
    }).collect();
    order.sort_unstable();
    // Partial stacks across the entire network fill before any fresh slot.
    // Keep an additional fallback pass for containers whose partial tier won.
    let partial: Vec<_> = order.iter().copied().filter(|(tier,_,_)| *tier == 1).collect();
    order.extend(partial.iter().map(|(_,priority,i)| (if reach == Reach::Smart && policies[*i].category.is_some() { 2 } else { 3 },*priority,*i)));
    order.sort_unstable();
    for (tier, _, i) in order {
        let held: u64 = slots[i].as_ref().iter().flatten().filter(|s| s.item == item).map(|s| u64::from(s.count)).sum();
        let room = if policies[i].target == 0 { u32::MAX } else {
            u64::from(policies[i].target).saturating_sub(held).min(u64::from(u32::MAX)) as u32
        };
        let n = stack.count.min(room);
        let mut part = ItemStack { item, count: n };
        if tier == 1 {
            for dst in slots[i].as_mut().iter_mut().flatten().filter(|s| s.item == item && s.count < cap) {
                let amount = part.count.min(cap - dst.count);
                dst.count += amount; part.count -= amount;
            }
        } else {
            ops::put(std::slice::from_mut(&mut slots[i]), &mut part, cap, Reach::Anywhere);
        }
        stack.count -= n - part.count;
        if stack.count == 0 { break; }
    }
}

pub fn take<S: AsRef<[Option<ItemStack>]> + AsMut<[Option<ItemStack>]>>(slots: &mut [S], policies: &[Policy], item: Item, amount: u32) -> u32 {
    let mut taken = 0;
    for (s, p) in slots.iter_mut().zip(policies) {
        if !p.enabled { continue; }
        let held: u64 = s.as_ref().iter().flatten().filter(|s| s.item == item).map(|s| u64::from(s.count)).sum();
        let available = held.saturating_sub(u64::from(p.reserve)).min(u64::from(amount - taken)) as u32;
        taken += ops::take(std::slice::from_mut(s), item, available);
        if taken == amount { break; }
    }
    taken
}
