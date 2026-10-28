//! Small collision shapes shared by attacks, component targeting and presentation.
use super::*;
use crate::world::BlockId as B;
use crate::world::inventory::DroppedStack;
pub(super) const DRIVE_HEALTH: f32 = 60.0;
pub(super) const CUT_CAPACITY: u32 = 4096;
pub(super) const LOADER: Vec3 = Vec3::new(1.05, 1.25, 0.15);
pub(super) const TRACTION: Vec3 = Vec3::new(-0.95, 0.65, 0.35);
pub(super) const CONTROL: Vec3 = Vec3::new(0.0, 1.25, 1.25);

fn component(e: &Enemy, at: Vec3) -> Option<bool> {
    if local(e, LOADER).distance(at) < 0.48 {
        Some(true)
    } else if local(e, TRACTION).distance(at) < 0.48 {
        Some(false)
    } else {
        None
    }
}
pub(in crate::world::adventure) fn hit(e: &Enemy, at: Vec3, padding: f32) -> bool {
    let p = Quat::from_rotation_y(-e.yaw) * (at - e.pos);
    (p.x.abs() < 0.9 + padding
        && p.z.abs() < 1.0 + padding
        && p.y > -padding
        && p.y < 1.65 + padding)
        || [LOADER, TRACTION, CONTROL]
            .into_iter()
            .any(|o| local(e, o).distance(at) < 0.48 + padding)
}
/// Pick the nearest exposed drive near the melee ray, otherwise the hull.
pub(in crate::world::adventure) fn melee_point(e: &Enemy, eye: Vec3, ray: Vec3) -> Vec3 {
    [LOADER, TRACTION]
        .into_iter()
        .map(|o| local(e, o))
        .filter(|p| {
            let d = *p - eye;
            let t = d.dot(ray);
            t > 0.0 && t < MELEE_REACH + ARMED_REACH_BONUS && (d - ray * t).length() < 0.45
        })
        .min_by(|a, b| a.distance_squared(eye).total_cmp(&b.distance_squared(eye)))
        .unwrap_or(e.center())
}
pub(in crate::world::adventure) fn component_hit(
    e: &mut Enemy,
    at: Vec3,
    damage: f32,
    frost: bool,
) {
    let Some(loader) = component(e, at) else {
        return;
    };
    let hp = if loader {
        &mut e.engine.loading_damage
    } else {
        &mut e.engine.traction_damage
    };
    *hp = (*hp + damage.max(0.0)).min(DRIVE_HEALTH);
    if frost {
        e.engine.jam = e.engine.jam.max(4.0);
    }
    // A disabling hit cancels the operation immediately; no extra contact next tick.
    if *hp >= DRIVE_HEALTH {
        if let Some(r) = &mut e.engine.run {
            r.interrupt();
        }
        e.tell = 0.0;
    }
}
fn cuttable(b: B) -> bool {
    matches!(
        b,
        B::Stone
            | B::Limestone
            | B::WeatheredBricks
            | B::Dirt
            | B::Grass
            | B::Sand
            | B::Gravel
            | B::Clay
            | B::Planks
            | B::Log
            | B::Cobblestone
    )
}
/// At most 384 sampled cells and 192 admitted writes per tick. Only the front
/// face is excavated; hard construction and factory assemblies stop the carriage.
pub(super) fn cut_advance(e: &mut Enemy, world: &mut World, dt: f32, dirty: &mut Vec<IVec3>) {
    if e.engine.cut_cells >= CUT_CAPACITY || dirty.len() > 1856 {
        return;
    }
    let s = world.voxel_scale.meters();
    let forward = e.lunge;
    let right = Vec3::new(-forward.z, 0.0, forward.x);
    let front = e.pos + forward * 1.15;
    let mut edits = Vec::new();
    let nx = (2.4 / s).ceil() as usize;
    let ny = (1.9 / s).ceil() as usize;
    // A deterministic cursor makes fine-scale faces finish over multiple ticks.
    let total = nx * ny;
    let start = e.attack as usize % total;
    for i in 0..total.min(384) {
        let k = (start + i) % total;
        let p = front
            + right * ((k % nx) as f32 * s - 1.2 + s * 0.5)
            + Vec3::Y * ((k / nx) as f32 * s + s * 0.5);
        let cell = (p / s).floor().as_ivec3();
        let b = world.get(cell);
        if !cuttable(b) || edits.iter().any(|(q, _)| *q == cell) {
            continue;
        }
        if e.engine.loading_damage >= DRIVE_HEALTH && world.inventory.dropped.len() >= 1000 {
            break;
        }
        if edits.len() >= 192 || e.engine.cut_cells + edits.len() as u32 >= CUT_CAPACITY {
            break;
        }
        edits.push((cell, b));
    }
    e.attack = e.attack.wrapping_add(384.min(total) as u32);
    let started = std::time::Instant::now();
    let mut applied = Vec::new();
    world.set_many_committed(
        &edits.iter().map(|(p, _)| (*p, B::Air)).collect::<Vec<_>>(),
        |p, old, _| applied.push((p, old)),
    );
    if !edits.is_empty() && std::env::var_os("VOXEL_OSSUARY_STATE").is_some() {
        println!(
            "[ossuary-cut] requested={} applied={} edit_ms={:.3}",
            edits.len(),
            applied.len(),
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
    for (p, b) in applied {
        e.engine.cut_cells += 1;
        dirty.push(p);
        if let Some(stack) = e.engine.cargo.iter_mut().find(|v| v.item == Item::Block(b)) {
            stack.count += 1;
        } else {
            e.engine.cargo.push(ItemStack::new(Item::Block(b), 1));
        }
    }
    if e.engine.loading_damage >= DRIVE_HEALTH {
        release_cargo(e, world);
    }
    // Sweep with the same body collision as other motion; no tunnel teleport.
    move_ground(e, world, forward, (dt * 2.0).min(s * 0.5));
}
/// Transfer only assemblies that actually finished manufacturing. Pending jobs
/// remain spent stock, and the fixed cutter is recovered only by securing it.
pub(in crate::world::adventure) fn release_hardware(e: &mut Enemy) {
    for module in [
        e.engine.installed.take(),
        e.engine.transfer.take().map(|v| v.0),
        e.engine.loose.take().map(|v| v.0),
    ]
    .into_iter()
    .flatten()
    {
        e.engine.cargo.push(ItemStack::new(module.item(), 1));
    }
}
pub(in crate::world::adventure) fn release_cargo(e: &mut Enemy, world: &mut World) {
    let mut retained = Vec::new();
    for stack in std::mem::take(&mut e.engine.cargo) {
        let mut left = stack.count;
        while left > 0 && world.inventory.dropped.len() < 1024 {
            let count = left.min(world.inventory.stack_limit(stack.item));
            world.inventory.dropped.push(DroppedStack {
                pos: e.pos + Vec3::Y * 0.3,
                stack: ItemStack {
                    item: stack.item,
                    count,
                },
            });
            left -= count;
        }
        if left > 0 {
            retained.push(ItemStack {
                item: stack.item,
                count: left,
            });
        }
    }
    e.engine.cargo = retained;
}
pub(in crate::world::adventure) fn focus(
    e: &Enemy,
    world: &World,
    eye: Vec3,
    ray: Vec3,
) -> Option<reliquaries::RelicFocus> {
    if e.kind != KIND || e.health <= 0.0 || e.engine.secured {
        return None;
    }
    let at = local(e, CONTROL);
    let d = at - eye;
    let t = d.dot(ray);
    if d.length() > 3.5 || t <= 0.0 || (d - ray * t).length() > 0.6 || !line(world, eye, at) {
        return None;
    }
    Some(reliquaries::RelicFocus {
        title: "Ossuary Engine · control",
        action: if e.engine.loading_damage >= DRIVE_HEALTH
            && e.engine.traction_damage >= DRIVE_HEALTH
        {
            "[F] Secure Cutter"
        } else {
            "Side Drives Active"
        },
    })
}
pub(in crate::world::adventure) fn secure(e: &mut Enemy, world: &mut World) -> bool {
    if e.engine.secured
        || e.engine.loading_damage < DRIVE_HEALTH
        || e.engine.traction_damage < DRIVE_HEALTH
    {
        return false;
    }
    let item = Item::Block(Kind::MiningDrill.block());
    if !world.inventory.can_insert(ItemStack::new(item, 1)) {
        world.expedition.say("Inventory Full");
        return true;
    }
    world.inventory.try_insert(ItemStack::new(item, 1));
    world.inventory.notify_pickup(item, 1);
    // The legacy kit mirror is reconciled at the end of adventure ticks.
    // Credit it with the same insertion so reconciliation preserves the item.
    world.factory.kits[Kind::MiningDrill as usize] = world.inventory.count_item(item);
    // The cutter module occupies the fixed cutting mount when installed.
    if e.engine.installed == Some(Module::Cutter) {
        e.engine.installed = None;
    }
    // Consumed assembly never appears again in module salvage or completion.
    e.engine.secured = true;
    e.engine.run = None;
    release_cargo(e, world);
    e.health = 0.0; // Existing one-shot encounter settlement owns the final record.
    world.expedition.say("Cutter Secured · Mining Drill");
    true
}
