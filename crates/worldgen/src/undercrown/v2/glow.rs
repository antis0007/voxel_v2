//! Generator 36 cavern light: wall lamps round every cavern at a walking
//! height above whatever ground lies under them, and seams of crystal through
//! cliff rock. Before 36 a cavern had no lamp of its own, so the Crown Chasm
//! was lit only by the rooms that open onto it.
use super::{Layout, Mech, Shape, Space, Style};
use crate::{BlockId as B, value_noise3};
use glam::Vec3;

/// First generator with cavern lamps and crystal seams.
pub const VERSION: u32 = 36;
/// Spacing of lamps round a cavern wall, metres.
const LAMP: f32 = 7.0;

impl Layout {
    /// A wall lamp in a cavern's void: on the beat round the wall, against
    /// rock, one and a half to two and a half metres over the ground below.
    pub(super) fn cave_lamp(&self, s: &Space, m: Vec3, v: f32, mech: Mech) -> Option<B> {
        let Shape::Cave { c, rad, .. } = s.shape else {
            return None;
        };
        let (nx, nz) = ((m.x - c.x) / rad.x, (m.z - c.z) / rad.z);
        if nx.hypot(nz) < 0.6 {
            return None;
        }
        let arc = nz.atan2(nx) * (rad.x + rad.z) * 0.5;
        let off = (arc / LAMP - (arc / LAMP).round()).abs() * LAMP;
        if off >= v * 0.75 {
            return None;
        }
        let dir = if nx.abs() >= nz.abs() { Vec3::X * nx.signum() } else { Vec3::Z * nz.signum() };
        let solid = |p: Vec3| self.sample_with(p, v, mech, false).is_some_and(|b| b.is_solid());
        let low = (2.5 / v).round() * v;
        if !solid(m - Vec3::Y * low) || (1..(low / v) as i32).any(|k| solid(m - Vec3::Y * (k as f32 * v))) {
            return None;
        }
        self.sconce(m, v, mech, dir)
    }
}

/// A thin seam of crystal through cliff rock, in clusters.
pub(super) fn seam(style: Style, m: Vec3) -> bool {
    if style != Style::Cliff {
        return false;
    }
    let ridge = (value_noise3(m.x * 0.09, m.y * 0.09, m.z * 0.09, 6101) - 0.5).abs();
    ridge < 0.02 && value_noise3(m.x * 0.03, m.y * 0.03, m.z * 0.03, 6103) > 0.55
}
