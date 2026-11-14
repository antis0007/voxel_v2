use super::*;
use crate::world::inventory::{Item, ItemStack, SurvivalInventory};

fn world() -> World {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    w.inventory = SurvivalInventory::empty();
    w.sync_from_inventory();
    w
}

fn pipe() -> Item {
    Item::Block(world::factory::Kind::Pipe.block())
}

fn person(count: u32, slot: usize) -> Personal {
    let mut p = Personal {
        inventory: SurvivalInventory::empty(),
        ..Default::default()
    };
    let stack = Some(ItemStack::new(pipe(), count));
    match slot {
        0 => p.inventory.cursor = stack,
        1 => p.inventory.pack[0] = stack,
        2 => p.inventory.craft_grid[0] = stack,
        _ => p.inventory.hotbar[0] = stack,
    }
    p
}

#[test]
fn personal_kits_survive_fresh_join_and_never_copy_between_players() {
    for slot in 0..4 {
        let mut w = world();
        w.inventory.pack[0] = Some(ItemStack::new(pipe(), 3));
        w.sync_from_inventory();
        let host = w.inventory.clone();
        let mut a = person(8, slot);
        let a_inventory = a.inventory.clone();
        let mut b = Personal {
            inventory: SurvivalInventory::empty(),
            ..Default::default()
        };
        let b_inventory = b.inventory.clone();
        for _ in 0..3 {
            a.swap(&mut w);
            w.reconcile_legacy_to_inventory();
            assert_eq!(w.inventory, a_inventory);
            w.sync_from_inventory();
            a.swap(&mut w);
            assert_eq!(w.factory.kits[world::factory::Kind::Pipe as usize], 3);
            assert_eq!(w.inventory, host);
            b.swap(&mut w);
            w.reconcile_legacy_to_inventory();
            assert_eq!(
                w.inventory, b_inventory,
                "another player must not inherit kits"
            );
            w.sync_from_inventory();
            b.swap(&mut w);
            a.swap(&mut w);
            w.reconcile_legacy_to_inventory();
            assert_eq!(
                w.inventory, a_inventory,
                "fresh admission must not erase kits"
            );
            a.swap(&mut w);
        }
    }
}

#[test]
fn personal_kits_keep_settled_legacy_spending_private() {
    let mut w = world();
    let mut a = person(8, 0);
    let mut b = person(2, 1);
    a.swap(&mut w);
    w.factory.kits[world::factory::Kind::Pipe as usize] -= 1;
    w.reconcile_legacy_to_inventory();
    assert_eq!(w.inventory.count_item(pipe()), 7);
    a.swap(&mut w);
    b.swap(&mut w);
    w.reconcile_legacy_to_inventory();
    assert_eq!(w.inventory.count_item(pipe()), 2);
    b.swap(&mut w);
    a.swap(&mut w);
    w.reconcile_legacy_to_inventory();
    assert_eq!(w.inventory.count_item(pipe()), 7);
    a.swap(&mut w);
    assert_eq!(w.inventory.count_item(pipe()), 0);
}

#[test]
fn personal_kits_rebuild_from_saved_slots_without_new_schema() {
    let mut w = world();
    let a = person(8, 0);
    let saved = pack(&a).unwrap();
    let mut restored: Personal = unpack(&saved).unwrap();
    restored.swap(&mut w);
    w.reconcile_legacy_to_inventory();
    assert_eq!(w.inventory, a.inventory);
    restored.swap(&mut w);
    assert_eq!(restored.inventory, a.inventory);
    assert_eq!(w.factory.kits[world::factory::Kind::Pipe as usize], 0);
}

#[test]
#[ignore = "manual bounded mount benchmark"]
fn personal_kits_mount_benchmark() {
    let mut w = world();
    let mut a = person(8, 0);
    let start = std::time::Instant::now();
    for _ in 0..10_000 {
        a.swap(&mut w);
        a.swap(&mut w);
    }
    eprintln!(
        "personal_kits_mount_pair_us={:.3}",
        start.elapsed().as_secs_f64() * 1e6 / 10_000.0
    );
    assert_eq!(a.inventory.count_item(pipe()), 8);
}
// Replay the serialized semantic command through the same authoritative handler
// that runs while a remote player's Personal state is mounted.
fn selected_for_person(
    w: &mut World,
    personal: &mut Personal,
    body: &Player,
    actor: PlayerId,
    recipe: usize,
    bulk: bool,
) {
    let command = actions::Command::Inventory(inventory::Command::CraftRecipe(
        recipe,
        world::inventory::selected_recipe_key(recipe).unwrap(),
        bulk,
    ));
    let wire = pack(&command).unwrap();
    let decoded: actions::Command = unpack_limit(&wire, 65_536).unwrap();
    personal.swap(w);
    let result = actions::apply(
        decoded,
        w,
        body,
        &DayNightCycle::default(),
        actor,
        0,
        false,
        &mut actions::Tools::default(),
        &mut world::edit_history::EditHistory::new(world::edit_history::REMOTE_CELLS),
        &mut HashSet::new(),
        &mut HashSet::new(),
    );
    personal.swap(w);
    result.unwrap();
}

