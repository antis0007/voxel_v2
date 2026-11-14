use super::*;

fn id(kind: factory::Kind) -> usize {
    all_recipes().iter().position(|r| r.output.item == Item::Block(kind.block())).unwrap()
}
fn supply(id: usize, batches: u32) -> SurvivalInventory {
    let mut inv = SurvivalInventory::empty();
    for &(item, count) in all_recipes()[id].ingredients {
        assert_eq!(inv.try_insert(ItemStack::new(item, count * batches)).1, 0);
    }
    inv
}
fn bytes(inv: &SurvivalInventory) -> Vec<u8> { let mut b = Vec::new(); inv.encode(&mut b); b }
fn unchanged(before: &SurvivalInventory, after: &SurvivalInventory) {
    assert_eq!(bytes(before), bytes(after));
    assert_eq!(before, after);
}

#[test]
fn selected_crafting_every_catalog_route_produces_exact_output() {
    let mut conflicts = 0;
    for (id, recipe) in all_recipes().iter().enumerate() {
        let mut inv = supply(id, 1);
        let mut staged = inv.clone();
        conflicts += usize::from(!staged.stage_recipe_into_grid(recipe, true));
        assert_eq!(inv.craft_selected_recipe(id, false, true), Ok(recipe.output), "{}", recipe.name);
        assert_eq!(inv.cursor, Some(recipe.output), "{}", recipe.name);
        assert!(inv.pack.iter().chain(inv.hotbar.iter()).chain(inv.craft_grid.iter()).all(Option::is_none), "{}", recipe.name);
        assert_eq!(inv.crafted, recipe.output.count);
    }
    assert!(conflicts >= 11, "expected generated-kit conflicts, got {conflicts}");
    println!("Exact recipes: {}; formerly blocked grid routes: {conflicts}", all_recipes().len());
}

#[test]
fn selected_crafting_bulk_keeps_recipe_identity_and_exact_costs() {
    for kind in [factory::Kind::Pipe, factory::Kind::Assembler, factory::Kind::AlloyKiln] {
        let id = id(kind);
        let recipe = &all_recipes()[id];
        let mut inv = supply(id, 5);
        assert_eq!(inv.craft_selected_recipe(id, true, true), Ok(ItemStack::new(recipe.output.item, recipe.output.count * 5)));
        assert_eq!(inv.count_item(recipe.output.item), recipe.output.count * 5);
        for &(item, _) in recipe.ingredients { assert_eq!(inv.count_item(item), 0); }
        assert!(inv.craft_grid.iter().all(Option::is_none));
    }
}

#[test]
fn selected_crafting_cursor_and_split_inputs_are_withdrawn_before_output() {
    let id = all_recipes().iter().position(|r| r.name == "Copper helm").unwrap();
    let mut inv = SurvivalInventory::empty();
    inv.cursor = Some(ItemStack::new(Item::Resource(4), 1));
    inv.hotbar[2] = Some(ItemStack::new(Item::Resource(6), 1));
    inv.craft_grid[8] = Some(ItemStack::new(Item::Resource(6), 1));
    inv.craft_grid[4] = Some(ItemStack::new(Item::Block(BlockId::Dirt), 3));
    assert_eq!(inv.craft_selected_recipe(id, false, true), Ok(all_recipes()[id].output));
    assert_eq!(inv.cursor, Some(all_recipes()[id].output));
    assert_eq!(inv.craft_grid[4], Some(ItemStack::new(Item::Block(BlockId::Dirt), 3)));
    assert_eq!(inv.count_item(Item::Resource(6)), 0);
    assert_eq!(inv.count_item(Item::Resource(4)), 0);
}

