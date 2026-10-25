//! Builds a `System` from validated data.
use super::{BodyId, Spin, System};
use crate::data::{OrbitDef, RotationPeriod, SystemDef};
use crate::kepler::{Elements, G};
use crate::spin::lock_phase;
use crate::validate::{SystemError, validate};
use std::collections::HashMap;
use std::f64::consts::PI;

/// `BodyId` is a u16.
const MAX_BODIES: usize = u16::MAX as usize + 1;

impl System {
    /// Validates `def` and compiles it for ticks of `tick_seconds`.
    ///
    /// A barycenter's mass is the sum of its components; the authored value is ignored.
    pub fn compile(def: &SystemDef, tick_seconds: f64) -> Result<System, SystemError> {
        validate(def)?;
        let bad = |reason: &'static str| SystemError::BadOrbit {
            body: def.id.clone(),
            reason,
        };
        if !(tick_seconds.is_finite() && tick_seconds > 0.0) {
            return Err(bad("tick_seconds must be finite and positive"));
        }
        if def.bodies.len() > MAX_BODIES {
            return Err(bad("a system holds at most 65536 bodies"));
        }
        let order = parent_first(def)?;
        let slot: HashMap<&str, BodyId> = order
            .iter()
            .enumerate()
            .map(|(new, &src)| (def.bodies[src].id.as_str(), BodyId(new as u16)))
            .collect();
        let find = |name: &str| slot.get(name).copied();
        let bodies: Vec<_> = order.iter().map(|&src| &def.bodies[src]).collect();
        let n = bodies.len();
        let mut system = System {
            names: bodies.iter().map(|b| b.id.clone()).collect(),
            kinds: bodies.iter().map(|b| b.kind.clone()).collect(),
            mass: bodies.iter().map(|b| b.mass_kg).collect(),
            radius: bodies.iter().map(|b| b.radius_m).collect(),
            parent: Vec::with_capacity(n),
            elements: vec![None; n],
            period_ticks: vec![0; n],
            mean_anomaly0: vec![0.0; n],
            spin: Vec::with_capacity(n),
        };
        for body in &bodies {
            let parent = match body.parent.as_deref() {
                Some(name) => Some(find(name).ok_or_else(|| SystemError::UnknownParent {
                    body: body.id.clone(),
                    parent: name.to_string(),
                })?),
                None => None,
            };
            system.parent.push(parent);
        }
        let components = |i: usize, a: &str, b: &str| match (find(a), find(b)) {
            (Some(a), Some(b)) => Ok((a, b)),
            _ => Err(SystemError::BadBinary(bodies[i].id.clone())),
        };
        // Children first, so a barycenter that is itself a component has its mass.
        for i in (0..n).rev() {
            if let Some(binary) = &bodies[i].binary {
                let (a, b) = components(i, &binary.a, &binary.b)?;
                let (ma, mb) = (system.mass(a), system.mass(b));
                if !(ma > 0.0 && mb > 0.0) {
                    return Err(SystemError::BadBinary(bodies[i].id.clone()));
                }
                system.mass[i] = ma + mb;
            }
        }
        let rails = Rails {
            seconds_per_tick: tick_seconds * def.time_scale,
        };
        for i in 0..n {
            if let (Some(orbit), Some(parent)) = (&bodies[i].orbit, system.parent[i]) {
                let mu = G * (system.mass(parent) + system.mass[i]);
                let (elements, ticks) = rails.elements(orbit, mu);
                system.set_rail(i, elements, ticks, orbit);
            }
            if let Some(binary) = &bodies[i].binary {
                let (a, b) = components(i, &binary.a, &binary.b)?;
                system.set_binary_rails(a, b, &binary.orbit, &rails);
            }
        }
        let spins = bodies.iter().enumerate().map(|(i, body)| {
            body.rotation.as_ref().map(|r| {
                let locked = r.period == RotationPeriod::Locked;
                let mut spin = Spin {
                    tilt: r.tilt_deg.to_radians(),
                    north: r.north_deg.to_radians(),
                    period_ticks: match r.period {
                        RotationPeriod::Seconds(s) => rails.ticks(s),
                        RotationPeriod::Locked => system.period_ticks[i].max(1),
                    },
                    locked,
                    phase0: 0.0,
                };
                if locked {
                    let (relative, _) = system.relative_state(BodyId(i as u16), 0);
                    spin.phase0 = lock_phase(&spin, relative);
                }
                spin
            })
        });
        system.spin = spins.collect();
        Ok(system)
    }

    fn set_rail(&mut self, i: usize, elements: Elements, ticks: u64, orbit: &OrbitDef) {
        self.elements[i] = Some(elements);
        self.period_ticks[i] = ticks;
        self.mean_anomaly0[i] = orbit.mean_anomaly_deg.to_radians();
    }

    /// Splits a binary's relative orbit between its two components.
    fn set_binary_rails(&mut self, a: BodyId, b: BodyId, orbit: &OrbitDef, rails: &Rails) {
        let (ma, mb) = (self.mass(a), self.mass(b));
        let total = ma + mb;
        let (relative, ticks) = rails.elements(orbit, G * total);
        // Each component traces the relative orbit scaled by the other's share
        // of the mass; mu scales by the cube so the period stays the pair's.
        let (share_a, share_b) = (mb / total, ma / total);
        let comp_a = Elements {
            a: relative.a * share_a,
            mu: relative.mu * share_a.powi(3),
            ..relative
        };
        let comp_b = Elements {
            a: relative.a * share_b,
            mu: relative.mu * share_b.powi(3),
            arg_peri: relative.arg_peri + PI,
            ..relative
        };
        self.set_rail(a.0 as usize, comp_a, ticks, orbit);
        self.set_rail(b.0 as usize, comp_b, ticks, orbit);
    }
}

