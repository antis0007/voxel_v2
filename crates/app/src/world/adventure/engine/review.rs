//! Disposable first-person review of the production arena and combat paths.
use super::*;
pub fn install(world: &mut World, player: &mut Player) {
    let s = world.voxel_scale.meters();
    let origin = Vec3::new(0.0, 80.0, 0.0);
    let shrine = sites::boss_access::ossuary_review(s);
    let mut edits = Vec::new();
    for y in -2..=(10.0 / s) as i32 {
        for z in (10.0 / s) as i32..=(28.0 / s) as i32 {
            for x in (-13.0 / s) as i32..=(13.0 / s) as i32 {
                let cell = IVec3::new(x, (80.0 / s) as i32 + y, z);
                let local = (cell.as_vec3() + Vec3::splat(0.5)) * s - origin;
                if let Some(b) = shrine.local_block(local) {
                    edits.push((cell, b));
                }
            }
        }
    }
    world.set_many(&edits);
    let site = IVec3::new(998, 998, 0);
    let mut e = Enemy::new(site, 3, KIND, origin + Vec3::new(0.0, 1.02, 18.0), 0);
    e.yaw = std::f32::consts::PI;
    // The cutting review starts after a sighting, then places the camera behind
    // the partition. Production sensing still requires a real sighting.
    e.last_seen = origin + Vec3::new(4.0, 1.02, 24.5);
    e.alert = 4.0;
    let state = std::env::var("VOXEL_OSSUARY_STATE").unwrap_or_default();
    if matches!(state.as_str(), "cutting" | "baseline") {
        e.pos.x = 4.0;
        e.home = e.pos;
    }
    if state == "damaged" {
        e.engine.loading_damage = DRIVE_HEALTH;
    }
    if state == "disabled" {
        e.engine.loading_damage = DRIVE_HEALTH;
        e.engine.traction_damage = DRIVE_HEALTH;
    }
    world.adventure.enemies.clear();
    world.adventure.engines.clear();
    world
        .adventure
        .records
        .insert(site, reliquaries::AWAKENED | 256 | reliquaries::PHYSICAL);
    if state != "baseline" {
        world.adventure.enemies.push(e);
    }
    world.adventure.wand = 2;
    world.adventure.loadout.slots = [
        if state == "engineering" { 2 } else { 0 },
        wands::EMPTY,
        wands::EMPTY,
        wands::EMPTY,
    ];
    world.adventure.loadout.aux_slots = [wands::EMPTY; 4];
    world.adventure.health = world.adventure.max_health();
    world.adventure.hunger = 50.0;
    player.pos = (origin + Vec3::new(0.0, 1.02, 24.5)) / s;
    if matches!(state.as_str(), "cutting" | "baseline") {
        player.pos.x = 4.0 / s;
    }
    player.precise_pos = player.pos.as_dvec3();
    player.vel = Vec3::ZERO;
    player.yaw = 0.0;
    player.pitch = -0.06;
    println!(
        "[ossuary-review] state={state} scale={s} max_probes=384 max_edits=192 capacity={CUT_CAPACITY}"
    );
}
pub fn observe(world: &World) {
    let t = world.adventure.time;
    use std::sync::atomic::{AtomicU32, Ordering};
    static LAST: AtomicU32 = AtomicU32::new(u32::MAX);
    if LAST.swap((t * 2.0) as u32, Ordering::Relaxed) == (t * 2.0) as u32 {
        return;
    }
    if let Some(e) = world.adventure.enemies.iter().find(|e| e.kind == KIND) {
        println!(
            "[ossuary-review] t={t:.2} health={:.1} action={:?} phase={:?} loader={:.1} traction={:.1} jam={:.2} cut={} cargo={} stock={} pos={:?}",
            e.health,
            e.engine.run.as_ref().map(|r| r.kind),
            e.engine.run.as_ref().map(|r| r.phase()),
            e.engine.loading_damage,
            e.engine.traction_damage,
            e.engine.jam,
            e.engine.cut_cells,
            e.engine.cargo.iter().map(|v| v.count).sum::<u32>(),
            e.engine.stock,
            e.pos
        );
    } else {
        let complete = world
            .adventure
            .records
            .get(&IVec3::new(998, 998, 0))
            .is_some_and(|m| m & 8 != 0);
        println!(
            "[ossuary-review] t={t:.2} complete={complete} drills={} drops={}",
            world
                .inventory
                .count_item(Item::Block(Kind::MiningDrill.block())),
            world.inventory.dropped.len()
        );
    }
}

