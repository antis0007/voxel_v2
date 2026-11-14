use super::*;

const PLANKS: Item = Item::Block(BlockId::Planks);

fn workbench() -> &'static CraftingRecipe {
    all_recipes().iter().find(|r| r.output.item == Item::Block(BlockId::Workbench)).unwrap()
}

fn assert_slots_equal(actual: &SurvivalInventory, expected: &SurvivalInventory) {
    assert_eq!(actual.pack, expected.pack);
    assert_eq!(actual.hotbar, expected.hotbar);
    assert_eq!(actual.craft_grid, expected.craft_grid);
    assert_eq!(actual.cursor, expected.cursor);
    assert_eq!(actual.crafted, expected.crafted);
}

#[test]
fn crafting_cursor_staging_shaped_consumes_each_input_once() {
    let mut inv = SurvivalInventory::empty();
    inv.cursor = Some(ItemStack::new(PLANKS, 4));
    assert!(inv.stage_recipe_into_grid(workbench(), false));
    assert_eq!(inv.matching_grid_recipe(false), Some((workbench(), 1)));
    assert_eq!(inv.craft_grid.iter().flatten().count(), 4);
    assert_eq!(inv.count_item(PLANKS), 4);
    assert_eq!(inv.cursor, None);
}

#[test]
fn crafting_cursor_staging_mixed_sources_are_lossless() {
    for (pack, hotbar, cursor) in [(1, 0, 3), (0, 1, 3), (1, 1, 2)] {
        let mut inv = SurvivalInventory::empty();
        inv.pack[2] = (pack > 0).then(|| ItemStack::new(PLANKS, pack));
        inv.hotbar[4] = (hotbar > 0).then(|| ItemStack::new(PLANKS, hotbar));
        inv.cursor = Some(ItemStack::new(PLANKS, cursor));
        assert!(inv.stage_recipe_into_grid(workbench(), false));
        assert_eq!(inv.matching_grid_recipe(false), Some((workbench(), 1)),
            "pack={pack}, hotbar={hotbar}, cursor={cursor}");
        assert_eq!(inv.count_item(PLANKS), 4);
        assert_eq!(inv.cursor, None);
    }
}

#[test]
fn crafting_cursor_staging_shapeless_kit_preserves_each_input() {
    let recipe = all_recipes().iter().find(|r| r.output.item == Item::Block(BlockId::CoalGenerator)).unwrap();
    assert!(!recipe.shaped());
    let mut inv = SurvivalInventory::empty();
    let (held, count) = recipe.ingredients[0];
    assert!(count > 1);
    inv.cursor = Some(ItemStack::new(held, count));
    for (slot, &(item, count)) in recipe.ingredients[1..].iter().enumerate() {
        inv.pack[slot] = Some(ItemStack::new(item, count));
    }
    assert!(inv.stage_recipe_into_grid(recipe, true));
    assert_eq!(inv.matching_grid_recipe(true), Some((recipe, 1)));
    for &(item, count) in recipe.ingredients {
        assert_eq!(inv.count_item(item), count);
    }
    assert_eq!(inv.cursor, None);
}

#[test]
fn crafting_cursor_staging_repeat_stacks_one_more_batch() {
    let mut inv = SurvivalInventory::empty();
    inv.cursor = Some(ItemStack::new(PLANKS, 9));
    for batches in [1, 2, 2] {
        assert!(inv.stage_recipe_into_grid(workbench(), false));
        assert_eq!(inv.matching_grid_recipe(false), Some((workbench(), batches)));
        assert_eq!(inv.count_item(PLANKS), 9);
    }
}

#[test]
fn crafting_cursor_staging_rejections_preserve_inventory() {
    let mut inv = SurvivalInventory::empty();
    inv.cursor = Some(ItemStack::new(PLANKS, 3));
    let before = inv.clone();
    assert!(!inv.stage_recipe_into_grid(workbench(), false));
    assert_slots_equal(&inv, &before);
    let axe = all_recipes().iter().find(|r| r.name == "Wooden axe").unwrap();
    inv.cursor = Some(ItemStack::new(PLANKS, 4));
    let before = inv.clone();
    assert!(!inv.stage_recipe_into_grid(axe, false));
    assert_slots_equal(&inv, &before);
}

#[test]
fn crafting_cursor_staging_crafts_output_to_the_emptied_cursor() {
    let mut inv = SurvivalInventory::empty();
    inv.cursor = Some(ItemStack::new(PLANKS, 4));
    assert!(inv.stage_recipe_into_grid(workbench(), false));
    assert_eq!(inv.craft_from_grid(false, false), Some(workbench().output));
    assert_eq!(inv.cursor, Some(workbench().output));
    assert_eq!(inv.count_item(PLANKS), 0);
    assert!(inv.craft_grid.iter().all(Option::is_none));
    assert_eq!(inv.crafted, workbench().output.count);
}

#[cfg(feature = "multiplayer")]
#[test]
fn crafting_cursor_staging_replays_the_same_semantic_command() {
    let mut client = SurvivalInventory::empty();
    client.cursor = Some(ItemStack::new(PLANKS, 4));
    let mut host = client.clone();
    crate::multiplayer::inventory::begin();
    let staged = client.stage_recipe_into_grid(workbench(), false);
    let commands = crate::multiplayer::inventory::finish();
    assert!(staged);
    assert_eq!(commands.len(), 1);
    for command in commands { command.apply(&mut host, false).unwrap(); }
    assert_slots_equal(&host, &client);
    assert_eq!(host.matching_grid_recipe(false), Some((workbench(), 1)));
}
