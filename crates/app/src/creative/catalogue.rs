//! Creative equipment catalogue and atomic hotbar selection, shared by host/local play.
use crate::world::{
    World,
    inventory::{ArmorTier, EquipSlot, FoodKind, Item, ItemStack},
};
use anyhow::{Result, ensure};
use std::sync::LazyLock;

pub fn items() -> &'static [Item] {
    static ITEMS: LazyLock<Vec<Item>> = LazyLock::new(|| {
        let mut items = Vec::new();
        items.extend((0..crate::world::factory::RESOURCE_COUNT as u8).map(Item::Resource));
        items.extend((1..=5).map(Item::Weapon));
        for tier in [ArmorTier::Copper, ArmorTier::Steel] {
            items.extend(
                EquipSlot::ALL
                    .into_iter()
                    .map(|slot| Item::Armor(slot, tier)),
            );
        }
        items.extend([
            Item::Bucket(0),
            Item::Armor(EquipSlot::Head, ArmorTier::Lumen),
            Item::Armor(EquipSlot::Head, ArmorTier::Survey),
            Item::Armor(EquipSlot::Head, ArmorTier::Runesight),
            Item::Armor(EquipSlot::Legs, ArmorTier::Kinetic),
            Item::Armor(EquipSlot::Feet, ArmorTier::Kinetic),
            Item::Armor(EquipSlot::Chest, ArmorTier::Ballast),
            Item::Armor(EquipSlot::Chest, ArmorTier::Burner),
            Item::Armor(EquipSlot::Chest, ArmorTier::Induction),
            Item::Armor(EquipSlot::Feet, ArmorTier::Slime),
            Item::Tool(19),
        ]);
        items.extend([1, 2, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 40, 41].map(Item::Tool));
        items.extend(FoodKind::ALL.into_iter().map(Item::Food));
        items
    });
    &ITEMS
}

/// Fill a hotbar slot; displaced equipment must fit elsewhere before committing.
pub fn select(world: &mut World, item: Item, count: u32, slot: usize) -> Result<()> {
    select_many(world, &[(item, count, slot)])
}

pub fn select_row(world: &mut World, items: &[Item]) -> Result<()> {
    ensure!(
        !items.is_empty() && items.len() <= super::HOTBAR_SLOTS,
        "Invalid Selection"
    );
    let selections: Vec<_> = items
        .iter()
        .enumerate()
        .map(|(slot, &item)| (item, world.inventory.stack_limit(item), slot))
        .collect();
    select_many(world, &selections)
}

fn select_many(world: &mut World, selections: &[(Item, u32, usize)]) -> Result<()> {
    ensure!(world.rules.mode.creative_tools(), "Creative Only");
    ensure!(
        !selections.is_empty() && selections.len() <= super::HOTBAR_SLOTS,
        "Invalid Selection"
    );
    let mut next = world.inventory.clone();
    let mut selected_slots = 0u16;
    for &(item, count, slot) in selections {
        ensure!(slot < world.inventory.hotbar.len(), "Invalid Hotbar Slot");
        ensure!(selected_slots & (1 << slot) == 0, "Duplicate Hotbar Slot");
        selected_slots |= 1 << slot;
        let available = match item {
            Item::Block(block) => block.material().tags.selectable,
            _ => items().contains(&item),
        };
        ensure!(available, "Item Unavailable");
        ensure!(
            count > 0 && count <= world.inventory.stack_limit(item),
            "Invalid Quantity"
        );
        let previous = next.hotbar[slot].replace(ItemStack::new(item, count));
        if let Some(previous) = previous {
            if previous.item == item {
                next.hotbar[slot] = Some(ItemStack::new(item, count.max(previous.count)));
            } else {
                ensure!(
                    next.try_insert_pack_first(previous).1 == 0,
                    "Inventory Full"
                );
            }
        }
    }
    world.inventory = next;
    world.sync_from_inventory();
    Ok(())
}
