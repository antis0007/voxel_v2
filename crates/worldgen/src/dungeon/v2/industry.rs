//! Real material cells in contained work bays, not decorative liquid surfaces.
use super::{Kind, Room};
use crate::BlockId as B;

pub(super) fn furnish(r: &Room, x: f32, z: f32, y: f32, v: f32) -> Option<B> {
    if r.kind != Kind::Forge || y < 0.0 { return None; }
    let (a, b, len, wid) = if r.w() >= r.d() {
        (x, z, r.w(), r.d())
    } else { (z, x, r.d(), r.w()) };
    // Opposed hot and cold bays leave the centre cross and wall approaches open.
    if (a - len * 0.5).abs() < 1.5 || (b - wid * 0.5).abs() < 1.5 { return None; }
    let ca = len * 0.28;
    let hot = b < wid * 0.5;
    let cb = if hot { 3.5 } else { wid - 3.5 };
    let (dx, dz) = ((a - ca).abs(), (b - cb).abs());
    if dx < 2.5 && dz < 2.0 {
        let rim = dx >= 2.5 - v.max(0.75) || dz >= 2.0 - v.max(0.75);
        if y < v { return Some(B::Basalt); }
        if y < v + 1.0 {
            return Some(if rim { B::Basalt } else if hot { B::Lava } else { B::Water });
        }
        if y < 3.5 { return Some(B::Air); }
    }
    // Workbench and feedstock are on the other side of the room cross.
    if (a - len * 0.73).abs() < 1.0 && (b - cb).abs() < 1.0 && y < v {
        return Some(if hot { B::Smelter } else { B::Workbench });
    }
    if (a - len * 0.73).abs() < 1.5 && (b - cb - 2.0).abs() < 0.5
        && y < v { return Some(if hot { B::CoalOre } else { B::IronOre }); }
    None
}
