use super::ops::{self, Slots, SLOTS};
use super::*;
use crate::world::inventory::{HOTBAR_SLOTS, PACK_SLOTS};
use crate::world::{ChunkData, WorldType};
use engine_core::VoxelScalePreset;
use std::collections::HashSet;
use std::sync::Arc;

const COBBLE: Item = Item::Block(BlockId::Cobblestone);
const COPPER: Item = Item::Resource(6);
const IRON: Item = Item::Resource(8);

fn st(item: Item, count: u32) -> Option<ItemStack> {
    Some(ItemStack { item, count })
}

fn chest(stacks: &[(Item, u32)]) -> Slots {
    let mut s = [None; SLOTS];
    for (slot, &(item, n)) in s.iter_mut().zip(stacks) {
        *slot = st(item, n);
    }
    s
}

fn totals(c: &[Slots]) -> Vec<(Item, u64)> {
    let mut out: Vec<(Item, u64)> = Vec::new();
    for s in c.iter().flatten().flatten() {
        match out.iter_mut().find(|(i, _)| *i == s.item) {
            Some((_, n)) => *n += u64::from(s.count),
            None => out.push((s.item, u64::from(s.count))),
        }
    }
    out.sort_by_key(|(i, _)| sorting::sort_key(*i));
    out
}

#[test]
fn consolidate_merges_the_three_chest_example() {
    let mut c = vec![
        chest(&[(COBBLE, 184), (COPPER, 73)]),
        chest(&[(COBBLE, 221), (COPPER, 41), (IRON, 26)]),
        chest(&[(COBBLE, 107), (COPPER, 54), (IRON, 32)]),
    ];
    let before = totals(&c);
    let freed = ops::consolidate(&mut c, |_| 256);
    assert_eq!(totals(&c), before);
    assert_eq!(freed, 8 - 4, "512 cobble = 2 slots, copper 1, iron 1");
    let stacks = |i| c.iter().flatten().flatten().filter(|s| s.item == i).map(|s| s.count).collect::<Vec<_>>();
    assert_eq!(stacks(COBBLE), vec![256, 256]);
    assert_eq!(stacks(COPPER), vec![168]);
    assert_eq!(stacks(IRON), vec![58]);
    // Copper lands where most of it was (chest A), iron in chest C.
    assert!(c[0].iter().flatten().any(|s| s.item == COPPER));
    assert!(c[2].iter().flatten().any(|s| s.item == IRON));
}

#[test]
fn consolidate_leaves_settled_items_and_oversize_stacks_alone() {
    let mut c = vec![chest(&[(COBBLE, 64), (COBBLE, 20)]), chest(&[(COPPER, 9)])];
    let snapshot = c.clone();
    assert_eq!(ops::consolidate(&mut c, |_| 64), 0);
    assert_eq!(c, snapshot, "one partial stack per item is already consolidated");
    // A 300 stack above the 64 cap raises the cap instead of splitting.
    let mut c = vec![chest(&[(COBBLE, 300), (COBBLE, 10)]), chest(&[(COBBLE, 5)])];
    ops::consolidate(&mut c, |_| 64);
    assert_eq!(totals(&c), vec![(COBBLE, 315)]);
    assert!(c.iter().flatten().flatten().count() <= 3);
}

/// Deterministic pseudo-random layouts: consolidation never creates or destroys units.
#[test]
fn consolidate_conserves_units_across_layouts() {
    let mut seed = 0x9e37_79b9_u32;
    let mut next = |m: u32| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed % m
    };
    let items = [COBBLE, COPPER, IRON, Item::Resource(9), Item::Wand { id: 1, look: 1 }];
    for _ in 0..200 {
        let mut c: Vec<Slots> = (0..1 + next(5)).map(|_| [None; SLOTS]).collect();
        for slots in &mut c {
            for slot in slots.iter_mut() {
                if next(3) == 0 {
                    let item = items[next(items.len() as u32) as usize];
                    let cap = item.max_stack();
                    *slot = st(item, 1 + next(cap));
                }
            }
        }
        let before = (totals(&c), c.iter().flatten().flatten().count());
        let freed = ops::consolidate(&mut c, Item::max_stack);
        assert_eq!(totals(&c), before.0);
        assert_eq!(c.iter().flatten().flatten().count() + freed, before.1);
        assert!(c.iter().flatten().flatten().all(|s| s.count >= 1 && s.count <= s.item.max_stack()));
    }
}

