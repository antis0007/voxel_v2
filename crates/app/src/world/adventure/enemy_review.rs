//! Opt-in disposable native review arena. Never enabled by ordinary gameplay.
use super::{Enemy, World};
use crate::{player::Player, world::BlockId};
use glam::{IVec3, Vec3};
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};
static ARMED: AtomicBool = AtomicBool::new(false);

pub fn arm() {
    if mode() == "combat" && !ARMED.swap(true, Ordering::Relaxed) {
        println!("[enemy-review] actual AI armed at capture measurement start");
    }
}

fn scenario() -> &'static str {
    static SCENE: OnceLock<String> = OnceLock::new();
    SCENE.get_or_init(|| std::env::var("VOXEL_ENEMY_REVIEW_SCENE").unwrap_or_default())
}
pub fn automated() -> bool {
    if super::super::habitats::review::enabled() {return true;}
    matches!(mode(), "duel" | "dodge" | "progression")
}
pub fn manual() -> bool {
    mode() == "manual"
}

fn mode() -> &'static str {
    static MODE: OnceLock<String> = OnceLock::new();
    MODE.get_or_init(|| std::env::var("VOXEL_REVIEW_ENEMIES").unwrap_or_default())
}
fn single() -> Option<u32> {
    static KIND: OnceLock<Option<u32>> = OnceLock::new();
    *KIND.get_or_init(|| {
        std::env::var("VOXEL_ENEMY_REVIEW_KIND")
            .ok()
            .and_then(|v| v.parse().ok())
    })
}
pub fn install(world: &mut World, player: &mut Player) {
    if super::super::habitats::review::enabled() {return super::super::habitats::review::install(world,player);}
    if mode().is_empty() {
        return;
    }
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::Relaxed) {
        return;
    }
    let scale = world.voxel_scale.meters();
    let mut edits = Vec::new();
    for x in -10..=10 {
        for z in -8..=9 {
            for y in 79..=85 {
                let obstacle =
                    scenario() == "obstacle" && x == 0 && (1..=5).contains(&z) && y <= 82;
                let block = if y == 79 || (z == -8 && y < 84) || obstacle {
                    BlockId::StoneBricks
                } else {
                    BlockId::Air
                };
                let a = (Vec3::new(x as f32, y as f32, z as f32) / scale)
                    .floor()
                    .as_ivec3();
                let n = scale.recip().round() as i32;
                for dx in 0..n {
                    for dy in 0..n {
                        for dz in 0..n {
                            edits.push((a + IVec3::new(dx, dy, dz), block));
                        }
                    }
                }
            }
        }
    }
    world.set_many(&edits);
    world.adventure.enemies.clear();
    let kinds: Vec<u32> = if scenario() == "wildlife" {
        vec![super::wildlife::SHEEP, super::wildlife::GOAT]
    } else if scenario() == "pack" {
        vec![12, 0, 11]
    } else if scenario() == "thunderhen" {
        vec![super::thunderhen::KIND]
    } else {
        single().map_or_else(|| vec![5, 3, 2, 9], |kind| vec![kind])
    };
    let count = kinds.len();
    for (i, kind) in kinds.into_iter().enumerate() {
        let p = Vec3::new(
            (i as f32 - (count - 1) as f32 * 0.5) * 2.4,
            if kind == 2 { 80.9 } else { 80.02 },
            0.0,
        );
        let site = if mode() == "progression" {
            IVec3::new(991, 80, 2)
        } else {
            IVec3::new(0, 80, 0)
        };
        let mut e = Enemy::new(site, i as u32, kind, p, 0);
        if std::env::var_os("VOXEL_ENEMY_REVIEW_ENRAGED").is_some() {
            e.health *= 0.45;
        }
        e.yaw = std::f32::consts::PI;
        e.cooldown = 1.0 + i as f32 * 0.35;
        world.adventure.enemies.push(e);
    }
    world.adventure.wand = 2;
    if mode() == "progression" {
        world.adventure.spell_log = super::discovery::SpellLog::starter();
        world.adventure.discoveries_ready = true;
        world.adventure.records.clear();
    }
    if automated() {
        world.adventure.respawn_point = Some(Vec3::new(0.0, 80.12, 7.0));
    }
    camera(player, scale);
    if let Some(enemy) = world.adventure.enemies.first() {
        let direction =
            (enemy.center() - player.eye(world.voxel_scale) * scale).normalize_or_zero();
        player.yaw = direction.x.atan2(-direction.z);
        player.pitch = direction.y.asin();
    }
    println!(
        "[enemy-review] disposable arena mode={} kind={:?}; gameplay damage/windows unchanged",
        mode(),
        single()
    );
}
pub(super) fn after(world: &mut World, player: &Player) {
    if scenario() == "wildlife" { super::wildlife::observe_review(world); }
    if super::super::habitats::review::enabled() {return super::super::habitats::review::observe(world,player);}
    if mode().is_empty() {
        return;
    }
    if mode() == "combat" && !ARMED.load(Ordering::Relaxed) {
        for e in &mut world.adventure.enemies {
            e.pos = e.home;
            e.tell = 0.0;
            e.recovery = 0.0;
            e.cooldown = 0.5;
        }
        world.adventure.projectiles.clear();
        world.adventure.hazards.clear();
        return;
    }
    if mode() == "poses" || mode() == "materials" {
        let t = if mode() == "materials" { 0.0 } else { world.adventure.time };
        let cycle = t % 4.0;
        for e in &mut world.adventure.enemies {
            e.pos = e.home;
            e.yaw = std::f32::consts::PI;
            e.cooldown = 999.0;
            e.gait = if cycle < 1.0 { 1.0 } else { 0.0 };
            e.phase = t * 0.6;
            e.tell = if (1.0..2.0).contains(&cycle) {
                e.windup() * (2.0 - cycle)
            } else {
                0.0
            };
            e.recovery = if (2.0..2.24).contains(&cycle) {
                2.24 - cycle
            } else {
                0.0
            };
            e.lunge = Vec3::Z;
        }
        world.adventure.projectiles.clear();
        world.adventure.hazards.clear();
        if mode() == "materials" {
            // Hold an ordinary mining-tool windup beside live creature models.
            // This exercises the same model batch that previously inherited turf.
            world.adventure.weapon = 0;
            world.adventure.combo = 0;
            world.adventure.melee_cd = world.adventure.swing_duration() * 0.78;
        }
    }
    if automated() || manual() {
        observe(world, player);
        return;
    }
    static PHASE: std::sync::Mutex<(usize, usize, u32)> = std::sync::Mutex::new((0, 0, 0));
    let telling = world
        .adventure
        .enemies
        .iter()
        .filter(|e| e.tell > 0.0)
        .count();
    let recovering = world
        .adventure
        .enemies
        .iter()
        .filter(|e| e.recovery > 0.0)
        .count();
    let health = world.adventure.health;
    if let Ok(mut last) = PHASE.lock() {
        let state = (telling, recovering, health.to_bits());
        if last.0 != telling
            || last.1 != recovering
            || health < f32::from_bits(last.2) - 0.25
            || last.2 == 0
        {
            println!(
                "[enemy-review] t={:.2} windups={} recoveries={} player_hp={:.1} projectiles={}",
                world.adventure.time,
                telling,
                recovering,
                health,
                world.adventure.projectiles.len()
            );
            *last = state;
        }
    }
}
pub fn camera(player: &mut Player, scale: f32) -> bool {
    if super::super::habitats::review::enabled() {return super::super::habitats::review::camera(player,scale);}
    if mode().is_empty() {
        return false;
    }
    if automated() || manual() {
        static PLACED: AtomicBool = AtomicBool::new(false);
        if PLACED.swap(true, Ordering::Relaxed) {
            return false;
        }
    }
    let z = if automated() || manual() {
        7.0
    } else if single().is_some() {
        3.8
    } else {
        6.6
    };
    player.pos = Vec3::new(0.0, 80.12, z) / scale;
    player.yaw = 0.0;
    player.pitch = -0.12;
    true
}