pub fn automatic() -> bool {
    matches!(
        std::env::var("VOXEL_OSSUARY_STATE").as_deref(),
        Ok("combat" | "engineering")
    )
}
pub fn casting(world: &World) -> bool {
    automatic()
        && world.adventure.time > 3.0
        && world.adventure.enemies.iter().any(|e| {
            e.kind == KIND
                && e.health > 0.0
                && (e.engine.loading_damage < DRIVE_HEALTH
                    || e.engine.traction_damage < DRIVE_HEALTH)
        })
}
pub fn input(world: &mut World, player: &mut Player, input: &mut crate::player::Input) {
    if !automatic() {
        return;
    }
    let Some(e) = world
        .adventure
        .enemies
        .iter()
        .find(|e| e.kind == KIND && e.health > 0.0)
    else {
        return;
    };
    let engineering = std::env::var("VOXEL_OSSUARY_STATE").as_deref() == Ok("engineering");
    let loader = e.engine.loading_damage < DRIVE_HEALTH;
    let traction = e.engine.traction_damage < DRIVE_HEALTH;
    let offset = if engineering {
        if loader {
            mechanisms::LOADER
        } else if traction {
            mechanisms::TRACTION
        } else {
            mechanisms::CONTROL
        }
    } else {
        Vec3::Y * 1.0
    };
    let target = local(e, offset);
    let scale = world.voxel_scale.meters();
    let dir = (target - player.eye(world.voxel_scale) * scale).normalize_or_zero();
    player.yaw = dir.x.atan2(-dir.z);
    player.pitch = dir.y.asin();
    let goal = if engineering {
        let outside = Vec3::new(
            offset.x.signum() * 3.5,
            0.0,
            if loader || traction { 1.7 } else { 3.2 },
        );
        local(e, outside)
    } else {
        e.pos
            + Vec3::new(
                if ((world.adventure.time / 4.0) as u32).is_multiple_of(2) {
                    -3.5
                } else {
                    3.5
                },
                0.0,
                5.0,
            )
    };
    let delta = flat(goal - player.pos * scale);
    let forward = flat(player.forward_dir()).normalize_or_zero();
    let right = Vec3::new(-forward.z, 0.0, forward.x);
    input.forward = delta.dot(forward) > 0.25;
    input.back = delta.dot(forward) < -0.25;
    input.right = delta.dot(right) > 0.25;
    input.left = delta.dot(right) < -0.25;
    if engineering && !loader && !traction {
        // Uses exactly the normal focused F interaction after walking to the control.
        reliquaries::interact(world, player);
    }
}

/// Separate from timing runs: uses the actual region writer and full decoder.
/// It reads back both an injured actor and the final one-time completion record.
pub fn checkpoint(
    world: &mut World,
    player: &mut Player,
    day: &mut crate::world::DayNightCycle,
    saves: &mut crate::world::storage::SaveService,
) {
    use std::sync::atomic::{AtomicU8, Ordering};
    static STAGE: AtomicU8 = AtomicU8::new(0);
    if std::env::var_os("VOXEL_OSSUARY_CHECKPOINT").is_none() {
        return;
    }
    let Some(root) = std::env::var_os("VOXEL_CAPTURE_DIR") else {
        return;
    };
    if std::env::var_os("VOXEL_SAVE_DIR").map(std::path::PathBuf::from)
        != Some(std::path::PathBuf::from(root).join("disposable-saves"))
    {
        return;
    }
    let site = IVec3::new(998, 998, 0);
    let stage = STAGE.load(Ordering::Relaxed);
    if stage == 0 || stage == 2 {
        let ready = if stage == 0 {
            world.adventure.enemies.iter().any(|e| {
                e.kind == KIND && (e.engine.loading_damage > 0.0 || e.engine.cut_cells > 0)
            })
        } else {
            world
                .adventure
                .records
                .get(&site)
                .is_some_and(|m| m & 8 != 0)
        };
        if ready {
            crate::world::storage::set_autosave_enabled(false);
            saves.request(true);
            STAGE.store(stage + 1, Ordering::Relaxed);
            println!("[ossuary-checkpoint] request stage={stage}");
        }
    } else if stage == 1 || stage == 3 {
        saves.tick(Some(world), player, day);
        if saves.busy() {
            return;
        }
        STAGE.store(stage + 1, Ordering::Relaxed);
        if saves.failed {
            eprintln!("[ossuary-checkpoint] save failed: {:?}", saves.status);
            return;
        }
        match crate::world::storage::load_review_checkpoint() {
            Ok(session) => {
                let a = &session.world.adventure;
                let complete = a.records.get(&site).is_some_and(|m| m & 8 != 0);
                let drills = session
                    .world
                    .inventory
                    .count_item(Item::Block(Kind::MiningDrill.block()));
                println!(
                    "[ossuary-checkpoint] loaded stage={stage} version={} complete={complete} drills={drills}",
                    crate::world::storage::SAVE_VERSION
                );
                if let Some(saved) = a.engines.get(&site) {
                    let mut e = Enemy::new(site, 3, KIND, Vec3::ZERO, 0);
                    restore(&mut e, saved);
                    println!(
                        "[ossuary-checkpoint] actor health={} loader={} traction={} stock={} cut={} cargo={} cooldown={} attack={:?}",
                        e.health,
                        e.engine.loading_damage,
                        e.engine.traction_damage,
                        e.engine.stock,
                        e.engine.cut_cells,
                        e.engine.cargo.iter().map(|v| v.count).sum::<u32>(),
                        e.cooldown,
                        e.engine.run
                    );
                }
            }
            Err(error) => eprintln!("[ossuary-checkpoint] load failed: {error:#}"),
        }
    }
}
