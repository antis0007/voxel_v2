//! Phase changes, burning and reactions on a synthetic chemistry: the same mechanisms the real table and
//! `assets/reactions.ron` drive, without naming a block.
use crate::test_support::*;
use crate::*;
use engine_core::material::{MaterialId, MovementClass as C, SimMaterial};
use engine_core::reaction::{Reaction, ReactionTable, SimRules};
use glam::IVec3;

const ICE: u8 = 10;
const STEAM: u8 = 11;
const BASALT: u8 = 12;
const LOG: u8 = 13;
const FIRE: u8 = 14;
const ASH: u8 = 15;

fn id(m: u8) -> MaterialId {
    MaterialId(m)
}

/// Kelvin to the cell's 0.1 K.
fn dk(kelvin: f32) -> u16 {
    (kelvin * 10.0).round() as u16
}

fn chemistry() -> SimRules {
    let mut m = materials();
    let like = |base: SimMaterial, class| SimMaterial { class, ..base };
    m[WATER as usize] =
        SimMaterial { boil: dk(373.15), boils_into: id(STEAM), cool: dk(273.15), cools_into: id(ICE), ..m[WATER as usize] };
    m[ICE as usize] = SimMaterial { melt: dk(308.15), melts_into: id(WATER), ..like(m[STONE as usize], C::Solid) };
    m[STEAM as usize] = SimMaterial { cool: dk(348.15), cools_into: id(WATER), ..m[SMOKE as usize] };
    m[BASALT as usize] = m[STONE as usize];
    m[LOG as usize] = SimMaterial { ignite: dk(533.15), burns_into: id(ASH), fuel: 20, ..like(m[FOAM as usize], C::Solid) };
    m[FIRE as usize] = like(m[SMOKE as usize], C::Plasma);
    m[ASH as usize] = like(m[SAND as usize], C::Powder);
    let mut r = ReactionTable::default();
    let steam_hot = dk(383.15);
    r.add_pair(id(LAVA), id(WATER), Reaction { a_into: id(BASALT), b_into: id(STEAM), b_temp: steam_hot, ..Reaction::ALWAYS })
        .unwrap();
    r.add_pair(id(FIRE), id(WATER), Reaction { a_into: id(AIR), b_into: id(STEAM), b_temp: steam_hot, ..Reaction::ALWAYS })
        .unwrap();
    let flame = Reaction { a_above: dk(533.15), b_into: id(FIRE), b_temp: dk(1100.0), rate: Reaction::rate_from(0.25), ..Reaction::ALWAYS };
    r.add_pair(id(LOG), id(AIR), flame).unwrap();
    r.add_unary(id(FIRE), Reaction { a_into: id(SMOKE), rate: Reaction::rate_from(0.1), ..Reaction::ALWAYS }).unwrap();
    r.add_unary(id(SMOKE), Reaction { a_into: id(AIR), rate: Reaction::rate_from(0.05), ..Reaction::ALWAYS }).unwrap();
    SimRules::new(m, r)
}

/// A stone floor at y = 0 across a 6×6 patch, so things rest on something.
fn floored() -> BrickStore {
    let mut s = BrickStore::new(200);
    fill(&mut s, IVec3::new(-3, 0, -3), IVec3::new(3, 1, 3), Cell::filled(STONE));
    s
}

fn run(s: &mut BrickStore, t: &SimRules, ticks: u32) {
    for tick in 0..ticks {
        step(s, t, 5, tick);
    }
}

#[test]
fn ice_melts_above_its_point_and_not_below() {
    let t = chemistry();
    let (mut warm, mut cold) = (floored(), floored());
    let p = IVec3::new(0, 1, 0);
    put(&mut warm, p, hot(ICE, dk(320.0)));
    put(&mut cold, p, hot(ICE, dk(300.0)));
    run(&mut warm, &t, 8);
    run(&mut cold, &t, 8);
    assert_eq!(at(&warm, p).material, WATER, "ice above 308 K became water where it stood");
    assert_eq!(at(&cold, p).material, ICE);
}

/// One heat phase on a single block: `subject` in cell 0 among stone at the same temperature, so nothing
/// moves and nothing conducts; only the reaction pass acts.
fn in_block(t: &SimRules, subject: Cell) -> Cell {
    let mut cells = [Cell { temperature: subject.temperature, ..Cell::filled(STONE) }; 8];
    cells[0] = subject;
    block_rule(t, &mut cells, 5, 0, IVec3::ZERO);
    cells[0]
}

#[test]
fn water_boils_and_steam_condenses_only_past_the_hysteresis() {
    let t = chemistry();
    for (start, kelvin, becomes) in
        [(WATER, 374.0, STEAM), (WATER, 372.0, WATER), (STEAM, 350.0, STEAM), (STEAM, 347.0, WATER), (WATER, 273.0, ICE)]
    {
        assert_eq!(in_block(&t, hot(start, dk(kelvin))).material, becomes, "{start} at {kelvin} K");
    }
}

#[test]
fn a_phase_change_keeps_volume_and_temperature() {
    let t = chemistry();
    let c = in_block(&t, Cell { volume: 40, ..hot(ICE, dk(320.0)) });
    assert_eq!((c.material, c.volume, c.temperature), (WATER, 40, dk(320.0)));
}

#[test]
fn nothing_reacts_between_heat_ticks() {
    let t = chemistry();
    let mut cells = [Cell { temperature: dk(400.0), ..Cell::filled(STONE) }; 8];
    cells[0] = hot(WATER, dk(400.0));
    block_rule(&t, &mut cells, 5, 2, IVec3::ZERO); // tick 1: not a heat tick
    assert_eq!(cells[0].material, WATER);
}

