//! Body orientation from spin: axis tilt, spin phase and tidal lock.
use crate::kepler::Elements;
use crate::system::{BodyId, Spin, System};
use glam::{DQuat, DVec3};
use std::f64::consts::{FRAC_PI_2, TAU};

impl Spin {
    /// Rotation carrying body +Y onto the spin axis, before any spin.
    fn axis_frame(&self) -> DQuat {
        // A turn about -Z leans +Y toward +X by the tilt; the turn about +Y
        // then carries +X to the `north` azimuth (+X toward -Z).
        DQuat::from_rotation_y(self.north) * DQuat::from_rotation_z(-self.tilt)
    }

    /// Spin angle about body +Y at `tick`, radians.
    fn angle(&self, tick: u64) -> f64 {
        // Integer reduction first, the same no-drift rule as the orbit phase.
        let period = self.period_ticks.max(1);
        self.phase0 + TAU * ((tick % period) as f64 / period as f64)
    }
}

impl System {
    /// Rotation from the body frame to the system frame at `tick`.
    ///
    /// Body +Y is the spin axis and the body turns positively about it.
    /// A body without rotation data keeps the system frame.
    pub fn orientation(&self, b: BodyId, tick: u64) -> DQuat {
        match self.spin(b) {
            Some(spin) => spin.axis_frame() * DQuat::from_rotation_y(spin.angle(tick)),
            None => DQuat::IDENTITY,
        }
    }
}

/// Aligns a locked spin with its orbit and faces +X at the parent.
///
/// The axis becomes the orbit normal `(sin i·sin node, cos i, sin i·cos node)`,
/// which is +Y tilted by `i` toward azimuth `node - 90°`; the authored tilt and
/// north are ignored. `relative` is the body's parent-relative position at
/// tick 0. The spin runs at the mean motion, so the lock is exact on a
/// circular orbit and librates on an eccentric one.
pub(crate) fn lock_to_orbit(spin: &mut Spin, elements: &Elements, relative: DVec3) {
    spin.tilt = elements.i;
    spin.north = elements.node - FRAC_PI_2;
    let toward = spin.axis_frame().inverse() * -relative;
    // A turn of φ about +Y carries +X to (cos φ, 0, -sin φ). The parent
    // direction lies in the orbit plane, across the axis, so this is exact.
    spin.phase0 = f64::atan2(-toward.z, toward.x);
}

#[cfg(test)]
mod tests;
