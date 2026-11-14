//! Generator 51 detail that resolves with the voxel size, as the dungeon's
//! generator 48 layer does: projecting plinths and cornices on hall walls,
//! chandeliers in the large halls, jointed flagstones on every floor and a
//! kerb along each passage. Measures are metres on voxel centres, so a
//! quarter-metre moulding is one voxel at 0.25 m, rounds to one at 0.5 m and
//! is absent at 1 m.
use super::{Layout, Shape, Space, Style, palette};
use crate::BlockId as B;
use glam::Vec3;

/// First generator with the fine layer.
pub const VERSION: u32 = crate::gates::FINE_ARCHITECTURE_GENERATOR;
/// Height the chandelier plate hangs at over the floor, metres.
const LAMP: f32 = 3.2;

impl Layout {
    /// Mouldings and a chandelier inside a hall's void; `None` keeps the
    /// hall's own dressing.
    pub(super) fn fine_hall(&self, s: &Space, m: Vec3, v: f32) -> Option<B> {
        let Shape::Hall { x0, x1, z0, z1, floor, h, .. } = s.shape else {
            return None;
        };
        if self.version < VERSION || v > 0.5 || s.sealed {
            return None;
        }
        let fy = m.y - floor;
        let wall = (m.x - x0).min(x1 - m.x).min(m.z - z0).min(z1 - m.z);
        let trim = palette(s.style)[2];
        // Wet halls keep their plinth band clear of the water line.
        let dry = s.water.is_none();
        let plinth = if !dry { 0.0 } else if fy < 0.3 { 0.45 } else if fy < 0.6 { 0.3 } else { 0.0 };
        let below = h - fy;
        let cornice = if h >= 4.0 && below > 0.0 { if below < 0.3 { 0.45 } else if below < 0.6 { 0.3 } else { 0.0 } } else { 0.0 };
        if wall >= 0.0 && wall < f32::max(plinth, cornice) {
            return Some(trim);
        }
        self.chandelier(s, (x0, x1, z0, z1), floor, h, m, v)
    }

    fn chandelier(&self, s: &Space, (x0, x1, z0, z1): (f32, f32, f32, f32), floor: f32, h: f32, m: Vec3, v: f32) -> Option<B> {
        let (cx, cz) = ((x0 + x1) * 0.5, (z0 + z1) * 0.5);
        if (x1 - x0).min(z1 - z0) < 9.0 || h < LAMP + 1.5 || m.y - floor < LAMP - v {
            return None;
        }
        // Props stand under the middle of some halls; the lamp gives way.
        if self.clear.iter().any(|p| (p.x - cx).hypot(p.z - cz) < 2.5) {
            return None;
        }
        let (dx, dz, fy) = (m.x - cx, m.z - cz, m.y - floor);
        let ring = dx.hypot(dz);
        let plate = (LAMP / v).round() * v;
        let [_, _, trim, _] = palette(s.style);
        if fy < plate {
            return (ring < 1.45 && !(0.5..0.9).contains(&ring)).then_some(trim);
        }
        let candle = |a: f32, b: f32| a.abs() < v * 0.5 + 0.01 && (b.abs() - 1.05).abs() < v * 0.5 + 0.01;
        if fy < plate + v && (candle(dx, dz) || candle(dz, dx)) {
            return Some(B::Torch);
        }
        if dx.abs() < v * 0.5 + 0.01 && dz.abs() < v * 0.5 + 0.01 {
            let link = ((fy / v) as i32).rem_euclid(2) == 0;
            return Some(if link { B::SilverBlock } else { trim });
        }
        None
    }

    /// A kerb along a passage's floor edges, a quarter metre high.
    pub(super) fn fine_kerb(&self, style: Style, lateral: f32, half: f32, above: f32, v: f32) -> Option<B> {
        (self.version >= VERSION && v <= 0.5 && half - lateral.abs() < 0.3 && above < 0.25).then(|| palette(style)[2])
    }
}

/// Floor stone: staggered flags with trim joints, enlarged at coarse voxel
/// scales so each slab retains an interior; `None` keeps the plain floor.
pub(super) fn flagstone(version: u32, style: Style, m: Vec3, v: f32) -> Option<B> {
    if version < VERSION || v > 1.0 {
        return None;
    }
    let [_, _, trim, floor] = palette(style);
    // Leave room for a flag's interior even at the one-metre voxel scale.
    let width = 1.5_f32.max(v * 3.0);
    let depth = 2.0_f32.max(v * 4.0);
    let course = (m.z / depth).floor() as i32;
    let shift = if course.rem_euclid(2) == 0 { 0.0 } else { width * 0.5 };
    let near = |a: f32, period: f32| {
        let d = (a - v * 0.5).rem_euclid(period);
        d.min(period - d) < v * 0.5
    };
    if near(m.x + shift, width) || near(m.z, depth) {
        return Some(trim);
    }
    Some(floor)
}
