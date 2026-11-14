//! Dressing that follows the spaces themselves: stone courses, pilasters
//! and portal arches in the walls, a cornice under each vault, wall sconces
//! in halls, rounds and passages, and stalactites, stalagmites and rubble in
//! the caverns. Sconces are never solid and only hang where the wall behind
//! them is.
use super::index::Cell;
use super::shape::{At, xz};
use super::{Layout, Mech, Shape, Space, Tunnel, palette, stone};
use crate::undercrown::plan::Style;
use crate::{BlockId as B, hash2, value_noise3};
use glam::Vec3;

/// Spacing of sconces along a wall, metres.
const SCONCE: f32 = 6.0;
/// Spacing of sconces along a passage, metres.
const PASSAGE: f32 = 8.0;

/// Distance from `along` to the nearest of evenly spaced points on a wall
/// of length `len`, about `spacing` apart and centred on the wall.
fn rhythm(along: f32, len: f32, spacing: f32) -> f32 {
    let n = (len / spacing).round().max(1.0);
    let s = len / n;
    ((along.rem_euclid(s)) - s * 0.5).abs()
}

/// True in the one voxel, counted from the wall's start, that holds the
/// nearest of those points.
fn on_beat(along: f32, len: f32, spacing: f32, v: f32) -> bool {
    let n = (len / spacing).round().max(1.0);
    let s = len / n;
    let target = ((along / s).floor().clamp(0.0, n - 1.0) + 0.5) * s;
    (along / v).floor() == (target / v).floor()
}

/// Which wall of a hall a point faces: (distance past the wall, distance
/// along it, wall length, outward unit step).
fn face(x0: f32, x1: f32, z0: f32, z1: f32, m: Vec3) -> Option<(f32, f32, f32, Vec3)> {
    let in_x = m.x >= x0 && m.x < x1;
    let in_z = m.z >= z0 && m.z < z1;
    if in_z && !in_x {
        return Some(if m.x < x0 { (x0 - m.x, m.z - z0, z1 - z0, Vec3::NEG_X) } else { (m.x - x1, m.z - z0, z1 - z0, Vec3::X) });
    }
    if in_x && !in_z {
        return Some(if m.z < z0 { (z0 - m.z, m.x - x0, x1 - x0, Vec3::NEG_Z) } else { (m.z - z1, m.x - x0, x1 - x0, Vec3::Z) });
    }
    None
}

/// The wall torch whose support lies one step along `dir`.
fn torch_facing(dir: Vec3) -> B {
    if dir.x > 0.5 {
        B::TorchWallWest
    } else if dir.x < -0.5 {
        B::TorchWallEast
    } else if dir.z > 0.5 {
        B::TorchWallNorth
    } else {
        B::TorchWallSouth
    }
}

impl Layout {
    /// A wall torch at `m` if the cell one voxel along `dir` is solid.
    pub(super) fn sconce(&self, m: Vec3, v: f32, mech: Mech, dir: Vec3) -> Option<B> {
        self.sample_with(m + dir * v, v, mech, false).is_some_and(|b| b.is_solid()).then(|| torch_facing(dir))
    }

    /// Shell stone for a point of rock whose first owner is `owner`.
    pub(super) fn wall(&self, cell: &Cell, owner: Owner, style: Style, m: Vec3, v: f32) -> B {
        let [_, alt, trim, _] = palette(style);
        // Portal arches: the ring of a passage's shell where it meets a room.
        for &ti in &cell.tunnels {
            let t = &self.tunnels[ti as usize];
            if t.at(m) != At::Shell {
                continue;
            }
            let f = t.frame(m);
            if f.lateral >= f.half + 1.0 || m.y >= f.floor + f.roof + 1.0 || m.y < f.floor {
                continue;
            }
            let d = (xz(t.b) - xz(t.a)).normalize_or_zero();
            let step = Vec3::new(d.x, 0.0, d.y);
            for q in [m + step, m - step] {
                if self.index.at(q).spaces.iter().any(|&si| self.spaces[si as usize].at(q) == At::Void) {
                    return palette(t.style)[2];
                }
            }
        }
        if self.version >= super::wilds::VERSION
            && let Owner::Space(si) = owner
            && let Some(b) = super::wilds::natural(&self.spaces[si], m)
        {
            return b;
        }
        if let Owner::Space(si) = owner
            && let Shape::Hall { x0, x1, z0, z1, floor, h, .. } = self.spaces[si].shape
            && let Some((_, along, len, _)) = face(x0, x1, z0, z1, m)
        {
            let fy = m.y - floor;
            if (0.0..1.0).contains(&fy) || (h - 0.6..h).contains(&fy) {
                return trim;
            }
            if fy >= 0.0 && fy < h && rhythm(along, len, SCONCE) < 0.6 {
                return alt;
            }
        }
        if self.version >= super::glow::VERSION && super::glow::seam(style, m) {
            return B::Crystal;
        }
        stone(style, m, v)
    }

