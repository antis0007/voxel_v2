//! The dam-break scenario on the synthetic table: conservation, replay from the action log and
//! continuation from a checkpoint, plus the checkpoint codec's refusals.
use crate::test_support::*;
use crate::*;
use glam::IVec3;

const SEED: u32 = 0xda4b;

fn small() -> DamBreak {
    DamBreak { reservoir: IVec3::new(12, 8, 6), wall: STONE, liquid: WATER, release_tick: 5 }
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
    let (t, d) = (table(), small());
    let mut s = built(&d);
    let before = liquid(&s, &d);
    assert_eq!(before, d.liquid_cells() * VOLUME_FULL as u32);
    let (blo, bhi) = d.basin();
    run_log(&mut s, &t, SEED, &[], 0..d.release_tick).unwrap();
    assert_eq!(total(&s, blo, bhi, WATER), 0, "the basin is dry while the dam stands");
    run_log(&mut s, &t, SEED, &d.actions(), d.release_tick..d.release_tick + 400).unwrap();
    assert_eq!(liquid(&s, &d), before, "the release creates and destroys no water");
    let far = IVec3::new(d.basin().1.x - d.reservoir.x, blo.y, blo.z);
    assert!(total(&s, far, bhi, WATER) > 0, "water reaches the far half of the basin");
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
}

/// `cargo test -p engine_sim --release dam_break_200k_tick_time -- --ignored --nocapture`
#[test]
#[ignore = "timing: run in release"]
fn dam_break_200k_tick_time() {
    let t = table();
    let d = DamBreak { reservoir: DAM_BREAK_200K, wall: STONE, liquid: WATER, release_tick: 1 };
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
    println!(
        "dam_break_200k CPU: {} bricks, p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms over {} ticks",
        s.bricks_in_use(),
        p(0.5),
        p(0.95),
        p(1.0),
        times.len()
    );
}
