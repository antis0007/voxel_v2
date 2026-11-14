use super::*;
use engine_sim::atmosphere as gas;

fn cabin_docked(world: &World, h: &Habitat) -> bool {
    world.orbital.express.ride.is_some_and(|r| r.ascending && r.docked()
        && r.cabin_m().as_vec3().distance(h.origin.as_vec3()) < 1.0)
}

fn panel(world: &World, h: &Habitat, door: usize) -> Vec<IVec3> {
    let n = world.voxel_scale.meters().recip().round() as i32;
    let mut cells = Vec::new();
    for x in 0..n { for y in 0..3 * n { for z in -3 * n / 2..3 * n / 2 {
        cells.push(h.origin * n + IVec3::new(HATCH_X[door] * n + x, y, z));
    } } }
    cells
}

fn sealed(world: &World, h: &Habitat, door: usize) -> bool {
    panel(world, h, door).into_iter().all(|p| world.get(p).is_solid())
}

fn set_door(world: &mut World, h: &mut Habitat, door: usize, open: bool, player: &Player) -> bool {
    let cells = panel(world, h, door);
    if !open && !h.doors_open[door] { return sealed(world, h, door); }
    if open && !sealed(world, h, door) { return false; }
    let expected = if open { B::SteelBlock } else { B::Air };
    if cells.iter().any(|&cell| world.get(cell) != expected) {
        world.expedition.say("Airlock obstructed"); return false;
    }
    let owners: std::collections::HashSet<_> = cells.iter().copied()
        .map(crate::world::chunk_coord_from_world_block).collect();
    if owners.into_iter().any(|chunk| !world.admit_edit_chunk(chunk)) { return false; }
    if !open {
        let (a, b) = player.bounds(world.voxel_scale);
        if cells.iter().any(|p| p.as_vec3().cmplt(b).all() && (p.as_vec3() + Vec3::ONE).cmpgt(a).all()) {
            world.expedition.say("Door obstructed"); return false;
        }
    }
    let block = if open { B::Air } else { B::SteelBlock };
    let edits: Vec<_> = cells.iter().copied().map(|p| (p, block)).collect();
    world.set_many_mechanism(&edits);
    let success = cells.iter().all(|&p| world.get(p) == block);
    if success { h.doors_open[door] = open; }
    for p in cells {
        let m = (p.as_vec3() * world.voxel_scale.meters()).floor().as_ivec3() - h.origin;
        for offset in [IVec3::ZERO, IVec3::NEG_X, IVec3::NEG_Y, IVec3::NEG_Z] {
            if let Some(i) = h.index(m + offset) { h.dirty.insert(i); }
        }
    }
    success
}

fn focus(world: &World, p: &Player) -> Option<(usize, usize)> {
    if world.orbital.express.ride.is_some_and(|r| !r.deck && !(r.ascending && r.docked())) {
        return None;
    }
    let eye = p.eye(world.voxel_scale) * world.voxel_scale.meters();
    for (i, h) in world.orbital.habitats.habitats.iter().enumerate() {
        if !h.ready || !h.dirty.is_empty() { continue; }
        for (door, x) in HATCH_X.into_iter().enumerate() {
            let x = x as f32;
            let center = h.origin.as_vec3() + Vec3::new(x + 0.5, 1.5, 0.0);
            let delta = center - eye;
            if delta.length() < 3.5 && delta.normalize_or_zero().dot(p.forward_dir()) > 0.5 {
                let scale = world.voxel_scale.meters();
                if let Some((hit, _)) = crate::world::raycast_block(world, eye / scale,
                    delta.normalize_or_zero(), delta.length() / scale) {
                    let at = (hit.as_vec3() + Vec3::splat(0.5)) * scale - h.origin.as_vec3();
                    if !(x..x + 1.0).contains(&at.x) || !(0.0..3.0).contains(&at.y)
                        || at.z.abs() >= 1.5 { continue; }
                }
                return Some((i, door));
            }
        }
    }
    None
}

pub fn prompt(world: &World, p: &Player) -> Option<crate::world::interaction::Prompt> {
    let (i, door) = focus(world, p)?;
    let h = &world.orbital.habitats.habitats[i];
    let title = if door == 1 { "Station hatch" }
        else if cabin_docked(world, h) { "Cabin hatch" }
        else { "EVA hatch" };
    let action = if h.cycle >= 0 { "Equalizing" }
        else if h.doors_open[door] { "[F] Close" }
        else { "[F] Cycle airlock" };
    Some(crate::world::interaction::Prompt { title, action })
}

pub fn interact(world: &mut World, p: &Player) -> bool {
    let Some((i, door)) = focus(world, p) else { return false };
    let h = &world.orbital.habitats.habitats[i];
    if h.cycle >= 0 { return true; }
    let mut h = world.orbital.habitats.habitats.remove(i);
    world.orbital.habitats.building = Some((h.origin, h.grid));
    if h.doors_open[door] {
        set_door(world, &mut h, door, false, p);
    } else if !sealed(world, &h, door) {
        world.expedition.say("Airlock seal damaged");
    } else if set_door(world, &mut h, 1 - door, false, p) {
        h.cycle = door as i8;
    }
    world.orbital.habitats.building = None;
    world.orbital.habitats.habitats.insert(i, h);
    true
}

pub(super) fn step(world: &mut World, h: &mut Habitat, p: &Player) {
    if h.cycle < 0 || !h.dirty.is_empty() { return; }
    // Physical damage to either panel faults the cycle instead of recreating it.
    if !sealed(world, h, 0) || !sealed(world, h, 1) {
        h.cycle = -1; world.expedition.say("Airlock seal damaged"); return;
    }
    let target = h.cycle as usize;
    let desired = if target == 0 {
        if cabin_docked(world, h) { gas::pressure(h.cabin, cabin_volume()) }
        else { 0.0 }
    } else {
        h.index(IVec3::new(12, 1, 0)).map_or(0.0, |i| gas::pressure(h.gas[i], h.volumes[i]))
    };
    let current = pressure::chamber_pressure(h);
    if (current - desired).abs() < 1500.0 {
        if set_door(world, h, target, true, p) { h.cycle = -1; }
        return;
    }
    let chamber = pressure::chamber(h);
    let budget = config().pressure_pa * 0.5; // half a reference cubic metre per 0.1 s
    let each = budget / chamber.len().max(1) as f64;
    for i in chamber {
        if current > desired {
            let capacity = config().tank_pressure_pa * config().tank_volume_m3;
            let room = (capacity - h.tank[0] - h.tank[1]).max(0.0);
            let amount = each.min((current - desired) * h.volumes[i]);
            let recovered = amount.min(room);
            gas::transfer(&mut h.gas[i], &mut h.tank, recovered);
            gas::transfer(&mut h.gas[i], &mut h.escaped, amount - recovered);
        } else {
            let amount = each.min((desired - current) * h.volumes[i]);
            gas::transfer(&mut h.tank, &mut h.gas[i], amount);
        }
    }
    if current < desired && h.tank[0] + h.tank[1] < 1.0 {
        h.cycle = -1; world.expedition.say("Airlock gas reserve empty");
    }
}
