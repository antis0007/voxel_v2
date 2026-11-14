//! Generator 51 facade detail that resolves with the voxel size. Openings get
//! framed (sills, jambs, lintels, door architraves) and windows a glazed
//! pane set back in the wall with a mullion cross; storeys get a string
//! course; at a quarter metre the wall between pilasters steps back one voxel
//! so the facade reads as panelled. At 1 m nothing changes: the wall is one
//! voxel thick and these measures fall between voxel centres.
use super::{Wing, Windows};
use crate::BlockId as B;

/// First generator with fine facades.
pub const VERSION: u32 = crate::gates::FINE_ARCHITECTURE_GENERATOR;
/// Width of the frame round an opening, metres.
const FRAME: f32 = 0.25;

/// Where a facade voxel sits.
pub(super) struct Face {
    /// Depth into the wall from the outer face, metres.
    pub depth: f32,
    /// Along the face, metres.
    pub u: f32,
    /// Along the face within one cell.
    pub along: f32,
    /// Height over this storey's floor surface.
    pub rel: f32,
    /// Height over this storey's base.
    pub fy: f32,
    pub storey: usize,
    pub side: u8,
    pub corner: bool,
}

impl Wing {
    /// Fine facade blocks, before the coarse window, plinth and wall rules.
    pub(super) fn fine_facade(&self, f: &Face, v: f32, slab: f32) -> Option<B> {
        if !self.fine || v > 0.5 || f.corner {
            return None;
        }
        // Architraves round the ground-floor doors (the openings themselves
        // were cut before this runs).
        if f.storey == 0 && f.fy >= slab {
            for door in self.doors.iter().filter(|d| d.side == f.side) {
                let off = (f.u - door.u).abs();
                if off < door.hw.max(v * 0.75) + FRAME && f.rel < door.height + FRAME {
                    return Some(self.pal.frame);
                }
            }
        }
        // String course along each floor line above the ground.
        if f.storey > 0 && f.fy < slab {
            return Some(self.pal.frame);
        }
        let centre = (f.along - self.cell * 0.5).abs();
        let window = match self.windows {
            Windows::Panes => Some((v.max(0.55), 0.9, 2.3)),
            Windows::Slits => Some((v.max(0.22), 0.9, 2.7)),
            _ => None,
        };
        if let Some((half, lo, hi)) = window
            && f.fy >= slab
        {
            let opening = centre < half && f.rel > lo && f.rel < hi;
            if opening {
                // A pane set back from the face; slits stay open.
                if self.windows == Windows::Panes && (FRAME..FRAME * 2.0).contains(&f.depth) {
                    let mid = (lo + hi) * 0.5;
                    let mullion = centre < v * 0.5 + 0.01 || (f.rel - mid).abs() < v * 0.5 + 0.01;
                    return Some(if mullion { self.pal.frame } else { self.pal.glass });
                }
                return Some(B::Air);
            }
            if centre < half + FRAME && f.rel > lo - FRAME && f.rel < hi + FRAME {
                return Some(self.pal.frame);
            }
        }
        // A taller plinth with a capping course.
        if f.storey == 0 && f.rel < 1.15 {
            return Some(self.pal.plinth);
        }
        // Panels: the face steps back between pilasters at the cell lines.
        let pilaster = f.along < 0.3 || f.along > self.cell - 0.3;
        if v <= 0.25 && f.depth < FRAME && !pilaster && self.windows != Windows::Glazed && f.rel > 1.15 && f.rel < self.h - slab - 0.3 {
            return Some(B::Air);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::VERSION;

    /// Every voxel of a building's box at a scale.
    fn raster(b: &Building, v: f32) -> Vec<Option<B>> {
        let (x0, z0, x1, z1) = b.extent();
        let mut out = Vec::new();
        let mut y = -0.5 * v;
        while y < b.top + 1.0 {
            let mut z = z0 - 1.0;
            while z < z1 + 1.0 {
                let mut x = x0 - 1.0;
                while x < x1 + 1.0 {
                    out.push(b.sample(Vec3::new(x + v * 0.5, y + v * 0.5, z + v * 0.5), v, f32::MIN));
                    x += v;
                }
                z += v;
            }
            y += v;
        }
        out
    }

    fn pair(kind: u32, seed: u32) -> (Rc<Building>, Rc<Building>) {
        let old = site_building_version(kind, seed, false, VERSION - 1).expect("kind has a building");
        let new = site_building_version(kind, seed, false, VERSION).expect("kind has a building");
        (old, new)
    }

    #[test]
    fn coarse_facades_are_unchanged_and_fine_ones_gain_detail() {
        for kind in [0u32, 2, 3, 4, 5] {
            let (old, new) = pair(kind, 777 + kind);
            assert_eq!(raster(&old, 1.0), raster(&new, 1.0), "kind {kind}: 1 m buildings must not change");
            let (a, b) = (raster(&old, 0.25), raster(&new, 0.25));
            let changed = a.iter().zip(&b).filter(|(p, q)| p != q).count();
            assert!(changed > 200, "kind {kind}: only {changed} voxels gained detail at 0.25 m");
        }
    }

    #[test]
    fn fine_facades_never_close_a_doorway() {
        for kind in [0u32, 2, 3, 4, 5] {
            for &v in &[0.5f32, 0.25] {
                let (old, new) = pair(kind, 4242 + kind);
                for w in &new.wings {
                    for door in &w.doors {
                        // March through the wall at the door's middle, from
                        // outside to a metre in, at walking heights.
                        let slab = v.max(0.4);
                        for k in 0..((1.6 / v) as i32) {
                            let depth = -0.3 + k as f32 * v;
                            for h in [0.3f32, 1.0, 1.7] {
                                let (lx, lz) = match door.side {
                                    0 => (door.u, depth),
                                    1 => (door.u, w.depth() - depth),
                                    2 => (depth, door.u),
                                    _ => (w.width() - depth, door.u),
                                };
                                let p = Vec3::new(w.x0 + lx, slab + h, w.z0 + lz);
                                let (a, b) = (old.sample(p, v, f32::MIN), new.sample(p, v, f32::MIN));
                                if a == Some(B::Air) || a.is_none() {
                                    assert!(b == a || b == Some(B::Air), "kind {kind} at {v}: refinement filled door cell {p} with {b:?}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
