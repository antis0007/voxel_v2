use super::*;

/// Flat floor at y < 0 and a wall at x = 10.
struct Room;
impl Solid for Room {
    fn solid(&self, c: IVec3) -> bool {
        c.y < 0 || c.x == 10
    }
}

const LIMITS: Limits = Limits {
    speed: 6.0,
    burst: 3.0,
    ascent: 2.6,
    gravity: 20.0,
    terminal: 40.0,
    hang: 0.6,
    half_width: 0.3,
    height: 1.8,
    skin: 0.02,
    bound: 1.0e6,
};
const DT: f32 = 1.0 / 120.0;

/// Feeds a path sampled at 120 Hz; returns the corrections and the first reason.
fn walk(v: &mut Validator, path: impl Fn(f32) -> Vec3, seconds: f32) -> (usize, Option<Reason>) {
    let (mut ack, mut corrections, mut first) = (0, 0, None);
    for i in 1..=(seconds / DT) as u32 {
        match v.step(&Room, path(i as f32 * DT), DT, ack) {
            Verdict::Correct { id, .. } => {
                corrections += 1;
                ack = id;
                first = first.or(v.last_reason);
            }
            Verdict::Accept | Verdict::Ignored => {}
        }
    }
    (corrections, first)
}

#[test]
fn honest_sprint_dash_and_jumps_are_accepted() {
    let mut v = Validator::new(LIMITS, Vec3::new(0.0, 0.0, 0.0));
    // 6 voxels/s along z, a 16 voxels/s dash for 0.16 s, hops of 1.2 voxels.
    let path = |t: f32| {
        let dash = (t - 1.0).clamp(0.0, 0.16) * 10.0;
        let hop = (t * std::f32::consts::PI).sin().max(0.0) * 1.2;
        Vec3::new(0.0, hop, t * 6.0 + dash)
    };
    assert_eq!(walk(&mut v, path, 4.0), (0, None));
}

#[test]
fn a_sustained_speed_hack_is_corrected() {
    let mut v = Validator::new(LIMITS, Vec3::ZERO);
    let (n, reason) = walk(&mut v, |t| Vec3::new(0.0, 0.0, -t * 9.0), 3.0);
    assert!(n > 0);
    assert_eq!(reason, Some(Reason::Speed));
    assert!(v.accepted().z > -3.0 * 6.0 - LIMITS.burst - 0.1, "{}", v.accepted());
}

#[test]
fn walking_through_a_wall_is_corrected_even_in_one_step() {
    let mut v = Validator::new(LIMITS, Vec3::new(9.0, 0.0, 0.0));
    assert!(matches!(v.step(&Room, Vec3::new(11.5, 0.0, 0.0), DT, 0), Verdict::Correct { .. }));
    assert_eq!(v.last_reason, Some(Reason::Solid));
    assert_eq!(v.accepted(), Vec3::new(9.0, 0.0, 0.0));
}

#[test]
fn unpermitted_flight_is_corrected_and_permitted_flight_is_not() {
    let rise = |t: f32| Vec3::new(0.0, t * 3.0, 0.0);
    let mut v = Validator::new(LIMITS, Vec3::ZERO);
    // Steady rise breaks the gravity curve or the ascent budget, whichever comes first.
    assert!(matches!(walk(&mut v, rise, 3.0).1, Some(Reason::Ascent | Reason::Float)));
    let mut v = Validator::new(LIMITS, Vec3::ZERO);
    v.flying_allowed = true;
    assert_eq!(walk(&mut v, rise, 3.0), (0, None));
}

#[test]
fn poses_in_flight_are_ignored_until_the_correction_is_acknowledged() {
    let mut v = Validator::new(LIMITS, Vec3::ZERO);
    let Verdict::Correct { id, .. } = v.step(&Room, Vec3::new(50.0, 0.0, 0.0), DT, 0) else { panic!() };
    assert_eq!(v.step(&Room, Vec3::new(50.1, 0.0, 0.0), DT, 0), Verdict::Ignored);
    assert_eq!(v.step(&Room, Vec3::new(0.0, 0.0, 0.04), DT, id), Verdict::Accept);
}

#[test]
fn server_placement_and_knockback_do_not_count_against_the_client() {
    let mut v = Validator::new(LIMITS, Vec3::ZERO);
    let Verdict::Correct { id, position } = v.place(Vec3::new(-40.0, 0.0, 5.0)) else { panic!() };
    assert_eq!(v.step(&Room, position, DT, id), Verdict::Accept);
    v.impulse(8.0, 0.0);
    assert_eq!(v.step(&Room, position + Vec3::X * 8.0, DT, id), Verdict::Accept);
}

