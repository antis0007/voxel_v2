//! Compiled star system: bodies in parent-first order, evaluated per tick.
//!
//! Positions are f64 rails for rendering and the map. Sim-facing values will
//! be fixed-point once a consumer exists (space design 8a.4).
use crate::data::BodyKind;
use crate::kepler::{Elements, G};
use glam::DVec3;
use std::f64::consts::TAU;

mod compile;

/// Index of a body in a compiled system's parent-first storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BodyId(pub u16);

/// Compiled spin of one body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spin {
    /// Axis tilt from system +Y, radians.
    ///
    /// A locked body takes its orbit's.
    pub tilt: f64,
    /// Direction the axis leans, radians in the XZ plane from +X toward -Z;
    /// a locked body's comes from its orbit.
    pub north: f64,
    /// Ticks per turn, at least 1. A locked body uses its own orbit's period
    /// in ticks, or its binary's for a component.
    pub period_ticks: u64,
    /// Spin period follows the orbit: the body's own, or its binary's for
    /// a component.
    pub locked: bool,
    /// Spin angle at tick 0, radians; turns a locked body's +X to its parent.
    pub phase0: f64,
}

/// One star system on rails, stored as columns in parent-first order.
///
/// Every column is indexed by `BodyId.0`, and a parent's index is always
/// below its children's, so one forward pass evaluates the hierarchy.
#[derive(Clone, Debug)]
pub struct System {
    names: Vec<String>,
    kinds: Vec<BodyKind>,
    /// Barycenters hold the sum of their two components.
    mass: Vec<f64>,
    radius: Vec<f64>,
    parent: Vec<Option<BodyId>>,
    /// Parent-relative orbit; `None` only for the root.
    elements: Vec<Option<Elements>>,
    /// At least 1 wherever `elements` is `Some`, 0 elsewhere.
    period_ticks: Vec<u64>,
    /// Mean anomaly at tick 0, radians.
    mean_anomaly0: Vec<f64>,
    spin: Vec<Option<Spin>>,
}

/// Positions (m) and velocities of every body at one tick.
///
/// Indexed by `BodyId.0`, system frame, root at the origin. Prefer the
/// `position` and `velocity` accessors to indexing the fields. Velocities are
/// metres per system second; multiply by the system's `time_scale` for metres
/// per tick second.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub position: Vec<DVec3>,
    pub velocity: Vec<DVec3>,
}

impl Snapshot {
    /// Position of `b` in the system frame, metres.
    pub fn position(&self, b: BodyId) -> DVec3 {
        self.position[b.0 as usize]
    }

    /// Velocity of `b` in the system frame, metres per system second.
    pub fn velocity(&self, b: BodyId) -> DVec3 {
        self.velocity[b.0 as usize]
    }
}

impl System {
    /// Number of bodies.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// True when there are no bodies, which a compiled system never is.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Finds a body by its data id.
    pub fn id(&self, name: &str) -> Option<BodyId> {
        let i = self.names.iter().position(|n| n == name)?;
        Some(BodyId(i as u16))
    }

    /// Data id of a body.
    pub fn name(&self, b: BodyId) -> &str {
        &self.names[b.0 as usize]
    }

    /// Kind of a body as authored.
    pub fn kind(&self, b: BodyId) -> &BodyKind {
        &self.kinds[b.0 as usize]
    }

    /// Body the given body orbits; `None` for the root.
    pub fn parent(&self, b: BodyId) -> Option<BodyId> {
        self.parent[b.0 as usize]
    }

    /// Mass in kg; a barycenter's is the sum of its components.
    pub fn mass(&self, b: BodyId) -> f64 {
        self.mass[b.0 as usize]
    }

    /// Radius in metres.
    pub fn radius(&self, b: BodyId) -> f64 {
        self.radius[b.0 as usize]
    }

    /// Spin data, if the body rotates.
    pub fn spin(&self, b: BodyId) -> Option<Spin> {
        self.spin[b.0 as usize]
    }

    /// Parent-relative orbit; `None` for the root.
    pub(crate) fn elements(&self, b: BodyId) -> Option<&Elements> {
        self.elements[b.0 as usize].as_ref()
    }

    /// Gravity at the surface in m/s², 0 for a body without radius.
    pub fn surface_gravity(&self, b: BodyId) -> f64 {
        let r = self.radius(b);
        if r > 0.0 {
            G * self.mass(b) / (r * r)
        } else {
            0.0
        }
    }

    /// Writes every body's state at `tick` into `out`, reusing its storage.
    pub fn evaluate(&self, tick: u64, out: &mut Snapshot) {
        let n = self.len();
        out.position.resize(n, DVec3::ZERO);
        out.velocity.resize(n, DVec3::ZERO);
        for i in 0..n {
            let (pos, vel) = self.relative_state(BodyId(i as u16), tick);
            // Parents precede children, so the parent's entry is already this tick's.
            let (base_pos, base_vel) = match self.parent[i] {
                Some(p) => (out.position[p.0 as usize], out.velocity[p.0 as usize]),
                None => (DVec3::ZERO, DVec3::ZERO),
            };
            out.position[i] = base_pos + pos;
            out.velocity[i] = base_vel + vel;
        }
    }

    /// Parent-relative position and velocity at `tick`; zero for the root.
    pub(crate) fn relative_state(&self, b: BodyId, tick: u64) -> (DVec3, DVec3) {
        let i = b.0 as usize;
        let Some(elements) = &self.elements[i] else {
            return (DVec3::ZERO, DVec3::ZERO);
        };
        // Integer reduction first, so the phase at tick 10^12 is the phase at
        // its remainder bit for bit; a float time would lose digits instead.
        let period = self.period_ticks[i].max(1);
        let fraction = (tick % period) as f64 / period as f64;
        elements.state_at(self.mean_anomaly0[i] + TAU * fraction)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_rails;