    /// Dressing inside a hall's void: the cornice and the sconces.
    pub(super) fn hall_dress(&self, s: &Space, cell: &Cell, m: Vec3, v: f32, mech: Mech) -> Option<B> {
        let Shape::Hall { x0, x1, z0, z1, floor, h, .. } = s.shape else {
            return None;
        };
        let fy = m.y - floor;
        let near = |w: f32| w >= 0.0 && w < v.max(0.5);
        let walls = [(m.x - x0, m.z - z0, z1 - z0, Vec3::NEG_X), (x1 - m.x, m.z - z0, z1 - z0, Vec3::X), (m.z - z0, m.x - x0, x1 - x0, Vec3::NEG_Z), (z1 - m.z, m.x - x0, x1 - x0, Vec3::Z)];
        if h >= 4.0 && (h - 0.6..h - 0.6 + v.max(0.5)).contains(&fy) && walls.iter().any(|w| near(w.0)) {
            let clear = cell.tunnels.iter().all(|&ti| self.tunnels[ti as usize].at(m) == At::Out);
            if clear {
                return Some(palette(s.style)[2]);
            }
        }
        let band = s.plinth(floor) - 0.6;
        if !(band..band + v).contains(&fy) {
            return None;
        }
        for (d, along, len, dir) in walls {
            if d >= 0.0 && d < v && along > 1.0 && along < len - 1.0 && on_beat(along, len, SCONCE, v) {
                return self.sconce(m, v, mech, dir);
            }
        }
        None
    }

    /// Sconces round a domed room, eight to the circle.
    pub(super) fn round_dress(&self, x: f32, z: f32, r: f32, floor: f32, m: Vec3, v: f32, mech: Mech) -> Option<B> {
        let (dx, dz) = (m.x - x, m.z - z);
        let d = dx.hypot(dz);
        if d < r - v || !(2.0..2.0 + v).contains(&(m.y - floor)) {
            return None;
        }
        let sector = std::f32::consts::TAU / 8.0;
        let off = (dz.atan2(dx).rem_euclid(sector) - sector * 0.5).abs() * r;
        if off >= v * 0.75 {
            return None;
        }
        let dir = if dx.abs() >= dz.abs() { Vec3::X * dx.signum() } else { Vec3::Z * dz.signum() };
        self.sconce(m, v, mech, dir)
    }

    /// Sconces along a passage wall, alternating sides every `PASSAGE` metres.
    pub(super) fn passage_dress(&self, t: &Tunnel, m: Vec3, v: f32, mech: Mech) -> Option<B> {
        if t.gate.is_some() || t.sealed {
            return None;
        }
        let f = t.frame(m);
        let len = t.length();
        if len < 6.0 || f.lateral < f.half - v || !(1.9..1.9 + v).contains(&(m.y - f.floor)) {
            return None;
        }
        let along = f.t * len;
        if along < 2.0 || along > len - 2.0 || !on_beat(along, len, PASSAGE, v) {
            return None;
        }
        let k = (along / (len / (len / PASSAGE).round().max(1.0))).floor() as i32;
        if (k.rem_euclid(2) == 0) != (f.side > 0.0) {
            return None;
        }
        let (a, b) = (xz(t.a), xz(t.b));
        let n = (b - a).perp().normalize_or_zero() * f.side;
        let dir = if n.x.abs() >= n.y.abs() { Vec3::X * n.x.signum() } else { Vec3::Z * n.y.signum() };
        self.sconce(m, v, mech, dir)
    }

