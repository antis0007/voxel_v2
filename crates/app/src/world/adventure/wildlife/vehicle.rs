//! Villagers, boats and ridden herd members as passive actors.
use super::*;
use crate::world::travel::Ride;

pub(crate) const VILLAGER: u32 = 40;
pub(crate) const BOAT: u32 = 41;
const VILLAGER_Z: i32 = 8;
const BOAT_Z: i32 = 9;
pub(crate) const VILLAGERS: u32 = 3;
const SPAWN_METRES: f32 = 75.0;
const RIDE_SPEED: f32 = 5.0;
const BOAT_SPEED: f32 = 4.0;
/// Hull depth below the water line, in metres.
const DRAFT: f32 = 0.12;
const HOP_SPEED: f32 = 6.5;

pub(crate) enum Target {
    Villager(u32),
    Mount(u32),
    Boat(u32),
}

fn villager_id(e: &Enemy) -> Option<u32> {
    (e.kind == VILLAGER && e.site.z == VILLAGER_Z).then_some(e.site.x as u32)
}
fn boat_id(e: &Enemy) -> Option<u32> {
    (e.kind == BOAT && e.site.z == BOAT_Z).then_some(e.site.x as u32)
}
fn grown_member(world: &World, e: &Enemy) -> Option<u32> {
    husbandry::member_id(e).filter(|id| world.adventure.wildlife.herd.members.get(id).is_some_and(|m| m.born.is_none()))
}

/// Villager, boat or grown herd member under the crosshair within `reach` metres.
pub(crate) fn target(world: &World, eye: Vec3, forward: Vec3, reach: f32) -> Option<Target> {
    world.adventure.enemies.iter()
        .filter(|e| e.health > 0.0 && passive(e.kind))
        .filter_map(|e| {
            let found = villager_id(e).map(Target::Villager)
                .or_else(|| boat_id(e).map(Target::Boat))
                .or_else(|| grown_member(world, e).map(Target::Mount))?;
            let c = e.center();
            let t = (c - eye).dot(forward);
            let radius = bestiary::species(e.kind).map_or(0.6, |s| s.body_width * 0.5 + 0.25);
            ((0.0..=reach).contains(&t) && (eye + forward * t).distance(c) <= radius).then_some((t, found))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, found)| found)
}

pub(crate) fn villager_centre(world: &World, id: u32) -> Option<Vec3> {
    world.adventure.enemies.iter().find(|e| villager_id(e) == Some(id) && e.health > 0.0).map(Enemy::center)
}

/// Height in metres a hull floats at over the water column under `p`, if that column has an open surface.
pub(crate) fn float_y(world: &World, p: Vec3) -> Option<f32> {
    let s = world.voxel_scale.meters();
    let cell = (p / s).floor().as_ivec3();
    let column = glam::IVec2::new(cell.x, cell.z);
    let top = world.water_surface_top(column, cell.y)
        .or_else(|| world.water_surface_top(column, cell.y - 2))?;
    let fill = f32::from(world.water_volume(IVec3::new(cell.x, top, cell.z))) / 8.0;
    Some((top as f32 + fill) * s - DRAFT)
}

/// Home of villager `i`: a hashed spot around the homestead, on the generated surface.
fn villager_home(world: &World, i: u32) -> Vec3 {
    let anchor = crate::world::workshop::home(&world.sample_config()).unwrap_or(Vec3::ZERO);
    let h = engine_core::hash::hash_u32(world.seed ^ 0x7111_a6e5 ^ i.wrapping_mul(0x9e37_79b9));
    let angle = (h & 1023) as f32 / 1023.0 * std::f32::consts::TAU;
    let radius = 6.0 + ((h >> 10) & 255) as f32 / 255.0 * 8.0;
    let s = world.voxel_scale.meters();
    let mut p = Vec3::new(anchor.x + angle.cos() * radius, 0.0, anchor.z + angle.sin() * radius);
    let cell = (p / s).floor().as_ivec3();
    p.y = (world.terrain_height_at(cell.x, cell.z) + 1) as f32 * s + 0.02;
    p
}

