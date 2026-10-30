//! The dam-break scenario on the synthetic table: conservation, replay from the action log and
//! continuation from a checkpoint, plus the codecs' refusals.
use crate::test_support::*;
use crate::*;
use glam::IVec3;

const SEED: u32 = 0xda4b;

/// At the old fixed layout, straddling negative chunk boundaries, and a million cells out.
const ORIGINS: [IVec3; 3] = [IVec3::new(0, 1, 0), IVec3::new(-40, -37, -70), IVec3::new(1 << 20, 5, -(1 << 20))];

fn small_at(origin: IVec3) -> DamBreak {
    DamBreak { origin, reservoir: IVec3::new(12, 8, 6), wall: STONE, liquid: WATER, release_tick: 5 }
}

fn small() -> DamBreak {
    small_at(ORIGINS[0])
}

fn built(d: &DamBreak) -> BrickStore {
    let mut s = BrickStore::new(d.capacity());
    d.build(&mut s).unwrap();
    s
}

fn liquid(s: &BrickStore, d: &DamBreak) -> u32 {
    let (lo, hi) = d.bounds();
    total(s, lo, hi, WATER)
}

#[test]
fn the_dam_holds_until_the_log_removes_it_then_water_crosses_and_is_conserved() {
    let t = table();
    for origin in ORIGINS {
        let d = small_at(origin);
        let mut s = built(&d);
        let before = liquid(&s, &d);
        assert_eq!(before, d.liquid_cells() * VOLUME_FULL as u32, "{origin}");
        let (blo, bhi) = d.basin();
        run_log(&mut s, &t, SEED, &[], 0..d.release_tick).unwrap();
        assert_eq!(total(&s, blo, bhi, WATER), 0, "{origin}: the basin is dry while the dam stands");
        run_log(&mut s, &t, SEED, &d.actions(), d.release_tick..d.release_tick + 400).unwrap();
        assert_eq!(liquid(&s, &d), before, "{origin}: the release creates and destroys no water");
        let far = IVec3::new(bhi.x - d.reservoir.x, blo.y, blo.z);
        assert!(total(&s, far, bhi, WATER) > 0, "{origin}: water reaches the far half of the basin");
    }
}

#[test]
fn two_runs_of_the_same_log_agree_every_tick() {
    let (t, d) = (table(), small());
    let (mut a, mut b) = (built(&d), built(&d));
    let log = d.actions();
    for tick in 0..120 {
        run_log(&mut a, &t, SEED, &log, tick..tick + 1).unwrap();
        run_log(&mut b, &t, SEED, &log, tick..tick + 1).unwrap();
        assert_eq!(a.state_hash(), b.state_hash(), "runs diverge at tick {tick}");
    }
    assert_eq!(a.encode_checkpoint(), b.encode_checkpoint());
}

#[test]
fn one_long_run_and_tick_by_tick_runs_agree() {
    let (t, d) = (table(), small());
    let log = d.actions();
    let (mut whole, mut steps) = (built(&d), built(&d));
    run_log(&mut whole, &t, SEED, &log, 0..90).unwrap();
    for tick in 0..90 {
        run_log(&mut steps, &t, SEED, &log, tick..tick + 1).unwrap();
    }
    assert_eq!(whole.encode_checkpoint(), steps.encode_checkpoint());
}

#[test]
fn a_store_restored_from_a_checkpoint_continues_to_the_same_bytes() {
    let (t, d) = (table(), small());
    let log = d.actions();
    let end = d.release_tick + 150;
    let mut whole = built(&d);
    run_log(&mut whole, &t, SEED, &log, 0..end).unwrap();
    // Before the release the restored store still has to apply the log; after it, mid-flood.
    for cut in [d.release_tick - 2, d.release_tick + 40] {
        let mut part = built(&d);
        run_log(&mut part, &t, SEED, &log, 0..cut).unwrap();
        let bytes = part.encode_checkpoint();
        let mut restored = BrickStore::decode_checkpoint(&bytes).unwrap();
        restored.check_invariants();
        assert_eq!(restored.encode_checkpoint(), bytes, "decode then encode is the identity");
        assert_eq!(restored.active_bricks(), part.active_bricks());
        run_log(&mut restored, &t, SEED, &log, cut..end).unwrap();
        assert_eq!(restored.encode_checkpoint(), whole.encode_checkpoint(), "cut at tick {cut}");
        assert_eq!(restored.pool_bytes(), whole.pool_bytes(), "the GPU upload matches too");
    }
}

#[test]
fn a_run_resumes_from_saved_bytes_and_a_saved_log() {
    let (t, d) = (table(), small_at(ORIGINS[1]));
    let log = decode_log(&encode_log(&d.actions())).unwrap();
    assert_eq!(log, d.actions());
    let end = d.release_tick + 120;
    let mut whole = RunCheckpoint { tick: 0, seed: SEED, store: built(&d) };
    whole.resume(&t, &log, end).unwrap();
    assert_eq!(whole.tick, end);

    let mut first = RunCheckpoint { tick: 0, seed: SEED, store: built(&d) };
    first.resume(&t, &log, d.release_tick + 30).unwrap();
    let mut second = RunCheckpoint::decode(&first.encode()).unwrap();
    assert_eq!((second.tick, second.seed), (first.tick, SEED));
    second.resume(&t, &log, end).unwrap();
    assert_eq!(second.store.encode_checkpoint(), whole.store.encode_checkpoint());
    assert!(second.resume(&t, &log, 3).unwrap().is_empty(), "an end in the past steps nothing");
    assert_eq!(second.tick, end);
}

