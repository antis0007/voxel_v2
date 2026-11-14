//! A recognisable lookout lantern above the existing stair and study.
use crate::BlockId as B;

pub(super) fn crown(x: f32, y: f32, z: f32, v: f32) -> Option<B> {
    let r = x.hypot(z);
    if r >= 6.4 || !(12.0..22.0).contains(&y) { return None; }
    if y < 12.0 + v && r < 4.8 { return Some(B::StoneBricks); }
    if y < 15.0 {
        let pier = x.abs().min(z.abs()) < v.max(0.55) && r > 3.5 && r < 4.8;
        let sill = r > 4.0 && r < 4.8 && y < 13.0;
        return Some(if pier || sill { B::Limestone } else { B::Air });
    }
    // A steep hollow cap with a broad eave; no solid pyramid inside it.
    let roof_r = ((21.0 - y) * 0.82).max(0.25);
    if r < roof_r && r > (roof_r - v.max(0.65)).max(0.0) {
        return Some(B::Basalt);
    }
    if r < 0.5 && y >= 20.0 { return Some(B::WeatheredBricks); }
    Some(B::Air)
}