/// Converts authored periods to whole ticks.
struct Rails {
    seconds_per_tick: f64,
}

impl Rails {
    /// Whole ticks nearest to `period_s`, at least 1.
    fn ticks(&self, period_s: f64) -> u64 {
        // `as` saturates, so an absurdly long period stays finite.
        (period_s / self.seconds_per_tick).round().max(1.0) as u64
    }

    /// Elements on the whole-tick period, so velocity matches the tick phase.
    fn elements(&self, orbit: &OrbitDef, mu: f64) -> (Elements, u64) {
        let ticks = self.ticks(orbit.period_s);
        let period = ticks as f64 * self.seconds_per_tick;
        let elements = Elements::from_period(
            period,
            mu,
            orbit.e,
            orbit.i_deg.to_radians(),
            orbit.node_deg.to_radians(),
            orbit.peri_deg.to_radians(),
        );
        (elements, ticks)
    }
}

/// Source indices sorted by depth, so every parent precedes its children.
fn parent_first(def: &SystemDef) -> Result<Vec<usize>, SystemError> {
    let index: HashMap<&str, usize> = def
        .bodies
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id.as_str(), i))
        .collect();
    let mut depth = Vec::with_capacity(def.bodies.len());
    for body in &def.bodies {
        let mut at = body;
        let mut d = 0usize;
        while let Some(parent) = at.parent.as_deref() {
            d += 1;
            // Validation rejects cycles; this bound keeps a bad input finite.
            if d > def.bodies.len() {
                return Err(SystemError::Cycle(body.id.clone()));
            }
            at = index.get(parent).map(|&i| &def.bodies[i]).ok_or_else(|| {
                SystemError::UnknownParent {
                    body: at.id.clone(),
                    parent: parent.to_string(),
                }
            })?;
        }
        depth.push(d);
    }
    let mut order: Vec<usize> = (0..def.bodies.len()).collect();
    order.sort_by_key(|&i| depth[i]);
    Ok(order)
}
