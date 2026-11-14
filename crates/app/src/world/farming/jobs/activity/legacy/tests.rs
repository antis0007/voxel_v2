//! Full JOB1/COM1 schema fixtures; genuine V20 writer files have a separate opt-in test.
use super::*;
use crate::world::{farming, farming::jobs, inventory::Item, ChunkData, WorldType};
use engine_core::VoxelScalePreset;
use std::sync::Arc;

const START: u64 = 100;
const SITE: u32 = 1 << 28;
const PILE: IVec3 = IVec3::new(8, 1, 10);
const TARGET: IVec3 = IVec3::new(9, 1, 8);

fn empty(clock: u64) -> World {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 17);
    w.adventure.enemies.clear();
    w.farming.clock = clock;
    w
}

fn walk(id: u32) -> State {
    State { worker: Some(id), worker_pos: Some([0.5, 1.0, 0.5]), motion_at: START,
        stockpile: Some(PILE.to_array()),
        activity: Some(Commitment { configuration: 0, start: START, end: START + 120,
            kind: Kind::Walk { from: [0.5, 1.0, 0.5], to: [12.5, 1.0, 0.5] },
            stamps: vec![Stamp { chunk: [0, 0, 0], version: None }] }),
        ..State::default() }
}

fn work(id: u32) -> State {
    let task = Task::Mine(TARGET, B::Stone as u16);
    State { worker: Some(id), worker_pos: Some([8.5, 1.0, 8.5]), motion_at: START,
        stockpile: Some(PILE.to_array()), mine: vec![(TARGET.to_array(), B::Stone as u16)],
        task: Some(task), work: 2,
        activity: Some(Commitment { configuration: 0, start: START, end: START + 10 * PULSE,
            kind: Kind::Work { task, base: 2, required: 12 },
            stamps: vec![Stamp { chunk: [0, 0, 0], version: None }] }),
        ..State::default() }
}

fn add_helper(j: &mut State, mut donor: State) {
    let mut worker = crew::Worker::default();
    worker.swap(&mut donor);
    j.helpers.push(worker);
}

fn old_schema<T: Serialize>(value: &T) -> String {
    // These are the only new persisted fields in the selected V20/V21 types.
    // New Wait/Campaign variants are deliberately never used in these fixtures.
    ron::to_string(value).unwrap().replace("configuration:0,", "").replace("territory:0.0,", "")
}

#[derive(Serialize, Deserialize)]
struct PersonFixture { home: communities::Home }
#[derive(Serialize, Deserialize)]
struct CityFixture {
    jobs: State,
    people: BTreeMap<u32, PersonFixture>,
    station: [i32; 3],
    fields: Vec<[i32; 3]>,
    stage: u8,
    wakes: BTreeMap<u32, u64>,
    next_plan: u64,
}
#[derive(Serialize, Deserialize)]
struct CommunitiesFixture { sites: BTreeMap<u32, CityFixture> }

fn town() -> String {
    let mut job = walk(SITE);
    add_helper(&mut job, work(SITE + 1));
    let people = crew::ids(&job).map(|id| (id, PersonFixture {
        home: communities::Home { destination: [8, 1, 8], entry: [8, 1, 8] },
    })).collect();
    let wakes = crew::ids(&job).map(|id| (id, 9999)).collect();
    old_schema(&CommunitiesFixture { sites: BTreeMap::from([(SITE, CityFixture {
        jobs: job, people, station: PILE.to_array(), fields: Vec::new(), stage: 2, wakes, next_plan: 9999,
    })]) })
}

