//! Sparse heat exchange, finite fuel, and conservative gas transport.
use super::{
    cellular::{admission, changed},
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
fn fuel(b: B) -> f32 {
    match b {
        B::Log => 160.0,
        B::Planks => 100.0,
        B::Leaves | B::PineLeaves | B::Bush => 30.0,
        B::MushroomStem => 60.0,
        _ => 0.0,
    }
}
pub(super) fn step(
    s: &mut Residency,
    w: &mut World,
    p: IVec3,
    visited: &mut HashSet<IVec3>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> usize {
    let Some((b, _)) = s.read(w, p) else { return 0 };
    let mut positions = vec![p];
    positions.extend(
        DIRS.into_iter()
            .map(|d| p + d)
            .filter(|q| super::cellular::allowed(w, *q)),
    );
    let mut ready = true;
    for &q in &positions {
        ready &= s.request_for(w, chunk(q), chunk(p));
    }
    if !ready || !w.sandbox.can_admit(&[p]) {
        w.activity.wake(p);
        return 0;
    }
    let mut c = w.sandbox.cell(p);
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
    // Equal heat capacity lattice: each pair exchanges equal/opposite energy.
    // One sixth of the difference prevents overshoot even with six neighbours.
    for &q in &positions[1..] {
        if !w.sandbox.can_admit(&[q]) {
            continue;
        }
        let mut n = w.sandbox.cell(q);
        let transfer = (c.temperature - n.temperature) * 0.04;
        if transfer.abs() > 0.01 {
            c.temperature -= transfer;
            n.temperature += transfer;
            w.sandbox.set(q, n);
            w.activity.wake(q);
        }
    }
    // The atmosphere is an open ambient heat reservoir, with slow cooling.
    c.temperature += (20.0 - c.temperature)
        * if matches!(b, B::Air | B::Steam | B::Smoke) {
            0.025
        } else {
            0.002
        };
    w.sandbox.set(p, c);
    let mut count = 0;
    let mut replace = None;
    if fuel(b) > 0.0 && c.temperature >= 260.0 {
        c.fuel = fuel(b);
        replace = Some(B::Fire);
    }
    if b == B::Fire {
        let oxygen = positions[1..]
            .iter()
            .any(|&q| s.read(w, q).is_some_and(|(b, _)| b == B::Air));
        let wet = positions[1..]
            .iter()
            .any(|&q| s.read(w, q).is_some_and(|(b, _)| b == B::Water));
        if wet || !oxygen {
            c.temperature = c.temperature.min(180.0);
            replace = Some(B::Ash);
            c.fuel = 0.0;
        } else {
            // Fuel supplies enough heat to overcome six-face diffusion and ignite neighbours.
            c.temperature = (c.temperature + 250.0).min(950.0);
            c.fuel = (c.fuel - 1.0).max(0.0);
            if c.fuel == 0.0 {
                replace = Some(B::Ash);
            } else if w.activity.tick % 4 == 0 {
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
            .find(|&q| s.read(w, q).is_some_and(|(nb, _)| nb == B::Water));
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
    if b == B::Ice && c.temperature >= 35.0 {
        replace = Some(B::Water);
        c.gas = 8;
        c.temperature = 12.0;
    }
    if b == B::Water && c.temperature >= 100.0 {
        replace = Some(B::Steam);
        c.gas = s.read(w, p).unwrap().1;
        c.temperature = 100.0;
    }
    if b == B::Steam {
        if c.gas == 0 {
            c.gas = 8;
        }
        if c.temperature < 75.0 {
            replace = Some(B::Water);
        }
    }
    if let Some(id) = replace {
        if s.prepare(w, &[p]) {
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
    w.sandbox.set(p, c);
    if replace.is_none() && matches!(b, B::Smoke | B::Steam) {
        if c.gas == 0 {
            c.gas = 8;
            w.sandbox.set(p, c);
        }
        // Vent only at the explicit open upper world boundary.
        if p.y + 1 >= w.sample_config().max_world_y() && s.prepare(w, &[p]) {
            w.set(p, B::Air);
            w.sandbox.set(p, ThermalCell::default());
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
            if moved == 0
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
            w.sandbox.set(q, n);
            if c.gas == 0 {
                w.set(p, B::Air);
                c = ThermalCell::default();
            }
            w.sandbox.set(p, c);
            visited.insert(q);
            changed(p, blocks, chunks);
            changed(q, blocks, chunks);
            count += 2;
            break;
        }
    }
    if active(b) || (c.temperature - 20.0).abs() >= 0.25 {
        w.activity.wake(p);
    }
    count
}
