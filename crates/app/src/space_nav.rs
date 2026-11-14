//! Star-system navigation: the player's fix, body contacts and the chosen target.
//!
//! Display only. Everything here reads the rails; nothing feeds the simulation.
use crate::planet::{influence, rails, void};
use engine_core::cube_sphere::atlas;
use engine_space::{BodyId, BodyKind, Snapshot, System};
use glam::{DMat3, DVec3, Vec3};
use std::sync::{Mutex, MutexGuard};

pub mod text;
mod transfer;
pub use transfer::{Transfer, transfer};

pub const TICKS_PER_SECOND: f64 = 120.0;
/// Half-angle of the cone in which a body counts as under the crosshair.
const AIM_CONE_RAD: f32 = 0.21;
/// A body wider than this fills the view and is the ground, not a contact.
const GROUND_ANGLE_RAD: f32 = 1.0;
/// Range samples further apart than this many ticks restart the closing speed.
const TRACK_GAP_TICKS: u64 = 240;

/// Where the player is in the star system at one tick.
pub struct Fix {
    pub tick: u64,
    /// System-frame metres.
    pub pos: DVec3,
    /// System axes to the world axes the camera looks along.
    to_view: DMat3,
    /// Body whose frame the player flies in.
    pub reference: BodyId,
    pub snapshot: Snapshot,
}

/// One body as seen from a fix.
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub body: BodyId,
    /// Unit direction in world axes.
    pub dir: Vec3,
    /// Metres to the surface.
    pub range_m: f64,
    pub angular_radius: f32,
}

impl Fix {
    /// Fix of an atlas point in metres.
    pub fn at(system: &System, tick: u64, eye_m: DVec3) -> Option<Fix> {
        let home = system.id("home")?;
        let snapshot = rails::with_snapshot(system, tick, Snapshot::clone);
        let rot = system.orientation(home, tick);
        let pos = snapshot.position(home) + rot * atlas::to_body(eye_m);
        let to_view = atlas::frame(eye_m).transpose() * DMat3::from_quat(rot.inverse());
        let held = system.dominant_body(&snapshot, pos);
        // Home's Laplace sphere ends below its moon; flight stays in home's
        // frame out to the Hill radius, so the readout does too.
        let near_home = void::home_reach(system).is_some_and(|reach| pos.distance(snapshot.position(home)) < reach);
        let reference = if system.parent(held).is_none() && near_home { home } else { held };
        Some(Fix { tick, pos, to_view, reference, snapshot })
    }

    pub fn contact(&self, system: &System, body: BodyId) -> Contact {
        let delta = self.snapshot.position(body) - self.pos;
        let radius = surface_radius(system, body);
        let distance = delta.length().max(1.0);
        Contact {
            body,
            dir: (self.to_view * (delta / distance)).as_vec3(),
            range_m: (distance - radius).max(0.0),
            angular_radius: (radius / distance).min(1.0).asin() as f32,
        }
    }

    /// Every body off the ground, nearest first.
    pub fn contacts(&self, system: &System) -> Vec<Contact> {
        let mut list: Vec<Contact> = system
            .bodies()
            .filter(|&b| !matches!(system.kind(b), BodyKind::Barycenter))
            .map(|b| self.contact(system, b))
            .filter(|c| c.angular_radius < GROUND_ANGLE_RAD)
            .collect();
        list.sort_by(|a, b| a.range_m.total_cmp(&b.range_m));
        list
    }

    /// Height above the reference body's surface, metres.
    pub fn altitude_m(&self, system: &System) -> f64 {
        self.contact(system, self.reference).range_m
    }

    /// World-axes offset of a system-frame point from the eye, metres.
    pub fn view_of(&self, point: DVec3) -> DVec3 {
        self.to_view * (point - self.pos)
    }
}

/// Fix for the player of a planet world; `None` on flat worlds and in hell.
pub fn fix(world: &crate::world::World, player: &crate::player::Player) -> Option<Fix> {
    if world.world_type != crate::world::WorldType::Planet {
        return None;
    }
    let eye_m = player.precise_eye(world.voxel_scale) * f64::from(world.voxel_scale.meters());
    Fix::at(influence::system()?, world.orbital.planet_tick, eye_m)
}

