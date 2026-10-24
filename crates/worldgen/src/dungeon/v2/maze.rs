//! The labyrinth of false walls.
use super::{Complex, Room};
use crate::{BlockId as B, hash2};

/// Binary-tree maze on a 3 m grid; half its walls are illusions.
pub fn labyrinth(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    const G: f32 = 3.0;
    let (nx, nz) = ((r.w() / G).floor() as i32, (r.d() / G).floor() as i32);
    let (ci, cj) = ((rx / G).floor() as i32, (rz / G).floor() as i32);
    let (lx, lz) = (rx - ci as f32 * G, rz - cj as f32 * G);
    let opens_x = |i: i32, j: i32| -> bool {
        if j >= nz - 1 { true } else if i >= nx - 1 { false } else { hash2(i, j, r.salt.wrapping_add(31)) < 0.5 }
    };
    if fy >= r.h || ci <= 0 && cj <= 0 {
        return None;
    }
    let wall = |seg: (i32, i32, bool)| {
        if hash2(seg.0 * 2 + seg.2 as i32, seg.1, r.salt.wrapping_add(37)) < 0.5 { B::MirageBrick } else { c.pal.wall }
    };
    if lx < v && lz < v && ci > 0 && cj > 0 {
        return Some(c.pal.trim);
    }
    // A lamp in one cell in four, tucked into a corner beside its post.
    if hash2(ci, cj, r.salt.wrapping_add(39)) > 0.75 && (v..2.0 * v).contains(&lx) && (v..2.0 * v).contains(&lz) && fy < v {
        return Some(B::Torch);
    }
    if lx < v && ci > 0 && ci < nx && !opens_x(ci - 1, cj) {
        return Some(wall((ci, cj, true)));
    }
    if lz < v && cj > 0 && cj < nz && opens_x(ci, cj - 1) {
        return Some(wall((ci, cj, false)));
    }
    None
}
