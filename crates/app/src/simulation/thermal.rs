//! Sparse heat exchange, finite fuel, and conservative gas transport.
use super::{
    cellular::{admission, changed},
    heat,
    residency::Residency,
    sandbox::ThermalCell,
};
use crate::world::{BlockId as B, World, chunk_coord_from_world_block as chunk};
use glam::IVec3;
use std::collections::HashSet;
const DIRS: [IVec3; 6] = [
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Z,
    IVec3::NEG_Z,
];
const LAVA_MOVE_PERIOD: u64 = 12;
const ACID_MOVE_PERIOD: u64 = 4;

pub(super) fn acid_reaction(block: B) -> Option<&'static engine_core::reaction::Reaction> {
    engine_content::SIM_RULES.reactions().pair(B::SulfuricAcid.sim_byte(), block.sim_byte())
}

fn corrode(
    s: &mut Residency,
    w: &mut World,
    p: IVec3,
    visited: &mut HashSet<IVec3>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> usize {
    use crate::world::effects::{Failure, dry_material_batch};
    use engine_core::material::MaterialId;

    let temperature = |world: &World, cell, block| {
        ((world.sandbox.cell_for(cell, block).temperature + 273.15) * 10.0).round() as u16
    };
    let seed = w.seed
        ^ (p.x as u32).wrapping_mul(0x9e37_79b1)
        ^ (p.y as u32).wrapping_mul(0x85eb_ca77)
        ^ (p.z as u32).wrapping_mul(0xc2b2_ae3d)
        ^ (w.activity.tick / ACID_MOVE_PERIOD) as u32;
    let mut waiting = false;
    // Check the floor first, then the other contacting faces.
    for (face, offset) in DIRS[1..].iter().chain(&DIRS[..1]).copied().enumerate() {
        let q = p + offset;
        let Some((block, _)) = s.read(w, q) else { continue };
        let Some(reaction) = acid_reaction(block) else { continue };
        if !super::cellular::allowed(w, q) {
            continue;
        }
        let conditioned = if reaction.on_second() { temperature(w, q, block) }
            else { temperature(w, p, B::SulfuricAcid) };
        if !reaction.holds(conditioned) {
            continue;
        }
        let roll = engine_core::hash::hash_u32(seed ^ (face as u32).wrapping_mul(0x27d4_eb2d));
        if !reaction.fires(roll) || !s.prepare(w, &[p, q]) {
            waiting = true;
            continue;
        }
        if w.get(p) != B::SulfuricAcid || w.get(q) != block {
            waiting = true;
            continue;
        }
        let product = |id: MaterialId, original| {
            if id == MaterialId::NONE { original } else { B::from_u32(u32::from(id.0)) }
        };
        let next = [(p, product(reaction.a_into, B::SulfuricAcid)), (q, product(reaction.b_into, block))];
        match dry_material_batch(w, &next, blocks, chunks) {
            Ok(()) => {
                visited.insert(q);
                return 2;
            }
            Err(Failure::Deferred | Failure::Conflict) => waiting = true,
            Err(Failure::Rejected | Failure::NoChange) => {}
        }
    }
    if waiting {
        w.activity.wake(p);
    }
    0
}

/// The legacy world stores one full voxel per cell for these liquids. Move it
/// downhill or from the top of a stack into an empty neighbour that can hold it.
fn liquid_destination(s: &mut Residency, w: &World, p: IVec3, liquid: B, period: u64) -> Result<Option<IVec3>, ()> {
    let down = p - IVec3::Y;
    let below = if super::cellular::allowed(w, down) {
        s.read(w, down).ok_or(())?.0
    } else {
        B::Bedrock
    };
    if below == B::Air {
        return Ok(Some(down));
    }
    if s.read(w, p + IVec3::Y).is_none_or(|(b, _)| b == liquid) {
        return Ok(None);
    }
    let sides = [IVec3::X, IVec3::Z, IVec3::NEG_X, IVec3::NEG_Z];
    let rotation = (w.activity.tick / period) as usize;
    let mut level = None;
    let mut waiting = false;
    for i in 0..sides.len() {
        let side = p + sides[(rotation + i) % sides.len()];
        if !s.read(w, side).is_some_and(|(b, _)| b == B::Air)
            || !super::cellular::allowed(w, side)
        {
            continue;
        }
        let landing = side - IVec3::Y;
        if !super::cellular::allowed(w, landing) {
            continue;
        }
        if !s.request_for(w, chunk(landing), chunk(p)) {
            waiting = true;
            continue;
        }
        match s.read(w, landing) {
            Some((B::Air, _)) => return Ok(Some(landing)),
            Some((support, _)) if below == liquid && support.is_solid() => {
                level = level.or(Some(side));
            }
            _ => {}
        }
    }
    if level.is_none() && waiting { Err(()) } else { Ok(level) }
}

pub(super) fn step_acid(
    s: &mut Residency,
    w: &mut World,
    p: IVec3,
    visited: &mut HashSet<IVec3>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> usize {
    let phase = ((p.x ^ p.y ^ p.z) as u64 & 1) * 2;
    if w.activity.tick % ACID_MOVE_PERIOD != phase {
        w.activity.wake(p);
        return 0;
    }
    let mut ready = true;
    for offset in DIRS {
        let q = p + offset;
        if super::cellular::allowed(w, q) {
            ready &= s.request_for(w, chunk(q), chunk(p));
        }
    }
    if !ready {
        w.activity.wake(p);
        return 0;
    }
    let q = match liquid_destination(s, w, p, B::SulfuricAcid, ACID_MOVE_PERIOD) {
        Ok(Some(q)) => q,
        Ok(None) => return corrode(s, w, p, visited, blocks, chunks),
        Err(()) => { w.activity.wake(p); return 0; }
    };
    if !admission(w, p, q) || !s.prepare(w, &[p, q])
        || crate::world::effects::dry_material_batch(w, &[(p, B::Air), (q, B::SulfuricAcid)], blocks, chunks)
            .is_err()
    {
        w.activity.wake(p);
        return 0;
    }
    visited.insert(q);
    2
}

pub(super) fn active(b: B) -> bool {
    crate::world::smeltery::metal(b).is_some() || matches!(b, B::Fire | B::Smoke | B::Steam | B::Lava)
}
pub(super) fn step(
    s: &mut Residency,
    w: &mut World,
    p: IVec3,
    visited: &mut HashSet<IVec3>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> usize {
    let Some((b, volume)) = s.read(w, p) else { return 0 };
    if b == B::Fire {
        w.sandbox.note_fire(p);
    }
    // Smoke and steam are visible but do not need a full neighbour solve at
    // 30 Hz. Staggering them halves the gas cost while keeping the cloud
    // moving; fire stays on every tick.
    if matches!(b, B::Smoke | B::Steam | B::Lava)
        && ((w.activity.tick as usize)
            .wrapping_add(p.x as usize)
            .wrapping_add(p.y as usize)
            .wrapping_add(p.z as usize)
            & 1)
            != 0
    {
        w.activity.wake(p);
        return 0;
    }
    // Small fixed array instead of a heap Vec: thermal::step is called for
    // every active gas cell and allocation dominated the sparse update path.
    let mut positions = [p; 7];
    let mut position_count = 1usize;
    for d in DIRS {
        let q = p + d;
        if super::cellular::allowed(w, q) {
            positions[position_count] = q;
            position_count += 1;
        }
    }
    let positions = &positions[..position_count];
    let mut ready = true;
    for &q in positions {
        ready &= s.request_for(w, chunk(q), chunk(p));
    }
    if !ready || !w.sandbox.can_admit(&[p]) {
        w.activity.wake(p);
        return 0;
    }
    // Read the six-face neighbourhood once. The old path re-read the same
    // source cells for heat exchange, ignition oxygen, wet contact and lava
    // quenching, multiplying residence-map lookups for every burning cell.
    let mut neighbours = [(B::Air, 0u8); 6];
    for (slot, &q) in neighbours.iter_mut().zip(&positions[1..]) {
        *slot = s.read(w, q).unwrap_or((B::Air, 0));
    }
    let mut c = w.sandbox.cell_for(p, b);
    if b == B::Steam && c.gas == 0 {
        c.gas = 8;
        c.temperature = 120.0;
    }
    if b == B::Lava {
        c.temperature = 1200.0;
    }
    if b == B::Fire && c.fuel == 0.0 {
        c.fuel = 80.0;
        c.temperature = 700.0;
    }
    w.sandbox.set(p, c);
    // Each face moves equal and opposite energy; capacities turn it into
    // temperature, so water warms slowly and copper passes heat on quickly.
    let own = heat::of(b);
    for (&q, &(nb, _)) in positions[1..].iter().zip(&neighbours) {
        if !w.sandbox.can_admit(&[q]) {
            continue;
        }
        let other = heat::of(nb);
        let mut n = w.sandbox.cell_for(q, nb);
        let flow = (c.temperature - n.temperature) * heat::conductance(own, other);
        if flow.abs() > 0.01 {
            c.temperature -= flow / own.capacity;
            n.temperature += flow / other.capacity;
            w.sandbox.set(q, n);
            w.activity.wake(q);
        }
    }
    // Open air is the reservoir at the local climate ambient. Ice under water
    // or ground has no air contact and changes only by conduction, so a lake
    // freezes from its surface down.
    let exposed = neighbours[..position_count - 1].iter().any(|(nb, _)| heat::of(*nb).gas);
    let rate = if matches!(b, B::Air | B::Steam | B::Smoke) {
        0.025
    } else if b == B::Ice && !exposed {
        0.0
    } else {
        0.002 / own.capacity
    };
    c.temperature += (w.sandbox.climate.ambient_for(p, b) - c.temperature) * rate;
    w.sandbox.set(p, c);
    let mut count = 0;
    let mut replace = None;
    let mut steam_at = None;
    if own.fuel > 0.0 && own.ignite_c.is_some_and(|t| c.temperature >= t) {
        c.fuel = own.fuel;
        replace = Some(B::Fire);
    }
    if b == B::Fire {
        let oxygen = neighbours
            .iter()
            .take(position_count - 1)
            .any(|(b, _)| *b == B::Air);
        let wet = neighbours
            .iter()
            .take(position_count - 1)
            .any(|(b, _)| *b == B::Water);
        if wet || !oxygen {
            c.temperature = c.temperature.min(180.0);
            replace = Some(B::Ash);
            c.fuel = 0.0;
            if wet {
                // Quenching flashes the contact water to a short steam puff
                // above the fire instead of silently deleting the flame.
                steam_at = Some(p + IVec3::Y);
            }
        } else {
            // Fuel supplies enough heat to overcome six-face diffusion and ignite neighbours.
            c.temperature = (c.temperature + 250.0).min(950.0);
            c.fuel = (c.fuel - 1.0).max(0.0);
            if c.fuel == 0.0 {
                replace = Some(B::Ash);
            } else if w.activity.tick.is_multiple_of(4) {
                let q = p + IVec3::Y;
                if admission(w, p, q)
                    && w.sandbox.can_admit(&[q])
                    && s.read(w, q).is_some_and(|(b, _)| b == B::Air)
                    && s.prepare(w, &[q])
                {
                    w.set(q, B::Smoke);
                    w.sandbox.set(
                        q,
                        ThermalCell {
                            temperature: 200.0,
                            fuel: 0.0,
                            gas: 2,
                        },
                    );
                    visited.insert(q);
                    changed(q, blocks, chunks);
                    count += 1;
                }
            }
        }
    }
    if b == B::Lava {
        let water_neighbor = positions[1..]
            .iter()
            .copied()
            .zip(neighbours.iter())
            .find(|(_, (nb, _))| *nb == B::Water)
            .map(|(q, _)| q);
        if let Some(wq) = water_neighbor {
            replace = Some(B::Basalt);
            c.temperature = 320.0;
            if w.sandbox.can_admit(&[wq]) {
                let mut wn = w.sandbox.cell(wq);
                wn.temperature = wn.temperature.max(140.0);
                w.sandbox.set(wq, wn);
                w.activity.wake(wq);
            }
        } else if w.activity.tick.is_multiple_of(8) && s.read(w, p + IVec3::Y).is_some_and(|(b, _)| b == B::Air) {
            // Open lava licks short flames into the air above it; the
            // positional hash keeps the flicker deterministic.
            let roll = engine_core::hash::hash_u32(
                (p.x as u32).wrapping_mul(0x9e37_79b1)
                    ^ (p.y as u32).wrapping_mul(0x85eb_ca77)
                    ^ (p.z as u32).wrapping_mul(0xc2b2_ae3d)
                    ^ w.activity.tick as u32,
            );
            let q = p + IVec3::Y;
            if roll % 100 < 4 && admission(w, p, q) && w.sandbox.can_admit(&[q]) && s.prepare(w, &[q]) {
                w.set(q, B::Fire);
                w.sandbox.set(q, ThermalCell { temperature: 900.0, fuel: 5.0, gas: 0 });
                visited.insert(q);
                changed(q, blocks, chunks);
                count += 1;
            }
        }
    }
    // All non-water authored phase pairs use the same conduction and latent
    // store as ice. Metal cools by exchanging energy, not by a fixed decrement.
    if !matches!(b, B::Ice | B::Water | B::Steam) {
        let sim = b.material().sim;
        if let (Some(point), Some(product)) = (own.melt_c, sim.melts_into) {
            if heat::latent(&mut c, point, true, own.fusion_k) { replace = Some(product); }
        } else if let (Some(point), Some(product)) = (own.cool_c, sim.cools_into) {
            if heat::latent(&mut c, point, false, own.fusion_k) { replace = Some(product); }
        }
    }
    if b == B::Ice
        && let Some(point) = own.melt_c
        && heat::latent(&mut c, point, true, own.fusion_k)
    {
        replace = Some(B::Water);
        c.gas = if crate::world::environment::State::enabled(w.generator_version) && c.gas > 0 { c.gas } else { 8 };
    }
    if b == B::Water && own.boil_c.is_some_and(|t| c.temperature >= t) {
        replace = Some(B::Steam);
        c.gas = volume;
        c.fuel = 0.0;
        c.temperature = own.boil_c.unwrap_or(c.temperature);
    } else if b == B::Water
        && volume >= 8
        && let Some(point) = own.cool_c
        && heat::latent(&mut c, point, false, own.fusion_k)
    {
        // Only a full cell freezes: World::set refuses ice from a partial one.
        replace = Some(B::Ice);
    }
    if b == B::Steam {
        if c.gas == 0 {
            c.gas = 8;
        }
        if own.cool_c.is_some_and(|t| c.temperature < t) {
            replace = Some(B::Water);
        }
    }
    if let Some(id) = replace {
        if s.prepare(w, &[p]) {
            if b == B::Fire && id != B::Fire {
                w.sandbox.clear_fire(p);
            }
            if id == B::Water {
                w.set_water_volume(p, c.gas.max(1));
                c.gas = 0;
            } else {
                w.set(p, id);
            }
            changed(p, blocks, chunks);
            count += 1;
        } else {
            w.activity.wake(p);
            return count;
        }
    }
    if let Some(q) = steam_at
        && admission(w, p, q)
            && w.sandbox.can_admit(&[q])
            && s.read(w, q).is_some_and(|(id, _)| id == B::Air)
            && s.prepare(w, &[q])
        {
            w.set(q, B::Steam);
            w.sandbox.set(
                q,
                ThermalCell {
                    temperature: 160.0,
                    fuel: 0.0,
                    gas: 4,
                },
            );
            visited.insert(q);
            changed(q, blocks, chunks);
            count += 1;
        }
    w.sandbox.set(p, c);
    // Stagger movement over two separated phases of the thermal checkerboard.
    // Adjacent cells must have a turn without letting a falling parcel hop twice
    // on consecutive ticks.
    let parity = (p.x ^ p.y ^ p.z) as u64 & 1;
    if b == B::Lava
        && replace.is_none()
        && w.activity.tick % LAVA_MOVE_PERIOD == parity * 5
        && let Ok(Some(q)) = liquid_destination(s, w, p, B::Lava, LAVA_MOVE_PERIOD)
        && w.sandbox.can_admit(&[q])
        && admission(w, p, q)
        && s.prepare(w, &[p, q])
        && crate::world::effects::dry_material_batch(w, &[(p, B::Air), (q, B::Lava)], blocks, chunks)
            .is_ok()
    {
        w.sandbox.cells.remove(&super::sandbox::key(p));
        w.sandbox.set(q, c);
        w.activity.wake(q);
        visited.insert(q);
        return count + 2;
    }
    if replace.is_none()
        && crate::world::smeltery::metal(b).is_some()
        && !crate::world::smeltery::contained(w, p)
    {
        if let Some(reacted) = crate::world::smeltery::alloy(w, p, blocks, chunks) { return count + reacted; }
        for d in std::iter::once(IVec3::NEG_Y).chain(crate::world::smeltery::SIDES) {
            let q = p + d;
            let empty = s.read(w, q).is_some_and(|(b, _)| b == B::Air);
            let drains = d == IVec3::NEG_Y || s.read(w, q - IVec3::Y).is_some_and(|(b, _)| b == B::Air);
            if empty && drains && s.prepare(w, &[p, q])
                && crate::world::smeltery::transfer(w, p, q, blocks, chunks) {
                visited.insert(q);
                return count + 2;
            }
        }
    }
    if replace.is_none() && matches!(b, B::Smoke | B::Steam) {
        if c.gas == 0 {
            c.gas = 8;
            w.sandbox.set(p, c);
        }
        // Vent only at the explicit open upper world boundary.
        if p.y + 1 >= w.sample_config().max_world_y() && s.prepare(w, &[p]) {
            w.set(p, B::Air);
            let air = w.sandbox.ambient_cell(p, B::Air);
            w.sandbox.set(p, air);
            changed(p, blocks, chunks);
            return count + 1;
        }
        let rotation = (w.activity.tick as usize) & 3;
        let sides = [IVec3::X, IVec3::Z, IVec3::NEG_X, IVec3::NEG_Z];
        for d in std::iter::once(IVec3::Y).chain((0..4).map(|i| sides[(i + rotation) & 3])) {
            let q = p + d;
            let Some((target, _)) = s.read(w, q) else {
                continue;
            };
            if target != B::Air && target != b {
                continue;
            }
            let mut n = w.sandbox.cell(q);
            let amount = if target == b {
                if n.gas == 0 { 8 } else { n.gas }
            } else {
                0
            };
            let moved = if d == IVec3::Y {
                c.gas.min(8 - amount)
            } else {
                c.gas.saturating_sub(amount) / 2
            };
            let exact = b == B::Steam && crate::world::environment::State::enabled(w.generator_version);
            let source_ul = w.environment.exact_water(p, c.gas);
            let target_ul = w.environment.exact_water(q, amount);
            let moved_ul = source_ul * u64::from(moved) / u64::from(c.gas.max(1));
            let mut affected = [p; 14];
            for (i, at) in [p, q].into_iter().enumerate() {
                for (j, d) in std::iter::once(IVec3::ZERO).chain(DIRS).enumerate() {
                    affected[i * 7 + j] = at + d;
                }
            }
            if moved == 0
                || (exact && !w.environment.can_encode(&[(p, source_ul - moved_ul), (q, target_ul + moved_ul)]))
                || (crate::world::environment::State::enabled(w.generator_version) && !w.environment.can_wake(&affected))
                || !w.sandbox.can_admit(&[q])
                || !admission(w, p, q)
                || !s.prepare(w, &[p, q])
            {
                continue;
            }
            n.temperature = (n.temperature * amount as f32 + c.temperature * moved as f32)
                / (amount + moved) as f32;
            n.gas = amount + moved;
            c.gas -= moved;
            w.set(q, b);
            if exact {
                n.gas = (target_ul + moved_ul).div_ceil(w.environment.quantum()) as u8;
                c.gas = (source_ul - moved_ul).div_ceil(w.environment.quantum()) as u8;
            }
            w.sandbox.set(q, n);
            if c.gas == 0 {
                w.set(p, B::Air);
                c = w.sandbox.ambient_cell(p, B::Air);
            }
            w.sandbox.set(p, c);
            if exact {
                w.environment.encode_water(q, target_ul + moved_ul);
                w.environment.encode_water(p, source_ul - moved_ul);
            }
            visited.insert(q);
            changed(p, blocks, chunks);
            changed(q, blocks, chunks);
            count += 2;
            break;
        }
    }
    let now = s.read(w, p).map_or(b, |(id, _)| id);
    if w.sandbox.settled(p, now, c) {
        // Water at its floor is settled though the air ambient is colder.
        w.sandbox.cells.remove(&super::sandbox::key(p));
    } else {
        w.activity.wake(p);
    }
    if active(now) {
        w.activity.wake(p);
    }
    count
}
