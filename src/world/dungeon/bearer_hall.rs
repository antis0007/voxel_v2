//! Static architecture in room-local metres. All solids are authoritative
//! voxels. The jib is a fixed support, not a working counterweight mechanism.
use super::B;

pub(super) const DEPTH: f32 = 7.0;
pub(super) const HEIGHT: f32 = 16.0;

pub(super) fn block(x: f32, y: f32, z: f32, w: f32, d: f32, seed: u32) -> B {
    let mid = d * 0.5;
    let side = (z - mid).abs();
    let landing = x < 4.0 || x >= w - 4.0;
    let bridge = side < 4.5;
    // Complete lower foundation and a broad processional bridge with clear
    // corridor mouths. Two supported ramps leave and rejoin the same landings.
    if y < -6.0 { return B::Basalt; }
    if (-1.0..0.0).contains(&y) && (landing || bridge) { return B::Basalt; }
    if z < 5.0 || z >= d - 5.0 {
        let run = (x - 4.0).min(w - 4.0 - x).max(0.0);
        if y < -(run * 0.6).min(6.0) { return B::Basalt; }
    }
    // These are real collision rails; both ends open onto level landings.
    if !landing && (3.5..4.5).contains(&side) && (0.0..1.0).contains(&y) {
        return B::Limestone;
    }
    // Piers join the deck to the foundation rather than floating under it.
    if ((x - 10.0).abs() < 1.0 || (x - (w - 10.0)).abs() < 1.0)
        && (side - 3.5).abs() < 1.0 && y < 0.0 {
        return if y < -5.0 || y > -2.0 { B::Limestone } else { B::Basalt };
    }
    // A seed-mirrored asymmetric diagonal jib, safely outside the central lane.
    let jib_z = mid + if seed & 1 == 0 { 7.0 } else { -7.0 };
    let across = (z - jib_z).abs();
    if (x - 6.0).abs() < 1.0 && across < 1.0 && y < 13.0 {
        return if y < -5.0 || (10.0..11.0).contains(&y) { B::Limestone } else { B::Basalt };
    }
    let beam_y = 13.0 - (x - 6.0) * (7.0 / (w - 12.0));
    if (6.0..w - 6.0).contains(&x) && across < 1.0 && (y - beam_y).abs() < 0.8 {
        return B::Basalt;
    }
    if (x - (w - 6.0)).abs() < 1.0 && across < 1.0 && y < 7.0 {
        return if y > 5.0 { B::Limestone } else { B::Basalt };
    }
    if (x - 6.0).abs() < 1.0 && (1.0..2.0).contains(&across) && (11.0..12.0).contains(&y) {
        return B::SilverBlock;
    }
    // Sparse ceiling ribs are carried by wall pilasters, without glowing roof
    // clusters. The existing material shader supplies the common pixel grid.
    let rib = (x - 6.0).abs() < 0.6 || (x - w * 0.5).abs() < 0.6
        || (x - (w - 6.0)).abs() < 0.6;
    if rib && (z < 1.0 || z >= d - 1.0) {
        return if y < 1.0 || y > HEIGHT - 2.0 { B::Limestone } else { B::Basalt };
    }
    if rib && y >= HEIGHT - 1.0 { return B::Limestone; }
    // Quiet memorials flank the exit; they have no circuit or puzzle role.
    if x >= w - 2.0 && (7.0..10.0).contains(&side) && (0.0..2.0).contains(&y) {
        return if y < 1.0 { B::Basalt } else { B::Limestone };
    }
    for lx in [2.0, w - 2.0] {
        for lz in [2.0, d - 2.0] {
            if (x - lx).abs() < 0.6 && (z - lz).abs() < 0.6 {
                if (0.0..1.0).contains(&y) { return B::Limestone; }
                if (1.0..2.0).contains(&y) { return B::Torch; }
            }
        }
    }
    // Two lower lamps reveal the service route depth and its destinations.
    if (x - w * 0.5).abs() < 0.6 && ((1.0..2.0).contains(&z) || (d-2.0..d-1.0).contains(&z)) {
        if (-6.0..-5.0).contains(&y) { return B::Limestone; }
        if (-5.0..-4.0).contains(&y) { return B::Torch; }
    }
    B::Air
}