#[test]
fn put_prefers_partial_stacks_then_holders_then_anywhere() {
    let mut c = vec![chest(&[(COBBLE, 1)]), chest(&[(COPPER, 250)]), [None; SLOTS]];
    let mut s = ItemStack { item: COPPER, count: 10 };
    ops::put(&mut c, &mut s, 256, Reach::Existing);
    assert_eq!(s.count, 0);
    assert_eq!(c[1][0], st(COPPER, 256));
    assert_eq!(c[1][1], st(COPPER, 4), "holder's empty slot before other chests");
    let mut iron = ItemStack { item: IRON, count: 5 };
    ops::put(&mut c, &mut iron, 256, Reach::Existing);
    assert_eq!(iron.count, 5, "no chest holds iron yet");
    ops::put(&mut c, &mut iron, 256, Reach::Anywhere);
    assert_eq!(iron.count, 0);
    assert_eq!(c[0][1], st(IRON, 5), "nearest container wins");
}

#[test]
fn take_never_overdraws() {
    let mut c = vec![chest(&[(COPPER, 30)]), chest(&[(COPPER, 70)])];
    assert_eq!(ops::take(&mut c, COPPER, 100), 100);
    assert_eq!(ops::take(&mut c, COPPER, 1), 0);
    assert!(c.iter().flatten().all(Option::is_none));
}

const A: IVec3 = IVec3::new(4, 9, 4);
const B: IVec3 = IVec3::new(8, 9, 4);
const FAR: IVec3 = IVec3::new(30, 9, 30);

fn world() -> World {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 31);
    let mut blocks = Box::new([BlockId::Air; 32 * 32 * 32]);
    for y in 0..9 {
        for z in 0..32 {
            for x in 0..32 {
                blocks[ChunkData::local_index(x, y, z)] = BlockId::Stone;
            }
        }
    }
    w.chunks.insert(IVec3::ZERO, Arc::new(ChunkData {
        modified_bounds: None,
        structural_bounds: None,
        blocks,
        light: Box::new([0; 32 * 32 * 32]),
        torches: HashSet::new(),
    }));
    w.inventory.hotbar = [None; HOTBAR_SLOTS];
    w.inventory.pack = [None; PACK_SLOTS];
    for p in [A, B, FAR] {
        w.set(p, BlockId::Chest);
    }
    w.block_data.set_chest(A, chest(&[(COPPER, 34), (COBBLE, 100)]));
    w.block_data.set_chest(B, chest(&[(COPPER, 171), (COPPER, 900)]));
    w.block_data.set_chest(FAR, chest(&[(COPPER, 5)]));
    w
}

fn here() -> Vec3 {
    Vec3::new(4.5, 10.0, 4.5)
}

#[test]
fn discovers_nearby_chests_and_aggregates() {
    let w = world();
    let found = discover(&w, here(), RADIUS_M, None);
    assert_eq!(found.iter().map(|c| c.source).collect::<Vec<_>>(), vec![Source::Chest(A), Source::Chest(B)]);
    let idx = index(&w, &found);
    let copper = idx.stock.iter().find(|s| s.item == COPPER).expect("copper indexed");
    assert_eq!(copper.total, 1105);
    assert_eq!(copper.sources, vec![(0, 34), (1, 1071)]);
    assert_eq!(idx.free_slots, 2 * SLOTS - 4);
    assert_eq!(idx.search("ingot").count(), 1);
    assert_eq!(idx.search("BUILDING").count(), 1);
    // A far chest joins only as the scan's anchor.
    assert_eq!(discover(&w, here(), RADIUS_M, Some(FAR)).len(), 3);
}

#[test]
fn broken_chest_drops_out_of_moves() {
    let mut w = world();
    let found = discover(&w, here(), RADIUS_M, None);
    w.set(B, BlockId::Air);
    assert_eq!(withdraw(&mut w, &found, COPPER, 500), 34, "only chest A remains");
    assert_eq!(w.inventory.count_item(COPPER), 34);
    assert!(discover(&w, here(), RADIUS_M, None).iter().all(|c| c.source != Source::Chest(B)));
}

