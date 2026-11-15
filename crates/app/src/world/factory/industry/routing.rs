use super::*;
use std::collections::BTreeMap;

fn allows(n: &Node) -> bool {
    if n.ruptured || n.gas_vent {
        return true;
    }
    if !n.enabled {
        return false;
    }
    match n.kind {
        Kind::DustFilter => n.filter_credit > 0 && n.residue.iter().sum::<u32>() < WASTE_CAPACITY,
        Kind::WetScrubber if !processor(n) => {
            n.water > 0 && n.energy > 0 && n.residue.iter().sum::<u32>() < WASTE_CAPACITY
        }
        _ => true,
    }
}

fn accepts(kind: Kind, mode: u32, species: usize) -> bool {
    match kind {
        Kind::WetScrubber => mode == 0,
        Kind::GasTank => match mode {
            1 => species == FUEL,
            2 => species == OXYGEN,
            3 => species == STEAM,
            _ => true,
        },
        Kind::GasGenerator => species == if mode == 1 { STEAM } else { FUEL },
        _ => true,
    }
}

fn selects(selection: u8, species: usize) -> bool {
    selection == 0 || usize::from(selection) == species + 1
}

struct Port {
    mass: [u32; SPECIES],
    open: bool,
    kind: Kind,
    closed: u32,
    mode: u32,
    vent: bool,
    outlet: u8,
    gas: u8,
    flow: u32,
}

