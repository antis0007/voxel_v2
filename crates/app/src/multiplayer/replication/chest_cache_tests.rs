use super::*;
use crate::world::{factory::{self, Kind, sides}, inventory::{Item, ItemStack}};
use crate::simulation::{Budget, residency::Residency};

const POS: IVec3 = IVec3::new(-16, 128, -16);
fn world() -> World { World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7) }
fn chest(w: &mut World, at: IVec3, item: Option<ItemStack>) {
    w.set(at, BlockId::Chest); let mut slots = vec![None; BlockId::Chest.chest_slots()]; slots[0] = item;
    w.block_data.set_chest(at, slots);
}
fn count(w: &World, at: IVec3, item: Item) -> u32 {
    w.block_data.chest(at).iter().flatten().filter(|s| s.item == item).map(|s| s.count).sum()
}
fn total(w: &World) -> u32 {
    let n = &w.factory.nodes[&POS];
    2 * (count(w, POS-IVec3::X, Item::Block(BlockId::CopperOre))*64 + n.input[0])
        + n.output[9] + count(w, POS+IVec3::X, Item::Resource(9))*64
}
#[test]
fn factory_chest_cache_keeps_both_sides_of_transfers_in_one_image() {
    let mut host = world();
    chest(&mut host, POS-IVec3::X, Some(ItemStack::new(Item::Block(BlockId::CopperOre), 4)));
    chest(&mut host, POS+IVec3::X, None);
    let n = factory::fixture_place(&mut host, POS, Kind::Crusher).unwrap(); n.enabled = false; n.output[9] = 128;
    for face in 0..6 { sides::apply_role(n, face, sides::Role::Off as u8); }
    sides::apply_role(n, 0, sides::Role::Out as u8); sides::apply_role(n, 1, sides::Role::In as u8);
    let now = std::time::Instant::now(); let mut image = FactoryImage::default();
    let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
    let mut peers = [(world(), Mirror::default()), (world(), Mirror::default())];
    for stage in 0..3 {
        if stage > 0 {
            host.factory.nodes.get_mut(&POS).unwrap().enabled = stage == 2;
            factory::tick_world(&mut host, &Player::new(), 0.25, &mut sources, &mut cursor, Budget::UNBOUNDED, &mut blocks, &mut chunks);
        }
        assert_eq!(total(&host), 640);
        image.refresh(&mut host, now + std::time::Duration::from_millis(stage*50)).unwrap();
        if stage > 0 { peers[1] = (world(), Mirror::default()); }
        for (peer, mirror) in &mut peers {
            let bytes = capture_shared(&host, &Player::new(), &DayNightCycle::default(), stage, Vec::new(), mirror, &image).unwrap();
            for _ in 0..2 {
                apply(&bytes, peer, &mut Player::new(), &mut DayNightCycle::default(), &mut blocks, &mut chunks).unwrap();
                assert_eq!(total(peer), 640, "stage {stage}");
                assert_eq!(peer.factory.nodes[&POS].input, host.factory.nodes[&POS].input);
                assert_eq!(peer.factory.nodes[&POS].output, host.factory.nodes[&POS].output);
                for at in [POS-IVec3::X, POS+IVec3::X] { assert_eq!(peer.block_data.chest(at), host.block_data.chest(at)); }
            }
        }
    }
    assert_eq!(host.factory.nodes[&POS].input[0], 64);
    assert_eq!(count(&host, POS-IVec3::X, Item::Block(BlockId::CopperOre)), 3);
    assert_eq!(count(&host, POS+IVec3::X, Item::Resource(9)), 2);
}
#[test]
fn factory_chest_cache_reuses_unchanged_storage_and_retains_periodic_refresh() {
    let mut w = world(); factory::fixture_place(&mut w, POS, Kind::Crusher).unwrap();
    let now = std::time::Instant::now(); let mut image = FactoryImage::default(); image.refresh(&mut w, now).unwrap();
    let (before, due) = (image.text.clone(), image.due); w.factory.nodes.get_mut(&POS).unwrap().energy = 17;
    image.refresh(&mut w, now + std::time::Duration::from_millis(100)).unwrap();
    assert_eq!(image.text, before); assert_eq!(image.due, due);
    image.refresh(&mut w, now + std::time::Duration::from_millis(250)).unwrap(); assert_ne!(image.text, before);
    let before = image.text.clone(); w.factory.nodes.get_mut(&POS).unwrap().energy = 18;
    chest(&mut w, POS+IVec3::X, None);
    image.refresh(&mut w, now + std::time::Duration::from_millis(251)).unwrap(); assert_ne!(image.text, before);
    assert_eq!(image.storage_revision, Some(w.block_data.storage_revision));
    let due = image.due; image.refresh(&mut w, now + std::time::Duration::from_millis(252)).unwrap(); assert_eq!(image.due, due);
}
#[test]
#[ignore = "wall-clock factory serialization; run without concurrent builds"]
fn factory_chest_cache_loaded_refresh_cost() {
    use std::time::{Duration, Instant};
    for machines in [32, 128, 512] {
        let mut w = world();
        for i in 0..machines { factory::fixture_place(&mut w, POS+IVec3::X*i, Kind::Crusher).unwrap(); }
        let now = Instant::now(); let mut image = FactoryImage::default(); image.refresh(&mut w, now).unwrap();
        let due = image.due; let start = Instant::now();
        for _ in 0..512 { image.refresh(&mut w, now+Duration::from_millis(1)).unwrap(); }
        let hit_us = start.elapsed().as_secs_f64()*1e6/512.0; assert_eq!(image.due, due);
        let mut times = Vec::new();
        for i in 0..128 {
            w.factory.nodes.get_mut(&POS).unwrap().output[9] = (i%2)*64;
            w.block_data.set_chest(POS-IVec3::X, vec![None; BlockId::Chest.chest_slots()]);
            let start = Instant::now(); image.refresh(&mut w, now+Duration::from_millis(2)).unwrap(); times.push(start.elapsed().as_nanos());
            assert_eq!(image.storage_revision, Some(w.block_data.storage_revision));
        }
        times.sort_unstable();
        eprintln!("[factory-cache-load] machines={machines} hit_mean_us={hit_us:.3} forced_p50_us={:.1} forced_p95_us={:.1} forced_max_us={:.1} bytes={}",
            times[64] as f64/1000.0, times[121] as f64/1000.0, times[127] as f64/1000.0, image.text.len());
    }
}
