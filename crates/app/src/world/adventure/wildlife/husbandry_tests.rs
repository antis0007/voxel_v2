use super::husbandry::{AREA_CAP, COOLDOWN_TICKS, FLEECE_TICKS, GROW_TICKS, Member, SHEARS, service};
use super::*;
use crate::world::WorldType;
use crate::world::inventory::{Item, ItemStack};
use engine_core::VoxelScalePreset;

const FOOD: Item = Item::Block(BlockId::Bush);

fn pasture() -> (World, f32) {
    let w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 2738958700);
    let floor = (w.terrain_height_at(0, 0) + 1) as f32 + 0.02;
    (w, floor)
}

fn add(w: &mut World, kind: u32, pos: Vec3, slot: u32) -> usize {
    w.adventure.enemies.push(Enemy::new(IVec3::new(slot as i32, 40, 6), slot % 2, kind, pos, 0));
    w.adventure.enemies.len() - 1
}

fn give(w: &mut World, item: Item, count: u32) {
    w.inventory.hotbar[0] = Some(ItemStack::new(item, count));
}

fn feed_at(w: &mut World, i: usize) -> bool {
    let c = w.adventure.enemies[i].center();
    feed(w, c + Vec3::Z * 2.0, Vec3::NEG_Z, 0)
}

fn at_second(w: &mut World, ticks: u64) {
    w.farming.clock = (w.farming.clock + ticks).next_multiple_of(120);
}

#[test]
fn husbandry_breeding_needs_food_and_respects_cooldown() {
    let (mut w, floor) = pasture();
    let a = add(&mut w, SHEEP, Vec3::new(0.0, floor, 0.0), 0);
    let b = add(&mut w, SHEEP, Vec3::new(2.8, floor, 0.0), 1);
    give(&mut w, Item::Block(BlockId::Stone), 8);
    assert!(!feed_at(&mut w, a), "only the RON food is accepted");
    give(&mut w, FOOD, 8);
    assert!(feed_at(&mut w, a));
    assert_eq!(w.inventory.count_item(FOOD), 7);
    assert!(feed_at(&mut w, a), "a fed animal takes the use");
    assert_eq!(w.inventory.count_item(FOOD), 7, "and eats nothing more");
    at_second(&mut w, 0);
    service(&mut w, Vec3::ZERO);
    assert_eq!(w.adventure.wildlife.herd.members.len(), 1, "one fed sheep does not breed");
    assert!(feed_at(&mut w, b));
    at_second(&mut w, 1);
    service(&mut w, Vec3::ZERO);
    let herd = &w.adventure.wildlife.herd.members;
    assert_eq!(herd.len(), 3);
    let young: Vec<_> = herd.values().filter(|m| m.born.is_some()).collect();
    assert_eq!(young.len(), 1);
    assert_eq!(young[0].kind, SHEEP);
    assert!(herd.values().filter(|m| m.born.is_none()).all(|m| m.cooldown_until == w.farming.clock + COOLDOWN_TICKS));
    let calf = w.adventure.enemies.iter().position(|e| e.youth > 0.99).expect("young body spawned");
    assert!(feed_at(&mut w, b));
    assert!(w.expedition.message.contains("Cooldown"), "{}", w.expedition.message);
    assert!(feed_at(&mut w, calf));
    assert!(w.expedition.message.contains("Young"), "{}", w.expedition.message);
    assert_eq!(w.inventory.count_item(FOOD), 6, "cooldown and youth eat nothing");
    at_second(&mut w, COOLDOWN_TICKS);
    for i in [a, b] {
        assert!(feed_at(&mut w, i));
    }
    service(&mut w, Vec3::ZERO);
    assert_eq!(w.adventure.wildlife.herd.members.len(), 4, "a rested pair breeds again");
}

