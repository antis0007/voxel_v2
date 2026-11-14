use super::*;

fn recipe(name: &str) -> &'static CraftingRecipe {
    all_recipes().iter().find(|r| r.name == name).unwrap()
}

fn supply(batches: u32) -> SurvivalInventory {
    let mut inv = SurvivalInventory::empty();
    inv.pack[1] = Some(ItemStack::new(Item::Resource(6), batches * 2));
    inv.cursor = Some(ItemStack::new(Item::Resource(4), batches));
    inv
}

fn bytes(inv: &SurvivalInventory) -> Vec<u8> {
    let mut result = Vec::new();
    inv.encode(&mut result);
    result
}

#[test]
fn recipe_selection_helm_and_coil_keep_indices_and_costs() {
    let helm = recipe("Copper helm");
    let coil = recipe("Hand-wound induction coil");
    assert_eq!(all_recipes().iter().position(|r| r == coil), Some(15));
    assert_eq!(all_recipes().iter().position(|r| r == helm), Some(59));
    assert_eq!(helm.ingredients, coil.ingredients);
    assert_eq!(helm.ingredients, &[(Item::Resource(6), 2), (Item::Resource(4), 1)]);
    assert!(helm.needs_bench() && coil.needs_bench());
    assert_eq!(helm.output, ItemStack::new(Item::Armor(EquipSlot::Head, ArmorTier::Copper), 1));
    assert_eq!(coil.output, ItemStack::new(Item::Resource(16), 1));
}

#[test]
fn recipe_selection_stages_repeated_batches_and_crafts_the_requested_output() {
    for name in ["Copper helm", "Hand-wound induction coil"] {
        let r = recipe(name);
        let mut inv = supply(3);
        for n in 1..=3 {
            assert!(inv.stage_recipe_into_grid(r, true), "{name}");
            assert_eq!(inv.matching_grid_recipe(true), Some((r, n)));
            assert_eq!(inv.count_item(Item::Resource(6)), 6);
            assert_eq!(inv.count_item(Item::Resource(4)), 3);
        }
        assert_eq!(inv.craft_from_grid(true, true), Some(ItemStack::new(r.output.item, 3)));
        assert_eq!(inv.count_item(r.output.item), 3);
        assert_eq!(inv.count_item(Item::Resource(6)), 0);
        assert_eq!(inv.count_item(Item::Resource(4)), 0);
        assert!(inv.craft_grid.iter().all(Option::is_none));
    }
}

#[test]
fn recipe_selection_manual_helm_and_coil_survive_grid_offsets() {
    for name in ["Copper helm", "Hand-wound induction coil"] {
        let r = recipe(name);
        for row in 0..=3 - r.shape().unwrap().height {
            let mut inv = SurvivalInventory::empty();
            for (cell, item, count) in r.cells(true, 2).unwrap() {
                inv.craft_grid[cell + row * 3] = Some(ItemStack::new(item, count));
            }
            assert_eq!(inv.matching_grid_recipe(true), Some((r, 2)));
            assert_eq!(inv.craft_from_grid(false, true), Some(r.output));
            assert_eq!(inv.matching_grid_recipe(true), Some((r, 1)));
            assert_eq!(inv.cursor, Some(r.output));
        }
    }
}

#[test]
fn recipe_selection_missing_bench_or_output_space_preserves_every_slot() {
    let r = recipe("Copper helm");
    let mut inv = supply(1);
    let before = bytes(&inv);
    assert!(!inv.stage_recipe_into_grid(r, false));
    assert_eq!(bytes(&inv), before);
    assert!(inv.stage_recipe_into_grid(r, true));
    inv.cursor = Some(ItemStack::new(Item::Resource(16), 1));
    let before = bytes(&inv);
    assert_eq!(inv.craft_from_grid(false, true), None);
    assert_eq!(bytes(&inv), before);
}

#[test]
fn recipe_selection_conflicting_route_never_commits_another_output() {
    let mut conflict = *recipe("Copper helm");
    conflict.name = "Conflicting test recipe";
    conflict.output = ItemStack::new(Item::Block(BlockId::Stone), 1);
    let mut inv = supply(2);
    inv.craft_grid[8] = Some(ItemStack::new(Item::Block(BlockId::Dirt), 3));
    let before = bytes(&inv);
    assert!(!inv.stage_recipe_into_grid(&conflict, true));
    assert_eq!(bytes(&inv), before);
}

