use super::*;
use std::collections::HashMap;

const SEALED: Option<ChunkConnectivity> = Some(ChunkConnectivity::SEALED);

/// Walks a box with every chunk in view; `known` overrides unknown (open) chunks.
fn walk(min: IVec3, max: IVec3, camera: IVec3, known: &HashMap<IVec3, ChunkConnectivity>) -> Reach {
    let grid = Grid::spanning(min, max).expect("small box");
    let mut links = vec![None; grid.cells()];
    for (&c, &l) in known {
        links[grid.index(c).expect("inside")] = Some(l);
    }
    Reach::walk(grid, &links, camera, |_| true).expect("camera inside")
}

/// Open space joining exactly two faces.
fn corridor(a: usize, b: usize) -> ChunkConnectivity {
    ChunkConnectivity::from_components([(1u8 << a) | (1 << b)])
}

fn sealed_room(camera: IVec3) -> HashMap<IVec3, ChunkConnectivity> {
    let mut shell = HashMap::new();
    for y in -1..=1 {
        for z in -1..=1 {
            for x in -1..=1 {
                let offset = IVec3::new(x, y, z);
                if offset != IVec3::ZERO {
                    shell.insert(camera + offset, SEALED.expect("const"));
                }
            }
        }
    }
    shell
}

#[test]
fn sealed_room_hides_chunks_beyond_it() {
    for camera in [IVec3::ZERO, IVec3::new(-5, -3, -7), IVec3::new(-40_000, 2, 31_000)] {
        let reach = walk(camera - IVec3::splat(3), camera + IVec3::splat(3), camera, &sealed_room(camera));
        assert!(reach.reaches(camera));
        // Face neighbours are the visible walls; an edge neighbour hides behind two of them.
        assert!(reach.reaches(camera + IVec3::X) && reach.reaches(camera + IVec3::NEG_Y));
        assert!(!reach.reaches(camera + IVec3::new(0, -1, 1)));
        for far in [IVec3::new(2, 0, 0), IVec3::new(-3, 0, 0), IVec3::new(0, 2, -2), IVec3::new(3, 3, 3)] {
            assert!(!reach.reaches(camera + far), "{far} beyond a sealed room from {camera}");
        }
    }
}

#[test]
fn open_corridor_through_the_wall_keeps_chunks_visible() {
    for camera in [IVec3::ZERO, IVec3::new(-5, -3, -7)] {
        let mut room = sealed_room(camera);
        room.insert(camera + IVec3::X, corridor(0, 1));
        let reach = walk(camera - IVec3::splat(3), camera + IVec3::splat(3), camera, &room);
        assert!(reach.reaches(camera + IVec3::new(2, 0, 0)));
        assert!(reach.reaches(camera + IVec3::new(3, 0, 0)));
        // Past the corridor the open region fans out, but only away from the camera.
        assert!(reach.reaches(camera + IVec3::new(3, 2, -1)));
        assert!(!reach.reaches(camera + IVec3::new(-2, 0, 0)), "behind the room's sealed -X side");
    }
}

#[test]
fn bent_corridor_needs_both_faces_linked() {
    // One-chunk-high slab: x row 0..=3 at z = 0, all other chunks sealed.
    let (min, max) = (IVec3::new(-1, 0, -1), IVec3::new(4, 0, 2));
    let mut known = HashMap::new();
    for z in -1..=2 {
        for x in -1..=4 {
            known.insert(IVec3::new(x, 0, z), ChunkConnectivity::SEALED);
        }
    }
    let camera = IVec3::ZERO;
    known.insert(camera, ChunkConnectivity::ALL_OPEN);
    known.insert(IVec3::new(1, 0, 0), corridor(0, 5)); // enters -X, leaves +Z
    known.insert(IVec3::new(1, 0, 1), ChunkConnectivity::ALL_OPEN);
    known.insert(IVec3::new(2, 0, 0), ChunkConnectivity::ALL_OPEN);
    let reach = walk(min, max, camera, &known);
    assert!(reach.reaches(IVec3::new(1, 0, 1)), "the bend opens toward +Z");
    assert!(reach.reaches(IVec3::new(2, 0, 1)), "reached via the bend");
    // (2,0,0) is only adjacent to the bend's sealed +X side and (2,0,1)'s -Z face,
    // which would step back toward the camera on z.
    assert!(!reach.reaches(IVec3::new(2, 0, 0)));
    assert!(!reach.reaches(IVec3::new(3, 0, 0)));
}

#[test]
fn unknown_chunks_are_open_and_outside_the_box_is_reached() {
    let camera = IVec3::new(-2, -2, -2);
    let reach = walk(IVec3::splat(-4), IVec3::ZERO, camera, &HashMap::new());
    assert!(reach.reaches(IVec3::splat(-4)) && reach.reaches(IVec3::ZERO));
    assert!(reach.reaches(IVec3::splat(50)));
}

#[test]
fn out_of_view_chunks_stop_the_walk() {
    let grid = Grid::spanning(IVec3::ZERO, IVec3::new(4, 0, 0)).expect("small box");
    let links = vec![None; grid.cells()];
    let reach = Reach::walk(grid, &links, IVec3::ZERO, |c| c.x != 2).expect("camera inside");
    assert!(reach.reaches(IVec3::X) && !reach.reaches(IVec3::new(2, 0, 0)));
    assert!(!reach.reaches(IVec3::new(4, 0, 0)));
}

#[test]
fn spilling_neighbour_of_a_reached_chunk_is_drawn() {
    let grid = Grid::spanning(IVec3::ZERO, IVec3::new(3, 0, 0)).expect("small box");
    let mut links = vec![SEALED; grid.cells()];
    links[0] = None;
    let torch = ChunkConnectivity::compute(
        &crate::mesh::GeneratedChunk::new(IVec3::ZERO, vec![crate::world::BlockId::Torch; 34 * 34 * 34]),
        &std::sync::atomic::AtomicBool::new(false),
    );
    assert!(torch.is_some_and(ChunkConnectivity::spills));
    links[2] = torch;
    links[3] = torch;
    let reach = Reach::walk(grid, &links, IVec3::ZERO, |_| true).expect("camera inside");
    // (1) is sealed and cannot be left, (2) spills next to it, (3) does not touch a reached chunk.
    assert!(reach.reaches(IVec3::X) && reach.reaches(IVec3::new(2, 0, 0)));
    assert!(!reach.reaches(IVec3::new(3, 0, 0)));
}

#[test]
fn grid_index_round_trips_with_negative_coordinates() {
    let grid = Grid::spanning(IVec3::new(-7, -3, -900), IVec3::new(-2, 4, -895)).expect("small box");
    for i in 0..grid.cells() {
        assert_eq!(grid.index(grid.coord(i)), Some(i));
    }
    assert_eq!(grid.index(IVec3::new(-8, 0, -897)), None);
    assert!(Grid::spanning(IVec3::ZERO, IVec3::splat(200)).is_none(), "oversized box skips culling");
}

#[test]
fn eye_is_recovered_from_a_perspective_view_projection() {
    let eye = glam::Vec3::new(-100.5, 37.25, 4000.0);
    let view = Mat4::look_to_rh(eye, glam::Vec3::new(0.3, -0.2, 1.0).normalize(), glam::Vec3::Y);
    let proj = Mat4::perspective_rh(1.2, 1.7, 0.05, 900.0);
    let found = eye_from(proj * view).expect("perspective");
    assert!((found - eye).length() < 0.05, "{found} vs {eye}");
    assert!(eye_from(Mat4::orthographic_rh(-1.0, 1.0, -1.0, 1.0, 0.1, 10.0) * view).is_none());
}
