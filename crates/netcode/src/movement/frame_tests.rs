use super::super::{Limits, Reason, Solid, Validator, Verdict};
use crate::body::{BodyState, VELOCITY_STEPS};
use crate::carrier::{Carrier, CarrierHistory, Pose};
use crate::link::{Impairment, Link};
use glam::{DVec3, IVec3, Quat, Vec3};

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
const DT: f64 = 1.0 / 120.0;
const TURN: f64 = std::f64::consts::PI / 6.0;
const SPEED: f64 = 20.0;
/// Client gravity, stronger than the weakest the server allows.
const FALL: f32 = 25.0;

/// Ground below `floor` in the world; the hull deck is 17 x 17 cells flush with it.
enum Grid {
    World { floor: i32 },
    Deck,
}

impl Solid for Grid {
    fn solid(&self, c: IVec3) -> bool {
        match self {
            Grid::World { floor } => c.y < *floor,
            Grid::Deck => c.y < 0 && c.x.abs() <= 8 && c.z.abs() <= 8,
        }
    }
}

/// Hull circling `origin` at 20 voxels/s and 30 deg/s; local +z is forward.
#[derive(Clone, Copy)]
struct Hull {
    origin: DVec3,
    /// Peak roll in radians, so world up moves in the deck grid.
    roll: f32,
}

impl Hull {
    fn pose(&self, t: f64) -> Pose {
        let (r, a) = (SPEED / TURN, t * TURN);
        let roll = Quat::from_rotation_z(self.roll * (0.5 * t).sin() as f32);
        Pose {
            position: self.origin + DVec3::new(r * a.cos(), 0.0, r * a.sin()),
            rotation: Quat::from_rotation_y(-a as f32) * roll,
        }
    }

    fn floor(&self) -> i32 {
        self.origin.y.floor() as i32
    }
}

struct Outcome {
    corrections: usize,
    first: Option<Reason>,
    validator: Validator,
}

/// Client reports `(frame, position)` each 120 Hz tick over a 150 ms RTT, 5 % loss link.
fn run(hull: Hull, seconds: f64, mut client: impl FnMut(u64) -> (u32, Vec3)) -> Outcome {
    let link = Impairment { latency_us: 75_000, jitter_us: 4_000, loss_per_mille: 50, ..Default::default() };
    let (mut up, mut down) = (Link::new(link, 7), Link::new(link, 8));
    let mut history = CarrierHistory::new(DT, 256);
    history.push(0, hull.pose(0.0));
    let (frame, start) = client(0);
    let mut v = Validator::new(LIMITS, start);
    let mut ack = match v.place_in_frame(frame, Vec3::Y, start) {
        Verdict::Correct { id, .. } => id,
        _ => 0,
    };
    let (mut last, mut out) = (0u64, Outcome { corrections: 0, first: None, validator: Validator::new(LIMITS, start) });
    for tick in 1..(seconds / DT) as u64 {
        let now = (tick as f64 * DT * 1e6) as u64;
        history.push(tick, hull.pose(tick as f64 * DT));
        let (frame, p) = client(tick);
        let mut bytes = tick.to_le_bytes().to_vec();
        bytes.extend(frame.to_le_bytes());
        bytes.extend(p.to_array().iter().flat_map(|c| c.to_le_bytes()));
        bytes.extend(ack.to_le_bytes());
        up.send(now, bytes);
        while let Some(m) = up.poll(now) {
            let at = u64::from_le_bytes(m[..8].try_into().unwrap());
            let frame = u32::from_le_bytes(m[8..12].try_into().unwrap());
            let c = |i: usize| f32::from_le_bytes(m[12 + i * 4..16 + i * 4].try_into().unwrap());
            let echoed = u16::from_le_bytes(m[24..26].try_into().unwrap());
            if at <= last {
                continue;
            }
            let dt = ((at - last) as f64 * DT) as f32;
            last = at;
            let grid = if frame == 0 { Grid::World { floor: hull.floor() } } else { Grid::Deck };
            let carrier = |f: u32| if f == 1 { history.carrier(at) } else { None };
            if let Verdict::Correct { id, .. } = v.step_in_frame(&grid, carrier, frame, Vec3::new(c(0), c(1), c(2)), dt, echoed) {
                out.corrections += 1;
                out.first = out.first.or(v.last_reason);
                down.send(now, id.to_le_bytes().to_vec());
            }
        }
        while let Some(m) = down.poll(now) {
            ack = u16::from_le_bytes(m[..2].try_into().unwrap());
        }
    }
    out.validator = v;
    out
}

fn seconds(tick: u64) -> f32 {
    (tick as f64 * DT) as f32
}

/// Circle of radius 4 at 5 voxels/s with 1.2-voxel hops.
fn deck_walk(tick: u64) -> (u32, Vec3) {
    let t = seconds(tick);
    let hop = (t * std::f32::consts::PI).sin().max(0.0) * 1.2;
    (1, Vec3::new(4.0 * (1.25 * t).cos(), hop, 4.0 * (1.25 * t).sin()))
}

const STILL: Hull = Hull { origin: DVec3::ZERO, roll: 0.0 };
const ROLLING: Hull = Hull { origin: DVec3::new(0.0, 0.0, 0.0), roll: 0.05 };
/// Far from the origin, below zero and crossing chunk edges on every axis it moves.
const FAR: Hull = Hull { origin: DVec3::new(-98_765.4, -321.0, 65_432.1), roll: 0.0 };

#[test]
fn walking_aboard_a_turning_moving_hull_is_never_corrected() {
    for hull in [STILL, ROLLING, FAR] {
        let out = run(hull, 60.0, deck_walk);
        assert_eq!((out.corrections, out.first), (0, None));
        assert_eq!(out.validator.frame(), 1);
    }
}