#[test]
fn husbandry_area_cap_stops_breeding() {
    let (mut w, floor) = pasture();
    let a = add(&mut w, GOAT, Vec3::new(0.0, floor, 0.0), 0);
    let b = add(&mut w, GOAT, Vec3::new(2.8, floor, 0.0), 1);
    for i in 0..AREA_CAP - 2 {
        let m = Member { kind: GOAT, home: Vec3::X * i as f32, pos: Vec3::new(i as f32, floor, 9.0), health: 30.0,
            yaw: 0.0, born: None, cooldown_until: 0, ready_until: 0, fleece_at: 0 };
        w.adventure.wildlife.herd.members.insert(1000 + i as u32, m);
    }
    give(&mut w, FOOD, 8);
    assert!(feed_at(&mut w, a) && feed_at(&mut w, b));
    let before = w.adventure.wildlife.herd.members.len();
    assert_eq!(before, AREA_CAP);
    at_second(&mut w, 0);
    service(&mut w, Vec3::new(500.0, 0.0, 0.0));
    assert_eq!(w.adventure.wildlife.herd.members.len(), before, "the area cap holds");
    assert!(w.expedition.message.contains("Herd Full"), "{}", w.expedition.message);
    assert!(w.adventure.wildlife.herd.members.values().all(|m| m.cooldown_until == 0 && m.ready_until == 0));
}

#[test]
fn husbandry_young_grow_up_and_scale() {
    let (mut w, floor) = pasture();
    let m = Member { kind: SHEEP, home: Vec3::new(0.0, floor, 0.0), pos: Vec3::new(0.0, floor, 0.0), health: 24.0,
        yaw: 0.0, born: Some(0), cooldown_until: 0, ready_until: 0, fleece_at: 0 };
    w.adventure.wildlife.herd.members.insert(5, m);
    at_second(&mut w, 0);
    service(&mut w, Vec3::ZERO);
    assert_eq!(w.adventure.enemies.len(), 1, "a nearby member is respawned");
    at_second(&mut w, GROW_TICKS / 2);
    service(&mut w, Vec3::ZERO);
    let half = w.adventure.enemies[0].youth;
    assert!((0.4..0.6).contains(&half), "{half}");
    at_second(&mut w, GROW_TICKS);
    service(&mut w, Vec3::ZERO);
    assert_eq!(w.adventure.wildlife.herd.members[&5].born, None);
    service(&mut w, Vec3::ZERO);
    assert_eq!(w.adventure.enemies[0].youth, 0.0);
}

#[test]
fn husbandry_drops_come_from_the_bestiary_table() {
    let (mut w, floor) = pasture();
    for kind in [SHEEP, GOAT] {
        let rows = bestiary::species(kind).expect("grazer row").drops;
        assert!(!rows.is_empty(), "{kind} has a RON drop table");
        let meat = Item::Food(crate::world::inventory::FoodKind::RawMutton);
        assert!(rows.iter().any(|r| r.item == meat && r.min >= 1 && r.max == 2), "{kind} drops 1-2 raw mutton");
        let i = add(&mut w, kind, Vec3::new(0.0, floor, 0.0), 0);
        let e = w.adventure.enemies.remove(i);
        let mut state = std::mem::take(&mut w.adventure.wildlife);
        w.inventory.dropped.clear();
        fall(&mut state, &mut w, &e);
        let first: Vec<_> = w.inventory.dropped.iter().map(|d| (d.stack.item, d.stack.count)).collect();
        assert_eq!(first.len(), rows.len());
        for ((item, count), row) in first.iter().zip(rows) {
            assert_eq!(*item, row.item);
            assert!((row.min..=row.max).contains(count));
        }
        w.inventory.dropped.clear();
        fall(&mut state, &mut w, &e);
        let again: Vec<_> = w.inventory.dropped.iter().map(|d| (d.stack.item, d.stack.count)).collect();
        assert_eq!(first, again, "drop counts roll per identity");
        let mut young = e;
        young.youth = 0.5;
        w.inventory.dropped.clear();
        fall(&mut state, &mut w, &young);
        assert!(w.inventory.dropped.is_empty(), "young drop nothing");
    }
}