pub fn route(world: &mut World, state: &mut State) {
    let size = super::super::step(world);
    let mut energy_used = 0;
    for n in world.factory.nodes.values_mut()
        .filter(|n| matches!(n.kind, Kind::DustFilter | Kind::WetScrubber))
    {
        let before = n.energy;
        capture(n);
        energy_used += before.saturating_sub(n.energy);
    }
    world.factory.stats.power(0, energy_used);
    let before: BTreeMap<_, _> = world
        .factory
        .nodes
        .iter()
        .filter(|(_, n)| super::super::ports::supports(n.kind, super::super::ports::Domain::Gas))
        .map(|(&p, n)| ((p.x, p.y, p.z), Port {
            mass: n.exhaust,
            open: allows(n),
            kind: n.kind,
            closed: n.target,
            mode: n.mode,
            vent: n.gas_vent,
            outlet: if n.ruptured { 0 } else { n.gas_vent_side },
            gas: if n.ruptured { 0 } else { n.gas_vent_species },
            flow: port_flow(n),
        }))
        .collect();
    for (&(x, y, z), source) in &before {
        let p = IVec3::new(x, y, z);
        let Port { mass, open: enabled, kind, closed, vent, outlet: vent_side, gas, flow, .. } = *source;
        if !enabled || mass.iter().all(|&v| v == 0) {
            continue;
        }
        for (side, d) in DIRS.iter().enumerate() {
            let q = p + *d * size;
            let Some(other) = before.get(&(q.x, q.y, q.z)) else {
                continue;
            };
            if !component(other.kind) {
                continue;
            }
            if !other.open
                || (kind == Kind::ExhaustDuct && closed >> side & 1 != 0)
                || (other.kind == Kind::ExhaustDuct && other.closed >> (side ^ 1) & 1 != 0)
            {
                continue;
            }
            let target_room = world
                .factory
                .nodes
                .get(&q)
                .map_or(0, |n| gas_capacity(n).saturating_sub(n.exhaust.iter().sum()));
            let mut room = target_room.min(flow.max(other.flow));
            for i in 0..SPECIES {
                if !accepts(other.kind, other.mode, i) {
                    continue;
                }
                if selects(gas, i) && ((vent_side != 0 && usize::from(vent_side) != side + 1)
                    || (vent && vent_side == 0))
                {
                    continue;
                }
                let available = world.factory.nodes.get(&p).map_or(0, |n| n.exhaust[i]);
                let pressure = if component(kind) {
                    mass[i].saturating_sub(other.mass[i])
                } else {
                    mass[i]
                };
                let moved = available
                    .min(room)
                    .min(pressure.div_ceil(4));
                if moved == 0 {
                    continue;
                }
                if let Some(n) = world.factory.nodes.get_mut(&p) {
                    n.exhaust[i] -= moved;
                    n.activity = 1.0;
                }
                if let Some(n) = world.factory.nodes.get_mut(&q) {
                    n.exhaust[i] += moved;
                    n.activity = 1.0;
                }
                room -= moved;
            }
        }
    }
    // Newly received gas must reach an outlet before the next pressure snapshot.
    for (&(x, y, z), source) in &before {
        if !source.open {
            continue;
        }
        let p = IVec3::new(x, y, z);
        let Port { kind, vent, outlet: vent_side, gas, .. } = *source;
        let Some(n) = world.factory.nodes.get(&p) else {
            continue;
        };
        if n.exhaust.iter().all(|&mass| mass == 0) {
            continue;
        }
        let draining = n.gas_vent;
        let release = kind == Kind::Chimney || draining || (!component(kind) && !DIRS.iter().any(|d| {
            let q = p + *d * size;
            before.get(&(q.x, q.y, q.z)).is_some_and(|port| component(port.kind))
        }));
        if !release {
            continue;
        }
        let target = if n.ruptured {
            outlet(world, p)
        } else if vent_side != 0 {
            let side = usize::from(vent_side) - 1;
            let adjacent = p + DIRS[side] * size;
            if before.get(&(adjacent.x, adjacent.y, adjacent.z)).is_some_and(|port| component(port.kind)) {
                continue;
            }
            face_outlet(world, p, side)
        } else if kind == Kind::Chimney {
            let top = p + IVec3::new(size / 2, size, size / 2);
            (!world.get(top).is_solid()).then_some(top)
        } else {
            outlet(world, p)
        };
        let Some(target) = target else {
            if vent && let Some(n) = world.factory.nodes.get_mut(&p) {
                n.status = "Vent blocked";
            }
            continue;
        };
        let mut stock = n.exhaust;
        if !n.ruptured && draining {
            for (i, amount) in stock.iter_mut().enumerate() {
                if !selects(gas, i) {
                    *amount = 0;
                }
            }
        }
        if stock.iter().all(|&mass| mass == 0) {
            continue;
        }
        let released = stock;
        if industry::accept(world, state, target, &mut stock) {
            if let Some(n) = world.factory.nodes.get_mut(&p) {
                for (i, amount) in released.into_iter().enumerate() {
                    state.imported[i] += u64::from(amount);
                    n.exhaust[i] -= amount;
                }
                if component(kind) {
                    n.status = if n.ruptured { "Leaking" } else { "Venting" };
                }
            }
        }
    }
    // Intake fans remove local airborne stock into the same capture chamber.
    for &(x, y, z) in before.keys() {
        let p = IVec3::new(x, y, z);
        let Some(n) = world.factory.nodes.get(&p) else {
            continue;
        };
        if !matches!(n.kind, Kind::DustFilter | Kind::WetScrubber) || processor(n) || !allows(n) {
            continue;
        }
        let Some(q) = outlet(world, p) else {
            continue;
        };
        let key = crate::world::industry::geometry::owner(world, state, q);
        let Some(m) = state.air.get_mut(&key) else {
            continue;
        };
        let Some(n) = world.factory.nodes.get_mut(&p) else {
            continue;
        };
        let mut room = GAS_CAPACITY.saturating_sub(n.exhaust.iter().sum());
        for i in 0..SPECIES {
            let amount = m[i].min(u64::from(room.min(256))) as u32;
            m[i] -= u64::from(amount);
            n.exhaust[i] += amount;
            state.withdrawn[i] += u64::from(amount);
            room -= amount;
        }
    }
}

#[cfg(test)]
mod tests;
