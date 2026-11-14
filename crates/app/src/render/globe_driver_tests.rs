use super::*;

const A: TileId = TileId { face: 0, level: 1, x: 0, y: 0 };
const B: TileId = TileId { face: 0, level: 1, x: 1, y: 0 };
const C: TileId = TileId { face: 0, level: 1, x: 0, y: 1 };

#[test]
fn the_surface_skips_then_drops_the_worker_after_30_seconds() {
    assert_eq!(driver_step(Tier::Surface, false, 0.0), Step::Skip);
    assert_eq!(driver_step(Tier::Surface, false, 29.9), Step::Skip);
    assert_eq!(driver_step(Tier::Surface, false, 30.0), Step::DropWorker);
    assert_eq!(driver_step(Tier::Surface, true, 500.0), Step::Run);
    assert_eq!(driver_step(Tier::Transition, false, 0.0), Step::Run);
    assert_eq!(driver_step(Tier::Orbit, false, 0.0), Step::Run);
}

#[test]
fn cadences_are_seconds_and_a_clock_step_back_is_due() {
    assert!(due(None, 0.0, 30.0));
    assert!(!due(Some(10.0), 39.9, 30.0));
    assert!(due(Some(10.0), 40.0, 30.0));
    assert!(due(Some(10.0), 5.0, 30.0));
}

fn cfg(seed: u32, voxel_scale: engine_core::VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version: 53, seed, world_type: engine_voxel::WorldType::Hills, voxel_scale }
}

#[test]
fn a_different_world_resets_the_globe() {
    use engine_core::VoxelScalePreset::{HalfMeter, OneMeter};
    assert!(!config_changed(None, cfg(1, OneMeter)), "the first world has nothing to drop");
    assert!(!config_changed(Some(cfg(1, OneMeter)), cfg(1, OneMeter)));
    assert!(config_changed(Some(cfg(1, OneMeter)), cfg(2, OneMeter)));
    assert!(config_changed(Some(cfg(1, OneMeter)), cfg(1, HalfMeter)));
}

impl GlobeDriver {
    fn capacities(&self) -> Vec<usize> {
        let mut c = self.select_scratch.capacities().to_vec();
        c.extend([
            self.selection.draw.capacity(),
            self.selection.wanted.capacity(),
            self.keep.capacity(),
            self.request.capacity(),
            self.sent.capacity(),
            self.shell_ready.capacity(),
        ]);
        c
    }
}

#[test]
fn a_steady_frame_reuses_every_buffer_and_sends_no_request() {
    // Close enough that level-3 tiles under the eye want to split; a radius multiple
    // holds that at any planet size.
    let eye = DVec3::new(0.0, 0.0, chart::RADIUS_M * 2.5);
    let ready = |t: TileId| t.level < 4;
    let mut driver = GlobeDriver::default();
    assert!(driver.plan(&ready, eye, 900.0, 512), "the first list goes out");
    driver.take_ready_faces(0);
    assert!(!driver.plan(&ready, eye, 900.0, 512));
    driver.take_ready_faces(0);
    let before = driver.capacities();
    assert!(!driver.plan(&ready, eye, 900.0, 512), "an identical list is not resent");
    driver.take_ready_faces(0);
    assert_eq!(driver.capacities(), before);
    assert!(!driver.selection.wanted.is_empty());
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
