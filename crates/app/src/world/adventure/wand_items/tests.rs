use super::*;

fn wand_ids(inv: &SurvivalInventory) -> Vec<u16> {
    inv.hotbar
        .iter()
        .chain(inv.pack.iter())
        .flatten()
        .filter_map(|s| match s.item {
            Item::Wand { id, .. } => Some(id),
            _ => None,
        })
        .collect()
}

#[test]
fn crafted_chassis_keeps_the_table_stats() {
    let plain = wands::Loadout::default();
    for tier in 1..=3 {
        let a = stats_for(tier, &plain, 2);
        let b = wands::stats(tier, plain.mode, 2);
        assert_eq!((a.name, a.cast_delay, a.recharge_time, a.mana_max), (b.name, b.cast_delay, b.recharge_time, b.mana_max));
    }
    assert_eq!(charge_scale(&plain), 1.0);
}

#[test]
fn found_wands_are_deterministic_and_varied() {
    let a = generate(1234, 2);
    assert_eq!(a, generate(1234, 2));
    assert!(wands::projectile(a.loadout.slot(0)), "a found wand leads with a bolt");
    let mut titles = std::collections::HashSet::new();
    let mut looks = std::collections::HashSet::new();
    let mut delays = std::collections::HashSet::new();
    for seed in 1..200 {
        let w = generate(seed * 7919, 1 + seed % 3);
        let s = stats_for(w.tier, &w.loadout, 0);
        assert!(s.cast_delay > 0.0 && s.recharge_time > 0.0 && s.mana_max > 0.0);
        assert!((0..wands::MAX_SLOTS).all(|i| w.loadout.slot(i) == wands::EMPTY || w.loadout.slot(i) <= MAX_SPELL_ID));
        titles.insert(s.name);
        looks.insert(look(&w));
        delays.insert((s.cast_delay * 1000.0) as u32);
    }
    assert!(titles.len() >= 20 && looks.len() >= 60 && delays.len() >= 40, "{} {} {}", titles.len(), looks.len(), delays.len());
}

#[test]
fn wand_items_round_trip_and_never_stack() {
    let item = Item::Wand { id: 513, look: look(&generate(9, 3)) };
    let (tag, payload) = item.encode_u32();
    assert_eq!(Item::decode_u32(tag, payload).unwrap(), item);
    assert!(Item::decode_u32(8, 5).is_err(), "a look without a tier is corrupt");
    assert_eq!(item.max_stack(), 1);
}

#[test]
fn a_bench_wand_becomes_an_item_and_wands_swap_without_losing_decks() {
    let mut a = Adventure::default();
    let mut inv = SurvivalInventory::empty();
    a.wand = 1;
    a.sync_wands(&mut inv, 0, Vec3::ZERO);
    assert_eq!(wand_ids(&inv), vec![0]);
    assert_eq!(a.active_wand, Some(0));
    // Edit the held deck, then take a found wand into another hotbar slot.
    a.loadout.set_slot(1, 4);
    let found = a.mint_wand(generate(77, 3)).unwrap();
    inv.hotbar[3] = Some(ItemStack::new(found, 1));
    a.sync_wands(&mut inv, 3, Vec3::ZERO);
    assert_eq!(a.active_wand, Some(1));
    assert_eq!(a.wand, 3);
    assert_eq!(a.loadout.seed, generate(77, 3).loadout.seed);
    // The first wand kept the edit.
    assert_eq!(a.wand_bank[0].loadout.slot(1), 4);
    // Selecting a block slot keeps the last wand in hand.
    a.sync_wands(&mut inv, 5, Vec3::ZERO);
    assert_eq!(a.active_wand, Some(1));
    // Spells on the found wand were learned.
    let first = a.wand_bank[1].loadout.slot(0);
    assert!(a.spell_log.has(first));
    // Dropping the held wand hands control to the remaining one.
    inv.hotbar[3] = None;
    a.sync_wands(&mut inv, 5, Vec3::ZERO);
    assert_eq!(a.active_wand, Some(0));
    assert_eq!(a.wand, 1);
}

#[test]
fn losing_every_wand_disarms_without_minting_a_new_one() {
    let mut a = Adventure::default();
    let mut inv = SurvivalInventory::empty();
    a.wand = 2;
    a.sync_wands(&mut inv, 0, Vec3::ZERO);
    inv.hotbar = Default::default();
    a.sync_wands(&mut inv, 0, Vec3::ZERO);
    a.sync_wands(&mut inv, 0, Vec3::ZERO);
    assert_eq!((a.wand, a.active_wand, a.wand_bank.len()), (0, None, 1));
}

#[test]
fn the_bank_round_trips() {
    let mut a = Adventure::default();
    a.mint_wand(generate(5, 1));
    a.mint_wand(generate(6, 3));
    a.active_wand = Some(1);
    let mut bytes = Vec::new();
    a.encode_wand_bank(&mut bytes);
    let mut words = bytes.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
    let mut b = Adventure::default();
    b.decode_wand_bank(&mut || words.next().ok_or_else(|| anyhow::anyhow!("short"))).unwrap();
    assert_eq!(b.wand_bank, a.wand_bank);
    assert_eq!(b.active_wand, Some(1));
}