/// Script ordinary input through update_player, preserving collision and movement physics.
pub fn input(world: &World, player: &mut Player, input: &mut crate::player::Input) {
    if super::super::habitats::review::enabled() {return super::super::habitats::review::input(world,player,input);}
    if !automated() {
        return;
    }
    let Some(enemy) = world.adventure.enemies.iter().find(|e| e.health > 0.0) else {
        return;
    };
    let scale = world.voxel_scale.meters();
    let direction = (enemy.center() - player.eye(world.voxel_scale) * scale).normalize_or_zero();
    player.yaw = direction.x.atan2(-direction.z);
    player.pitch = direction.y.asin();
    let t = world.adventure.time;
    let target = if scenario() == "obstacle" {
        Vec3::new(0.0, 80.0, 7.0)
    } else if matches!(scenario(), "ambush" | "cache") || mode() == "progression" {
        if t < 4.5 {
            Vec3::new(0.0, 80.0, 7.0)
        } else if t < 8.0 {
            Vec3::new(0.0, 80.0, 2.8)
        } else {
            Vec3::new(3.5, 80.0, 5.5)
        }
    } else if scenario() == "camp" {
        Vec3::new(0.0, 80.0, 3.8)
    } else {
        Vec3::new((t * 0.65).sin() * 5.5, 80.0, 4.5 + (t * 0.65).cos() * 1.5)
    };
    let delta = (target - player.pos * scale) * Vec3::new(1.0, 0.0, 1.0);
    let forward = player.flat_forward();
    let right = Vec3::new(-forward.z, 0.0, forward.x);
    input.forward = delta.dot(forward) > 0.2;
    input.back = delta.dot(forward) < -0.2;
    input.right = delta.dot(right) > 0.2;
    input.left = delta.dot(right) < -0.2;
}
pub fn casting(world: &World) -> bool {
    if super::super::habitats::review::enabled() {return super::super::habitats::review::casting(world);}
    matches!(mode(), "duel" | "progression")
        && world.adventure.time > 3.0
        && world.adventure.enemies.iter().any(|e| e.health > 0.0)
}