#[test]
fn selected_crafting_rejects_missing_inputs_bench_and_invalid_ids_atomically() {
    let id = id(factory::Kind::Pipe);
    let mut inv = supply(id, 1);
    let before = inv.clone();
    assert_eq!(inv.craft_selected_recipe(id, true, false), Err(CraftBlocker::Workbench));
    unchanged(&before, &inv);
    assert_eq!(inv.craft_selected_recipe(usize::MAX, true, true), Err(CraftBlocker::Ingredients));
    unchanged(&before, &inv);
    inv.remove_item(all_recipes()[id].ingredients[0].0, 1);
    let before = inv.clone();
    assert_eq!(inv.craft_selected_recipe(id, true, true), Err(CraftBlocker::Ingredients));
    unchanged(&before, &inv);
}

#[test]
fn selected_crafting_incompatible_or_overfull_cursor_preserves_every_input() {
    let id = id(factory::Kind::Pipe);
    for held in [ItemStack::new(Item::Block(BlockId::Dirt), 1), ItemStack::new(all_recipes()[id].output.item, SurvivalInventory::empty().stack_limit(all_recipes()[id].output.item))] {
        let mut inv = supply(id, 2);
        inv.cursor = Some(held);
        let before = inv.clone();
        assert!(inv.craft_selected_recipe(id, false, true).is_err());
        unchanged(&before, &inv);
    }
}

#[test]
fn selected_crafting_partial_max_stops_before_losing_an_output() {
    let id = id(factory::Kind::Pipe);
    let recipe = &all_recipes()[id];
    let mut inv = SurvivalInventory::empty();
    let stone = Item::Block(BlockId::Stone);
    let stone_stack = Some(ItemStack::new(stone, inv.stack_limit(stone)));
    inv.pack.fill(stone_stack);
    inv.hotbar.fill(stone_stack);
    for (i, &(item, count)) in recipe.ingredients.iter().enumerate() {
        inv.pack[i] = Some(ItemStack::new(item, count * 3));
    }
    inv.pack[26] = Some(ItemStack::new(recipe.output.item, inv.stack_limit(recipe.output.item) - recipe.output.count));
    assert_eq!(inv.craft_selected_recipe(id, true, true), Ok(recipe.output));
    for &(item, count) in recipe.ingredients { assert_eq!(inv.count_item(item), count * 2); }
    assert_eq!(inv.crafted, recipe.output.count);
    let before = inv.clone();
    assert_eq!(inv.craft_selected_recipe(id, true, true), Err(CraftBlocker::OutputSpace));
    unchanged(&before, &inv);
}

#[test]
fn selected_crafting_full_pack_can_use_slot_freed_by_ingredients() {
    let id = id(factory::Kind::Pipe);
    let recipe = &all_recipes()[id];
    let mut inv = SurvivalInventory::empty();
    let stone = Item::Block(BlockId::Stone);
    let stone_stack = Some(ItemStack::new(stone, inv.stack_limit(stone)));
    inv.pack.fill(stone_stack);
    inv.hotbar.fill(stone_stack);
    for (i, &(item, count)) in recipe.ingredients.iter().enumerate() { inv.pack[i] = Some(ItemStack::new(item, count)); }
    assert_eq!(inv.craft_selected_recipe(id, true, true), Ok(recipe.output));
}

#[test]
fn selected_crafting_max_has_a_fixed_work_budget() {
    let id = id(factory::Kind::Pipe);
    let recipe = &all_recipes()[id];
    let mut inv = supply(id, 70);
    assert_eq!(inv.craft_selected_recipe(id, true, true), Ok(ItemStack::new(recipe.output.item, recipe.output.count * MAX_SELECTED_BATCHES)));
    for &(item, count) in recipe.ingredients { assert_eq!(inv.count_item(item), count * 6); }
    assert_eq!(inv.craft_selected_recipe(id, true, true), Ok(ItemStack::new(recipe.output.item, recipe.output.count * 6)));
}