#[test]
fn withdraw_is_exact_and_capped_by_free_space() {
    let mut w = world();
    let found = discover(&w, here(), RADIUS_M, None);
    assert_eq!(withdraw(&mut w, &found, COPPER, 300), 300);
    assert_eq!(w.inventory.count_item(COPPER), 300);
    assert_eq!(index(&w, &found).stock.iter().find(|s| s.item == COPPER).map(|s| s.total), Some(805));
    // Fill every free slot with wands; nothing more fits beyond the partial stack.
    for (i, slot) in w.inventory.hotbar.iter_mut().chain(w.inventory.pack.iter_mut()).enumerate() {
        if slot.is_none() {
            *slot = st(Item::Wand { id: i as u16, look: 1 }, 1);
        }
    }
    let room = w.inventory.remaining_capacity_for(COPPER);
    assert_eq!(withdraw(&mut w, &found, COPPER, 10_000), room);
    assert_eq!(w.inventory.count_item(COPPER) as u64 + index(&w, &found).stock.iter().find(|s| s.item == COPPER).map_or(0, |s| s.total), 1105);
}

#[test]
fn deposit_keeps_hotbar_and_stateful_items() {
    let mut w = world();
    let found = discover(&w, here(), RADIUS_M, None);
    w.inventory.hotbar[0] = st(COPPER, 10);
    w.inventory.pack[0] = st(COPPER, 50);
    w.inventory.pack[1] = st(IRON, 7);
    w.inventory.pack[2] = st(Item::Wand { id: 4, look: 1 }, 1);
    assert_eq!(deposit(&mut w, &found, Reach::Existing), 50, "no chest holds iron");
    assert_eq!(w.inventory.hotbar[0], st(COPPER, 10));
    assert_eq!(deposit(&mut w, &found, Reach::Anywhere), 7);
    assert_eq!(w.inventory.pack[2], st(Item::Wand { id: 4, look: 1 }, 1));
    assert_eq!(w.block_data.chest(A)[2], st(IRON, 7), "nearest chest takes the fallback");
}

#[test]
fn restock_tops_up_hotbar() {
    let mut w = world();
    let found = discover(&w, here(), RADIUS_M, None);
    w.inventory.hotbar[0] = st(COBBLE, 200);
    assert_eq!(restock(&mut w, &found), 56);
    assert_eq!(w.inventory.hotbar[0], st(COBBLE, 256));
    assert_eq!(w.block_data.chest(A)[1], st(COBBLE, 44));
}

#[test]
fn consolidate_preview_matches_apply() {
    let mut w = world();
    let found = discover(&w, here(), RADIUS_M, None);
    let preview = consolidate_preview(&w, &found);
    assert_eq!(consolidate(&mut w, &found), preview);
    assert_eq!(index(&w, &found).stock.iter().find(|s| s.item == COPPER).map(|s| s.total), Some(1105));
}

#[test]
fn smart_deposit_obeys_assignment_priority_reserves_and_protection() {
    let mut w = world();
    let found = discover(&w, here(), RADIUS_M, None);
    policy::set(&mut w, B, policy::Policy { assigned: Some(COPPER), priority: 10, target: 1200, ..Default::default() });
    w.inventory.pack[0] = st(COPPER, 200);
    w.inventory.pack[1] = st(COPPER, 40);
    w.inventory.toggle_protected(crate::world::inventory::SlotAddr::Pack(1));
    w.inventory.minimums.push((COPPER, 60));
    assert_eq!(deposit(&mut w, &found, Reach::Smart), 180);
    assert_eq!(w.inventory.pack[1], st(COPPER, 40));
    assert_eq!(w.block_data.chest(B).iter().flatten().filter(|s| s.item==COPPER).map(|s| s.count).sum::<u32>(), 1200);
    assert_eq!(w.inventory.count_item(COPPER), 60);
    let mut p = policy::get(&w, A); p.reserve = 80; policy::set(&mut w, A, p);
    assert_eq!(withdraw(&mut w, &chest_container(A), COPPER, 1000), 5);
}

fn chest_container(p: IVec3) -> [Container; 1] { [Container { source: Source::Chest(p), pos: p, distance_m: 0.0 }] }

#[test]
fn stale_duplicate_sources_cannot_duplicate_items_and_unloads_remove_access() {
    let mut w = world();
    let c = chest_container(A)[0];
    assert_eq!(withdraw(&mut w, &[c,c], COPPER, 100), 34);
    w.chunks.remove(&IVec3::ZERO);
    assert_eq!(withdraw(&mut w, &[c], COBBLE, 100), 0);
    assert!(index(&w, &[c]).stock.is_empty());
}

#[test]
fn excluded_containers_and_reserved_stock_survive_consolidation() {
    let mut w = world();
    policy::set(&mut w, B, policy::Policy { enabled: false, ..Default::default() });
    let found = discover(&w, here(), RADIUS_M, None);
    assert_eq!(found.len(), 1);
    policy::set(&mut w, A, policy::Policy { reserve: 30, ..Default::default() });
    assert_eq!(withdraw(&mut w, &found, COPPER, u32::MAX), 4);
    consolidate(&mut w, &found);
    assert_eq!(w.block_data.chest(A)[0], st(COPPER, 30));
    assert_eq!(w.block_data.chest(B)[0], st(COPPER, 171));
}

