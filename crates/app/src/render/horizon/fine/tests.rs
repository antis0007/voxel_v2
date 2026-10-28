use super::*;

#[test]
fn rings_tile_evenly_and_stay_on_the_centre_snap() {
    for (ring, &(half, cell)) in RINGS.iter().enumerate() {
        let width = tile::CELLS as f32 * cell;
        assert_eq!(2.0 * half / width, TILES as f32, "ring {ring}");
        assert_eq!(512.0 % width, 0.0, "ring {ring} tiles must not move on recentring");
        if ring > 0 {
            let (inner, _) = RINGS[ring - 1];
            assert_eq!((half - inner) % width, 0.0, "ring {ring} hole on tile edges");
        }
    }
}

#[test]
fn rings_cover_the_square_once() {
    // Every 16 m cell of the outer square lies in exactly one ring's tile.
    let step = 16.0;
    let n = (2.0 * HALF_M / step) as usize;
    for zi in 0..n {
        for xi in 0..n {
            let (x, z) = ((xi as f32 + 0.5) * step, (zi as f32 + 0.5) * step);
            let owners = (0..RINGS.len())
                .filter(|&ring| {
                    let (half, cell) = RINGS[ring];
                    let width = tile::CELLS as f32 * cell;
                    let (lx, lz) = (x - (HALF_M - half), z - (HALF_M - half));
                    (0.0..2.0 * half).contains(&lx)
                        && (0.0..2.0 * half).contains(&lz)
                        && in_ring(ring, (lx / width) as usize, (lz / width) as usize)
                })
                .count();
            assert_eq!(owners, 1, "cell ({x}, {z})");
        }
    }
}

#[test]
fn tiles_are_world_anchored_for_negative_and_large_centres() {
    // Centres are snapped to 512 m; every tile corner must land on its ring's
    // world tile lattice so recentring reuses the same knots.
    for center in [(0.0, 0.0), (-512.0, 1024.0), (-4096.0, -7680.0), (2_097_152.0, -1_048_576.0)] {
        for ring in 0..RINGS.len() {
            let width = tile::CELLS as f32 * RINGS[ring].1;
            for (tx, tz) in [(0, 0), (TILES - 1, 0), (5, TILES - 1)] {
                let (x, z) = Place { ring, tx, tz }.origin(center);
                assert_eq!(x.rem_euclid(width), 0.0, "ring {ring} centre {center:?}");
                assert_eq!(z.rem_euclid(width), 0.0, "ring {ring} centre {center:?}");
            }
        }
    }
}

#[test]
fn a_voxel_of_error_shrinks_on_screen_with_distance() {
    assert!(pixels_per_voxel(6144.0, 1.0) < pixels_per_voxel(3072.0, 1.0));
    assert_eq!(pixels_per_voxel(0.0, 1.0), pixels_per_voxel(700.0, 1.0));
    assert!((pixels_per_voxel(6144.0, 0.25) * 4.0 - pixels_per_voxel(6144.0, 1.0)).abs() < 1e-5);
}

#[test]
fn shader_ring_half_width_matches() {
    let shader = include_str!("../../../shaders/world_shader.wgsl");
    assert!(shader.contains(&format!("const FINE_RING_HALF_M: f32 = {HALF_M:.1};")));
}
