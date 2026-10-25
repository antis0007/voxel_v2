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
pub(super) fn active(b: B) -> bool {
    matches!(b, B::Fire | B::Smoke | B::Steam | B::Lava)
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