fn fixture(clock: u64, local: Option<&State>, town: Option<&str>) -> Vec<u8> {
    // The unchanged empty VXF framing/real herd payload comes from the current
    // writer. Only the explicitly legacy JOB1 and COM1 RON is appended here.
    let mut bytes = farming::sidecar::encode(&empty(clock));
    bytes.truncate(bytes.len() - 8);
    for (tag, text) in [(b"JOB1", local.map(old_schema)), (b"COM1", town.map(str::to_owned))] {
        if let Some(text) = text {
            bytes.extend_from_slice(tag);
            bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
        }
    }
    let checksum = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64,
        |sum, &b| (sum ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    bytes.extend_from_slice(&checksum.to_le_bytes());
    bytes
}

fn load(bytes: &[u8]) -> World {
    let (state, herd) = farming::sidecar::decode(bytes).unwrap();
    let clock = state.clock;
    let mut w = empty(clock);
    w.farming = state;
    farming::sidecar::apply_herd(&mut w, herd, clock).unwrap();
    w
}

fn resume_main(w: &mut World) -> bool {
    let mut j = std::mem::take(&mut w.farming.jobs);
    let result = resume(w, &mut j);
    w.farming.jobs = j;
    result
}

#[test]
fn legacy_checkpoint_job1_walk_preserves_position_and_bounds_delayed_service() {
    let clock = START + 60;
    let text = old_schema(&walk(0));
    assert!(!text.contains("configuration") && !text.contains("territory"));
    let mut w = load(&fixture(clock, Some(&walk(0)), None));
    assert_eq!(w.farming.jobs.worker_pos, Some([6.5, 1.0, 0.5]));
    assert_eq!(w.farming.jobs.motion_at, clock);
    assert!(w.farming.jobs.activity.is_none() && w.farming.jobs.path.is_empty());
    w.farming.clock += 9000;
    rebuild(&mut w);
    assert!(!resume_main(&mut w));
    assert_eq!(w.farming.jobs.worker_pos, Some([6.5, 1.0, 0.5]));
    assert!(w.adventure.enemies.is_empty());
}

#[test]
fn legacy_checkpoint_job1_work_preserves_only_checkpoint_progress() {
    let clock = START + 5 * PULSE;
    let mut w = load(&fixture(clock, Some(&work(0)), None));
    assert_eq!(w.farming.jobs.work, 7);
    assert!(w.farming.jobs.activity.is_none());
    w.farming.clock += 9000;
    rebuild(&mut w);
    assert!(!resume_main(&mut w));
    assert_eq!(w.farming.jobs.work, 7);
    assert!(w.farming.jobs.carry.is_none() && w.inventory.dropped.is_empty());
}

#[test]
fn legacy_checkpoint_job1_helper_keeps_one_owner_and_carried_stock() {
    let clock = START + 60;
    let mut j = walk(0);
    j.carry = Some(ItemStack::new(Item::Block(B::Planks), 3));
    add_helper(&mut j, work(1));
    let w = load(&fixture(clock, Some(&j), None));
    assert_eq!(crew::ids(&w.farming.jobs).collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(w.farming.jobs.carry, j.carry);
    let helper = crew::view(&w.farming.jobs, 1).unwrap();
    let expected = (2 + 60 / PULSE).min(12) as u8;
    assert_eq!(helper.work, expected);
    assert!(helper.activity.is_none() && helper.path.is_empty());
    assert_eq!(w.farming.jobs.worker_pos, Some([6.5, 1.0, 0.5]));
    assert!(w.adventure.enemies.is_empty() && w.inventory.dropped.is_empty());
}

#[test]
fn legacy_checkpoint_com1_offscreen_community_rebuilds_owners_and_wakes() {
    let clock = START + 60;
    let w = load(&fixture(clock, None, Some(&town())));
    let j = communities::state_for(&w, SITE).unwrap();
    assert_eq!(j.worker_pos, Some([6.5, 1.0, 0.5]));
    assert!(j.activity.is_none());
    assert!(crew::view(j, SITE + 1).unwrap().activity.is_none());
    let saved: CommunitiesFixture = ron::from_str(&ron::to_string(&w.farming.communities).unwrap()).unwrap();
    assert_eq!(saved.sites[&SITE].wakes, BTreeMap::from([(SITE, clock), (SITE + 1, clock)]));
    assert_eq!(saved.sites[&SITE].next_plan, 9999);
    assert!(w.farming.communities.valid(&w.farming.jobs, clock));
    assert!(w.adventure.enemies.is_empty());
}

#[test]
fn legacy_checkpoint_load_save_reload_is_idempotent_for_job1_and_com2() {
    let clock = START + 60;
    let mut j = walk(0);
    j.carry = Some(ItemStack::new(Item::Block(B::Planks), 3));
    let w = load(&fixture(clock, Some(&j), Some(&town())));
    let first = farming::sidecar::encode(&w);
    assert!(first.windows(4).any(|bytes| bytes == b"COM2"));
    let again = load(&first);
    assert_eq!(farming::sidecar::encode(&again), first);
    assert_eq!(again.farming.jobs.carry, j.carry);
    assert_eq!(crew::ids(&again.farming.jobs).count(), 1);
    assert_eq!(crew::ids(communities::state_for(&again, SITE).unwrap()).count(), 2);
    assert!(again.adventure.enemies.is_empty() && again.inventory.dropped.is_empty());
}

#[test]
fn legacy_checkpoint_nonzero_configuration_mismatch_is_not_migrated() {
    let clock = START + 5 * PULSE;
    let mut reference = empty(clock);
    rebuild(&mut reference);
    let other = reference.farming.activities.fingerprint.wrapping_add(1).max(1);
    let mut j = work(0);
    j.activity.as_mut().unwrap().configuration = other;
    let mut w = load(&fixture(clock, Some(&j), None));
    assert_eq!(w.farming.jobs.work, 2);
    assert_eq!(w.farming.jobs.activity.as_ref().unwrap().configuration, other);
    rebuild(&mut w);
    assert!(!resume_main(&mut w));
    assert_eq!(w.farming.jobs.work, 2);
}

#[test]
fn legacy_checkpoint_nonzero_stale_geometry_keeps_existing_rejection() {
    let clock = START + 60;
    let mut reference = empty(clock);
    rebuild(&mut reference);
    let mut j = walk(0);
    j.activity.as_mut().unwrap().configuration = reference.farming.activities.fingerprint;
    let mut w = load(&fixture(clock, Some(&j), None));
    assert!(w.farming.jobs.activity.is_some());
    w.regions.mark_dirty(IVec3::ZERO);
    rebuild(&mut w);
    assert!(!resume_main(&mut w));
    assert_eq!(w.farming.jobs.worker_pos, Some([0.5, 1.0, 0.5]));
}

fn resident_terrain(w: &mut World) {
    let mut blocks = Box::new([B::Air; 32 * 32 * 32]);
    for x in 0..32 { for z in 0..32 {
        blocks[ChunkData::local_index(x, 0, z)] = B::Stone;
    }}
    blocks[ChunkData::local_index(PILE.x, PILE.y, PILE.z)] = B::Chest;
    blocks[ChunkData::local_index(TARGET.x, TARGET.y, TARGET.z)] = B::Stone;
    w.chunks.insert(IVec3::ZERO, Arc::new(ChunkData { modified_bounds: None,
        structural_bounds: None, blocks, light: Box::new([0; 32 * 32 * 32]),
        torches: Default::default() }));
}

#[test]
fn legacy_checkpoint_stale_terrain_requires_fresh_task_validation_before_rewards() {
    let clock = START + 20 * PULSE;
    let mut w = load(&fixture(clock, Some(&work(0)), None));
    assert_eq!(w.farming.jobs.work, 12);
    assert!(w.farming.jobs.activity.is_none());
    resident_terrain(&mut w);
    let feet = Vec3::new(8.5, 1.0, 8.5);
    let player = Vec3::splat(-100.0);
    let task = w.farming.jobs.task.unwrap();
    assert!(jobs::task_still_valid(&w, feet, player, &w.farming.jobs, PILE, task));
    w.set(TARGET, B::Air);
    assert_ne!(w.regions.source_version(IVec3::ZERO), None);
    assert!(!jobs::task_still_valid(&w, feet, player, &w.farming.jobs, PILE, task));
    let inventory = w.inventory.clone();
    let stock = w.block_data.chest(PILE);
    let mut j = std::mem::take(&mut w.farming.jobs);
    jobs::work(&mut w, feet, player, &mut j);
    w.farming.jobs = j;
    assert_ne!(w.farming.jobs.task, Some(task));
    assert_eq!(w.resident_block(TARGET), Some(B::Air));
    assert_eq!(w.block_data.chest(PILE), stock);
    assert_eq!(w.inventory, inventory);
    assert!(w.farming.jobs.carry.is_none() && w.adventure.enemies.is_empty());
}

#[test]
fn legacy_checkpoint_future_dated_commitment_is_cleared_without_advancing() {
    let mut j = work(0);
    j.work = 1;
    let w = load(&fixture(START - 1, Some(&j), None));
    assert_eq!(w.farming.jobs.work, 1);
    assert_eq!(w.farming.jobs.worker_pos, j.worker_pos);
    assert!(w.farming.jobs.activity.is_none());
}

#[test]
fn legacy_checkpoint_rest_elapsed_is_materialized_once() {
    let mut j = walk(0);
    j.activity.as_mut().unwrap().kind = Kind::Rest { at: [0.5, 1.0, 0.5] };
    j.routine.dwell = 5;
    let w = load(&fixture(START + 60, Some(&j), None));
    assert_eq!(w.farming.jobs.routine.dwell, 65);
    assert_eq!(w.farming.jobs.routine.last, START + 60);
    let again = load(&farming::sidecar::encode(&w));
    assert_eq!(again.farming.jobs.routine.dwell, 65);
}

#[test]
fn legacy_checkpoint_paid_batch_keeps_receipt_without_crediting_output() {
    #[derive(Serialize)]
    struct BatchFixture {
        source: [i32; 3], station: [i32; 3], bill: u8, recipe: String,
        inputs: Vec<ItemStack>, output: ItemStack, progress: u8, smelt: bool,
    }
    let mut j = work(0);
    let station = IVec3::new(8, 1, 9);
    let task = Task::Produce(production::Task::Craft(station, 0));
    j.task = Some(task);
    j.activity.as_mut().unwrap().kind = Kind::Work { task, base: 2, required: 12 };
    let input = ItemStack::new(Item::Block(B::Log), 2);
    let output = ItemStack::new(Item::Block(B::Planks), 8);
    let receipt = BatchFixture { source: PILE.to_array(), station: station.to_array(),
        bill: 0, recipe: "fixture-paid-input".into(), inputs: vec![input], output,
        progress: 2, smelt: false };
    j.batch = Some(ron::from_str(&ron::to_string(&receipt).unwrap()).unwrap());
    let w = load(&fixture(START + 5 * PULSE, Some(&j), None));
    assert_eq!(w.farming.jobs.work, 7);
    assert_eq!(w.farming.jobs.batch.clone().unwrap().recovery(), vec![input]);
    assert!(w.farming.jobs.carry.is_none() && w.inventory.dropped.is_empty());
    assert!(w.block_data.chest(PILE).iter().all(Option::is_none));
    let encoded = farming::sidecar::encode(&w);
    let again = load(&encoded);
    assert_eq!(farming::sidecar::encode(&again), encoded);
    assert_eq!(again.farming.jobs.batch.unwrap().recovery(), vec![input]);
}
#[test]
#[ignore = "requires genuine files from the separate frozen V20 test-only exporter"]
fn legacy_checkpoint_genuine_v20_writer_fixtures() {
    let root = std::path::PathBuf::from(std::env::var_os("AXIOM_V20_LEGACY_FIXTURES_DIR")
        .expect("set AXIOM_V20_LEGACY_FIXTURES_DIR to the V20 exporter output"));
    let walked = load(&std::fs::read(root.join("primary-walk.vxf")).unwrap());
    assert_eq!(walked.farming.jobs.worker_pos, Some([6.5, 1.0, 0.5]));
    assert!(walked.farming.jobs.activity.is_none());
    let worked = load(&std::fs::read(root.join("primary-work.vxf")).unwrap());
    assert_eq!(worked.farming.jobs.work, 7);
    assert!(worked.farming.jobs.activity.is_none());
    let helpers = load(&std::fs::read(root.join("helper-walk.vxf")).unwrap());
    assert_eq!(crew::view(&helpers.farming.jobs, 1).unwrap().pos, Some([6.5, 1.0, 0.5]));
    assert!(crew::view(&helpers.farming.jobs, 1).unwrap().activity.is_none());
    let city = load(&std::fs::read(root.join("community-walk.vxf")).unwrap());
    assert_eq!(communities::state_for(&city, SITE).unwrap().worker_pos, Some([6.5, 1.0, 0.5]));
    assert!(communities::state_for(&city, SITE).unwrap().activity.is_none());
    for w in [&walked, &worked, &helpers, &city] {
        let bytes = farming::sidecar::encode(w);
        assert_eq!(farming::sidecar::encode(&load(&bytes)), bytes);
        assert!(w.adventure.enemies.is_empty() && w.inventory.dropped.is_empty());
    }
}
// Separate supplementary integration tests; no production behavior changes.
mod service_tests;
