//! Conduction: energy-based exchange, stochastic rounding, settling, the max principle, insulators and
//! the environment sink. Empty space is a sink at its own temperature that does not warm up.
use crate::pinned::{HEAT_TICK_INTERVAL, mul_div_rem, mulhi};
use crate::test_support::*;
use crate::*;
use glam::IVec3;
use proptest::prelude::*;

const AMBIENT: u16 = AMBIENT_DECIKELVIN;

/// Lowest and highest temperature over the non-empty cells of a box.
fn range(s: &BrickStore, lo: IVec3, hi: IVec3) -> (u16, u16) {
    let temps: Vec<u16> = (lo.y..hi.y)
        .flat_map(|y| (lo.z..hi.z).flat_map(move |z| (lo.x..hi.x).map(move |x| IVec3::new(x, y, z))))
        .map(|p| at(s, p))
        .filter(|c| c.volume > 0)
        .map(|c| c.temperature)
        .collect();
    (*temps.iter().min().unwrap(), *temps.iter().max().unwrap())
}

/// A stone bar along x, hot in its first half, wrapped in foam so nothing leaves it.
fn sealed_bar() -> (BrickStore, IVec3, IVec3) {
    let mut s = BrickStore::new(500);
    let (lo, hi) = (IVec3::new(0, 4, 4), IVec3::new(16, 5, 5));
    fill(&mut s, lo - IVec3::ONE, hi + IVec3::ONE, Cell::filled(FOAM));
    fill(&mut s, lo, IVec3::new(8, 5, 5), hot(STONE, 4000));
    fill(&mut s, IVec3::new(8, 4, 4), hi, hot(STONE, AMBIENT));
    (s, lo, hi)
}

#[test]
fn a_sealed_bar_equalises_conserves_energy_to_rounding_and_settles() {
    let (t, (mut s, lo, hi)) = (table(), sealed_bar());
    let before = energy(&s, &t, lo, hi);
    let ticks = settle(&mut s, &t, 9, 20_000);
    let after = energy(&s, &t, lo, hi);
    let drift = before.abs_diff(after) as f64 / before as f64;
    println!("settled in {ticks} ticks, energy drift {drift:.6}");
    // Rounding is unbiased, so the drift is a random walk of single steps, not a leak.
    assert!(drift < 0.002, "energy moved by {:.4}%", drift * 100.0);
    let temp = |x| at(&s, IVec3::new(x, 4, 4)).temperature as i32;
    for x in lo.x..hi.x - 1 {
        assert!((temp(x) - temp(x + 1)).abs() <= 2, "step of {} at x={x}", temp(x) - temp(x + 1));
    }
    assert!(temp(0) < 4000 && temp(15) > AMBIENT as i32, "the hot end cooled and the cold end warmed");
    assert_eq!(s.active_bricks(), 0, "a settled thermal field costs nothing");
}

#[test]
fn temperatures_stay_between_the_extremes() {
    let (t, (mut s, lo, hi)) = (table(), sealed_bar());
    for tick in 0..300 {
        step(&mut s, &t, 4, tick);
        let (min, max) = range(&s, lo, hi);
        assert!(min >= AMBIENT - 1 && max <= 4000 + 1, "tick {tick}: {min}..{max} left the initial range");
    }
}

#[test]
fn a_hot_object_cools_into_the_environment_and_a_world_of_air_stays_empty() {
    let (t, mut s) = (table(), BrickStore::new(500));
    put(&mut s, IVec3::new(4, 4, 4), hot(STONE, 5000));
    put(&mut s, IVec3::new(12, 4, 4), hot(DIRT, 5000));
    let ticks = settle(&mut s, &t, 3, 20_000);
    println!("cooled to the environment in {ticks} ticks");
    assert!(at(&s, IVec3::new(4, 4, 4)).temperature <= AMBIENT + 2, "stone ended at the ambient");
    assert!(at(&s, IVec3::new(12, 4, 4)).temperature <= AMBIENT + 2, "dirt ended at the ambient");
    assert_eq!(at(&s, IVec3::new(5, 4, 4)), Cell::EMPTY, "the air beside it never warmed");
    assert_eq!(s.bricks_in_use(), 2, "heat did not spread into the surrounding air: still the two bricks");
}

#[test]
fn high_heat_capacity_cools_more_slowly() {
    let (t, mut s) = (table(), BrickStore::new(500));
    put(&mut s, IVec3::new(4, 4, 4), hot(STONE, 5000));
    put(&mut s, IVec3::new(12, 4, 4), hot(HEAVY, 5000));
    for tick in 0..100 {
        step(&mut s, &t, 2, tick);
    }
    let dropped = |x| 5000 - at(&s, IVec3::new(x, 4, 4)).temperature as i32;
    assert!(dropped(4) > dropped(12) * 3, "stone lost {} against the heavy solid's {}", dropped(4), dropped(12));
}