#[test]
fn husbandry_fed_wild_animal_leaves_the_wild_records() {
    let (mut w, floor) = pasture();
    let feet = Vec3::new(0.0, floor, 0.0);
    super::tests::load_spawn_neighborhood(&mut w, feet);
    let mut adventure = std::mem::take(&mut w.adventure);
    spawn(&mut adventure, &w, feet);
    w.adventure = adventure;
    let i = w.adventure.enemies.iter().position(|e| passive(e.kind)).expect("wild grazer");
    let key = id(&w.adventure.enemies[i]);
    give(&mut w, FOOD, 2);
    assert!(feed_at(&mut w, i));
    assert_eq!(w.adventure.wildlife.residents[&key].health, 0.0, "the wild record never respawns");
    assert_eq!(w.adventure.wildlife.herd.members.len(), 1);
    let mut bytes = Vec::new();
    w.adventure.wildlife.encode(&w.adventure.enemies, &mut bytes);
    let mut words = bytes.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap()));
    let restored = State::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("truncated"))).unwrap();
    assert_eq!(restored.residents[&key].health, 0.0);
}

#[test]
fn husbandry_herd_survives_the_farming_sidecar() {
    use crate::world::farming::sidecar;
    let (mut w, floor) = pasture();
    let a = add(&mut w, GOAT, Vec3::new(0.0, floor, 0.0), 0);
    give(&mut w, FOOD, 2);
    assert!(feed_at(&mut w, a));
    let calf = Member { kind: SHEEP, home: Vec3::new(4.0, floor, 4.0), pos: Vec3::new(5.0, floor, 4.0), health: 20.0,
        yaw: 1.0, born: Some(0), cooldown_until: 7, ready_until: 0, fleece_at: 0 };
    w.adventure.wildlife.herd.members.insert(9, calf);
    w.farming.clock = 600;
    let bytes = sidecar::encode(&w);
    let (state, herd) = sidecar::decode(&bytes).expect("decodes");
    let mut fresh = pasture().0;
    sidecar::apply_herd(&mut fresh, herd, state.clock).expect("herd decodes");
    assert_eq!(fresh.adventure.wildlife.herd, w.adventure.wildlife.herd);
    assert!(sidecar::apply_herd(&mut fresh, &herd[..herd.len() - 1], state.clock).is_err());
}

const FIBRE: Item = Item::Resource(4);

fn shear_at(w: &mut World, i: usize) -> bool {
    let c = w.adventure.enemies[i].center();
    shear(w, c + Vec3::Z * 2.0, Vec3::NEG_Z, 0)
}

#[test]
fn husbandry_shearing_yields_fibre_once_per_regrowth() {
    let (mut w, floor) = pasture();
    let sheep = add(&mut w, SHEEP, Vec3::new(0.0, floor, 0.0), 0);
    let goat = add(&mut w, GOAT, Vec3::new(0.0, floor, 6.0), 1);
    give(&mut w, FOOD, 1);
    assert!(!shear_at(&mut w, sheep), "only held shears cut");
    give(&mut w, SHEARS, 1);
    assert!(!shear_at(&mut w, goat), "goats carry no fleece row");
    assert!(shear_at(&mut w, sheep));
    let first = w.inventory.count_item(FIBRE);
    let row = bestiary::species(SHEEP).and_then(|s| s.shear).expect("sheep fleece row");
    assert!((row.min..=row.max).contains(&first), "{first}");
    assert_eq!(w.adventure.wildlife.herd.members.len(), 1, "a sheared wild sheep joins the herd");
    assert!(shear_at(&mut w, sheep));
    assert_eq!(w.inventory.count_item(FIBRE), first, "a shorn sheep yields nothing");
    assert!(w.expedition.message.contains("Fleece"), "{}", w.expedition.message);
    at_second(&mut w, FLEECE_TICKS - 120);
    assert!(shear_at(&mut w, sheep));
    assert_eq!(w.inventory.count_item(FIBRE), first, "still regrowing one second early");
    at_second(&mut w, 120);
    assert!(shear_at(&mut w, sheep));
    assert!(w.inventory.count_item(FIBRE) > first, "the regrown fleece cuts again");
    assert_eq!(w.inventory.count_item(SHEARS), 1, "shears are kept");
}

#[test]
fn husbandry_young_sheep_cannot_be_sheared() {
    let (mut w, floor) = pasture();
    let lamb = add(&mut w, SHEEP, Vec3::new(0.0, floor, 0.0), 0);
    w.adventure.enemies[lamb].youth = 0.5;
    give(&mut w, SHEARS, 1);
    assert!(shear_at(&mut w, lamb));
    assert_eq!(w.inventory.count_item(FIBRE), 0);
    assert!(w.expedition.message.contains("Young"), "{}", w.expedition.message);
}