#[test]
fn recipe_selection_bulk_shadow_keeps_last_valid_batch() {
    let r = all_recipes().iter().find(|r| r.output.item == Item::Block(factory::Kind::Assembler.block())).unwrap();
    let mut inv = SurvivalInventory::empty();
    for &(item, count) in r.ingredients {
        assert_eq!(inv.try_insert(ItemStack::new(item, count * 2)).1, 0);
    }
    assert!(inv.stage_recipe_into_grid(r, true));
    let before = bytes(&inv);
    assert!(!inv.stage_recipe_into_grid(r, true));
    assert_eq!(bytes(&inv), before);
    assert_eq!(inv.matching_grid_recipe(true), Some((r, 1)));
    assert_eq!(inv.craft_from_grid(false, true), Some(r.output));
}

#[test]
fn recipe_selection_current_and_legacy_inventory_codec_keep_staged_output() {
    for version in [20, 68, crate::world::storage::SAVE_VERSION] {
        let r = recipe("Copper helm");
        let mut inv = supply(2);
        assert!(inv.stage_recipe_into_grid(r, true));
        let mut encoded = Vec::new();
        inv.encode_versioned(&mut encoded, version);
        let mut offset = 0;
        let mut restored = SurvivalInventory::decode_versioned(&mut || {
            let word = u32::from_le_bytes(encoded[offset..offset + 4].try_into()?);
            offset += 4;
            Ok(word)
        }, 1.0, version).unwrap();
        assert_eq!(offset, encoded.len());
        assert_eq!(restored.craft_grid, inv.craft_grid);
        assert_eq!(restored.matching_grid_recipe(true), Some((r, 1)));
        assert_eq!(restored.craft_from_grid(true, true), Some(r.output));
    }
}

#[cfg(feature = "multiplayer")]
#[test]
fn recipe_selection_host_replays_staging_and_crafting_without_wrong_outputs() {
    for name in ["Copper helm", "Hand-wound induction coil"] {
        let r = recipe(name);
        let mut client = supply(3);
        let mut host = client.clone();
        crate::multiplayer::inventory::begin();
        assert!(client.stage_recipe_into_grid(r, true));
        assert!(client.stage_recipe_into_grid(r, true));
        assert_eq!(client.craft_from_grid(true, true), Some(ItemStack::new(r.output.item, 2)));
        let commands = crate::multiplayer::inventory::finish();
        assert_eq!(commands.len(), 3);
        for command in commands { command.apply(&mut host, true).unwrap(); }
        assert_eq!(bytes(&host), bytes(&client));
        assert_eq!(host.count_item(r.output.item), 2);
        assert_eq!(host.count_item(Item::Resource(6)), 2);
        assert_eq!(host.count_item(Item::Resource(4)), 1);
    }
}

#[test]
#[ignore = "bounded staging microbenchmark"]
fn recipe_selection_staging_cost() {
    let r = recipe("Copper helm");
    let seed = supply(1);
    for checked in [false, true] {
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let mut inv = seed.clone();
            let accepted = if checked { inv.stage_recipe_into_grid(r, true) } else {
                let mut next = inv.clone();
                let accepted = next.stage_recipe_in_place(r, true);
                if accepted { inv = next; }
                accepted
            };
            assert!(std::hint::black_box(accepted));
            std::hint::black_box(inv);
        }
        println!("recipe staging checked={checked}: {:.2} us/operation", start.elapsed().as_secs_f64() * 100.0);
    }
}

#[cfg(feature = "multiplayer")]
#[test]
fn recipe_selection_rejected_machine_route_is_a_host_noop() {
    let r = all_recipes().iter().find(|r| r.output.item == Item::Block(factory::Kind::Pipe.block())).unwrap();
    let mut client = SurvivalInventory::empty();
    for &(item, count) in r.ingredients {
        assert_eq!(client.try_insert(ItemStack::new(item, count)).1, 0);
    }
    let mut host = client.clone();
    let before = bytes(&client);
    crate::multiplayer::inventory::begin();
    assert!(!client.stage_recipe_into_grid(r, true));
    let commands = crate::multiplayer::inventory::finish();
    assert_eq!(commands.len(), 1);
    for command in commands { command.apply(&mut host, true).unwrap(); }
    assert_eq!(bytes(&host), before);
    assert_eq!(bytes(&client), before);
}