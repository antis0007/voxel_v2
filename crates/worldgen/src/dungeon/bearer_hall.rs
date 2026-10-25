//! Static architecture in room-local metres. All solids are authoritative
//! voxels. The jib is a fixed support, not a working counterweight mechanism.
use super::B;

pub(super) const DEPTH: f32 = 7.0;
pub(super) const HEIGHT: f32 = 16.0;

/// Version 45 additions preserve the nine-metre deck, rail openings, ramps
/// and mechanism clearance. Lights are actual removable torch voxels.
pub(super) fn articulation(x: f32, y: f32, z: f32, w: f32, d: f32, v: f32) -> Option<B> {
    let side = (z - d * 0.5).abs();
    let landing = x < 4.0 || x >= w - 4.0;
    if (-1.0..0.0).contains(&y) && (landing || side < 4.5) {
        // A pale worn route reads against the dark foundations and metal jib.
        return Some(if side < 2.5 { B::Limestone } else { B::StoneBricks });
    }
    for station in [4.0, w * 0.5, w - 4.0] {
        if (x - station).abs() < v.max(0.7) && (side - 5.5).abs() < v.max(0.7) {
            if (-6.0..1.0).contains(&y) { return Some(B::StoneBricks); }
            if (1.0..1.0 + v).contains(&y) && (station..station + v).contains(&x)
                && (5.5..5.5 + v).contains(&side) { return Some(B::Torch); }
        }
    }
    for station in [6.0, w * 0.5, w - 6.0] {
        let bay = (x - station).abs();
        let wall = z.min(d - z);
        let capital = (11.5..13.0).contains(&y);
        let base = (0.0..1.5).contains(&y);
        let reach: f32 = if capital || base { 1.6 } else { 0.8 };
        if bay < reach.max(v) && wall < reach.max(v) && (0.0..13.0).contains(&y) {
            return Some(if capital || base { B::Limestone } else { B::StoneBricks });
        }
        // Stepped ribs rise toward the central ceiling; their supports line
        // up with the pilasters rather than floating across an empty box.
        let rib_y = 13.0 + (wall / 3.0).min(2.0).floor();
        if bay < v.max(0.65) && y >= rib_y && y < rib_y + v.max(0.8) {
            return Some(B::Limestone);
        }
    }
    // Two high wall lights expose the springing and ceiling, not a sky fill.
    if (x - w * 0.5).abs() < v.max(0.7) && z.min(d-z) < 2.6 && z.min(d-z) >= 1.6 {
        if (10.0..11.0).contains(&y) { return Some(B::StoneBricks); }
        if (11.0..11.0 + v).contains(&y) && (w * 0.5..w * 0.5 + v).contains(&x)
            && (1.6..1.6 + v).contains(&z.min(d-z)) { return Some(B::Torch); }
    }
    None
}

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
        return if !(-5.0..=-2.0).contains(&y) { B::Limestone } else { B::Basalt };
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
        return if !(1.0..=HEIGHT - 2.0).contains(&y) { B::Limestone } else { B::Basalt };
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

#[cfg(test)]
mod painterly_tests {
    use super::*;

    #[test]
    fn painterly_hall_keeps_central_route_and_supported_lights() {
        for v in [1.0, 0.5, 0.25] {
            for ix in 0..(30.0 / v) as i32 {
                let x = (ix as f32 + 0.5) * v;
                for iz in 0..(4.0 / v) as i32 {
                    let z = 12.0 + (iz as f32 + 0.5) * v;
                    for iy in 0..(3.0 / v) as i32 {
                        let y = (iy as f32 + 0.5) * v;
                        let b = articulation(x, y, z, 30.0, 28.0, v)
                            .unwrap_or_else(|| block(x, y, z, 30.0, 28.0, 1));
                        assert_eq!(b, B::Air, "route obstruction at {x},{y},{z}");
                    }
                }
            }
            for x in [4.0, 15.0, 26.0] {
                assert_eq!(articulation(x, 1.0 + v * 0.5, 8.5, 30.0, 28.0, v), Some(B::Torch));
                assert!(articulation(x, 0.5, 8.5, 30.0, 28.0, v).is_some_and(|b| b.is_solid()));
            }
            let mut lamps = 0;
            for ix in 0..(30.0 / v) as i32 {
                for iz in 0..(28.0 / v) as i32 {
                    for y in [1.0 + v * 0.5, 11.0 + v * 0.5] {
                        if articulation((ix as f32 + 0.5) * v, y,
                            (iz as f32 + 0.5) * v, 30.0, 28.0, v) == Some(B::Torch) {
                            lamps += 1;
                        }
                    }
                }
            }
            assert_eq!(lamps, 8, "lamp clusters at {v} m");
        }
    }
}