#[test]
fn husbandry_fleece_survives_the_sidecar_and_old_files_load() {
    use crate::world::farming::sidecar;
    let (mut w, floor) = pasture();
    let sheep = add(&mut w, SHEEP, Vec3::new(0.0, floor, 0.0), 0);
    give(&mut w, SHEARS, 1);
    w.farming.clock = 600;
    assert!(shear_at(&mut w, sheep));
    let bytes = sidecar::encode(&w);
    let (state, herd) = sidecar::decode(&bytes).expect("decodes");
    let mut fresh = pasture().0;
    sidecar::apply_herd(&mut fresh, herd, state.clock).expect("herd decodes");
    assert_eq!(fresh.adventure.wildlife.herd, w.adventure.wildlife.herd);
    assert_eq!(fresh.adventure.wildlife.herd.members.values().next().map(|m| m.fleece_at), Some(600 + FLEECE_TICKS));
    // A herd written before shearing ends after the members: drop the count and one timer.
    let old = &herd[..herd.len() - 4 - 8];
    let mut fresh = pasture().0;
    sidecar::apply_herd(&mut fresh, old, state.clock).expect("pre-shearing herd decodes");
    assert!(fresh.adventure.wildlife.herd.members.values().all(|m| m.fleece_at == 0), "old herds start with full fleece");
}

#[test]
fn husbandry_fleece_feeds_real_recipes() {
    let recipes = crate::world::inventory::all_recipes();
    assert!(recipes.iter().any(|r| r.output.item == SHEARS), "shears are craftable");
    assert!(recipes.iter().any(|r| r.output.item == Item::Block(BlockId::Bed) && r.ingredients.iter().any(|&(i, _)| i == FIBRE)),
        "the bed takes the fibre");
}

#[test]
fn husbandry_tended_fowl_lay_once_per_regrowth() {
    use crate::world::inventory::FoodKind;
    let fowl = super::super::fauna::FOWL;
    let (mut w, floor) = pasture();
    let hen = add(&mut w, fowl, Vec3::new(0.0, floor, 0.0), 0);
    add(&mut w, fowl, Vec3::new(6.0, floor, 0.0), 1);
    let eggs = |w: &World| w.inventory.dropped.iter()
        .filter(|d| d.stack.item == Item::Food(FoodKind::Egg)).map(|d| d.stack.count).sum::<u32>();
    at_second(&mut w, 0);
    service(&mut w, Vec3::ZERO);
    assert_eq!(eggs(&w), 0, "wild fowl lay nothing");
    give(&mut w, FOOD, 4);
    assert!(feed_at(&mut w, hen));
    at_second(&mut w, 1);
    service(&mut w, Vec3::ZERO);
    assert_eq!(eggs(&w), 1, "a tended hen lays");
    at_second(&mut w, 1);
    service(&mut w, Vec3::ZERO);
    assert_eq!(eggs(&w), 1, "once per regrowth");
    at_second(&mut w, FLEECE_TICKS);
    service(&mut w, Vec3::ZERO);
    assert_eq!(eggs(&w), 2, "and again after it");
}

#[test]
fn husbandry_herd_of_a_removed_generated_species_still_loads() {
    use crate::world::farming::sidecar;
    let (mut w, floor) = pasture();
    let p = Vec3::new(0.0, floor, 0.0);
    let member = |kind, fleece_at| Member { kind, home: p, pos: p, health: 1.0, yaw: 0.0,
        born: None, cooldown_until: 0, ready_until: 0, fleece_at };
    let members = &mut w.adventure.wildlife.herd.members;
    members.insert(0, member(bestiary::FIRST_GENERATED + 999, 5));
    members.insert(1, member(SHEEP, 7));
    let bytes = sidecar::encode(&w);
    let (state, herd) = sidecar::decode(&bytes).expect("decodes");
    let mut fresh = pasture().0;
    sidecar::apply_herd(&mut fresh, herd, state.clock).expect("the herd loads without the removed species");
    let kept = &fresh.adventure.wildlife.herd.members;
    assert_eq!(kept.len(), 1);
    assert_eq!(kept.get(&1).map(|m| (m.kind, m.fleece_at)), Some((SHEEP, 7)), "fleece stays with its member");
}
