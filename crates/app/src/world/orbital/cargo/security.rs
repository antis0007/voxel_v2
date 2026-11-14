use super::*;

fn bay(p: Vec3) -> bool {
    (22.0..27.0).contains(&p.x) && (1.0..5.0).contains(&p.y) && (-3.0..3.0).contains(&p.z)
}
pub fn material(p: Vec3) -> Option<B> {
    if (19.0..21.0).contains(&p.x) && (0.0..1.0).contains(&p.y) && (-2.0..2.0).contains(&p.z) {
        return Some(B::SteelBlock);
    }
    if (14.0..15.0).contains(&p.x) && (0.0..1.0).contains(&p.y) && (0.0..1.0).contains(&p.z) {
        return Some(B::SalvageCrate);
    }
    if !bay(p) { return None; }
    if p.x < 23.0 { Some(B::StarGlass) }
    else if p.x >= 26.0 || p.y < 2.0 || p.y >= 4.0 || p.z.abs() >= 2.0
        || ((24.0..26.0).contains(&p.x) && p.z.abs() >= 1.0) { Some(B::SteelBlock) }
    else { None }
}

/// The field, booth and constructors share the ordinary voxel/projectile wall.
pub fn protected(world: &World, cell: IVec3) -> bool {
    let m = (cell.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters();
    world.orbital.habitats.habitats.iter().any(|h| {
        let p = m - h.origin.as_vec3();
        bay(p) && (world.orbital.habitats.cargo.stations.iter().any(|s| s.origin == h.origin && s.ready)
            || material(p).is_some_and(|b| world.get(cell) == b))
    })
}

pub(super) fn install(world: &mut World, s: &mut Station, budget: &mut usize) {
    let n = world.voxel_scale.meters().recip().round() as i32;
    let size = IVec3::new(5, 4, 6) * n;
    let booth = (size.x * size.y * size.z) as usize;
    let locker = (n * n * n) as usize;
    let pad_size = IVec3::new(2, 1, 4) * n;
    let total = booth + locker + (pad_size.x * pad_size.y * pad_size.z) as usize;
    let start = s.fixtures;
    let mut edits = Vec::new();
    while s.fixtures < total && *budget > 0 {
        let i = s.fixtures;
        let p = if i < booth {
            IVec3::new(22, 1, -3) * n + IVec3::new(i as i32 % size.x,
                i as i32 / (size.x * size.z), (i as i32 / size.x) % size.z)
        } else if i < booth + locker {
            let k = (i - booth) as i32;
            IVec3::new(14, 0, 0) * n + IVec3::new(k % n, k / (n * n), (k / n) % n)
        } else {
            let k = (i - booth - locker) as i32;
            IVec3::new(19, 0, -2) * n + IVec3::new(k % pad_size.x,
                k / (pad_size.x * pad_size.z), (k / pad_size.x) % pad_size.z)
        };
        if let Some(b) = material((p.as_vec3() + Vec3::splat(0.5)) / n as f32) {
            let cell = s.origin * n + p;
            let current = world.get(cell);
            // Retrofitting never overwrites a player's interior alterations.
            if current != B::Air && current != b { break; }
            edits.push((cell, b));
        }
        s.fixtures += 1;
        *budget -= 1;
    }
    world.set_many_mechanism(&edits);
    if edits.iter().any(|&(p, b)| world.get(p) != b) {
        s.fixtures = start;
        return;
    }
    if s.fixtures != total || world.caches.len() >= containers::LIMIT { return; }
    let anchor = locker_cell(world, s.origin);
    let id = if let Some(id) = world.caches.iter().position(|c| c.cells.contains(&anchor)) { id }
    else {
        let mut cells = Vec::new();
        for y in 0..n { for z in 0..n { for x in 0..n { cells.push(anchor + IVec3::new(x, y, z)); } } }
        let mut slots = [None; containers::SLOTS];
        for (slot, (item, count)) in slots.iter_mut().zip(SALVAGE_MANIFEST) {
            *slot = Some(ItemStack::new(item, count));
        }
        let id = world.caches.len();
        world.caches.push(containers::Cache { cells, slots });
        id
    };
    s.cache = Some(id);
    s.ready = true;
}

pub(super) fn advance(world: &mut World, player: &Player, s: &mut Station) {
    if !s.ready { return; }
    if s.shipment.as_ref().is_some_and(|q| q.ticks < LOAD_TICKS) && s.pause == 0 {
        s.spawn_clock = s.spawn_clock.saturating_sub(1);
        if s.spawn_clock == 0 && s.produced < 12 {
            s.spawn_clock = 4 * 120;
            for side in [-1.0, 1.0] {
                if s.guards.len() >= MAX_GUARDS || s.produced >= 12 { break; }
                let Some(slot) = (0..MAX_GUARDS as u32).find(|slot| !s.guards.iter().any(|r| r.slot == *slot)) else { break };
                let pos = s.origin.as_vec3() + Vec3::new(20.0, 1.0, side * 1.3);
                if !crate::world::adventure::habitat_space(world, pos, ROBOT)
                    || world.adventure.enemies.iter().any(|e| e.health > 0.0 && e.pos.distance_squared(pos) < 1.0) { continue; }
                s.guards.push(Guard { slot, pos, health: 60.0 });
                s.produced += 1;
                if player.pos.distance(pos / world.voxel_scale.meters()) * world.voxel_scale.meters() < 90.0 {
                    world.adventure.sounds.push(crate::audio::Event { cue: crate::audio::Cue::Clang,
                        pos: Some(pos), gain: 0.7, pitch: 1.3 });
                }
            }
        }
    }
    for g in &s.guards {
        if world.adventure.enemies.len() >= 80 { break; }
        if (g.pos - player.pos * world.voxel_scale.meters()).length() > 85.0
            || world.adventure.enemies.iter().any(|e| e.kind == ROBOT && e.site == s.origin && e.slot == g.slot)
            || !crate::world::adventure::habitat_space(world, g.pos, ROBOT) { continue; }
        let mut e = crate::world::adventure::Enemy::new(s.origin, g.slot, ROBOT, g.pos, 0);
        e.health = g.health;
        world.adventure.enemies.push(e);
    }
}
