//! Unrun supplement: production service boundaries, not a second migration implementation.
use super::*;
use crate::world::{adventure::Enemy, block_data, inventory::{FoodKind, all_recipes}};
use std::sync::atomic::{AtomicU64, Ordering};

const STOCK: IVec3 = IVec3::new(8, 8, 10);
const BENCH: IVec3 = IVec3::new(8, 8, 9);
const ORE: IVec3 = IVec3::new(9, 8, 8);
const OTHER_ORE: IVec3 = IVec3::new(9, 8, 13);
const DELAYED: u64 = 9600;
const COUNCIL_LATER: u64 = 1_000_000;
static CASE: AtomicU64 = AtomicU64::new(0);

fn isolated(clock: u64) -> World {
    let root = std::env::var_os("AXIOM_LEGACY_SERVICE_ARTIFACT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    assert!(root.is_absolute(), "fixture artifact root must be absolute");
    std::fs::create_dir_all(&root).unwrap();
    // Wall-clock time is only a directory-name nonce. The world seed, saved
    // clock, simulation steps and assertions remain deterministic and unchanged.
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_nanos();
    let dir = (0..128).find_map(|_| {
        let case = CASE.fetch_add(1, Ordering::Relaxed);
        let dir = root.join(format!("axiom-legacy-service-{}-{nonce}-{case}", std::process::id()));
        match std::fs::create_dir(&dir) {
            Ok(()) => Some(dir),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
            Err(error) => panic!("cannot create fixture directory {}: {error}", dir.display()),
        }
    }).expect("could not allocate a fresh fixture directory after 128 attempts");
    let mut w = World::with_seed_in(WorldType::Superflat, VoxelScalePreset::OneMeter, 17, &dir);
    w.adventure.enemies.clear();
    w.farming.clock = clock;
    w
}

fn load_service(bytes: &[u8]) -> World {
    let (state, herd) = farming::sidecar::decode(bytes).unwrap();
    let mut w = isolated(state.clock);
    let clock = state.clock;
    w.farming = state;
    farming::sidecar::apply_herd(&mut w, herd, clock).unwrap();
    w
}

fn terrain(w: &mut World) {
    let mut blocks = Box::new([B::Air; 32 * 32 * 32]);
    for x in 0..32 { for z in 0..32 {
        blocks[ChunkData::local_index(x, 7, z)] = B::Stone;
    }}
    for (at, block) in [(STOCK, B::Chest), (BENCH, B::Workbench), (ORE, B::Stone), (OTHER_ORE, B::Stone)] {
        blocks[ChunkData::local_index(at.x, at.y, at.z)] = block;
    }
    w.chunks.insert(IVec3::ZERO, Arc::new(ChunkData { modified_bounds: None,
        structural_bounds: None, blocks, light: Box::new([0; 32 * 32 * 32]),
        torches: Default::default() }));
}

fn quiet_body(id: u32, pos: Vec3) -> vehicle::ResidentBody {
    vehicle::ResidentBody::capture(&Enemy::new(IVec3::new(id as i32, 0, 8),
        0, vehicle::VILLAGER, pos, 0)).parked()
}

fn services(w: &mut World, ticks: u64) {
    for _ in 0..ticks {
        w.farming.clock += 1;
        jobs::service(w, Vec3::new(24.5, 8.02, 24.5));
    }
}

fn reload_in_place(w: &mut World) {
    let bytes = farming::sidecar::encode(w);
    let (farming, herd) = farming::sidecar::decode(&bytes).unwrap();
    let clock = farming.clock;
    w.farming = farming;
    farming::sidecar::apply_herd(w, herd, clock).unwrap();
}

fn stock(w: &World, item: Item) -> u32 {
    production::count(&w.block_data.chest(STOCK), item)
}

#[derive(Serialize)]
struct ProductionFixture {
    workers: Vec<u32>, beds: Vec<production::Bed>, bills: Vec<production::Bill>,
    meal: Item, meal_ticks: u64,
}

/// Obtain the paid receipt and Work commitment from the existing executor, then
/// encode the V20 field schema. These are not genuine V20 executable bytes.
fn paid_fixture() -> World {
    let mut source = isolated(120);
    terrain(&mut source);
    let recipe = all_recipes().iter().find(|r| r.name == "Oak planks").unwrap();
    assert_eq!(recipe.ingredients, &[(Item::Block(B::Log), 1)]);
    assert_eq!(recipe.output, ItemStack::new(Item::Block(B::Planks), 4));
    let mut slots = vec![None; block_data::CHEST_SLOTS];
    slots[0] = Some(ItemStack::new(Item::Block(B::Log), 2));
    source.block_data.set_chest(STOCK, slots);
    let pos = Vec3::new(8.5, 8.02, 8.5);
    let task = production::Task::Craft(BENCH, 0);
    let policy = ProductionFixture { workers: vec![31], beds: Vec::new(),
        bills: vec![production::Bill { recipe: recipe.name.into(), target: 4,
            station: BENCH.to_array(), worker: 31, once: true, per_resident: 0 }],
        meal: Item::Food(FoodKind::Bread), meal_ticks: 120000 };
    let mut j = State { worker: Some(31), worker_pos: Some(pos.to_array()),
        body: Some(quiet_body(31, pos)), motion_at: 120, stockpile: Some(STOCK.to_array()),
        task: Some(Task::Produce(task)),
        production: Some(ron::from_str(&ron::to_string(&policy).unwrap()).unwrap()),
        ..State::default() };
    rebuild(&mut source);
    assert!(!production::execute(&mut source, &mut j, task));
    assert_eq!(stock(&source, Item::Block(B::Log)), 1);
    assert_eq!(stock(&source, Item::Block(B::Planks)), 0);
    assert!(j.batch.is_some() && j.activity.is_some() && j.work == 0);
    j.activity.as_mut().unwrap().configuration = 0;
    let saved_clock = 120 + 5 * PULSE;
    let mut w = load_service(&fixture(saved_clock, Some(&j), None));
    terrain(&mut w);
    w.block_data = source.block_data.clone(); // Chest persistence is a separate sidecar.
    assert_eq!(w.farming.jobs.work, 5);
    assert!(w.farming.jobs.activity.is_none());
    w
}

fn assert_one_batch(w: &World) {
    assert_eq!(stock(w, Item::Block(B::Log)), 1, "paid input must not be debited twice");
    assert_eq!(stock(w, Item::Block(B::Planks)), 4, "one accepted batch credits one output");
    let production = w.farming.jobs.production.as_ref().unwrap();
    assert_eq!(production.counts.batches, 1);
    assert_eq!(production.made.get("Oak planks"), Some(&4));
    assert!(w.farming.jobs.batch.is_none() && w.farming.jobs.carry.is_none());
    assert!(w.inventory.dropped.is_empty());
}

#[test]
fn legacy_service_delayed_primary_uses_current_clock_for_new_work() {
    let mut w = paid_fixture();
    let inventory = w.inventory.clone();
    w.farming.clock = DELAYED;
    jobs::service(&mut w, Vec3::new(24.5, 8.02, 24.5));
    // The ordinary executor may perform one uncertified pulse. It may not
    // consume the entire pre-service delay as a newly certified interval.
    assert!((5..=6).contains(&w.farming.jobs.work));
    assert_eq!(stock(&w, Item::Block(B::Planks)), 0);
    assert_eq!(stock(&w, Item::Block(B::Log)), 1);
    if let Some(c) = &w.farming.jobs.activity {
        assert_eq!(c.start, DELAYED);
        assert_ne!(c.configuration, 0);
        assert!(c.current(&w));
    }
    assert_eq!(w.farming.jobs.motion_at, DELAYED);
    assert!(w.farming.jobs.batch.is_some());
    assert_eq!(w.inventory, inventory);
    assert!(w.adventure.enemies.is_empty());
}

fn mining(id: u32, at: IVec3) -> State {
    let pos = Vec3::new(8.5, 8.02, at.z as f32 + 0.5);
    let mut j = work(id);
    j.stockpile = Some(STOCK.to_array());
    j.worker_pos = Some(pos.to_array());
    j.body = Some(quiet_body(id, pos));
    j.mine = vec![(at.to_array(), B::Stone as u16)];
    j.task = Some(Task::Mine(at, B::Stone as u16));
    let required = (3.0 + 4.0 * B::Stone.material().physics.hardness).ceil().clamp(3.0, 60.0) as u8;
    assert!(required > 5, "fixture needs incomplete saved work");
    j.activity.as_mut().unwrap().kind = Kind::Work { task: j.task.unwrap(), base: 2, required };
    j.activity.as_mut().unwrap().end = START + u64::from(required - 2) * PULSE;
    j
}

fn city_schema(j: State) -> String {
    let people = crew::ids(&j).map(|id| (id, PersonFixture { home: communities::Home {
        destination: [20, 8, 20], entry: [20, 8, 20] } })).collect();
    let wakes = crew::ids(&j).map(|id| (id, COUNCIL_LATER)).collect();
    old_schema(&CommunitiesFixture { sites: BTreeMap::from([(SITE, CityFixture {
        jobs: j, people, station: BENCH.to_array(), fields: Vec::new(), stage: 2,
        wakes, next_plan: COUNCIL_LATER,
    })]) })
}

#[test]
fn legacy_service_delayed_helper_and_offscreen_com1_execute_real_schedule() {
    let checkpoint = START + 2 * PULSE;
    let mut local = State { stockpile: Some(STOCK.to_array()), ..State::default() };
    // Keep a quiet primary so the helper remains a helper throughout service.
    local.worker = Some(31);
    local.worker_pos = Some([20.5, 8.02, 20.5]);
    local.retry_at = COUNCIL_LATER;
    local.mine = vec![(ORE.to_array(), B::Stone as u16)];
    add_helper(&mut local, mining(32, ORE));
    let city = city_schema(mining(SITE, OTHER_ORE));
    let mut w = load_service(&fixture(checkpoint, Some(&local), Some(&city)));
    terrain(&mut w);
    assert_eq!(crew::view(&w.farming.jobs, 32).unwrap().work, 4);
    assert_eq!(communities::state_for(&w, SITE).unwrap().work, 4);
    w.farming.clock = DELAYED;
    jobs::service(&mut w, Vec3::new(24.5, 8.02, 24.5));
    // Helper zero has phase zero for this bounded roster. Check the contract
    // explicitly rather than silently testing a service call that did no work.
    assert!(crew::phase_indices(DELAYED % PULSE, 1).contains(&0));
    let helper = crew::view(&w.farming.jobs, 32).unwrap();
    let city = communities::state_for(&w, SITE).unwrap();
    for (progress, certificate) in [(helper.work, helper.activity), (city.work, city.activity.as_ref())] {
        assert!((4..=5).contains(&progress));
        let c = certificate.expect("resident supported work should receive a fresh certificate");
        assert_eq!(c.start, DELAYED);
        assert_ne!(c.configuration, 0);
        assert!(c.current(&w));
    }
    assert_eq!(crew::ids(&w.farming.jobs).collect::<Vec<_>>(), vec![31, 32]);
    assert_eq!(crew::ids(city).collect::<Vec<_>>(), vec![SITE]);
    assert!(w.farming.communities.valid(&w.farming.jobs, w.farming.clock));
    assert_eq!(w.resident_block(ORE), Some(B::Stone));
    assert_eq!(w.resident_block(OTHER_ORE), Some(B::Stone));
    assert!(w.adventure.enemies.is_empty() && w.inventory.dropped.is_empty());
}

#[test]
fn legacy_service_paid_batch_finishes_once_across_completion_reload() {
    let mut w = paid_fixture();
    let inventory = w.inventory.clone();
    w.farming.clock = DELAYED;
    jobs::service(&mut w, Vec3::new(24.5, 8.02, 24.5));
    assert_eq!(stock(&w, Item::Block(B::Planks)), 0);
    services(&mut w, 12 * PULSE);
    assert_one_batch(&w);
    reload_in_place(&mut w);
    services(&mut w, 24 * PULSE);
    assert_one_batch(&w);
    assert_eq!(w.inventory, inventory);
    assert!(w.adventure.enemies.is_empty());
}

#[test]
fn legacy_service_paid_batch_waits_for_valid_station_then_finishes_once() {
    let mut w = paid_fixture();
    w.set(BENCH, B::Air);
    w.farming.clock = DELAYED;
    jobs::service(&mut w, Vec3::new(24.5, 8.02, 24.5));
    services(&mut w, 20 * PULSE);
    assert_eq!(stock(&w, Item::Block(B::Planks)), 0);
    assert_eq!(stock(&w, Item::Block(B::Log)), 1);
    assert_eq!(w.farming.jobs.production.as_ref().unwrap().counts.batches, 0);
    assert_eq!(w.farming.jobs.batch.clone().unwrap().recovery(),
        vec![ItemStack::new(Item::Block(B::Log), 1)]);
    reload_in_place(&mut w);
    w.set(BENCH, B::Workbench);
    services(&mut w, 64 * PULSE);
    assert_one_batch(&w);
    reload_in_place(&mut w);
    services(&mut w, 24 * PULSE);
    assert_one_batch(&w);
}

fn walking(id: u32, z: f32) -> State {
    let mut j = walk(id);
    let from = [8.5, 8.02, z];
    j.stockpile = Some(STOCK.to_array());
    j.worker_pos = Some(from);
    j.body = Some(quiet_body(id, from.into()));
    j.activity.as_mut().unwrap().kind = Kind::Walk { from, to: [20.5, 8.02, z] };
    j
}

fn actor_count(w: &World, id: u32) -> usize {
    w.adventure.enemies.iter().filter(|e| vehicle::villager_id(e) == Some(id)).count()
}

#[test]
fn legacy_service_actor_restore_preserves_poses_and_unique_owners() {
    let mut local = walking(31, 8.5);
    local.carry = Some(ItemStack::new(Item::Block(B::Planks), 3));
    add_helper(&mut local, walking(32, 13.5));
    let mut town = walking(SITE, 18.5);
    add_helper(&mut town, walking(SITE + 1, 23.5));
    let mut w = load_service(&fixture(START + 60, Some(&local), Some(&city_schema(town))));
    terrain(&mut w);
    let expected: Vec<_> = crew::saved_workers(&w);
    assert_eq!(expected.len(), 4);
    assert!(expected.iter().all(|(_, at)| at.x == 14.5));
    let observer = Vec3::new(16.5, 8.02, 16.5);
    w.farming.clock = DELAYED;
    for _ in 0..3 { vehicle::service(&mut w, observer); }
    for &(id, at) in &expected {
        assert_eq!(actor_count(&w, id), 1);
        assert_eq!(vehicle::villager_feet(&w, id), Some(at));
    }
    // Checkpoint uses actors, not raw jobs equality. Repeated load while actors
    // remain must not spawn another identity or copy carried resources.
    reload_in_place(&mut w);
    vehicle::service(&mut w, observer);
    for &(id, at) in &expected {
        assert_eq!(actor_count(&w, id), 1);
        assert_eq!(vehicle::villager_feet(&w, id), Some(at));
    }
    w.adventure.enemies.clear(); // Explicit render-unload fixture, not worker death.
    let saved_chunk = w.chunks.get(&IVec3::ZERO).cloned().expect("resident fixture chunk");
    w.chunks.remove(&IVec3::ZERO);
    vehicle::service(&mut w, observer);
    assert!(expected.iter().all(|&(id, _)| actor_count(&w, id) == 0));
    assert!(!w.chunks.contains_key(&IVec3::ZERO));
    w.chunks.insert(IVec3::ZERO, saved_chunk);
    vehicle::service(&mut w, observer);
    for &(id, at) in &expected {
        assert_eq!(actor_count(&w, id), 1);
        assert_eq!(vehicle::villager_feet(&w, id), Some(at));
    }
    w.adventure.enemies.clear();
    let blocked = expected.iter().find(|&&(id, _)| id == 31).unwrap().1.floor().as_ivec3();
    w.set(blocked, B::Stone);
    vehicle::service(&mut w, observer);
    assert_eq!(actor_count(&w, 31), 0, "blocked saved pose must defer, not teleport");
    assert_eq!(crew::saved_workers(&w), expected);
    w.set(blocked, B::Air);
    vehicle::service(&mut w, observer);
    assert_eq!(actor_count(&w, 31), 1);
    assert_eq!(vehicle::villager_feet(&w, 31), expected.iter().find(|&&(id, _)| id == 31).map(|&(_, at)| at));
    assert_eq!(w.farming.jobs.carry, local.carry);
    assert_eq!(crew::saved_workers(&w), expected);
    assert!(w.inventory.dropped.is_empty());
}

#[test]
#[ignore = "broader fixed gameplay integration; run separately after the focused service tests"]
fn legacy_service_fixed_gameplay_actor_follows_replanned_pose() {
    let mut j = walking(31, 8.5);
    let target = IVec3::new(24, 8, 8);
    j.mine = vec![(target.to_array(), B::Stone as u16)];
    j.task = Some(Task::Mine(target, B::Stone as u16));
    let mut w = load_service(&fixture(START + 60, Some(&j), None));
    terrain(&mut w);
    w.set(target, B::Stone);
    let mut p = crate::player::Player::new();
    p.pos = Vec3::new(16.5, 8.02, 24.5);
    p.precise_pos = p.pos.as_dvec3();
    let mut input = crate::player::Input::default();
    w.farming.clock = 240;
    vehicle::service(&mut w, p.pos);
    assert_eq!(vehicle::villager_feet(&w, 31).unwrap().x, 14.5);
    let mut synchronized_samples = 0;
    let mut furthest = 14.5f32;
    for _ in 0..480 {
        crate::simulation::fixed::gameplay(&mut w, &mut p, &mut input, false,
            |_, _, _, _| crate::simulation::fixed::Actions::default());
        assert_eq!(actor_count(&w, 31), 1);
        let at = vehicle::villager_feet(&w, 31).unwrap();
        assert!(at.x >= 14.49, "restored walk rewound toward its old origin");
        furthest = furthest.max(at.x);
        if let Some(pose) = jobs::activity::pose(&w, 31) {
            assert!(at.distance(pose) < 0.03, "actor and certified presentation pose diverged");
            synchronized_samples += 1;
        }
    }
    assert!(synchronized_samples > 0, "fixture never reached a new certified walk");
    assert!(furthest > 15.0, "fixture never resumed actual movement");
    assert_eq!(crew::ids(&w.farming.jobs).collect::<Vec<_>>(), vec![31]);
    assert!(w.inventory.dropped.is_empty());
}