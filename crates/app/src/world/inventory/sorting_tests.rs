use super::*;
use crate::world::inventory::{ArmorTier, EquipSlot, FoodKind, HOTBAR_SLOTS, PACK_SLOTS};
use engine_content::BlockId;
use std::collections::HashMap;

const POOL: [Item; 10] = [
    Item::Block(BlockId::Stone),
    Item::Block(BlockId::Planks),
    Item::Block(BlockId::Workbench),
    Item::Resource(3),
    Item::Resource(10),
    Item::Food(FoodKind::FungalRation),
    Item::Weapon(1),
    Item::Armor(EquipSlot::Head, ArmorTier::Copper),
    Item::Wand { id: 4, look: 1 },
    Item::Wand { id: 9, look: 2 },
];

struct Rng(u32);
impl Rng {
    fn next(&mut self, below: u32) -> u32 {
        self.0 = engine_core::hash::hash_u32(self.0.wrapping_add(0x9E37_79B9));
        self.0 % below.max(1)
    }
}

fn totals(slots: &[Option<ItemStack>]) -> HashMap<Item, u64> {
    let mut map = HashMap::new();
    for stack in slots.iter().flatten() {
        *map.entry(stack.item).or_insert(0) += u64::from(stack.count);
    }
    map
}

/// A random pack, sometimes holding stacks above the scale's limit.
fn random_inventory(seed: u32) -> (SurvivalInventory, bool) {
    let mut rng = Rng(seed);
    let mut inv = SurvivalInventory::empty();
    inv.voxel_m = [1.0, 0.5, 0.25][rng.next(3) as usize];
    let mut oversized = false;
    for slot in &mut inv.pack {
        if rng.next(3) == 0 {
            continue;
        }
        let item = POOL[rng.next(POOL.len() as u32) as usize];
        let limit = SurvivalInventory::limit_at_scale(item, inv.voxel_m);
        let count = if rng.next(10) == 0 {
            oversized = true;
            limit + 1 + rng.next(limit * 2)
        } else {
            1 + rng.next(limit)
        };
        *slot = Some(ItemStack { item, count });
    }
    for (i, slot) in inv.hotbar.iter_mut().enumerate() {
        if rng.next(2) == 0 {
            *slot = Some(ItemStack::new(POOL[i % POOL.len()], 1 + rng.next(8)));
        }
    }
    (inv, oversized)
}

fn check_sorted(before: &SurvivalInventory, after: &SurvivalInventory, oversized: bool) {
    assert_eq!(totals(&before.pack), totals(&after.pack), "units changed");
    assert_eq!(before.hotbar, after.hotbar, "hotbar moved");
    let largest = |item: Item| {
        before.pack.iter().flatten().filter(|s| s.item == item).map(|s| s.count).max()
    };
    let used = after.pack.iter().take_while(|s| s.is_some()).count();
    assert!(after.pack[used..].iter().all(Option::is_none), "gap in sorted pack");
    let stacks: Vec<ItemStack> = after.pack.iter().flatten().copied().collect();
    for stack in &stacks {
        let limit = after.stack_limit(stack.item);
        assert!(stack.count > 0);
        assert!(stack.count <= limit.max(largest(stack.item).unwrap_or(0)));
        if !oversized {
            assert!(stack.count <= limit, "{stack:?} above limit {limit}");
        }
    }
    for pair in stacks.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        assert!(sort_key(a.item) <= sort_key(b.item), "{a:?} before {b:?}");
        if a.item == b.item {
            assert!(a.count >= b.count, "count not descending");
            let cap = after.stack_limit(a.item).max(largest(a.item).unwrap_or(0));
            assert_eq!(a.count, cap, "partial stack before another of the same item");
        }
    }
    let mut runs = 0;
    for (i, stack) in stacks.iter().enumerate() {
        if i == 0 || stacks[i - 1].item != stack.item {
            runs += 1;
        }
    }
    assert_eq!(runs, totals(&after.pack).len(), "one item split across runs");
}

#[test]
fn sort_preserves_every_unit_and_respects_limits() {
    for seed in 0..4000 {
        let (before, oversized) = random_inventory(seed);
        let mut after = before.clone();
        after.sort_pack();
        check_sorted(&before, &after, oversized);

        let mut again = after.clone();
        again.sort_pack();
        assert_eq!(again.pack, after.pack, "seed {seed}: not idempotent");

        // The result depends on the contents only, not on where they sat.
        let mut shuffled = before.clone();
        let mut rng = Rng(seed ^ 0xA5A5);
        for i in (1..PACK_SLOTS).rev() {
            shuffled.pack.swap(i, rng.next(i as u32 + 1) as usize);
        }
        shuffled.sort_pack();
        assert_eq!(shuffled.pack, after.pack, "seed {seed}: order depends on layout");
    }
}

#[test]
fn sort_orders_by_category_then_id_then_count() {
    let mut inv = SurvivalInventory::empty();
    let planks = Item::Block(BlockId::Planks);
    inv.pack[2] = Some(ItemStack::new(planks, 10));
    inv.pack[4] = Some(ItemStack::new(Item::Resource(3), 5));
    inv.pack[7] = Some(ItemStack::new(Item::Weapon(1), 1));
    inv.pack[8] = Some(ItemStack::new(Item::Resource(1), 9));
    inv.pack[11] = Some(ItemStack::new(Item::Food(FoodKind::FungalRation), 3));
    inv.pack[26] = Some(ItemStack::new(planks, 60));
    inv.sort_pack();
    let expected = [
        Some(ItemStack::new(Item::Weapon(1), 1)),
        Some(ItemStack::new(Item::Food(FoodKind::FungalRation), 3)),
        Some(ItemStack::new(Item::Resource(1), 9)),
        Some(ItemStack::new(Item::Resource(3), 5)),
        Some(ItemStack::new(planks, 64)),
        Some(ItemStack::new(planks, 6)),
        None,
    ];
    assert_eq!(inv.pack[..expected.len()], expected);
}

#[test]
fn oversized_stacks_from_a_larger_scale_still_fit() {
    let stone = Item::Block(BlockId::Stone);
    let mut inv = SurvivalInventory::empty();
    inv.voxel_m = 1.0;
    let fine_limit = SurvivalInventory::limit_at_scale(stone, 0.25);
    for (i, slot) in inv.pack.iter_mut().enumerate() {
        *slot = Some(ItemStack { item: stone, count: fine_limit - i as u32 });
    }
    let before = inv.clone();
    inv.sort_pack();
    check_sorted(&before, &inv, true);
    assert!(inv.pack.iter().all(Option::is_some));
}

#[test]
fn take_from_pack_never_reads_the_hotbar() {
    let stone = Item::Block(BlockId::Stone);
    let mut inv = SurvivalInventory::empty();
    inv.hotbar[HOTBAR_SLOTS - 1] = Some(ItemStack::new(stone, 30));
    inv.pack[5] = Some(ItemStack::new(stone, 20));
    inv.pack[9] = Some(ItemStack::new(stone, 4));
    assert_eq!(inv.take_from_pack(stone, 22), 22);
    assert_eq!(inv.pack_count(stone), 2);
    assert_eq!(inv.take_from_pack(stone, 99), 2);
    assert_eq!(inv.count_item(stone), 30);
}