/// Mirrors boat bodies into their records, then once a second loads nearby villagers and boats.
pub(crate) fn service(world: &mut World, feet: Vec3) {
    let today = crate::world::travel::day(world);
    for e in &world.adventure.enemies {
        if let Some(b) = boat_id(e).and_then(|id| world.travel.boats.get_mut(&id)) {
            b.pos = e.pos;
            b.yaw = e.yaw;
        }
    }
    if !world.farming.clock.is_multiple_of(120) {
        return;
    }
    let w: &World = world;
    let mut spawns = Vec::new();
    for i in 0..VILLAGERS {
        if w.travel.away.get(&i) == Some(&today) || w.adventure.enemies.iter().any(|e| villager_id(e) == Some(i)) {
            continue;
        }
        let home = villager_home(w, i);
        if home.distance_squared(feet) <= SPAWN_METRES * SPAWN_METRES && free(w, home, VILLAGER) && supported(w, home, 0.2) {
            let mut e = Enemy::new(IVec3::new(i as i32, 0, VILLAGER_Z), 0, VILLAGER, home, 0);
            e.home = home;
            spawns.push(e);
        }
    }
    for (&id, b) in &w.travel.boats {
        if b.pos.distance_squared(feet) <= SPAWN_METRES * SPAWN_METRES
            && !w.adventure.enemies.iter().any(|e| boat_id(e) == Some(id))
        {
            let mut e = Enemy::new(IVec3::new(id as i32, 0, BOAT_Z), 0, BOAT, b.pos, 0);
            e.home = b.pos;
            e.yaw = b.yaw;
            spawns.push(e);
        }
    }
    world.adventure.enemies.extend(spawns);
}

/// A destroyed boat forgets its record; a villager stays away for the rest of the day.
pub(super) fn fell(world: &mut World, e: &Enemy) {
    if let Some(id) = boat_id(e) {
        world.travel.boats.remove(&id);
        if world.travel.riding == Some(Ride::Boat(id)) {
            world.travel.riding = None;
        }
    } else if let Some(id) = villager_id(e) {
        let today = crate::world::travel::day(world);
        world.travel.away.insert(id, today);
    } else if let Some(id) = husbandry::member_id(e)
        && world.travel.riding == Some(Ride::Mount(id))
    {
        world.travel.riding = None;
    }
}

fn ridden(world: &World, e: &Enemy) -> bool {
    match world.travel.riding {
        Some(Ride::Boat(id)) => boat_id(e) == Some(id),
        Some(Ride::Mount(id)) => husbandry::member_id(e) == Some(id),
        None => false,
    }
}

/// Boats float in place and ridden bodies wait for the rider. True when the actor is handled.
pub(super) fn tick(e: &mut Enemy, world: &World, dt: f32) -> bool {
    if e.kind == BOAT {
        e.visible = false;
        e.gait = 0.0;
        match float_y(world, e.pos) {
            Some(y) => e.pos.y += (y - e.pos.y).clamp(-dt * 2.0, dt * 2.0),
            None if free(world, e.pos - Vec3::Y * dt * 3.0, BOAT) => e.pos.y -= dt * 3.0,
            None => {}
        }
        return true;
    }
    if ridden(world, e) {
        e.visible = false;
        e.alert = 0.0;
        return true;
    }
    false
}

/// Moves `pos` by `delta` over walkable ground, climbing one-metre steps; airborne bodies only need room.
fn walk(world: &World, kind: u32, pos: Vec3, delta: Vec3) -> Vec3 {
    let steps = (delta.length() / 0.12).ceil().max(1.0) as usize;
    let grounded = supported(world, pos, 0.2);
    let mut at = pos;
    for _ in 0..steps {
        let q = at + delta / steps as f32;
        if free(world, q, kind) && (!grounded || supported(world, q, 1.1)) {
            at = q;
            continue;
        }
        let Some(raised) = [0.25, 0.5, 0.75, 1.0].into_iter().map(|lift| q + Vec3::Y * lift)
            .find(|&r| grounded && supported(world, r, 0.2) && free(world, r, kind)) else { break };
        at = raised;
    }
    at
}

/// Steers the ridden actor along `heading` and returns the rider's feet in metres, or `None` once it is gone.
pub(crate) fn drive(world: &mut World, ride: Ride, heading: Vec3, hop: bool, dt: f32) -> Option<Vec3> {
    let i = world.adventure.enemies.iter().position(|e| e.health > 0.0 && match ride {
        Ride::Boat(id) => boat_id(e) == Some(id),
        Ride::Mount(id) => husbandry::member_id(e) == Some(id),
    })?;
    let e = &world.adventure.enemies[i];
    let (kind, pos) = (e.kind, e.pos);
    let w: &World = world;
    let (next, seat, jump) = if kind == BOAT {
        let q = pos + heading * BOAT_SPEED * dt;
        let next = float_y(w, q).map(|y| Vec3::new(q.x, y, q.z)).filter(|&n| free(w, n, BOAT)).unwrap_or(pos);
        (next, 0.15, false)
    } else {
        let next = walk(w, kind, pos, heading * RIDE_SPEED * dt);
        let height = bestiary::species(kind).map_or(1.0, |s| s.body_height);
        (next, height * 0.6, hop && supported(w, pos, 0.2))
    };
    let e = &mut world.adventure.enemies[i];
    let moved = (next - e.pos) * Vec3::new(1.0, 0.0, 1.0);
    e.pos = next;
    e.home = next;
    if jump {
        e.velocity.y = HOP_SPEED;
    }
    let travel = moved.length();
    e.gait += ((travel / dt.max(0.001) / RIDE_SPEED).clamp(0.0, 1.0) - e.gait) * (dt * 8.0).min(1.0);
    e.phase += travel * 4.0;
    if travel > 0.0001 {
        e.yaw = (-moved.x).atan2(-moved.z);
    }
    Some(e.pos + Vec3::Y * seat)
}