#[test]
fn an_insulator_passes_no_heat() {
    let (t, mut s) = (table(), BrickStore::new(100));
    let sealed = IVec3::new(4, 4, 4);
    for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
        put(&mut s, sealed + d, hot(FOAM, AMBIENT));
    }
    put(&mut s, sealed, hot(STONE, 5000));
    put(&mut s, IVec3::new(10, 4, 4), hot(STONE, 5000));
    for tick in 0..300 {
        step(&mut s, &t, 2, tick);
    }
    assert_eq!(at(&s, sealed).temperature, 5000, "a stone in a foam shell kept every bit of its heat");
    assert!(at(&s, IVec3::new(10, 4, 4)).temperature < 4500, "the same stone in open air cooled");
}

#[test]
fn heating_is_repeatable_and_depends_on_the_seed() {
    let t = table();
    let run = |seed| {
        let (mut s, ..) = sealed_bar();
        for tick in 0..120 {
            step(&mut s, &t, seed, tick);
        }
        s.state_hash()
    };
    assert_eq!(run(5), run(5));
    assert_ne!(run(5), run(6), "the rounding dice come from the seed");
}

#[test]
fn mul_div_rem_is_exact_and_mulhi_rounds_without_bias() {
    for (a, b, c) in [(4_000_000_000, 4, 5), (123_456_789, 987, 1_000_003), (7, 0, 5), (1, 1, 1)] {
        let (q, r) = mul_div_rem(a, b, c);
        assert_eq!(q as u128 * c as u128 + r as u128, a as u128 * b as u128);
        assert!(r < c);
    }
    // Over a uniform stream the test `mulhi(random, c) < r` is true r/c of the time.
    let (c, r) = (1_000_000u32, 250_000u32);
    let hits = (0..100_000u32).filter(|&i| mulhi(engine_core::hash::hash_u32(i), c) < r).count();
    assert!((hits as i64 - 25_000).abs() < 600, "{hits} of 100000 where 25000 are expected");
}

/// A hot stone cell in air: the drop in one heat step is `g * d / C`, with `g` carrying the interval's worth.
fn lone_hot_stone() -> (BrickStore, IVec3) {
    let mut s = BrickStore::new(10);
    let at_ = IVec3::new(4, 4, 4);
    put(&mut s, at_, hot(STONE, 5000));
    (s, at_)
}

#[test]
fn heat_conducts_only_on_heat_ticks() {
    let (t, (mut s, p)) = (table(), lone_hot_stone());
    for tick in 1..HEAT_TICK_INTERVAL {
        step(&mut s, &t, 1, tick);
        assert_eq!(at(&s, p).temperature, 5000, "tick {tick} is not a heat tick");
    }
    step(&mut s, &t, 1, 0);
    assert!(at(&s, p).temperature < 5000, "tick 0 is");
}

#[test]
fn a_heat_step_carries_the_skipped_ticks_energy() {
    let (t, (mut s, p)) = (table(), lone_hot_stone());
    step(&mut s, &t, 1, 0);
    // Stone to the environment: k = harmonic(2.5, 0.026 W/mK) = 5 (0.01 units), g = 5 * 4096 * interval,
    // thermal mass 200,000 per level, 64 levels, difference 2068: g d / C is about 13.2 steps of 0.1 K per
    // face, and a cell exchanges through all six faces across the two phases of a tick.
    let drop = 5000 - at(&s, p).temperature as i32;
    assert!((70..=88).contains(&drop), "one heat step dropped {drop}, expected about 6 x 13 (interval {HEAT_TICK_INTERVAL} x the single-tick rate)");
}

#[test]
fn a_brick_with_heat_still_flowing_stays_awake_between_heat_ticks() {
    let (t, (mut s, _)) = (table(), lone_hot_stone());
    step(&mut s, &t, 1, 0);
    assert!(s.active_bricks() > 0, "heat is flowing after a heat tick");
    for tick in 1..HEAT_TICK_INTERVAL {
        step(&mut s, &t, 1, tick);
        assert!(s.active_bricks() > 0, "tick {tick} did no heat work but must not put the brick to sleep");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// One block of stone: only temperature can change, and it stays within the range of the block's own
    /// cells (plus the one step that rounding may overshoot by).
    #[test]
    fn a_block_of_stone_only_trades_heat_within_its_range(temps in prop::collection::vec(2000u16..6000, 8), r in any::<u32>(), a in -50i32..50) {
        let t = table();
        let mut cells: [Cell; 8] = std::array::from_fn(|i| hot(STONE, temps[i]));
        let (min, max) = (*temps.iter().min().unwrap(), *temps.iter().max().unwrap());
        // A phase counter that belongs to a heat tick (a multiple of 2 x the interval).
        block_rule(&t, &mut cells, r, r.rotate_left(5) & !7, IVec3::new(a, a, a));
        for c in cells {
            prop_assert_eq!(c.material, STONE);
            prop_assert_eq!(c.volume, VOLUME_FULL);
            prop_assert!(c.temperature + 2 >= min && c.temperature <= max + 2, "{} left {}..{}", c.temperature, min, max);
        }
    }
}