    /// Stalactites, stalagmites and rubble in a cavern's void, kept off the
    /// passages, features, other spaces and props that share the cavern.
    pub(super) fn cave_dress(&self, si: usize, cell: &Cell, m: Vec3, v: f32) -> Option<B> {
        let s = &self.spaces[si];
        let Shape::Cave { c, rad, floor } = s.shape else {
            return None;
        };
        const GRID: f32 = 2.5;
        let (gx, gz) = ((m.x / GRID).floor() as i32, (m.z / GRID).floor() as i32);
        let roll = hash2(gx, gz, 4801);
        let size = hash2(gx, gz, 4803);
        let jx = (hash2(gx, gz, 4805) - 0.5) * 1.0;
        let jz = (hash2(gx, gz, 4807) - 0.5) * 1.0;
        let (cx, cz) = ((gx as f32 + 0.5) * GRID + jx, (gz as f32 + 0.5) * GRID + jz);
        let qh = ((cx - c.x) / rad.x).hypot((cz - c.z) / rad.z);
        let d = (m.x - cx).hypot(m.z - cz);
        let fy = m.y - floor;
        let [wall, alt, ..] = palette(s.style);
        let lit = matches!(s.style, Style::Ice | Style::Fungal) || (self.version >= super::wilds::VERSION && s.style == Style::Sea);
        let glow = lit && hash2(gx, gz, 4809) > 0.8;
        let body = |n: f32| match s.style {
            Style::Ice => B::Ice,
            _ if n > 0.7 => alt,
            _ => wall,
        };
        let rubble = qh > 0.72 && fy < v.max(0.6) && value_noise3(m.x * 0.7, 0.0, m.z * 0.7, 4811) > 0.7;
        let mite = roll > 0.7 && qh > 0.66 && qh < 0.98;
        let tite = roll < 0.3 && qh < 0.92;
        if !(rubble || mite || tite) || !self.keeps_clear(si, cell, m) {
            return None;
        }
        if mite {
            let tall = 0.8 + size * 3.2;
            let base = 0.45 + size * 0.45;
            if fy < tall && d < (base * (1.0 - fy / tall)).max(v * 0.5) {
                return Some(if glow && fy > tall * 0.6 { B::Crystal } else { body(size) });
            }
        }
        if tite {
            let top = c.y + rad.y * (1.0 - qh * qh).max(0.0).sqrt();
            let clearance = (rad.y * 0.6).max(5.0);
            let tip = (top - 1.0 - size * 5.0).max(floor + clearance);
            if m.y >= tip && tip < top && d < ((0.35 + size * 0.5) * (m.y - tip) / (top - tip).max(0.5)).max(v * 0.5) {
                return Some(if glow && m.y < tip + 0.8 { B::Crystal } else { body(1.0 - size) });
            }
        }
        rubble.then_some(B::Cobblestone)
    }

    /// No formation where a passage, another space, a feature or a prop is.
    fn keeps_clear(&self, si: usize, cell: &Cell, m: Vec3) -> bool {
        let pad = Vec3::new(1.5, 0.0, 1.5);
        cell.tunnels.iter().all(|&ti| self.tunnels[ti as usize].at(m) == At::Out)
            && cell.spaces.iter().all(|&o| o as usize == si || self.spaces[o as usize].at(m) == At::Out)
            && cell.features.iter().all(|&fi| {
                let (lo, hi) = self.features[fi as usize].bounds();
                m.x < lo.x - pad.x || m.x >= hi.x + pad.x || m.z < lo.z - pad.z || m.z >= hi.z + pad.z
            })
            && self.clear.iter().all(|p| (p.x - m.x).hypot(p.z - m.z) > 3.0)
    }
}

/// Which space or passage owns a point of shell.
#[derive(Clone, Copy)]
pub(super) enum Owner {
    Space(usize),
    Tunnel,
}