#[test]
fn a_speed_hack_aboard_is_corrected() {
    let out = run(ROLLING, 3.0, |tick| {
        let t = seconds(tick);
        (1, Vec3::new(3.0 * (3.0 * t).cos(), 0.0, 3.0 * (3.0 * t).sin()))
    });
    assert!(out.corrections > 0);
    assert_eq!(out.first, Some(Reason::Speed));
}

#[test]
fn hovering_aboard_is_caught() {
    let out = run(ROLLING, 4.0, |tick| (1, Vec3::new(0.0, (seconds(tick) * 3.0).min(2.4), 0.0)));
    assert!(out.corrections > 0);
    assert_eq!(out.first, Some(Reason::Float));
}

#[test]
fn a_report_in_an_unknown_frame_is_refused() {
    let out = run(STILL, 1.0, |tick| (if tick > 60 { 9 } else { 1 }, Vec3::ZERO));
    assert_eq!(out.first, Some(Reason::Frame));
}

#[test]
fn leaving_velocity_is_carrier_point_velocity_plus_the_jump() {
    let hull = ROLLING;
    let mut history = CarrierHistory::new(DT, 16);
    (0..16).for_each(|t| history.push(t, hull.pose(t as f64 * DT)));
    let (local, jump) = (Vec3::new(-3.0, 0.0, 5.0), Vec3::new(1.0, 7.0, 0.0));
    let carrier = history.carrier(8).unwrap();
    let h = 1e-4;
    let truth = (hull.pose(8.0 * DT + h).to_world(local) - hull.pose(8.0 * DT - h).to_world(local)) / (2.0 * h);
    let truth = truth.as_vec3() + hull.pose(8.0 * DT).rotation * jump;
    let q = |v: Vec3| BodyState::quantise(Vec3::ZERO, Quat::IDENTITY, v, Vec3::ZERO, 0).velocity;
    let (got, want) = (q(carrier.leave_velocity(local, jump)), q(truth));
    assert!((0..3).all(|i| (got[i] - want[i]).abs() <= 1), "{got:?} {want:?} step {}", 1.0 / VELOCITY_STEPS);
}

/// Walks inward aboard, jumps off with the carrier's motion, slides to a stop on
/// the ground and boards again when the deck comes round.
fn jump_off_and_reboard(hull: Hull) -> impl FnMut(u64) -> (u32, Vec3) {
    let (jump_at, jump) = (120u64, Vec3::new(-3.0, 7.0, 0.0));
    let mut left: Option<(f32, DVec3, Vec3)> = None;
    let mut boarded: Option<Vec3> = None;
    move |tick| {
        let t = seconds(tick);
        let pose = hull.pose(tick as f64 * DT);
        if let Some(local) = boarded {
            return (1, local);
        }
        if tick < jump_at {
            return (1, Vec3::new(-3.0 * t, 0.0, 0.0));
        }
        let (t0, w0, v0) = *left.get_or_insert_with(|| {
            let local = Vec3::new(-3.0 * t, 0.0, 0.0);
            (t, pose.to_world(local), rigid(hull, tick).leave_velocity(local, jump))
        });
        let s = t - t0;
        let air = 2.0 * v0.y / FALL;
        let horizontal = Vec3::new(v0.x, 0.0, v0.z);
        // Ballistic until the ground, then a linear stop over one second.
        let run = if s < air { horizontal * s } else { horizontal * (air + (s - air).min(1.0) * (1.0 - 0.5 * (s - air).min(1.0))) };
        let rise = if s < air { v0.y * s - 0.5 * FALL * s * s } else { 0.0 };
        let world = w0 + (run + Vec3::Y * rise).as_dvec3();
        let local = pose.to_local(world);
        if s > air + 1.0 && local.x.abs() < 6.0 && local.z.abs() < 6.0 {
            boarded = Some(Vec3::new(local.x, 0.0, local.z));
            return (1, Vec3::new(local.x, 0.0, local.z));
        }
        (0, world.as_vec3())
    }
}

/// Analytic carrier state of `hull` at `tick`.
fn rigid(hull: Hull, tick: u64) -> Carrier {
    let t = tick as f64 * DT;
    let pose = hull.pose(t);
    let velocity = ((hull.pose(t + 1e-4).position - hull.pose(t - 1e-4).position) / 2e-4).as_vec3();
    Carrier { pose, velocity, angular: Vec3::new(0.0, -TURN as f32, 0.0) }
}

#[test]
fn jumping_off_and_boarding_again_never_corrects() {
    for hull in [STILL, FAR] {
        let mut boarded_at = None;
        let mut client = jump_off_and_reboard(hull);
        let out = run(hull, 30.0, |tick| {
            let r = client(tick);
            if tick > 200 && r.0 == 1 {
                boarded_at = boarded_at.or(Some(tick));
            }
            r
        });
        assert_eq!((out.corrections, out.first), (0, None));
        assert!(boarded_at.is_some(), "deck never came round");
        assert_eq!(out.validator.frame(), 1);
    }
}

#[test]
fn poses_round_trip_far_from_the_origin() {
    for origin in [DVec3::new(-1.0e9, -40_000.5, 3.0e8), DVec3::new(2_147_483_000.0, 31.99, -32.01)] {
        let pose = Pose { position: origin, rotation: Quat::from_rotation_y(1.1) * Quat::from_rotation_x(0.3) };
        for local in [Vec3::new(31.9, -0.1, 32.1), Vec3::new(-200.0, 5.0, 0.5)] {
            assert!((pose.to_local(pose.to_world(local)) - local).length() < 1e-3, "{origin} {local}");
        }
    }
}
