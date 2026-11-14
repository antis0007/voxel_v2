//! Spells cast from a wand change real matter (update pack T10 first proof).
use super::*;
use super::wands::EMPTY;
use crate::fluid::VoxelScalePreset;
use crate::world::{BlockId as B, WorldType};
use glam::IVec3;

const CRYO_NOVA: u32 = 13;
const MAGMA_GLOB: u32 = 12;
/// Pool surface, one cell deep over a stone floor, well below the caster.
const POOL_Y: i32 = 130;

fn rig() -> (World, Player, Vec3) {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 17);
    let mut p = Player::new();
    p.pos = Vec3::new(0.5, 150.0, 0.5);
    p.flying = true;
    let eye = p.eye(VoxelScalePreset::OneMeter);
    let mut cells = Vec::new();
    for x in -5..=5 {
        for z in -5..=5 {
            cells.push((IVec3::new(x, POOL_Y - 1, z), B::Stone));
            let rim = x.abs() == 5 || z.abs() == 5;
            cells.push((IVec3::new(x, POOL_Y, z), if rim { B::Stone } else { B::Water }));
        }
    }
    w.set_many(&cells);
    let a = &mut w.adventure;
    a.wand = 3;
    a.rare_spells = !0;
    a.mana = 100.0;
    a.health = 100.0;
    a.loadout.aux_slots = [EMPTY; 4];
    // Cryo nova comes with the first wand; magma glob has to be found.
    a.spell_log.add(MAGMA_GLOB);
    (w, p, eye)
}

fn pool(w: &World, block: B) -> usize {
    let mut n = 0;
    for x in -4..=4 {
        for z in -4..=4 {
            n += usize::from(w.get(IVec3::new(x, POOL_Y, z)) == block);
        }
    }
    n
}

/// Casts a one-card deck straight down and lets the shot land.
fn cast_down(w: &mut World, p: &mut Player, eye: Vec3, card: u32) -> f32 {
    let mut a = std::mem::take(&mut w.adventure);
    // The adventure tick follows the held item, and this rig holds none.
    a.wand = 3;
    a.loadout.slots = [card, EMPTY, EMPTY, EMPTY];
    a.loadout.cursor = 0;
    a.cast_cd = 0.0;
    let before = a.mana;
    a.cast_wand(w, p, eye, Vec3::NEG_Y);
    let spent = before - a.mana;
    w.adventure = a;
    for _ in 0..120 {
        tick(w, p, 1.0 / 60.0, false, false, false);
    }
    spent
}

#[test]
fn a_cast_freezes_a_pool_into_standing_ice_and_a_second_spell_melts_it() {
    let (mut w, mut p, eye) = rig();
    let water = pool(&w, B::Water);
    assert_eq!(water, 81);

    let spent = cast_down(&mut w, &mut p, eye, CRYO_NOVA);
    let ice = pool(&w, B::Ice);
    assert!(spent > 0.0, "the cast cost no mana");
    assert!(ice >= 20, "only {ice} of {water} pool cells froze");
    assert!(B::Ice.is_solid(), "frozen pool must carry a walker");
    // A body set on the ice rests on it rather than sinking.
    let mut walker = Player::new();
    walker.pos = Vec3::new(0.5, (POOL_Y + 1) as f32 + 0.6, 0.5);
    walker.precise_pos = walker.pos.as_dvec3();
    let mut input = crate::player::Input::default();
    for _ in 0..240 {
        crate::player::update_player(&mut walker, &mut input, &w, 1.0 / 120.0);
    }
    assert!(walker.on_ground && !walker.swimming, "walker at {:?} did not stand on the ice", walker.pos);
    assert!(walker.pos.y >= (POOL_Y + 1) as f32 - 0.01);

    w.adventure.mana = 100.0;
    let spent = cast_down(&mut w, &mut p, eye, MAGMA_GLOB);
    assert!(spent > 0.0);
    let left = pool(&w, B::Ice);
    assert!(left < ice, "magma left all {ice} ice cells");
}

#[test]
fn a_frost_lance_ices_water_it_crosses_and_a_spark_does_not() {
    let (mut w, mut p, _) = rig();
    let y = POOL_Y as f32 + 0.5;
    let mut a = std::mem::take(&mut w.adventure);
    // Inside the pool. The stone rim is solid, so a shot started outside it
    // never reaches the water.
    a.emit_spell(0, Vec3::new(-3.2, y, 0.5), Vec3::X, None);
    w.adventure = a;
    for _ in 0..20 {
        tick(&mut w, &mut p, 1.0 / 60.0, false, false, false);
    }
    assert_eq!(pool(&w, B::Ice), 0, "a spark iced the pool");

    let mut a = std::mem::take(&mut w.adventure);
    a.emit_spell(2, Vec3::new(-3.2, y, 0.5), Vec3::X, None);
    w.adventure = a;
    for _ in 0..50 {
        tick(&mut w, &mut p, 1.0 / 60.0, false, false, false);
    }
    assert!(pool(&w, B::Ice) >= 3, "the lance left only {} ice cells", pool(&w, B::Ice));
}

#[test]
fn an_unfound_spell_in_the_deck_does_not_fire() {
    let (mut w, mut p, eye) = rig();
    w.adventure.spell_log = super::discovery::SpellLog::starter();
    assert!(!w.adventure.spell_log.grants(MAGMA_GLOB));
    let spent = cast_down(&mut w, &mut p, eye, MAGMA_GLOB);
    assert_eq!(spent, 0.0);
}

#[test]
fn an_empty_mana_pool_changes_nothing() {
    let (mut w, mut p, eye) = rig();
    w.adventure.mana = 0.0;
    cast_down(&mut w, &mut p, eye, CRYO_NOVA);
    assert_eq!(pool(&w, B::Ice), 0, "a cast without mana froze the pool");
    assert_eq!(pool(&w, B::Water), 81);
}
