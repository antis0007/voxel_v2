//! Check saved worker restoration without changing a gameplay checkpoint.
use super::*;
use anyhow::{Context, Result, ensure};
use crate::world::farming::{jobs, sidecar};
use std::path::Path;

pub(crate) fn check(root: &Path) -> Result<()> {
    let mut w = World::with_seed_in(crate::world::WorldType::Superflat,
        engine_core::VoxelScalePreset::OneMeter, 424242, &root.join("worker-reload-fixture"));
    let mut cells = Vec::new();
    for x in -3..=7 { for z in -1..=6 { for y in 17..=23 {
        cells.push((IVec3::new(x, y, z), if y == 17 { BlockId::Stone } else { BlockId::Air }));
    }}}
    w.set_many(&cells);
    let id = 31;
    let feet = Vec3::from([0.499_753_03, 18.815_29, 2.424_202_7]);
    let home = Vec3::new(4.5, 18.02, 2.5);
    ensure!(cave::loaded(&w, feet) && free(&w, feet, VILLAGER)
        && !resident_footing(&w, feet, VILLAGER, 0.2), "Airborne worker fixture is invalid");
    ensure!(cave::loaded(&w, home) && free(&w, home, VILLAGER) && supported(&w, home, 0.2),
        "Worker fallback home fixture is invalid");
    spawn_resident(&mut w, id, feet);
    jobs::assign(&mut w, id);
    w.travel.homes.insert(id, home);
    let saved = sidecar::encode(&w);
    w.farming = sidecar::decode(&saved)?.0;
    ensure!(jobs::crew::saved_workers(&w) == [(id, feet)], "Saved worker pose changed in serialization");
    w.adventure.enemies.clear();
    w.farming.clock = 120;
    service(&mut w, feet);
    let actor = villager_actor(&w, id).context("Airborne saved worker was not restored")?;
    ensure!(actor.pos == feet && actor.kind == VILLAGER && actor.site == IVec3::new(id as i32, 0, VILLAGER_Z),
        "Saved worker identity, kind or first pose changed");
    service(&mut w, feet);
    ensure!(w.adventure.enemies.iter().filter(|e| villager_id(e) == Some(id)).count() == 1,
        "Repeated restoration duplicated the worker");
    ensure!(villager_feet(&w, id) == Some(feet), "Repeated restoration moved the worker");

    let index = w.adventure.enemies.iter().position(|e| villager_id(e) == Some(id))
        .context("Restored worker disappeared")?;
    let mut actor = w.adventure.enemies.remove(index);
    navigation::gravity(&mut actor, &w, 1.0 / 120.0);
    ensure!(actor.pos.y < feet.y && actor.pos.distance(feet) < 0.025,
        "Restored worker did not resume ordinary gravity");
    for _ in 0..120 {
        navigation::gravity(&mut actor, &w, 1.0 / 120.0);
        ensure!(free(&w, actor.pos, actor.kind), "Restored worker fell through a solid body");
    }
    ensure!(resident_footing(&w, actor.pos, actor.kind, 0.2), "Restored worker did not land");

    let obstacle = feet.floor().as_ivec3();
    w.set(obstacle, BlockId::Stone);
    ensure!(!free(&w, feet, VILLAGER), "Worker obstruction fixture is invalid");
    service(&mut w, feet);
    ensure!(villager_actor(&w, id).is_none(), "Obstructed worker was spawned or moved to its home");
    w.set(obstacle, BlockId::Air);
    let coord = chunk_coord_from_world_block(obstacle);
    let chunk = w.chunks.get(&coord).cloned().context("Missing worker fixture chunk")?;
    w.chunks.remove(&coord);
    ensure!(!cave::loaded(&w, feet), "Worker unloaded fixture is invalid");
    service(&mut w, feet);
    ensure!(villager_actor(&w, id).is_none(), "Worker spawned in an unloaded neighborhood");
    ensure!(!w.chunks.contains_key(&coord), "Restoration loaded an unknown chunk");
    w.chunks.insert(coord, chunk);
    service(&mut w, feet);
    ensure!(villager_feet(&w, id) == Some(feet), "Deferred worker lost its saved pose");
    ensure!(jobs::crew::saved_workers(&w) == [(id, feet)], "Restoration rewrote the saved worker");

    jobs::assign(&mut w, id);
    ensure!(jobs::crew::saved_workers(&w).is_empty(), "Worker fixture was not released");
    w.adventure.enemies.clear();
    w.travel.homes.insert(id, feet);
    w.travel.guard_posts.insert(id + 1, feet);
    service(&mut w, feet);
    ensure!(villager_actor(&w, id).is_none() && villager_actor(&w, id + 1).is_none(),
        "New residents or guards spawned without support");
    w.travel.homes.insert(id, home);
    w.travel.guard_posts.insert(id + 1, home);
    service(&mut w, feet);
    ensure!(villager_feet(&w, id) == Some(home) && villager_feet(&w, id + 1) == Some(home),
        "Supported residents or guards failed to spawn");
    std::fs::write(root.join("worker-reload-regression.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "passed": true, "resident": id, "saved_feet": feet.to_array(), "first_pose_error_m": 0.0,
        "kind": VILLAGER, "duplicate_actors": 0, "gravity_landed": true,
        "obstructed_pose_deferred": true, "unloaded_pose_deferred": true,
        "new_resident_support_preserved": true, "guard_support_preserved": true
    }))?)?;
    println!("[worker-reload-check] PASS airborne pose, identity, collision, loaded chunks and duplicate prevention");
    Ok(())
}
