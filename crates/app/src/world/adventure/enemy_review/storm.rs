//! Disposable benchmark fixture. Casts use the production evaluator and the
//! normal projectile simulation. The receipt separates discovery from release.
use super::*;
use crate::world::progression::{sidecar, study::CONDUCTION_OBSERVED};
use super::super::{spells::STORM_SURGE, wands::{EMPTY, Loadout}};
use std::sync::atomic::AtomicU8;

static PHASE: AtomicU8 = AtomicU8::new(0);
const ORIGIN: Vec3 = Vec3::new(0.5, 81.5, 4.5);
const AIM: Vec3 = Vec3::new(0.5, 79.5, -0.5);

pub(super) fn enabled() -> bool {
    super::scenario() == "storm-surge" && crate::parse_benchmark_config().enabled
}

pub(super) fn install(world: &mut World) {
    if world.voxel_scale.meters() != 1.0 {
        println!("[storm-surge-review] requires 1 m voxels");
        return;
    }
    world.rules.mode = crate::world::rules::GameMode::Survival;
    world.adventure.creative = false;
    world.adventure.begin_journey();
    world.adventure.spell_log.add(14);
    let mut edits = Vec::new();
    for x in -4..=7 {
        for z in -10..=7 {
            edits.push((IVec3::new(x, 78, z), BlockId::StoneBricks));
            let connected = (-1..=1).contains(&x) && (-9..=0).contains(&z);
            let isolated = (4..=5).contains(&x) && (-3..=0).contains(&z);
            edits.push((IVec3::new(x, 79, z), if connected || isolated { BlockId::Water } else { BlockId::StoneBricks }));
            for y in 80..=83 {
                edits.push((IVec3::new(x, y, z), BlockId::Air));
            }
        }
    }
    world.set_many(&edits);
    PHASE.store(1, Ordering::Relaxed);
}

pub(super) fn camera(player: &mut Player, scale: f32) -> bool {
    player.pos = Vec3::new(0.5, 80.02, 4.5) / scale;
    player.yaw = 0.0;
    player.pitch = -0.32;
    true
}

fn cast(world: &mut World, player: &mut Player, spell: u32) -> bool {
    let mut adventure = std::mem::take(&mut world.adventure);
    adventure.wand = 3;
    adventure.loadout = Loadout { slots: [spell, EMPTY, EMPTY, EMPTY], aux_slots: [EMPTY; 4], ..Loadout::default() };
    adventure.mana = adventure.max_mana();
    let mana = adventure.mana;
    adventure.cast_wand(world, player, ORIGIN, (AIM - ORIGIN).normalize());
    let emitted = adventure.projectiles.iter().any(|projectile| projectile.spell == spell);
    println!("[storm-surge-review] spell={spell} emitted={emitted} mana_spent={:.2}", mana - adventure.mana);
    world.adventure = adventure;
    emitted
}

pub(super) fn after(world: &mut World, player: &mut Player) {
    let learned = world.progression.lessons & CONDUCTION_OBSERVED != 0;
    match PHASE.load(Ordering::Relaxed) {
        1 if world.adventure.time >= 1.0 => {
            let hidden = !world.adventure.effective_spell_log().has(STORM_SURGE);
            println!("[storm-surge-review] initially_hidden={hidden}");
            let emitted = hidden && cast(world, player, 14);
            PHASE.store(if emitted { 2 } else { 5 }, Ordering::Relaxed);
        }
        2 if learned && world.adventure.time >= 2.0 => {
            let bytes = sidecar::encode(&world.progression, &world.inventory);
            let persisted = sidecar::decode(&bytes).is_ok_and(|state| state.lessons & CONDUCTION_OBSERVED != 0);
            println!("[storm-surge-review] observed={learned} checkpoint_roundtrip={persisted}");
            let emitted = persisted && cast(world, player, STORM_SURGE);
            PHASE.store(if emitted { 3 } else { 5 }, Ordering::Relaxed);
        }
        3 if world.adventure.arcs.len() > 12 => {
            let confined = world.adventure.arcs.iter().all(|arc| arc.b.x < 3.0);
            println!("[storm-surge-review] surge_arcs={} confined={confined}", world.adventure.arcs.len());
            if confined { println!("[storm-surge-review] PASS"); }
            PHASE.store(5, Ordering::Relaxed);
        }
        2 | 3 if world.adventure.time >= 5.0 => {
            println!("[storm-surge-review] timed out phase={}", PHASE.load(Ordering::Relaxed));
            PHASE.store(5, Ordering::Relaxed);
        }
        _ => {}
    }
}