#[test]
fn personal_selected_crafting_keeps_two_players_and_host_independent() {
    let recipe_id = world::inventory::all_recipes().iter()
        .position(|recipe| recipe.output.item == pipe()).unwrap();
    let recipe = &world::inventory::all_recipes()[recipe_id];
    let mut w = world();
    for &(item, count) in recipe.ingredients {
        assert_eq!(w.inventory.try_insert(ItemStack::new(item, count * 5)).1, 0);
    }
    w.inventory.pack[0] = Some(ItemStack::new(pipe(), 3));
    w.sync_from_inventory();
    let host = w.inventory.clone();
    let bench = IVec3::new(0, 200, 0);
    w.set_many(&[(bench, BlockId::Workbench)]);
    let mut body = Player::new();
    body.pos = Vec3::new(1.5, 200.0, 0.5);
    body.precise_pos = body.pos.as_dvec3();
    let supplied = |batches| {
        let mut personal = Personal {
            inventory: SurvivalInventory::empty(),
            bench: Some(bench),
            ..Default::default()
        };
        for &(item, count) in recipe.ingredients {
            assert_eq!(personal.inventory.try_insert(ItemStack::new(item, count * batches)).1, 0);
        }
        personal
    };
    let mut a = supplied(1);
    let mut b = supplied(3);
    let b_before = b.inventory.clone();

    selected_for_person(&mut w, &mut a, &body, 1, recipe_id, false);
    assert_eq!(a.inventory.count_item(pipe()), recipe.output.count);
    assert_eq!(a.inventory.crafted, recipe.output.count);
    for &(item, _) in recipe.ingredients { assert_eq!(a.inventory.count_item(item), 0); }
    let a_after = a.inventory.clone();
    assert_eq!(b.inventory, b_before);
    assert_eq!(w.inventory, host);
    assert_eq!(w.factory.kits[world::factory::Kind::Pipe as usize], 3);

    // A cannot borrow B's supplies (or the host's supplies) after exhausting its own.
    selected_for_person(&mut w, &mut a, &body, 1, recipe_id, true);
    assert_eq!(a.inventory, a_after);
    assert_eq!(b.inventory, b_before);
    assert_eq!(w.inventory, host);

    selected_for_person(&mut w, &mut b, &body, 2, recipe_id, true);
    assert_eq!(b.inventory.count_item(pipe()), recipe.output.count * 3);
    assert_eq!(b.inventory.crafted, recipe.output.count * 3);
    for &(item, _) in recipe.ingredients { assert_eq!(b.inventory.count_item(item), 0); }
    assert_eq!(a.inventory, a_after);
    assert_eq!(w.inventory, host);
    assert_eq!(w.factory.kits[world::factory::Kind::Pipe as usize], 3);
    assert_eq!(w.factory.bench, None);
}

#[test]
fn personal_reconnect_records_restore_only_the_saved_players_inventory() {
    let mut w = world();
    w.inventory.pack[0] = Some(ItemStack::new(pipe(), 3));
    w.sync_from_inventory();
    let host = w.inventory.clone();
    let mut a = person(8, 0);
    a.inventory.hotbar[4] = Some(ItemStack::new(Item::Block(BlockId::Log), 2));
    a.inventory.craft_grid[4] = Some(ItemStack::new(Item::Block(BlockId::Planks), 3));
    a.inventory.resource_remainders[0] = 17;
    a.expedition.resources[0] = 17;
    let mut b = person(2, 1);
    b.inventory.cursor = Some(ItemStack::new(Item::Block(BlockId::Stone), 5));
    b.inventory.hotbar[3] = Some(ItemStack::new(Item::Block(BlockId::Log), 4));
    b.inventory.resource_remainders[1] = 31;
    b.expedition.resources[1] = 31;
    let a_inventory = a.inventory.clone();
    let b_inventory = b.inventory.clone();
    let mut a_body = Player::new();
    a_body.pos = Vec3::new(2.0, 200.0, 3.0);
    a_body.precise_pos = a_body.pos.as_dvec3();
    let mut b_body = Player::new();
    b_body.pos = Vec3::new(12.0, 200.0, 13.0);
    b_body.precise_pos = b_body.pos.as_dvec3();
    let saved_a = save_player(&a_body, &a).unwrap();
    let saved_b = save_player(&b_body, &b).unwrap();
    assert_eq!(w.inventory, host);
    let mut restored_a = load_player(&saved_a).unwrap();
    let mut restored_b = load_player(&saved_b).unwrap();
    assert_eq!(restored_a.player.pos, a_body.pos);
    assert_eq!(restored_b.player.pos, b_body.pos);
    assert_eq!(restored_a.personal.inventory, a_inventory);
    assert_eq!(restored_b.personal.inventory, b_inventory);

    // Repeated, interleaved admissions must neither erase private slots nor leak
    // inventory/legacy kit counters from the host or the previously mounted peer.
    for _ in 0..2 {
        restored_a.personal.swap(&mut w);
        w.reconcile_legacy_to_inventory();
        assert_eq!(w.inventory, a_inventory);
        assert_eq!(w.factory.kits[world::factory::Kind::Pipe as usize], 8);
        w.sync_from_inventory();
        restored_a.personal.swap(&mut w);
        assert_eq!(restored_a.personal.inventory, a_inventory);
        assert_eq!(restored_b.personal.inventory, b_inventory);
        assert_eq!(w.inventory, host);
        assert_eq!(w.factory.kits[world::factory::Kind::Pipe as usize], 3);

        restored_b.personal.swap(&mut w);
        w.reconcile_legacy_to_inventory();
        assert_eq!(w.inventory, b_inventory);
        assert_eq!(w.factory.kits[world::factory::Kind::Pipe as usize], 2);
        w.sync_from_inventory();
        restored_b.personal.swap(&mut w);
        assert_eq!(restored_a.personal.inventory, a_inventory);
        assert_eq!(restored_b.personal.inventory, b_inventory);
        assert_eq!(w.inventory, host);
        assert_eq!(w.factory.kits[world::factory::Kind::Pipe as usize], 3);
    }
    assert_eq!(a.inventory, a_inventory);
    assert_eq!(b.inventory, b_inventory);
}