/// Whether the player is high enough that the surface map shows nothing.
pub fn in_orbit(world: &crate::world::World, player: &crate::player::Player) -> bool {
    let (Some(system), Some(fix)) = (system(), fix(world, player)) else { return false };
    player.anchor.is_some()
        || player.void.is_some()
        || system.name(fix.reference) != "home"
        || fix.altitude_m(system) > crate::altitude::ORBIT_M
}

pub fn system() -> Option<&'static System> {
    influence::system()
}

/// Radius the ground is drawn at: home's is the atlas sphere, not the rails'.
pub fn surface_radius(system: &System, body: BodyId) -> f64 {
    if system.name(body) == "home" { atlas::RADIUS_M } else { system.radius(body) }
}

#[derive(Clone, Copy)]
struct Track {
    body: BodyId,
    tick: u64,
    range_m: f64,
}

struct State {
    target: Option<BodyId>,
    orbit_lines: bool,
    track: Option<Track>,
    closing_mps: f64,
}

static STATE: Mutex<State> = Mutex::new(State { target: None, orbit_lines: true, track: None, closing_mps: 0.0 });

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn target() -> Option<BodyId> {
    state().target
}

pub fn set_target(body: Option<BodyId>) {
    let mut s = state();
    if s.target != body {
        s.target = body;
        s.track = None;
        s.closing_mps = 0.0;
    }
}

pub fn orbit_lines() -> bool {
    state().orbit_lines
}

pub fn set_orbit_lines(on: bool) {
    state().orbit_lines = on;
}

/// Smoothed rate at which the range to `body` shrinks, m/s; negative when opening.
pub fn closing_mps(body: BodyId, tick: u64, range_m: f64) -> f64 {
    let mut s = state();
    match s.track {
        Some(t) if t.body == body && tick == t.tick => return s.closing_mps,
        Some(t) if t.body == body && tick > t.tick && tick - t.tick <= TRACK_GAP_TICKS => {
            let raw = (t.range_m - range_m) * TICKS_PER_SECOND / (tick - t.tick) as f64;
            s.closing_mps += (raw - s.closing_mps) * 0.25;
        }
        _ => s.closing_mps = 0.0,
    }
    s.track = Some(Track { body, tick, range_m });
    s.closing_mps
}

/// Next target: the body under the crosshair, else the next one outward.
///
/// `contacts` is nearest first. Past the farthest body the target clears.
pub fn pick(contacts: &[Contact], forward: Vec3, current: Option<BodyId>) -> Option<BodyId> {
    let ahead = contacts
        .iter()
        .filter(|c| Some(c.body) != current)
        .map(|c| (c, c.dir.angle_between(forward)))
        .filter(|(c, angle)| *angle < AIM_CONE_RAD.max(c.angular_radius))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((c, _)) = ahead {
        return Some(c.body);
    }
    match current.and_then(|b| contacts.iter().position(|c| c.body == b)) {
        Some(i) => contacts.get(i + 1).map(|c| c.body),
        None => contacts.first().map(|c| c.body),
    }
}

/// Targets the body ahead or the next one outward.
pub fn cycle_target(world: &crate::world::World, player: &crate::player::Player) {
    let (Some(system), Some(fix)) = (system(), fix(world, player)) else { return };
    set_target(pick(&fix.contacts(system), player.forward_dir(), target()));
}

/// Review captures: `VOXEL_REVIEW_NAV=<body>[,map]` targets a body once and can open the map.
pub fn review(show_map: &mut bool) -> bool {
    static SPEC: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    static APPLIED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let Some(spec) = SPEC.get_or_init(|| std::env::var("VOXEL_REVIEW_NAV").ok()) else { return false };
    if !APPLIED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        let mut parts = spec.split(',');
        set_target(parts.next().and_then(|name| system()?.id(name)));
        *show_map = parts.any(|part| part == "map");
    }
    true
}

#[cfg(test)]
#[path = "space_nav/tests.rs"]
mod tests;