#[test]
fn a_damaged_checkpoint_is_refused() {
    let (t, d) = (table(), small());
    let mut s = built(&d);
    run_log(&mut s, &t, SEED, &d.actions(), 0..d.release_tick + 3).unwrap();
    let good = s.encode_checkpoint();
    assert!(BrickStore::decode_checkpoint(&good).is_ok());

    let mut magic = good.clone();
    magic[0] ^= 1;
    assert_eq!(BrickStore::decode_checkpoint(&magic).err(), Some(CheckpointError::BadMagic));
    for len in [0, 7, 12, good.len() / 2, good.len() - 1] {
        assert_eq!(BrickStore::decode_checkpoint(&good[..len]).err(), Some(CheckpointError::Truncated), "{len}");
    }
    let mut long = good.clone();
    long.push(0);
    assert_eq!(BrickStore::decode_checkpoint(&long).err(), Some(CheckpointError::TrailingBytes));

    // Point the first chunk's first stored slot at the slot its neighbour uses.
    let bricks = u32::from_le_bytes(good[12..16].try_into().unwrap()) as usize;
    let free_at = 16 + bricks * BRICK_CELLS * 8;
    let free = u32::from_le_bytes(good[free_at..free_at + 4].try_into().unwrap()) as usize;
    let slots_at = free_at + 4 + free * 4 + 4 + 12 + 1 + 8;
    let slot = |b: &[u8], i: usize| u32::from_le_bytes(b[slots_at + 4 * i..][..4].try_into().unwrap());
    let stored: Vec<usize> = (0..BRICKS_PER_CHUNK).filter(|&i| slot(&good, i) != NO_SLOT).collect();
    assert!(stored.len() >= 2, "the first chunk holds two bricks");
    let mut twice = good.clone();
    let copy = slot(&good, stored[1]).to_le_bytes();
    twice[slots_at + 4 * stored[0]..][..4].copy_from_slice(&copy);
    assert!(matches!(BrickStore::decode_checkpoint(&twice), Err(CheckpointError::Inconsistent(_))));

    let run = RunCheckpoint { tick: 9, seed: SEED, store: s }.encode();
    assert_eq!(RunCheckpoint::decode(&run[..run.len() - 1]).err(), Some(CheckpointError::Truncated));
}

#[test]
fn a_damaged_log_is_refused() {
    let a = Action { tick: 3, lo: IVec3::new(-5, 0, 2), hi: IVec3::new(-1, 4, 3), cell: Cell::filled(WATER) };
    let good = encode_log(&[a, Action { tick: 1, ..a }]);
    assert_eq!(decode_log(&good).unwrap(), vec![a, Action { tick: 1, ..a }], "order is kept");
    assert_eq!(decode_log(&good[..good.len() - 1]).err(), Some(CheckpointError::Truncated));
    assert_eq!(decode_log(&[good.as_slice(), &[0]].concat()).err(), Some(CheckpointError::TrailingBytes));
    let inverted = encode_log(&[Action { lo: a.hi, hi: a.lo, ..a }]);
    assert!(matches!(decode_log(&inverted), Err(CheckpointError::Inconsistent(_))));
    let huge = encode_log(&[Action { lo: IVec3::splat(-1000), hi: IVec3::splat(1000), ..a }]);
    assert!(matches!(decode_log(&huge), Err(CheckpointError::Inconsistent(_))));
    // A count claiming more actions than the bytes hold is refused before allocating.
    let mut lying = encode_log(&[]);
    lying[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(decode_log(&lying).err(), Some(CheckpointError::Truncated));
}

/// `cargo test -p engine_sim --release dam_break_200k_tick_time -- --ignored --nocapture`
#[test]
#[ignore = "timing: run in release"]
fn dam_break_200k_tick_time() {
    let t = table();
    let d = DamBreak { origin: IVec3::new(0, 1, 0), reservoir: DAM_BREAK_200K, wall: STONE, liquid: WATER, release_tick: 1 };
    let mut s = built(&d);
    let before = liquid(&s, &d);
    let log = d.actions();
    let mut times = Vec::new();
    for tick in 0..600 {
        let start = std::time::Instant::now();
        run_log(&mut s, &t, SEED, &log, tick..tick + 1).unwrap();
        times.push(start.elapsed().as_secs_f64() * 1e3);
    }
    assert_eq!(liquid(&s, &d), before);
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    let p = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
    let worst = (0..times.len()).max_by(|&a, &b| times[a].total_cmp(&times[b])).unwrap_or(0);
    println!(
        "dam_break_200k CPU: {} bricks, p50 {:.2} ms, p95 {:.2} ms, p99 {:.2} ms, max {:.2} ms at tick {worst}, {} ticks",
        s.bricks_in_use(),
        p(0.5),
        p(0.95),
        p(0.99),
        p(1.0),
        times.len()
    );
}