#[test]
fn lava_meeting_water_leaves_basalt_and_hot_steam() {
    let t = chemistry();
    let mut s = floored();
    let (lava, water) = (IVec3::new(0, 1, 0), IVec3::new(1, 1, 0));
    put(&mut s, lava, hot(LAVA, dk(1400.0)));
    put(&mut s, water, Cell::filled(WATER));
    run(&mut s, &t, 4);
    assert_eq!(at(&s, lava).material, BASALT, "the basalt stays where the lava was");
    // The reaction itself, on one block: lava in cell 0 touching water in cell 1.
    let mut cells = [Cell::filled(STONE); 8];
    (cells[0], cells[1]) = (hot(LAVA, dk(1400.0)), Cell::filled(WATER));
    block_rule(&t, &mut cells, 5, 0, IVec3::ZERO);
    assert_eq!((cells[0].material, cells[1].material), (BASALT, STEAM));
    assert_eq!(cells[1].temperature, dk(383.15), "steam is born above its condensing point");
    assert!(cells[0].temperature > dk(1000.0), "the basalt keeps the lava's heat");
}

#[test]
fn a_cold_burnable_world_sleeps() {
    let t = chemistry();
    let mut s = floored();
    fill(&mut s, IVec3::new(-2, 1, -2), IVec3::new(2, 3, 2), Cell::filled(LOG));
    run(&mut s, &t, 8);
    assert_eq!(s.active_bricks(), 0, "logs at room temperature next to air cost nothing");
}

#[test]
fn a_burning_log_lights_the_air_burns_to_ash_and_the_fire_goes_out() {
    let t = chemistry();
    let mut s = floored();
    let p = IVec3::new(0, 1, 0);
    put(&mut s, p, hot(LOG, dk(700.0)));
    let mut saw_fire = false;
    let mut tick = 0;
    while tick < 4000 && (tick < 8 || s.active_bricks() > 0) {
        step(&mut s, &t, 5, tick);
        saw_fire |= (-3..3).any(|x| (1..4).any(|y| (-3..3).any(|z| at(&s, IVec3::new(x, y, z)).material == FIRE)));
        tick += 1;
    }
    assert!(saw_fire, "the hot log set the air next to it alight");
    assert!(s.active_bricks() == 0, "everything settled within 4000 ticks");
    assert_eq!(at(&s, p).material, ASH, "the log burned down to ash");
    let left: Vec<u8> = (-3..3)
        .flat_map(|x| (1..40).flat_map(move |y| (-3..3).map(move |z| IVec3::new(x, y, z))))
        .map(|q| at(&s, q).material)
        .filter(|&m| m == FIRE || m == SMOKE)
        .collect();
    assert!(left.is_empty(), "no fire or smoke outlives the burn: {left:?}");
}

#[test]
fn fire_meeting_water_is_quenched() {
    let t = chemistry();
    let mut cells = [Cell::filled(STONE); 8];
    (cells[0], cells[1]) = (hot(FIRE, dk(1100.0)), Cell::filled(WATER));
    block_rule(&t, &mut cells, 5, 0, IVec3::ZERO);
    assert_eq!(cells[0], Cell::EMPTY, "the flame is gone");
    assert_eq!((cells[1].material, cells[1].temperature), (STEAM, dk(383.15)), "the water it touched flashed to steam");
}

/// Known limit, measured (docs/PLAN2.md A4 actuals): steam born at 383 K in open air condenses again
/// within the same tick, because a gas cell sheds half its difference to empty space per exposed edge
/// on every heat phase (the A3 environment sink). Pinned here so a change to that model is noticed.
#[test]
fn steam_in_open_air_condenses_within_a_tick() {
    let t = chemistry();
    let mut s = floored();
    put(&mut s, IVec3::new(0, 1, 0), hot(STEAM, dk(383.15)));
    step(&mut s, &t, 5, 0);
    let steam = (-3..3).any(|x| (1..8).any(|y| (-3..3).any(|z| at(&s, IVec3::new(x, y, z)).material == STEAM)));
    assert!(!steam, "if steam now survives a tick in open air, update the A4 note and this test");
}

#[test]
fn reactions_are_reproducible_from_the_seed() {
    let t = chemistry();
    let world = |seed| {
        let mut s = floored();
        fill(&mut s, IVec3::new(-2, 1, -2), IVec3::new(2, 2, 2), hot(LOG, dk(700.0)));
        for tick in 0..60 {
            step(&mut s, &t, seed, tick);
        }
        (-3..3)
            .flat_map(|x| (1..12).flat_map(move |y| (-3..3).map(move |z| IVec3::new(x, y, z))))
            .map(|q| at(&s, q))
            .collect::<Vec<_>>()
    };
    assert_eq!(world(9), world(9));
    assert_ne!(world(9), world(10), "the dice come from the seed");
}

#[test]
fn single_cell_chances_are_rolled_once_per_heat_tick() {
    let t = chemistry();
    let mut s = BrickStore::new(400);
    fill(&mut s, IVec3::new(-16, 0, -16), IVec3::new(16, 1, 16), Cell::filled(STONE));
    fill(&mut s, IVec3::new(-16, 1, -16), IVec3::new(16, 2, 16), hot(FIRE, dk(1100.0)));
    step(&mut s, &t, 3, 0);
    let out = (-16..16).flat_map(|x| (-16..16).map(move |z| IVec3::new(x, 1, z))).filter(|&p| at(&s, p).material != FIRE).count();
    // 1024 flames at a rate of 0.1: about 102 go out; rolled on both phases of the tick it would be about 195.
    assert!((70..140).contains(&out), "{out} of 1024 flames went out in one heat tick");
}
