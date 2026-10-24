//! The cleft: a room split down its long axis by a rift eight metres deep.
//! A slab bridge crosses it mid-room and in front of every door in a long
//! wall; doors in the short walls open onto the solid ends. A ledge along one
//! side climbs from the pool at the bottom to every bridge, so a fall is a
//! detour, not a death; glow caps light the far wall and lamp posts mark
//! where each bridge leaves solid ground.
use super::{Complex, Corr, Room};
use crate::{BlockId as B, hash2, hash3, value_noise3};

/// Deepest a rift goes from the floor line to the pool surface, metres.
const DEPTH: f32 = 8.0;

struct Rift {
    along: f32,
    across: f32,
    a0: f32,
    a1: f32,
    half: f32,
    /// Distance along the rift to the nearest bridge; zero on one.
    gap: f32,
}

fn rift(corrs: &[Corr], r: &Room, rx: f32, rz: f32) -> Rift {
    let (w, d) = (r.w(), r.d());
    let along_x = w >= d;
    let (along, across, len, wid) = if along_x { (rx, rz, w, d) } else { (rz, rx, d, w) };
    let to = |a0: f32, a1: f32| (a0 - along).max(along - a1).max(0.0);
    let mut gap = to(len * 0.5 - 1.0, len * 0.5 + 1.0);
    // Corridors that cross the rift's axis land on long walls.
    for k in corrs.iter().filter(|k| k.along_x != along_x) {
        let reaches = if along_x { k.z1 > r.z0 - 1.0 && k.z0 < r.z1 + 1.0 } else { k.x1 > r.x0 - 1.0 && k.x0 < r.x1 + 1.0 };
        let (s0, s1) = if along_x { (k.x0 - r.x0, k.x1 - r.x0) } else { (k.z0 - r.z0, k.z1 - r.z0) };
        if reaches && s1 > 0.0 && s0 < len {
            gap = gap.min(to(s0, s1));
        }
    }
    Rift { along, across: across - wid * 0.5, a0: 2.5, a1: len - 2.5, half: (wid * 0.22).clamp(2.0, 4.0), gap }
}

/// How deep a rift can go: as far as its ledge descends between bridges,
/// whole metres, three to eight.
pub fn depth(corrs: &[Corr], r: &Room) -> f32 {
    let len = r.w().max(r.d());
    let deepest = (0..(len * 2.0) as i32)
        .map(|i| {
            let a = i as f32 * 0.5 + 0.25;
            let (rx, rz) = if r.w() >= r.d() { (a, r.d() * 0.5) } else { (r.w() * 0.5, a) };
            let k = rift(corrs, r, rx, rz);
            if a > k.a0 && a < k.a1 { k.gap } else { 0.0 }
        })
        .fold(0.0f32, f32::max);
    deepest.floor().clamp(3.0, DEPTH)
}

/// Signed distance into the rift across its width: positive inside.
fn inside(r: &Room, k: &Rift, fy: f32) -> f32 {
    if k.along <= k.a0 || k.along >= k.a1 {
        return -1.0;
    }
    // Ragged walls that narrow a little with depth.
    let wobble = value_noise3(k.along * 0.45, fy * 0.3, k.across.signum() * 3.0, r.salt.wrapping_add(91)) - 0.5;
    let taper = (-fy / r.rift.max(1.0)).clamp(0.0, 1.0) * 0.6;
    k.half + wobble * 1.4 - taper - k.across.abs()
}

pub fn below(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let k = rift(&c.corrs, r, rx, rz);
    let depth_in = inside(r, &k, fy);
    if depth_in < 0.0 {
        // Glow caps set into the far wall, away from the ledge.
        let lit = k.across < 0.0 && depth_in >= -v && fy < -1.5 && k.along > k.a0 && k.along < k.a1;
        return (lit && hash3((rx / v) as i32, (fy / v) as i32, (rz / v) as i32, r.salt) > 0.8).then_some(B::GlowCap);
    }
    let void = |fy: f32| {
        let depth = r.rift;
        if fy < -depth - 1.0 {
            return c.pal.wall;
        }
        if fy < -depth {
            return B::Water;
        }
        // Stalagmite stubs in the pool, low enough to step over.
        let (i, kk) = ((rx / 1.5).floor() as i32, (rz / 1.5).floor() as i32);
        let spike = hash2(i, kk, r.salt.wrapping_add(93));
        let (ox, oz) = (i as f32 * 1.5 + 0.75, kk as f32 * 1.5 + 0.75);
        let dist = (rx - ox).hypot(rz - oz);
        if spike > 0.8 && fy < -depth + 1.0 && dist < (1.0 - (fy + depth)).max(0.0) * 0.6 + v * 0.5 {
            return c.pal.alt;
        }
        B::Air
    };
    // The ledge along the +across wall, one step a voxel up to each bridge.
    // Its band is fixed, wider than the walls' wobble, so no step is lost.
    if k.across > k.half - 2.8 {
        let top = -((k.gap / v).ceil() * v).min(r.rift);
        if fy < top {
            return Some(c.pal.trim);
        }
    }
    if k.gap <= 0.0 {
        return Some(if fy >= -1.0 { if fy >= -v { c.pal.floor } else { c.pal.trim } } else { void(fy) });
    }
    Some(void(fy))
}

/// Lamp posts at the rift's edge on both sides of every bridge.
pub fn furnish(c: &Complex, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let k = rift(&c.corrs, r, rx, rz);
    if k.along <= k.a0 || k.along >= k.a1 || fy >= 2.2 + v {
        return None;
    }
    let edge = (k.across.abs() - k.half - 0.9).abs() < v.max(0.4);
    let beside = k.gap > 0.0 && k.gap < v.max(0.5);
    (edge && beside).then_some(if fy >= 2.2 { B::Torch } else { c.pal.trim })
}