#[test]
fn index_refreshes_after_mutations_but_not_unchanged_frames() {
    let mut w = world();
    let mut view = view::View::default();
    view.refresh(&w, A, 0.0);
    for frame in 1..600 { view.refresh(&w, A, f64::from(frame)/60.0); }
    assert_eq!(view.rebuilds, 1);
    w.block_data.set_chest(B, chest(&[(COPPER, 70)]));
    view.refresh(&w, A, 10.0);
    assert_eq!(view.rebuilds, 2);
    assert_eq!(view.index.stock.iter().find(|s|s.item==COPPER).unwrap().total, 104);
    w.set(B, BlockId::Air);
    view.refresh(&w, A, 10.01);
    assert_eq!(view.index.containers.len(), 1);
}

#[test]
fn sequential_authoritative_withdrawals_share_the_last_hundred() {
    let mut w = world();
    w.block_data.set_chest(A, chest(&[(COPPER,100)]));
    w.block_data.set_chest(B, [None; SLOTS]);
    let first = commands::apply(&mut w,A,here(),commands::Action::Withdraw(COPPER,100,None)).unwrap();
    w.inventory = crate::world::inventory::SurvivalInventory::empty();
    let second = commands::apply(&mut w,A,here(),commands::Action::Withdraw(COPPER,100,None)).unwrap();
    assert_eq!(first + second, 100);
    assert!(commands::apply(&mut w,A,here()+Vec3::splat(100.0),commands::Action::Restock).is_err());
}

#[test]
fn large_stacks_and_policies_survive_sidecar_roundtrip() {
    let mut w = world();
    w.block_data.set_chest(A, chest(&[(COBBLE, 16384)]));
    policy::set(&mut w, A, policy::Policy { assigned: Some(COBBLE), priority: 17, ..Default::default() });
    let decoded = block_data::sidecar::decode(&block_data::sidecar::encode(&w.block_data)).unwrap();
    assert_eq!(decoded.chest(A)[0], st(COBBLE,16384));
    assert_eq!(decoded.storage_policies, w.block_data.storage_policies);
}

#[test]
fn destruction_recovers_contents_when_loose_drop_budget_is_full() {
    let mut w=world();
    w.inventory.dropped=vec![crate::world::inventory::DroppedStack{pos:Vec3::ZERO,stack:ItemStack{item:IRON,count:1}};1024];
    w.set(A,BlockId::Air);
    assert_eq!(w.inventory.overflow.iter().filter(|s|s.item==COPPER).map(|s|s.count).sum::<u32>(),34);
    assert_eq!(w.inventory.overflow.iter().filter(|s|s.item==COBBLE).map(|s|s.count).sum::<u32>(),100);
}

#[test]
fn benchmark_thousand_containers_cached_index() {
    let mut w=world();
    for x in 0..16 { for z in 0..16 { for y in 9..14 {
        let p=IVec3::new(x,y,z);
        w.set(p,BlockId::Chest);
        w.block_data.set_chest(p, [st(COPPER,128);SLOTS]);
    }}}
    let started=std::time::Instant::now();
    let mut view=view::View::default();
    for frame in 0..600 { view.refresh(&w,A,f64::from(frame)/60.0); }
    assert!(view.index.containers.len() >= 1000);
    assert_eq!(view.rebuilds,1);
    eprintln!("[scanner-benchmark] containers={} frames=600 rebuilds=1 elapsed_ms={:.2}",view.index.containers.len(),started.elapsed().as_secs_f64()*1000.0);
}

#[cfg(feature = "multiplayer")]
#[test]
fn retried_authoritative_requests_transfer_only_once() {
    let mut w = world();
    w.block_data.set_chest(A, chest(&[(COPPER, 100)]));
    w.block_data.set_chest(B, [None; SLOTS]);
    let mut last = 0;
    let mut total = 0;
    for sequence in [1, 1, 2, 1, 2] {
        if crate::multiplayer::admit_sequence(last, sequence).unwrap() {
            total += commands::apply(&mut w, A, here(), commands::Action::Withdraw(COPPER, 60, None)).unwrap();
            last = sequence;
        }
    }
    assert_eq!(total, 100);
    assert_eq!(w.inventory.count_item(COPPER), 100);
    assert!(crate::multiplayer::admit_sequence(last, 4).is_err());
}

