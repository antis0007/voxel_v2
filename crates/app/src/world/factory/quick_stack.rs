//! Quick stack: pack resources a container already holds move into it.
use super::{CAPACITY, Kind, Node, RESOURCE_COUNT};
use crate::world::inventory::{Item, SurvivalInventory};

/// Resource units carried by one pack item.
const UNITS_PER_ITEM: u32 = 64;

/// Moves whole pack items into compartments that already hold them.
///
/// Hotbar, cursor and grid stacks stay put, and `resources` (the legacy unit
/// mirror of the inventory) drops by the same units so the post-action
/// reconcile does not put the items back. Returns the item count moved.
pub(super) fn quick_stack(
    n: &mut Node,
    inv: &mut SurvivalInventory,
    resources: &mut [u32; RESOURCE_COUNT],
) -> u32 {
    if n.kind != Kind::Storage {
        return 0;
    }
    let mut moved = 0;
    for i in 0..RESOURCE_COUNT {
        if n.input[i] == 0 || !n.accepts(i) {
            continue;
        }
        let room = CAPACITY.saturating_sub(n.input[i]) / UNITS_PER_ITEM;
        let items = inv.take_from_pack(
            Item::Resource(i as u8),
            room.min(resources[i] / UNITS_PER_ITEM),
        );
        n.input[i] += items * UNITS_PER_ITEM;
        resources[i] -= items * UNITS_PER_ITEM;
        moved += items;
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::inventory::ItemStack;
    use engine_content::BlockId;

    fn units(inv: &SurvivalInventory) -> [u32; RESOURCE_COUNT] {
        std::array::from_fn(|i| {
            inv.count_item(Item::Resource(i as u8)) * UNITS_PER_ITEM
                + inv.resource_remainders[i]
        })
    }

    #[test]
    fn quick_stack_moves_matching_pack_items_and_keeps_the_hotbar() {
        let mut inv = SurvivalInventory::empty();
        inv.hotbar[0] = Some(ItemStack::new(Item::Resource(2), 5));
        inv.pack[3] = Some(ItemStack::new(Item::Resource(2), 40));
        inv.pack[9] = Some(ItemStack::new(Item::Resource(2), 30));
        inv.pack[10] = Some(ItemStack::new(Item::Resource(4), 7));
        inv.pack[11] = Some(ItemStack::new(Item::Block(BlockId::Planks), 12));
        inv.resource_remainders[2] = 17;
        let mut node = Node::new(Kind::Storage);
        node.input[2] = 10 * UNITS_PER_ITEM;
        let mut resources = units(&inv);
        let hotbar = inv.hotbar;
        let before = node.input[2] + resources[2];

        let moved = quick_stack(&mut node, &mut inv, &mut resources);

        // 54 items of room: the compartment fills to CAPACITY.
        assert_eq!(moved, 54);
        assert_eq!(node.input[2], CAPACITY);
        assert_eq!(node.input[2] + resources[2], before);
        assert_eq!(resources, units(&inv));
        assert_eq!(inv.hotbar, hotbar);
        assert_eq!(inv.pack_count(Item::Resource(2)), 16);
        assert_eq!(inv.pack_count(Item::Resource(4)), 7, "absent from container");
        assert_eq!(inv.pack[11], Some(ItemStack::new(Item::Block(BlockId::Planks), 12)));
        assert_eq!(node.input[4], 0);
    }

    #[test]
    fn locked_barrel_takes_only_its_item() {
        let mut inv = SurvivalInventory::empty();
        inv.pack[0] = Some(ItemStack::new(Item::Resource(1), 3));
        inv.pack[1] = Some(ItemStack::new(Item::Resource(5), 3));
        let mut node = Node::new(Kind::Storage);
        node.mode = 2;
        node.filter = Some(1);
        node.input[1] = UNITS_PER_ITEM;
        // A stray compartment from before the lock must not attract more.
        node.input[5] = UNITS_PER_ITEM;
        let mut resources = units(&inv);

        assert_eq!(quick_stack(&mut node, &mut inv, &mut resources), 3);
        assert_eq!(node.input[1], 4 * UNITS_PER_ITEM);
        assert_eq!(node.input[5], UNITS_PER_ITEM);
        assert_eq!(inv.pack_count(Item::Resource(5)), 3);
        assert_eq!(resources, units(&inv));
    }

    #[test]
    fn non_storage_nodes_are_untouched() {
        let mut inv = SurvivalInventory::empty();
        inv.pack[0] = Some(ItemStack::new(Item::Resource(0), 3));
        let mut node = Node::new(Kind::Furnace);
        node.input[0] = UNITS_PER_ITEM;
        let mut resources = units(&inv);
        assert_eq!(quick_stack(&mut node, &mut inv, &mut resources), 0);
        assert_eq!(inv.pack_count(Item::Resource(0)), 3);
    }
}