#[test]
fn hovering_after_a_jump_is_corrected_downward() {
    let mut v = Validator::new(LIMITS, Vec3::ZERO);
    let (n, reason) = walk(&mut v, |t| Vec3::new(0.0, (t * 8.0).min(1.5), 0.0), 2.0);
    assert!(n > 0);
    assert_eq!(reason, Some(Reason::Float));
    assert!(v.accepted().y < 1.0, "{}", v.accepted());
}

#[test]
fn falling_slower_than_gravity_is_corrected() {
    let mut v = Validator::new(LIMITS, Vec3::new(0.0, 40.0, 0.0));
    assert_eq!(walk(&mut v, |t| Vec3::new(0.0, 40.0 - t * 2.0, 0.0), 2.0).1, Some(Reason::Float));
}

#[test]
fn an_honest_fall_is_accepted_and_the_server_reports_its_height() {
    let mut v = Validator::new(LIMITS, Vec3::new(0.0, 30.0, 0.0));
    let fall = |t: f32| Vec3::new(0.0, (30.0 - 0.5 * LIMITS.gravity * t * t).max(0.0), 0.0);
    assert_eq!(walk(&mut v, fall, 2.5), (0, None));
    assert_eq!(v.take_landing(), Some(30.0));
    assert_eq!(v.take_landing(), None);
}

#[test]
fn holding_height_against_a_wall_is_allowed_for_wall_runs() {
    let mut v = Validator::new(LIMITS, Vec3::new(9.65, 1.0, 0.0));
    assert_eq!(walk(&mut v, |t| Vec3::new(9.65, 1.0, t * 5.0), 1.0), (0, None));
}

#[test]
fn out_of_bounds_and_non_finite_poses_are_corrected() {
    let mut v = Validator::new(LIMITS, Vec3::ZERO);
    assert!(matches!(v.step(&Room, Vec3::new(f32::NAN, 0.0, 0.0), DT, 0), Verdict::Correct { .. }));
    assert_eq!(v.last_reason, Some(Reason::Bound));
    let mut v = Validator::new(LIMITS, Vec3::new(LIMITS.bound, 0.0, 0.0));
    assert!(matches!(v.step(&Room, Vec3::new(LIMITS.bound + 0.05, 0.0, 0.0), DT, 0), Verdict::Correct { .. }));
    assert_eq!(v.last_reason, Some(Reason::Bound));
}

#[test]
fn creeping_upward_does_not_refresh_the_hang_window() {
    let mut v = Validator::new(LIMITS, Vec3::ZERO);
    // A 1-voxel hop, then 0.001 voxel of rise each step, which stays inside the ascent budget.
    let creep = |t: f32| Vec3::new(0.0, (t * 8.0).min(1.0) + t * 0.12, 0.0);
    assert_eq!(walk(&mut v, creep, 3.0).1, Some(Reason::Float));
}

#[test]
fn a_withheld_ack_cannot_make_one_correction_sweep_without_limit() {
    let mut v = Validator::new(LIMITS, Vec3::new(0.0, 1_000.0, 0.0));
    let hover = Vec3::new(0.0, 1_000.0, 0.0);
    let id = (1..400).find_map(|_| match v.step(&Room, hover, DT, 0) {
        Verdict::Correct { id, .. } => Some(id),
        _ => None,
    });
    let id = id.expect("hover corrected");
    let before = v.accepted().y;
    assert_eq!(v.step(&Room, hover, 1.0e9, 0), Verdict::Ignored);
    assert!(matches!(v.step(&Room, hover, DT, id), Verdict::Correct { .. }));
    let after = v.accepted().y;
    assert!(after.is_finite() && after >= before - 16.0 - 1e-3, "{before} -> {after}");
}

#[test]
fn repeated_float_corrections_pull_a_hover_down_at_gravity() {
    let mut v = Validator::new(LIMITS, Vec3::new(0.0, 1_000.0, 0.0));
    walk(&mut v, |_| Vec3::new(0.0, 1_000.0, 0.0), 3.0);
    // Free fall after the hang: 0.5 * 20 * 2.4^2 = 57.6 voxels.
    assert!(v.accepted().y < 1_000.0 - 50.0, "{}", v.accepted());
}
