use super::*;

const A: TileId = TileId { face: 0, level: 1, x: 0, y: 0 };
const B: TileId = TileId { face: 0, level: 1, x: 1, y: 0 };
const C: TileId = TileId { face: 0, level: 1, x: 0, y: 1 };

#[test]
fn the_surface_skips_then_drops_the_worker_after_1800_frames() {
    assert_eq!(driver_step(Tier::Surface, false, 1), Step::Skip);
    assert_eq!(driver_step(Tier::Surface, false, 1_799), Step::Skip);
    assert_eq!(driver_step(Tier::Surface, false, 1_800), Step::DropWorker);
    assert_eq!(driver_step(Tier::Surface, true, 5_000), Step::Run);
    assert_eq!(driver_step(Tier::Transition, false, 0), Step::Run);
    assert_eq!(driver_step(Tier::Orbit, false, 0), Step::Run);
}

#[test]
fn only_a_populated_orbit_globe_hides_the_horizon() {
    assert!(hides_horizon(true, Tier::Orbit, 6));
    assert!(!hides_horizon(true, Tier::Orbit, 0));
    assert!(!hides_horizon(true, Tier::Transition, 6));
    assert!(!hides_horizon(false, Tier::Orbit, 6));
}

#[test]
fn requests_put_unready_drawn_tiles_first_then_wanted_without_duplicates() {
    let ready = |id: TileId| id == A;
    let mut out = Vec::new();
    request_order(&[A, B], &[C, B, C], &ready, &mut out);
    assert_eq!(out, vec![B, C]);
    let many: Vec<TileId> = (0..100).map(|k| TileId { face: 1, level: 7, x: k, y: 0 }).collect();
    request_order(&[C], &many, &|_| false, &mut out);
    assert_eq!(out.len(), MAX_REQUEST);
    assert_eq!(out[0], C);
    assert_eq!(out[1], many[0]);
}

#[test]
fn only_faces_in_front_of_the_limb_rebake() {
    let eye = chart::to_body(DVec3::new(0.0, 60_000.0, 0.0));
    let front: Vec<u8> = (0..6).filter(|f| face_in_front(*f, eye)).collect();
    assert_eq!(front, vec![0, 2, 3, 4, 5]);
    // On the surface the horizon is at 90 degrees, so the four side faces still reach it.
    assert!(!face_in_front(1, chart::to_body(DVec3::ZERO)));
}

#[test]
fn shell_faces_wait_when_tiles_used_the_frame_budget() {
    let face = 512 * 512;
    assert_eq!(shell_faces_within(2 * 1024 * 1024, 1_900_000, face, 6), 0);
    assert_eq!(shell_faces_within(2 * 1024 * 1024, 0, face, 6), 6);
    assert_eq!(shell_faces_within(2 * 1024 * 1024, 1_000_000, face, 6), 4);
}

#[test]
fn lean_tier_halves_draw_count_and_shell_size() {
    assert_eq!(lean_limits(true), (256, 256));
    assert_eq!(lean_limits(false), (512, 512));
}