fn observe(world: &mut World, player: &Player) {
    static TICK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);
    let a = &world.adventure;
    let tick = (a.time * 4.0) as u32;
    if TICK.swap(tick, Ordering::Relaxed) == tick {
        return;
    }
    let feet = player.pos * world.voxel_scale.meters();
    println!(
        "[review-player] t={:.2} pos={:.2},{:.2},{:.2} hp={:.1} mana={:.1}",
        a.time, feet.x, feet.y, feet.z, a.health, a.mana
    );
    for e in &a.enemies {
        println!(
            "[encounter-live] t={:.2} kind={} pos={:.2},{:.2},{:.2} hp={:.1} hidden={} emerging={:.2} tell={:.2} attack={} exposed={:.2} phase={} visible={} player_hp={:.1} shots={} hazards={}",
            a.time,
            e.kind,
            e.pos.x,
            e.pos.y,
            e.pos.z,
            e.health,
            e.encounter.concealed,
            e.encounter.emerge,
            e.tell,
            e.attack,
            e.encounter.exposed,
            e.encounter.boss_phase,
            e.visible,
            a.health,
            a.projectiles.len(),
            a.hazards.len()
        );
    }
    if mode() == "progression" && a.enemies.is_empty() && a.records.values().any(|n| n & 256 != 0) {
        static CHECKED: AtomicBool = AtomicBool::new(false);
        if CHECKED.swap(true, Ordering::Relaxed) {
            return;
        }
        let mut encoded = Vec::new();
        a.encode(&mut encoded);
        let mut words = encoded.chunks_exact(4);
        let decoded = super::Adventure::decode(
            &mut || {
                words
                    .next()
                    .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
                    .ok_or_else(|| anyhow::anyhow!("truncated review save"))
            },
            true,
            true,
        );
        println!(
            "[progression-live] records={:?} spell_words={:?} resources={:?} save_roundtrip={}",
            a.records,
            a.spell_log.words(),
            world.expedition.resources,
            decoded.as_ref().is_ok_and(|v| v.records == a.records)
        );
        let before = world.expedition.resources;
        let id = IVec3::new(991, 80, 2);
        let mut owned = std::mem::take(&mut world.adventure);
        owned.complete_site(world, id, Vec3::new(0.0, 80.0, 0.0));
        world.adventure = owned;
        println!(
            "[progression-live] duplicate_clear_awarded={}",
            before != world.expedition.resources
        );
    }
}

/// Exercise the real on-disk save service after a gameplay-earned room clear.
pub fn checkpoint(
    world: &mut World,
    player: &mut Player,
    day: &mut crate::world::DayNightCycle,
    saves: &mut crate::world::storage::SaveService,
) {
    if super::super::habitats::review::enabled() {return super::super::habitats::review::checkpoint(world,player,day,saves);}
    use std::sync::atomic::AtomicU8;
    static STAGE: AtomicU8 = AtomicU8::new(0);
    if mode() != "progression" {
        return;
    }
    let Some(root) = std::env::var_os("VOXEL_CAPTURE_DIR") else {
        return;
    };
    let expected = std::path::PathBuf::from(root).join("disposable-saves");
    if std::env::var_os("VOXEL_SAVE_DIR").map(std::path::PathBuf::from) != Some(expected) {
        return;
    }
    match STAGE.load(Ordering::Relaxed) {
        0 if world.adventure.records.values().any(|n| n & 256 != 0) => {
            crate::world::storage::set_autosave_enabled(false);
            saves.request(true);
            STAGE.store(1, Ordering::Relaxed);
        }
        1 => {
            saves.tick(Some(world), player, day);
            if saves.busy() {
                return;
            }
            STAGE.store(2, Ordering::Relaxed);
            if saves.failed {
                eprintln!("[progression-disk] save_failed={:?}", saves.status);
                return;
            }
            match crate::world::storage::load_review_checkpoint() {
                Ok(mut loaded) => {
                    let records = loaded.world.adventure.records == world.adventure.records;
                    let spells = loaded.world.adventure.spell_log.words()
                        == world.adventure.spell_log.words();
                    let materials = loaded.world.expedition.resources == world.expedition.resources;
                    let before = loaded.world.expedition.resources;
                    let mut owned = std::mem::take(&mut loaded.world.adventure);
                    owned.complete_site(
                        &mut loaded.world,
                        IVec3::new(991, 80, 2),
                        Vec3::new(0.0, 80.0, 0.0),
                    );
                    println!(
                        "[progression-disk] version={} records_equal={} spells_equal={} materials_equal={} duplicate_after_reload={}",
                        crate::world::storage::SAVE_VERSION,
                        records,
                        spells,
                        materials,
                        before != loaded.world.expedition.resources
                    );
                }
                Err(error) => eprintln!("[progression-disk] load_failed={error:#}"),
            }
        }
        _ => {}
    }
}