#[test]
fn selected_crafting_leaves_equipment_container_and_settings_untouched() {
    let id = id(factory::Kind::Pipe);
    let mut inv = supply(id, 1);
    inv.offhand = Some(ItemStack::new(Item::Block(BlockId::Dirt), 2));
    inv.protected = 3;
    inv.minimums.push((Item::Resource(6), 10));
    let before = inv.clone();
    assert!(inv.craft_selected_recipe(id, true, true).is_ok());
    assert_eq!(inv.offhand, before.offhand);
    assert_eq!(inv.equipment, before.equipment);
    assert_eq!(inv.container, before.container);
    assert_eq!(inv.protected, before.protected);
    assert_eq!(inv.minimums, before.minimums);
}

#[test]
fn selected_crafting_saved_results_need_no_pending_recipe_state() {
    let id = id(factory::Kind::Pipe);
    for version in [20, 68, crate::world::storage::SAVE_VERSION] {
        let mut inv = supply(id, 3);
        assert!(inv.craft_selected_recipe(id, false, true).is_ok());
        let mut encoded = Vec::new(); inv.encode_versioned(&mut encoded, version);
        let mut offset = 0;
        let mut restored = SurvivalInventory::decode_versioned(&mut || {
            let word = u32::from_le_bytes(encoded[offset..offset + 4].try_into()?); offset += 4; Ok(word)
        }, 1.0, version).unwrap();
        assert_eq!(offset, encoded.len());
        assert_eq!(restored.cursor, inv.cursor);
        assert!(restored.craft_selected_recipe(id, true, true).is_ok());
        assert_eq!(restored.count_item(all_recipes()[id].output.item), all_recipes()[id].output.count * 3);
    }
}

#[cfg(feature = "multiplayer")]
#[test]
fn selected_crafting_host_replays_one_serialized_command_per_action() {
    let id = id(factory::Kind::Pipe);
    for bulk in [false, true] {
        let mut client = supply(id, 3);
        let mut host = client.clone();
        crate::multiplayer::inventory::begin();
        assert!(client.craft_selected_recipe(id, bulk, true).is_ok());
        let commands = crate::multiplayer::inventory::finish();
        assert_eq!(commands.len(), 1);
        let wire = ron::to_string(&commands).unwrap();
        let decoded: Vec<crate::multiplayer::inventory::Command> = ron::from_str(&wire).unwrap();
        for command in decoded { command.apply(&mut host, true).unwrap(); }
        unchanged(&client, &host);
    }
}

#[cfg(feature = "multiplayer")]
#[test]
fn selected_crafting_host_rejects_invalid_ids_and_uses_authoritative_supply_and_bench() {
    use crate::multiplayer::inventory::Command;
    let id = id(factory::Kind::Pipe);
    let mut host = supply(id, 1);
    let before = host.clone();
    Command::CraftRecipe(id, selected_recipe_key(id).unwrap(), true).apply(&mut host, false).unwrap(); unchanged(&before, &host);
    assert!(Command::CraftRecipe(usize::MAX, 0, true).apply(&mut host, true).is_err()); unchanged(&before, &host);
    assert!(Command::CraftRecipe(id, selected_recipe_key(id).unwrap() ^ 1, true).apply(&mut host, true).is_err()); unchanged(&before, &host);
    Command::CraftRecipe(id, selected_recipe_key(id).unwrap(), true).apply(&mut host, true).unwrap();
    let before = host.clone();
    Command::CraftRecipe(id, selected_recipe_key(id).unwrap(), true).apply(&mut host, true).unwrap(); unchanged(&before, &host);
}

#[test]
#[ignore = "bounded selected crafting microbenchmark"]
fn selected_crafting_transaction_cost() {
    let id = id(factory::Kind::Pipe);
    for batches in [1, 64] {
        let seed = supply(id, batches);
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let mut inv = seed.clone();
            assert!(std::hint::black_box(inv.craft_selected_recipe(id, true, true)).is_ok());
            std::hint::black_box(inv);
        }
        println!("selected recipe {batches} batches: {:.2} us/action", start.elapsed().as_secs_f64() * 1000.0);
    }
}