#[test]
fn smart_partial_fallback_keeps_category_priority() {
    let mut slots = vec![chest(&[(COPPER, 250)]), [None; SLOTS]];
    let policies = vec![policy::Policy { category: Some(stacks::class(COPPER)), ..Default::default() },
        policy::Policy { priority: 100, ..Default::default() }];
    let mut incoming = ItemStack { item: COPPER, count: 30 };
    policy::deposit(&mut slots, &policies, &mut incoming, 256, Reach::Smart);
    assert_eq!(incoming.count, 0);
    assert_eq!(slots[0][0], st(COPPER, 256));
    assert_eq!(slots[0][1], st(COPPER, 24));
    assert!(slots[1].iter().all(Option::is_none));
}

fn brick_recipe() -> usize {
    crate::world::inventory::all_recipes().iter().position(|r| r.name == "Seared Brick").unwrap()
}

#[test]
fn batch_crafting_preview_and_commit_respect_reserves_locks_and_minimums() {
    let mut w = world();
    let clay = Item::Block(BlockId::Clay);
    let gravel = Item::Block(BlockId::Gravel);
    w.block_data.set_chest(A, chest(&[(clay, 10), (gravel, 10)]));
    w.block_data.set_chest(B, [None; SLOTS]);
    policy::set(&mut w, A, policy::Policy { reserve: 4, ..Default::default() });
    w.inventory.pack[0] = st(clay, 8);
    w.inventory.pack[1] = st(gravel, 8);
    w.inventory.toggle_protected(crate::world::inventory::SlotAddr::Pack(1));
    w.inventory.minimums.push((clay, 6));
    let found = discover(&w, here(), RADIUS_M, None);
    let index = index(&w, &found);
    assert_eq!(index.stock.iter().find(|s| s.item == clay).unwrap().available, 6);
    let supplies = crafting::Supplies::new(&w.inventory, &index);
    assert_eq!(supplies.count(clay), 8);
    assert_eq!(supplies.count(gravel), 6);
    assert_eq!(supplies.batches(&crate::world::inventory::all_recipes()[brick_recipe()], false), 3);
    let out = commands::apply(&mut w, A, here(), commands::Action::CraftBatch(brick_recipe(), 3)).unwrap();
    assert_eq!(out, 12);
    assert_eq!(w.inventory.count_item(Item::Block(BlockId::SearedBrick)), 12);
    assert_eq!(w.inventory.pack[1], st(gravel, 8));
    assert_eq!(w.inventory.count_item(clay), 6);
    assert_eq!(w.block_data.chest(A)[0], st(clay, 6));
    assert_eq!(w.block_data.chest(A)[1], st(gravel, 4));
}

#[test]
fn batch_failure_never_publishes_partial_ingredient_debits() {
    let mut w = world();
    let clay = Item::Block(BlockId::Clay);
    let gravel = Item::Block(BlockId::Gravel);
    w.block_data.set_chest(A, chest(&[(clay, 20), (gravel, 4)]));
    w.block_data.set_chest(B, [None; SLOTS]);
    let before = w.block_data.chest(A);
    for batches in [0, 3, 65, u32::MAX] {
        assert!(commands::apply(&mut w, A, here(), commands::Action::CraftBatch(brick_recipe(), batches)).is_err());
        assert_eq!(w.block_data.chest(A), before);
        assert_eq!(w.inventory.count_item(Item::Block(BlockId::SearedBrick)), 0);
    }
    w.inventory.hotbar.fill(st(IRON, 256));
    w.inventory.pack.fill(st(IRON, 256));
    assert!(commands::apply(&mut w, A, here(), commands::Action::CraftBatch(brick_recipe(), 1)).unwrap_err().to_string().contains("Inventory full"));
    assert_eq!(w.block_data.chest(A), before);
    assert!(w.inventory.pack.iter().all(|s| *s == st(IRON,256)));
}

#[test]
fn cached_availability_changes_when_reserves_change() {
    let mut w = world();
    let mut cached = view::View::default();
    cached.refresh(&w, A, 0.0);
    assert_eq!(cached.index.stock.iter().find(|s| s.item == COPPER).unwrap().available, 1105);
    policy::set(&mut w, B, policy::Policy { reserve: 1000, ..Default::default() });
    cached.refresh(&w, A, 0.01);
    let copper = cached.index.stock.iter().find(|s| s.item == COPPER).unwrap();
    assert_eq!((copper.total, copper.available), (1105,105));
    assert_eq!(cached.rebuilds, 2);
}
