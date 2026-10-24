//! Fixtures: colonnade columns, street lamps, braziers, the foundry's
//! furnace bank and lava fall, hanging garden lanterns, the frozen organ
//! and ice cascades. Light comes from torches (never solid, always on a
//! solid block), glow caps, crystal and lava.
use super::furnish::{Back, Here};
use super::palette;
use crate::undercrown::plan::Style;
use crate::{BlockId as B, hash2, value_noise3};

/// A square column with a wider base and capital, banded every 1.5 m.
pub(super) fn column(at: &Here, floor: f32, top: f32, style: Style) -> Option<B> {
    let [wall, alt, trim, _] = palette(style);
    let c = (at.lo + at.hi) * 0.5;
    let r = (at.m.x - c.x).abs().max((at.m.z - c.z).abs());
    let fy = at.m.y - floor;
    let ends = fy < at.snap(0.6) || at.m.y >= top - at.snap(0.6);
    let half = if ends { 1.1 } else { 0.6 };
    if r >= half && !at.centre() {
        return None;
    }
    Some(if ends {
        trim
    } else if ((fy / 1.5).floor() as i32).rem_euclid(2) == 1 {
        alt
    } else {
        wall
    })
}

/// A lamp post: a timber post, an iron cap and a torch on top.
pub(super) fn lamp(at: &Here, floor: f32, h: f32) -> Option<B> {
    let c = (at.lo + at.hi) * 0.5;
    let fy = at.m.y - floor;
    let post = at.centre() || ((at.m.x - c.x).abs() < 0.15 && (at.m.z - c.z).abs() < 0.15);
    let h = at.snap(h);
    if post && fy < h {
        return Some(if fy >= h - at.v { B::SilverBlock } else { B::Log });
    }
    (at.centre() && fy >= h && fy < h + at.v).then_some(B::Torch)
}

/// A stone pedestal with a fire on it.
pub(super) fn brazier(at: &Here, floor: f32, block: B) -> Option<B> {
    let c = (at.lo + at.hi) * 0.5;
    let fy = at.m.y - floor;
    let h = at.snap(0.9);
    let body = at.centre() || ((at.m.x - c.x).abs() < 0.45 && (at.m.z - c.z).abs() < 0.45);
    if body && fy < h {
        return Some(block);
    }
    (at.centre() && fy >= h && fy < h + at.v).then_some(B::Torch)
}

/// A bank of furnaces, one every four metres along the back wall: brick
/// bodies with a lava hearth behind a sill, open above it, and chimneys.
pub(super) fn furnaces(at: &Here, floor: f32, back: Back) -> Option<B> {
    let (dep, along, dl, _) = at.rel_to_back(back);
    // Measured from the front face.
    let front = dl - dep;
    let u = along.rem_euclid(4.0);
    if u >= 3.0 {
        return None;
    }
    let fy = at.m.y - floor;
    let body = at.snap(3.0);
    let mouth = (u - 1.5).abs() < 0.5;
    let sill = at.v.max(0.4);
    let hearth = at.snap(0.8);
    if mouth && front >= sill && dep >= 0.6 {
        if fy >= hearth && fy < hearth + at.v {
            return Some(B::Lava);
        }
        if fy >= hearth + at.v && fy < hearth + at.v + at.snap(0.9) {
            return None;
        }
    }
    if fy < body {
        return Some(if (fy / at.v.max(0.5)).floor() as i32 % 3 == 2 { B::CokeOvenBrick } else { B::BlastBrick });
    }
    (mouth && dep < dl * 0.6).then_some(B::BlastBrick)
}

/// A curtain of lava down a wall; crusts to basalt once cooled.
pub(super) fn lavafall(at: &Here, cooled: bool) -> Option<B> {
    let (along, _, la, _) = at.long();
    let wobble = (value_noise3(at.m.x * 0.6, at.m.y * 0.3, at.m.z * 0.6, 4911) - 0.5) * 0.6;
    ((along - la * 0.5).abs() < 0.7 + wobble).then_some(if cooled { B::Basalt } else { B::Lava })
}

/// Glow-cap lanterns hung on copper chains from the box's top, on a loose
/// four-metre grid; the lowest sits at `low`.
pub(super) fn lanterns(at: &Here, low: f32, seed: u32) -> Option<B> {
    let l = at.l();
    let (gx, gz) = ((l.x / 4.0).floor() as i32, (l.z / 4.0).floor() as i32);
    let roll = hash2(gx, gz, seed);
    if roll < 0.45 {
        return None;
    }
    let (cx, cz) = (at.lo.x + gx as f32 * 4.0 + 2.0, at.lo.z + gz as f32 * 4.0 + 2.0);
    let y = low + (roll - 0.45) * 4.0;
    let size = at.snap(0.8);
    let lamp = (at.m.x - cx).abs() < 0.5 && (at.m.z - cz).abs() < 0.5 || at.column(cx, cz);
    if lamp && at.m.y >= y && at.m.y < y + size {
        return Some(B::GlowCap);
    }
    (at.column(cx, cz) && at.m.y >= y + size).then_some(B::CopperBlock)
}

/// Frozen organ pipes along the box's long axis, tallest in the middle,
/// each with a crystal stop at its foot.
pub(super) fn organ(at: &Here, floor: f32, seed: u32) -> Option<B> {
    let (along, across, la, lc) = at.long();
    let i = (along / 1.1).floor() as i32;
    let ca = (i as f32 + 0.5) * 1.1;
    let d = (along - ca).hypot(across - lc * 0.5);
    let (px, pz) = if at.size().x >= at.size().z { (ca, lc * 0.5) } else { (lc * 0.5, ca) };
    if d >= 0.5 && !at.column(at.lo.x + px, at.lo.z + pz) {
        return None;
    }
    let mid = 1.0 - ((ca / la) * 2.0 - 1.0).abs();
    let tall = 2.5 + mid * 5.0 + (i as f32 * 0.9).sin().abs() * 1.5 + hash2(i, 0, seed) * 1.0;
    let fy = at.m.y - floor;
    if fy >= tall {
        return None;
    }
    Some(if fy >= at.snap(1.0) && fy < at.snap(1.0) + at.v { B::Crystal } else { B::Ice })
}

/// A frozen cascade down the back wall, thicker where the noise says.
pub(super) fn ice_sheet(at: &Here, back: Back) -> Option<B> {
    let (dep, along, _, wl) = at.rel_to_back(back);
    let taper = 1.0 - ((along / wl) * 2.0 - 1.0).abs();
    let n = value_noise3(at.m.x * 0.5, at.m.y * 0.25, at.m.z * 0.5, 4921);
    (dep < 0.3 + taper * n * 1.6).then_some(if n > 0.85 { B::Crystal } else { B::Ice })
}