/// Villager and boat bodies. True when `e` was drawn here.
pub(super) fn draw(e: &Enemy, m: &mut crate::mesh::models::Model, time: f32) -> bool {
    let mark = m.mark();
    let at = |x: f32, y: f32, z: f32| e.pos + Vec3::new(x, y, z);
    let r = Quat::IDENTITY;
    match e.kind {
        VILLAGER => {
            let robe = [[0.45, 0.32, 0.20], [0.30, 0.40, 0.26], [0.26, 0.32, 0.46]][e.site.x.rem_euclid(3) as usize];
            let skin = [0.78, 0.60, 0.46];
            let sway = (e.phase * 3.0).sin() * 0.05 * e.gait;
            let breathe = (time * 1.4 + e.site.x as f32).sin() * 0.008;
            m.cuboid(at(0.0, 0.45, 0.0), Vec3::new(0.56, 0.9, 0.40), Quat::from_rotation_x(sway), robe, 0.0);
            m.cuboid(at(0.0, 1.22 + breathe, 0.0), Vec3::new(0.58, 0.64, 0.38), r, robe, 0.0);
            m.cuboid(at(0.0, 1.24 + breathe, -0.26), Vec3::new(0.66, 0.18, 0.20), r, robe, 0.0);
            m.cuboid(at(0.0, 1.30 + breathe, -0.37), Vec3::new(0.30, 0.10, 0.04), r, skin, 0.0);
            m.cuboid(at(0.0, 1.78 + breathe, 0.0), Vec3::new(0.44, 0.48, 0.42), r, skin, 0.0);
            m.cuboid(at(0.0, 1.72 + breathe, -0.25), Vec3::new(0.10, 0.18, 0.10), r, [0.70, 0.52, 0.40], 0.0);
            for side in [-0.11, 0.11] {
                m.cuboid(at(side, 1.84 + breathe, -0.215), Vec3::new(0.07, 0.05, 0.02), r, [0.16, 0.20, 0.14], 0.0);
            }
        }
        BOAT => {
            let wood = [0.55, 0.38, 0.22];
            let dark = [0.40, 0.27, 0.16];
            m.cuboid(at(0.0, 0.05, 0.0), Vec3::new(1.20, 0.10, 2.0), r, dark, 0.0);
            for side in [-0.56, 0.56] {
                m.cuboid(at(side, 0.24, 0.0), Vec3::new(0.08, 0.36, 2.0), r, wood, 0.0);
            }
            for end in [-0.96, 0.96] {
                m.cuboid(at(0.0, 0.24, end), Vec3::new(1.20, 0.36, 0.08), r, wood, 0.0);
            }
            m.cuboid(at(0.0, 0.28, 0.25), Vec3::new(1.04, 0.06, 0.30), r, wood, 0.0);
        }
        _ => return false,
    }
    let flash = (e.flash / 0.18).clamp(0.0, 1.0) * 0.5;
    if flash > 0.0 {
        for v in &mut m.vertices[mark..] {
            for c in v.color.iter_mut() { *c = *c * (1.0 - flash) + flash; }
        }
    }
    m.rotate_since(mark, e.pos, Quat::from_rotation_y(e.yaw));
    true
}

/// Review hook: villager 0 held at `villager` and one boat record at `boat`, in metres.
pub(crate) fn review_scene(world: &mut World, villager: Vec3, boat: Vec3) {
    if let Some(e) = world.adventure.enemies.iter_mut().find(|e| villager_id(e) == Some(0)) {
        e.pos = villager;
        e.home = villager;
        e.yaw = 2.7;
        e.gait = 0.0;
        return;
    }
    let mut e = Enemy::new(IVec3::new(0, 0, VILLAGER_Z), 0, VILLAGER, villager, 0);
    e.home = villager;
    e.yaw = 2.7;
    world.adventure.enemies.push(e);
    if world.travel.boats.is_empty() {
        world.travel.add_boat(crate::world::travel::Boat { pos: boat, yaw: 0.6 });
    }
}
