//! Generator 48: a way out of an island room's void that holds in every
//! room. A straight stair stands in the void beside one aisle, rising one
//! step a voxel from the void floor to the aisle's slab. The plan picks the
//! first of eight placements (either aisle, either side, either way) that
//! stays inside the room and clear of every door walkway; the wall ledges
//! remain as scenery but crowded bridges could leave them unclimbable. A
//! room with no clear placement gets a void one metre deep, which is one
//! step at every scale.
use super::{Corr, Room, outline};

/// `Room::stair` when no placement fits.
pub const NONE: u8 = u8::MAX;
/// The stair's band beside the aisle slab, metres from the room's centre line.
const BAND: (f32, f32) = (1.0, 2.4);

/// Placement `k`: bit 0 picks the aisle (along z, else along x), bit 1 the
/// side of it, bit 2 the way the stair climbs. Returns, for a room-local
/// point, its distance along the climb and whether it is in the band.
fn frame(r: &Room, k: u8, rx: f32, rz: f32) -> (f32, bool) {
    let (w, d) = (r.w(), r.d());
    let (off, run) = if k & 1 == 0 { (rx - w * 0.5, rz - d * 0.5) } else { (rz - d * 0.5, rx - w * 0.5) };
    let side = if k & 2 == 0 { off } else { -off };
    let run = if k & 4 == 0 { run } else { -run };
    // The climb starts on the far side of the cross aisle.
    (run - BAND.0, (BAND.0..BAND.1).contains(&side) && run >= BAND.0)
}

/// Height of the stair's top over the floor line at a point, if the point is
/// on the stair: from `-depth` at its foot to the slab top at its head.
pub fn top(r: &Room, rx: f32, rz: f32, depth: f32, v: f32) -> Option<f32> {
    if r.stair == NONE {
        return None;
    }
    let (t, band) = frame(r, r.stair, rx, rz);
    (band && t < depth + 1.0).then(|| (-depth + (t / v).floor() * v + v).min(0.0))
}

fn walkway(corrs: &[Corr], r: &Room, mx: f32, mz: f32) -> bool {
    corrs.iter().any(|k| {
        if k.along_x {
            mz >= k.z0 - 0.5 && mz < k.z1 + 0.5 && k.x1 > r.x0 && k.x0 < r.x1
        } else {
            mx >= k.x0 - 0.5 && mx < k.x1 + 0.5 && k.z1 > r.z0 && k.z0 < r.z1
        }
    })
}

/// The first placement that stays inside the room and off the walkways.
pub fn choose(corrs: &[Corr], r: &Room, depth: f32) -> u8 {
    (0u8..8)
        .find(|&k| {
            let mut ok = true;
            let (mut x, step) = (0.25, 0.5);
            while x < r.w() && ok {
                let mut z = 0.25;
                while z < r.d() && ok {
                    let (t, band) = frame(r, k, x, z);
                    if band && t < depth + 1.0 {
                        ok = outline::depth(r, x, z) >= 0.5 && !walkway(corrs, r, r.x0 + x, r.z0 + z);
                    }
                    z += step;
                }
                x += step;
            }
            // The climb must fit before the wall.
            let (span, _) = if k & 1 == 0 { (r.d(), r.w()) } else { (r.w(), r.d()) };
            ok && span * 0.5 - BAND.0 - 0.6 >= depth + 1.0
        })
        .unwrap_or(NONE)
}
