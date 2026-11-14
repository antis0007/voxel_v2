//! Express lift ride: one cabin on a guide from the terminal floor to the
//! crown anchorage, run on the trajectory in `engine_space::express`.
//!
//! The cabin is a moving frame. The passenger's physics run in cabin-local
//! metres against a flat floor and four walls under the compensated cabin
//! field; the cabin's own position along the guide comes from the trajectory
//! each fixed step, so an outside observer, the panel and the physics all read
//! one state. Nothing here enters the save: a ride is transient, and a world
//! loaded mid-ride starts docked at the terminal.
use super::{CELL_M, State as Orbital, crown_position, footing_height, hold_height, site};
use crate::player::{Input, Player};
use crate::simulation::STEP_SECONDS;
use crate::world::{CHUNK_SIZE, World, chunk_coord_from_world_block, collision::collides_world_local};
use engine_space::express::{G0, Phase, Profile, Telemetry, telemetry};
use glam::{DVec3, IVec2, IVec3, Vec3};

/// Interior half extents, metres: floor 7 m square, 3 m of headroom.
pub const HALF_WIDTH_M: f32 = 3.5;
pub const HEIGHT_M: f32 = 3.0;
const WALK_MPS: f32 = 3.6;
const JUMP_MPS: f32 = 4.2;
/// Residency stays at the terminal's full radius inside this many seconds of
/// arrival, so the return doors open onto loaded ground.
pub const PREFETCH_S: f64 = 40.0;
/// Covers the tension socket and the starting terminal's passenger concourse.
const BOARDING_RADIUS_M: f64 = 64.0;
/// Door in the cabin's +x wall, open only while docked at the crown.
pub const DOOR_HALF_M: f32 = 1.1;
/// Station deck: concourse around the shaft and a gallery arm along +x.
pub const DECK_HEIGHT_M: f32 = 4.5;
pub const GALLERY_LEN_M: f32 = 26.0;
pub const GALLERY_HALF_M: f32 = 4.0;
const DECK_MARGIN_M: f32 = 0.35;
const DOOR_TRANSFER_X_M: f32 = HALF_WIDTH_M + 0.25;
/// Pressure rim spin holds this at the ring radius (architecture.rs).
pub const RIM_G: f64 = 0.3;
/// The guide and its shoe sit outside the cabin's rear wall.
pub const GUIDE_OFFSET_M: f32 = HALF_WIDTH_M + 0.9;
pub const SHELL_TRIM_M: f32 = 0.12;
pub const GUIDE_SHOE_HALF_M: f32 = 0.475;
pub const DOCK_APPROACH_M: f64 = 64.0;
const DOCK_INSET_M: f32 = 0.65;
pub const DOCK_SILL_GAP_M: f32 = super::habitat::THROAT_X_M + DOCK_INSET_M - HALF_WIDTH_M;
const BOW_SPAN_M: f32 = 18_000.0;
const OBSTRUCTED: &str = "Lift path obstructed";

/// Lateral offset of the passenger guide at a point along the span.
pub fn sway_m(bow: Vec3, along: f64, length: f64) -> Vec3 {
    let s = (along / length.max(1.0)).clamp(0.0, 1.0) as f32;
    let scale = (length as f32 / BOW_SPAN_M).powi(2);
    Vec3::new(bow.x, 0.0, bow.z) * scale * (std::f32::consts::PI * s).sin()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ride {
    pub ascending: bool,
    /// Fixed steps since departure activation.
    pub ticks: u64,
    pub profile: Profile,
    /// Cabin floor centre at the terminal, world metres.
    pub berth_m: DVec3,
    /// Passenger feet relative to the cabin floor centre, metres. On the deck
    /// the same frame continues: the docked cabin centre is the deck origin.
    pub local_m: Vec3,
    pub local_vel: Vec3,
    /// Passenger is on the station deck, cabin waiting docked at the crown.
    pub deck: bool,
    /// Concourse half width at this crown, from the anchorage class.
    pub hub_m: f32,
    /// Braid sag at the cabin's height now, world metres; updated each step.
    pub sway_m: Vec3,
}

/// Concourse half width: inside the anchorage keel, which stands at 6.5 % to
/// 9 % of the ring radius.
pub fn hub_half_m(ring_radius_m: f32) -> f32 {
    (ring_radius_m * 0.085).clamp(HALF_WIDTH_M + 4.0, 18.0)
}

/// Deck slabs in local metres: centre x/z and full width/depth.
pub fn deck_floor_spans(hub: f32) -> [(f32, f32, f32, f32); 6] {
    let shaft = HALF_WIDTH_M + DECK_MARGIN_M;
    [
        (0.0, -(hub + shaft) * 0.5, 2.0 * hub, hub - shaft),
        (0.0, (hub + shaft) * 0.5, 2.0 * hub, hub - shaft),
        (-(hub + shaft) * 0.5, 0.0, hub - shaft, 2.0 * shaft),
        ((hub + shaft) * 0.5, 0.0, hub - shaft, 2.0 * shaft),
        (hub + GALLERY_LEN_M * 0.5, 0.0, GALLERY_LEN_M, 2.0 * GALLERY_HALF_M),
        // Extend into the cabin so a passenger's footprint can cross the seam.
        (HALF_WIDTH_M - DECK_MARGIN_M * 0.5, 0.0,
            3.0 * DECK_MARGIN_M, 2.0 * DOOR_HALF_M),
    ]
}

/// Whether the passenger footprint stays on the rendered deck slabs.
pub fn deck_walkable(p: glam::Vec2, hub_m: f32) -> bool {
    let spans = deck_floor_spans(hub_m);
    [-1.0, 1.0].into_iter().all(|x| {
        [-1.0, 1.0].into_iter().all(|z| {
            let corner = p + glam::Vec2::new(x, z) * DECK_MARGIN_M;
            spans.iter().any(|&(cx, cz, sx, sz)| {
                (corner.x - cx).abs() <= sx * 0.5
                    && (corner.y - cz).abs() <= sz * 0.5
            })
        })
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub ride: Option<Ride>,
    blocked: bool,
    checked_revision: Option<u64>,
}

/// Panel readings and the actions available now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct View {
    pub pressure: Option<(f64, f64)>,
    pub riding: bool,
    pub ascending: bool,
    pub telemetry: Option<Telemetry>,
    pub route_m: f64,
    pub rated_g: f64,
    pub rated_mps: f64,
    pub can_board: bool,
    pub can_descend: bool,
    /// Passenger on the station deck: the cabin panel is out of view.
    pub on_deck: bool,
    pub board: Option<Board>,
}

/// Anchorage board readings shown on the station deck.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Board {
    pub class: &'static str,
    pub altitude_m: f64,
    /// Day fraction 0..1, local.
    pub time_of_day: f64,
    /// Seconds to the next sunrise or sunset, and which one.
    pub next_light_s: f64,
    pub next_is_sunrise: bool,
    pub rim_rpm: f64,
    pub rim_g: f64,
    pub ring_radius_m: f64,
    /// Braids within sight range of the crown.
    pub braids_in_sight: usize,
    pub door_open: bool,
}

/// Next sunrise or sunset from the day fraction: sunrise at 0.25, sunset at
/// 0.75 (world.rs sun angle). Returns the fraction of a day to wait.
pub fn next_light(time_of_day: f64) -> (f64, bool) {
    let t = time_of_day.rem_euclid(1.0);
    if t < 0.25 { (0.25 - t, true) } else if t < 0.75 { (0.75 - t, false) } else { (1.25 - t, true) }
}

/// Rim spin in rpm for `RIM_G` at the given radius.
pub fn rim_rpm(ring_radius_m: f64) -> f64 {
    (RIM_G * G0 / ring_radius_m.max(1.0)).sqrt() * 60.0 / std::f64::consts::TAU
}

fn board_readings(world: &World, day: &crate::world::DayNightCycle, ride: &Ride) -> Board {
    let crown = ride.berth_m + DVec3::Y * ride.profile.length_m;
    // Nearest corridor site: cell boundaries do not always contain their site.
    let centre = IVec2::new((crown.x / CELL_M as f64).floor() as i32, (crown.z / CELL_M as f64).floor() as i32);
    let here = (-1..=1).flat_map(|z| (-1..=1).map(move |x| IVec2::new(x, z)))
        .filter_map(|d| site(world.seed, centre + d))
        .min_by(|a, b| {
            let da = (a.x as f64 - crown.x).hypot(a.z as f64 - crown.z);
            let db = (b.x as f64 - crown.x).hypot(b.z as f64 - crown.z);
            da.total_cmp(&db)
        });
    let (class, r) = here.map(|s| (s.class.name(), s.class.upper_ring_radius_m() as f64))
        .unwrap_or(("Anchorage", 220.0));
    let t = day.time_of_day() as f64;
    let (next, sunrise) = next_light(t);
    let braids = super::visual_contacts(world.seed, day, &world.orbital, crown.as_vec3())
        .iter().filter(|c| c.distance_m <= super::SIGHT_RANGE_M).count();
    Board {
        class,
        altitude_m: crown.y,
        time_of_day: t,
        next_light_s: next * day.day_length_seconds() as f64,
        next_is_sunrise: sunrise,
        rim_rpm: rim_rpm(r),
        rim_g: RIM_G,
        ring_radius_m: r,
        braids_in_sight: braids,
        door_open: ride.ascending && ride.docked(),
    }
}

impl Ride {
    pub fn trip_s(&self) -> f64 {
        self.ticks as f64 * STEP_SECONDS as f64
    }

    pub fn telemetry(&self) -> Telemetry {
        telemetry(&self.profile, self.trip_s(), self.berth_m.y, self.ascending)
    }

    /// Cabin floor centre now, world metres.
    pub fn cabin_m(&self) -> DVec3 {
        self.shaft_m(self.along_m()) + self.sway_m.as_dvec3()
    }

    pub fn shaft_m(&self, along: f64) -> DVec3 {
        // The voxel airlock starts 3 m east of the crown datum. Ease the
        // cabin west before the tunnel, leaving its door beside the sill.
        let remaining = self.profile.length_m - along;
        let t = ((DOCK_APPROACH_M - remaining) / (DOCK_APPROACH_M - 8.0)).clamp(0.0, 1.0);
        self.berth_m + DVec3::Y * along - DVec3::X * (f64::from(DOCK_INSET_M) * t * t * (3.0 - 2.0 * t))
    }

    /// Distance up the guide now, metres.
    pub fn along_m(&self) -> f64 {
        let s = self.profile.sample(self.trip_s());
        if self.ascending { s.position_m } else { self.profile.length_m - s.position_m }
    }

    pub fn phase(&self) -> Phase {
        self.profile.phase(self.trip_s())
    }

    pub fn docked(&self) -> bool {
        self.phase() == Phase::Docked
    }
}

/// Resolve a passenger route at any generated corridor, independently of the
/// world's single freight receiver. The ground terminal has a fixed landing;
/// elsewhere the local floor can be far below the freight clearance datum.
fn boarding_route(world: &World, player: &Player) -> Option<(DVec3, f64, f32, IVec2)> {
    if world.ca_view.is_some() || !world.rules.mode.interacts() || world.adventure.is_dead()
        || !player.on_ground || player.flying || player.swimming
    {
        return None;
    }
    let feet = player.precise_pos * world.voxel_scale.meters() as f64;
    let centre = IVec2::new((feet.x / CELL_M as f64).floor() as i32,
        (feet.z / CELL_M as f64).floor() as i32);
    let cfg = world.sample_config();
    for z in -1..=1 {
        for x in -1..=1 {
            let Some(s) = site(world.seed, centre + IVec2::new(x, z)) else { continue };
            let distance = (feet.x - s.x as f64).hypot(feet.z - s.z as f64);
            // Keep existing receiver berths usable out to their 96 m alignment
            // limit, without requiring a receiver at any other elevator.
            let receiver_berth = s.cell == IVec2::ZERO
                && distance <= 102.0 && super::receiver::boarding_available(world, player);
            if distance > BOARDING_RADIUS_M && !receiver_berth { continue; }
            let floor = footing_height(&cfg, &world.orbital, s) as f64;
            if !receiver_berth && !(floor - 24.0..=floor + 24.0).contains(&feet.y) { continue; }
            let crown = crown_position(s, hold_height(&cfg, &world.orbital, s));
            let berth_y = if s.cell == IVec2::ZERO && engine_worldgen::meridian_terminal::enabled(&cfg) {
                floor
            } else {
                feet.y
            };
            let length = crown.y as f64 - berth_y;
            if length > 0.0 {
                let berth = DVec3::new(s.x as f64, berth_y, s.z as f64);
                return Some((berth, length, s.class.upper_ring_radius_m(), s.cell));
            }
        }
    }
    None
}

pub fn can_board(world: &World, player: &Player) -> bool {
    world.orbital.express.ride.is_none()
        && boarding_route(world, player).is_some()
}

/// Boards the corridor's persistent cabin and activates departure.
pub fn board(world: &mut World, player: &Player) -> bool {
    if world.orbital.express.ride.is_some() {
        return false;
    }
    let Some((mut berth, length, ring, cell)) = boarding_route(world, player) else {
        return false;
    };
    let existing = world.orbital.habitats.habitats.iter().find(|h| h.cell == cell);
    let crown = existing.map_or_else(
        || (berth + DVec3::Y * length).as_vec3().round(),
        |h| h.origin.as_vec3(),
    );
    berth.x = crown.x as f64;
    berth.z = crown.z as f64;
    berth = ground_berth(world, cell, berth);
    let ride = Ride {
        ascending: true,
        ticks: 0,
        profile: Profile::express(crown.y as f64 - berth.y),
        berth_m: berth,
        local_m: Vec3::ZERO,
        local_vel: Vec3::ZERO,
        deck: false,
        hub_m: hub_half_m(ring),
        sway_m: Vec3::ZERO,
    };
    if obstruction(world, ride.cabin_m(), ride.cabin_m(), false) {
        world.expedition.say(OBSTRUCTED);
        return false;
    }
    if existing.is_none() && !super::habitat::register(world, cell, crown, berth.as_vec3()) {
        return false;
    }
    world.orbital.express = State {
        ride: Some(ride), blocked: false, checked_revision: Some(world.revision()),
    };
    world.goals.note(crate::world::goals::Event::LiftBoarded);
    true
}

pub(super) fn ground_berth(world: &World, cell: IVec2, mut berth: DVec3) -> DVec3 {
    let cfg = world.sample_config();
    let scale = cfg.voxel_meters() as f64;
    let floor = if cell == IVec2::ZERO && engine_worldgen::meridian_terminal::enabled(&cfg) {
        // The terminal fills voxels whose centres are below the authored floor.
        (engine_worldgen::meridian_terminal::floor(&cfg) as f64 / scale - 0.5).ceil() * scale
    } else {
        let edge = (HALF_WIDTH_M + SHELL_TRIM_M) as f64;
        let rear = (GUIDE_OFFSET_M + GUIDE_SHOE_HALF_M) as f64;
        let lo = ((berth - DVec3::new(edge, 0.0, rear)) / scale).floor().as_ivec3();
        let hi = ((berth + DVec3::new(edge, 0.0, edge)) / scale).ceil().as_ivec3();
        let top = (lo.z..hi.z)
            .flat_map(|z| (lo.x..hi.x).map(move |x| (x, z)))
            .map(|(x, z)| world.terrain_height_at(x, z) + 1)
            .max()
            .unwrap_or_default();
        top as f64 * scale
    };
    berth.y = floor + SHELL_TRIM_M as f64 + 0.001;
    berth
}

fn obstruction(world: &World, from: DVec3, to: DVec3, start_clear: bool) -> bool {
    if start_clear && from == to { return false; }
    let scale = world.voxel_scale.meters() as f64;
    let edge = (HALF_WIDTH_M + SHELL_TRIM_M) as f64;
    let trim = SHELL_TRIM_M as f64;
    let guide = GUIDE_OFFSET_M as f64;
    let shoe = GUIDE_SHOE_HALF_M as f64;
    // Sweep the cabin and rear hardware across the whole step.
    let bounds = [
        (DVec3::new(-edge, -trim, -edge), DVec3::new(edge, HEIGHT_M as f64 + trim, edge)),
        (DVec3::new(-edge, 0.0, -guide - shoe), DVec3::new(edge, HEIGHT_M as f64, -edge)),
    ];
    bounds.into_iter().any(|(lower, upper)| {
        let min = (from.min(to) + lower) / scale;
        let max = (from.max(to) + upper) / scale;
        if !start_clear { return blocked_box(world, min, max); }
        // Only query newly occupied space until a world edit invalidates the
        // last clearance check. A cabin spans thousands of fine-scale voxels.
        (0..3).any(|axis| {
            let (mut lo, mut hi) = (min, max);
            if to[axis] > from[axis] {
                lo[axis] = (from[axis] + upper[axis]) / scale - 0.001;
            } else if to[axis] < from[axis] {
                hi[axis] = (from[axis] + lower[axis]) / scale + 0.001;
            } else {
                return false;
            }
            blocked_box(world, lo, hi)
        })
    })
}

fn blocked_box(world: &World, min: DVec3, max: DVec3) -> bool {
    let ceiling = world.sample_config().max_world_y();
    let lo = chunk_coord_from_world_block(min.floor().as_ivec3());
    let hi = chunk_coord_from_world_block(max.ceil().as_ivec3() - IVec3::ONE);
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let chunk = IVec3::new(x, y, z);
                let origin = chunk * CHUNK_SIZE;
                // Only built or saved chunks can obstruct the guide above
                // procedural terrain. Skip empty space without voxel reads.
                if origin.y >= ceiling && !world.chunks.contains_key(&chunk)
                    && !world.regions.view.records.contains_key(&chunk) {
                    continue;
                }
                let origin_d = origin.as_dvec3();
                let local_min = (min - origin_d).max(DVec3::ZERO).as_vec3();
                let local_max = (max - origin_d).min(DVec3::splat(CHUNK_SIZE as f64)).as_vec3();
                if collides_world_local(world, local_min, local_max, origin) { return true; }
            }
        }
    }
    false
}

/// Activates the return trip from a cabin docked at the crown.
pub fn descend(world: &mut World) -> bool {
    if world.orbital.express.ride.is_some_and(|r| {
        let crown = (r.berth_m + DVec3::Y * r.profile.length_m).as_vec3();
        world.orbital.habitats.habitats.iter().any(|h| (h.doors_open[0] || h.cycle == 0)
            && h.origin.as_vec3().distance(crown) < 1.0)
    }) {
        world.expedition.say("Seal cabin hatch before departure");
        return false;
    }
    let Some(ride) = world.orbital.express.ride.as_mut() else { return false };
    if !(ride.ascending && ride.docked() && !ride.deck) {
        return false;
    }
    ride.ascending = false;
    ride.ticks = 0;
    true
}

pub fn prompt(world: &World, player: &Player) -> Option<crate::world::interaction::Prompt> {
    if let Some(prompt) = super::cargo::prompt(world, player) { return Some(prompt); }
    if world.orbital.express.blocked {
        return Some(crate::world::interaction::Prompt { title: OBSTRUCTED, action: "[F] Return" });
    }
    let action = if world.orbital.express.ride.is_some_and(|r| r.ascending && r.docked() && !r.deck) {
        "[F] Descend"
    } else if can_board(world, player) {
        "[F] Board Lift"
    } else {
        return None;
    };
    Some(crate::world::interaction::Prompt { title: "Express Lift", action })
}

/// The normal use key boards at a base or starts the docked return trip.
pub fn interact(world: &mut World, player: &Player) -> bool {
    if super::cargo::interact(world, player) { return true; }
    let lift = &mut world.orbital.express;
    if lift.blocked && let Some(ride) = lift.ride.as_mut() {
        // Mirror the symmetric trajectory to reverse from the same position.
        let trip_ticks = (ride.profile.total_s() / STEP_SECONDS as f64).ceil() as u64;
        ride.ticks = trip_ticks.saturating_sub(ride.ticks);
        ride.ascending = !ride.ascending;
        lift.blocked = false;
        lift.checked_revision = None;
        world.expedition.say("Lift returning");
        return true;
    }
    if descend(world) || board(world, player) {
        world.expedition.say("Express Lift · Departing");
        return true;
    }
    false
}

pub fn view(world: &World, player: &Player, day: &crate::world::DayNightCycle) -> View {
    let pressure = super::habitat::readings(world, player);
    let Some(ride) = world.orbital.express.ride else {
        if pressure.is_some() { return View { pressure, on_deck: true, ..View::default() }; }
        return match boarding_route(world, player) {
            Some((_, length, _, _)) => {
                let p = Profile::express(length);
                View { can_board: true, route_m: p.length_m, rated_g: p.peak_acceleration() / G0,
                    rated_mps: p.peak_speed(), ..View::default() }
            }
            None => View::default(),
        };
    };
    let mut telemetry = ride.telemetry();
    if world.orbital.express.blocked {
        telemetry.speed_mps = 0.0;
        telemetry.drive_g = 0.0;
        telemetry.cabin_g = 1.0;
    }
    View {
        pressure,
        riding: true,
        ascending: ride.ascending,
        telemetry: Some(telemetry),
        route_m: ride.profile.length_m,
        rated_g: ride.profile.peak_acceleration() / G0,
        rated_mps: ride.profile.peak_speed(),
        can_board: false,
        can_descend: ride.ascending && ride.docked() && !ride.deck,
        on_deck: ride.deck,
        board: ride.deck.then(|| board_readings(world, day, &ride)),
    }
}

/// Altitude the chunk residency should plan for: the terminal's while the
/// cabin is about to arrive there, the real one otherwise.
pub fn residency_altitude_m(orbital: &Orbital, altitude_m: f64) -> f64 {
    match orbital.express.ride {
        Some(r) if !r.ascending && r.profile.arrival_s(r.trip_s()) <= PREFETCH_S => 0.0,
        _ => altitude_m,
    }
}

/// Drive cue for the soundscape: share of peak speed and signed share of peak
/// drive, both zero when not riding.
pub fn drive_cue(orbital: &Orbital) -> (f32, f32) {
    let Some(r) = orbital.express.ride else { return (0.0, 0.0) };
    // Docked: air handling only, a faint hum under the deck.
    if r.docked() || orbital.express.blocked { return (0.06, 0.0); }
    let s = r.profile.sample(r.trip_s());
    let a = r.profile.peak_acceleration().max(1e-9);
    let v = r.profile.peak_speed().max(1e-9);
    ((s.velocity_mps / v) as f32, (s.acceleration_mps2 / a) as f32)
}

/// Walls of the current frame: cabin box with its crown door, or the deck plan.
fn walls(ride: &mut Ride, dt: f32) {
    let door_open = ride.ascending && ride.docked();
    let wall = HALF_WIDTH_M - DECK_MARGIN_M;
    if ride.deck {
        // Deck collision, one axis at a time, so a wall slides instead of stopping.
        let was = ride.local_m - ride.local_vel * dt;
        let mut p = glam::Vec2::new(ride.local_m.x, was.z);
        if !deck_walkable(p, ride.hub_m) { p.x = was.x; ride.local_vel.x = 0.0; }
        p.y = ride.local_m.z;
        if !deck_walkable(p, ride.hub_m) { p.y = was.z; ride.local_vel.z = 0.0; }
        ride.local_m.x = p.x;
        ride.local_m.z = p.y;
        // Back through the door into the cabin.
        if ride.local_m.x <= HALF_WIDTH_M
            && ride.local_m.z.abs() <= DOOR_HALF_M - DECK_MARGIN_M {
            ride.deck = false;
        }
    } else {
        let in_door = door_open && ride.local_m.z.abs() <= DOOR_HALF_M - DECK_MARGIN_M;
        let east = if in_door { HALF_WIDTH_M + DECK_MARGIN_M } else { wall };
        ride.local_m.x = ride.local_m.x.clamp(-wall, east);
        ride.local_m.z = ride.local_m.z.clamp(-wall, wall);
        if in_door && ride.local_m.x > DOOR_TRANSFER_X_M {
            ride.deck = true;
        }
    }
}

/// One fixed step of the ride. True while the cabin carries the passenger.
/// A docked return trip releases the passenger at the terminal.
pub fn step(world: &mut World, player: &mut Player, input: &Input, steer: bool) -> bool {
    if world.orbital.express.ride.is_none() {
        let feet = player.precise_pos * world.voxel_scale.meters() as f64;
        if let Some(h) = world.orbital.habitats.habitats.iter()
            .find(|h| h.ready && (feet.as_vec3() - h.origin.as_vec3()).length() < 100.0) {
            let berth = ground_berth(world, h.cell, h.berth.as_dvec3());
            world.orbital.express.ride = Some(Ride {
                ascending: true, ticks: (1000.0 / STEP_SECONDS) as u64,
                profile: Profile::express(h.origin.y as f64 - berth.y),
                berth_m: berth, local_m: Vec3::ZERO, local_vel: Vec3::ZERO,
                deck: true, hub_m: hub_half_m(220.0), sway_m: Vec3::ZERO,
            });
        }
    }
    let Some(mut ride) = world.orbital.express.ride else { return false };
    if ride.deck {
        let local = (player.precise_pos * world.voxel_scale.meters() as f64 - ride.cabin_m()).as_vec3();
        if (-HALF_WIDTH_M..DOOR_TRANSFER_X_M).contains(&local.x)
            && local.z.abs() < DOOR_HALF_M - DECK_MARGIN_M
            && (0.0..HEIGHT_M).contains(&local.y) {
            ride.deck = false;
            ride.local_m = local;
            ride.local_vel = (player.vel * world.voxel_scale.meters()).clamp_length_max(WALK_MPS * 1.5);
        } else {
            // Station movement uses the ordinary voxel capsule, not deck bounds.
            return false;
        }
    }
    let dt = STEP_SECONDS;
    if !ride.docked() {
        let mut next = ride;
        next.ticks += 1;
        let bow = passenger_guide_bow(world);
        next.sway_m = sway_m(bow, next.along_m(), next.profile.length_m);
        let revision = Some(world.revision());
        let start_clear = world.orbital.express.checked_revision == revision;
        if obstruction(world, ride.cabin_m(), next.cabin_m(), start_clear) {
            if !world.orbital.express.blocked { world.expedition.say(OBSTRUCTED); }
            world.orbital.express.blocked = true;
        } else {
            world.orbital.express.blocked = false;
            world.orbital.express.checked_revision = revision;
            ride = next;
            if ride.docked() {
                if ride.ascending { world.goals.note(crate::world::goals::Event::CrownReached); }
                let line = if ride.ascending {
                    format!("Crown anchorage · Docked · {:.1} km", ride.telemetry().altitude_m / 1000.0)
                } else {
                    "Terminal · Arrived".to_string()
                };
                world.expedition.say_for(line, 6.0);
            }
        }
    }
    let cabin_g = if world.orbital.express.blocked {
        1.0
    } else {
        engine_space::express::cabin_gravity_g(&ride.profile, &ride.profile.sample(ride.trip_s()))
    };
    let field = (cabin_g * G0) as f32;
    // Cabin-local walk: flat floor, four walls, the compensated field as gravity.
    let heading = if steer { crate::world::travel::heading(input, player.yaw) } else { Vec3::ZERO };
    let on_floor = ride.local_m.y <= 0.0;
    let target = heading * if input.sprint { WALK_MPS * 1.5 } else { WALK_MPS };
    let response = if on_floor { 12.0 } else { 2.0 };
    ride.local_vel.x += (target.x - ride.local_vel.x) * (response * dt).min(1.0);
    ride.local_vel.z += (target.z - ride.local_vel.z) * (response * dt).min(1.0);
    if on_floor && steer && input.jump {
        ride.local_vel.y = JUMP_MPS;
    }
    ride.local_vel.y -= field * dt;
    ride.local_m += ride.local_vel * dt;
    walls(&mut ride, dt);
    if ride.docked() && !port_ready(world, &ride) {
        ride.deck = false;
        ride.local_m.x = ride.local_m.x.min(HALF_WIDTH_M - DECK_MARGIN_M);
    }
    if ride.local_m.y <= 0.0 {
        ride.local_m.y = 0.0;
        ride.local_vel.y = 0.0;
    }
    let ceiling = if ride.deck { DECK_HEIGHT_M - 1.9 } else { HEIGHT_M - 1.9 };
    if ride.local_m.y > ceiling {
        ride.local_m.y = ceiling;
        ride.local_vel.y = ride.local_vel.y.min(0.0);
    }
    let scale = world.voxel_scale.meters() as f64;
    let feet = ride.cabin_m() + ride.local_m.as_dvec3();
    player.precise_pos = feet / scale;
    player.pos = player.precise_pos.as_vec3();
    player.vel = if ride.deck { ride.local_vel / scale as f32 } else { Vec3::ZERO };
    player.on_ground = ride.local_m.y <= 0.0;
    player.flying = false;
    player.swimming = false;
    player.underwater = false;
    player.gravity_frame = glam::Quat::IDENTITY;
    let released = !ride.ascending && ride.docked();
    if released {
        world.orbital.express = State::default();
    } else {
        world.orbital.express.ride = Some(ride);
    }
    !released
}

pub fn port_ready(world: &World, ride: &Ride) -> bool {
    let crown = (ride.berth_m + DVec3::Y * ride.profile.length_m).as_vec3();
    world.orbital.habitats.habitats.iter().any(|h| h.ready
        && h.origin.as_vec3().distance(crown) < 1.0
        && !h.gas.is_empty() && h.gas.iter().all(|q| q[0] >= 0.0))
}

pub fn passenger_guide_bow(world: &World) -> Vec3 {
    world.orbital.dynamics.elevator_bow().clamp_length_max(2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ride(ascending: bool, trip_s: f64) -> Ride {
        Ride {
            ascending,
            ticks: (trip_s / STEP_SECONDS as f64).round() as u64,
            profile: Profile::express(18_000.0),
            berth_m: DVec3::new(10.0, 100.0, -20.0),
            local_m: Vec3::ZERO,
            local_vel: Vec3::ZERO,
            deck: false,
            hub_m: hub_half_m(220.0),
            sway_m: Vec3::ZERO,
        }
    }

    #[test]
    fn the_cabin_follows_the_braid_bow_and_rests_on_the_axis_at_both_ends() {
        let bow = Vec3::new(40.0, 7.0, -10.0);
        assert_eq!(sway_m(bow, 0.0, 18_000.0), Vec3::ZERO);
        assert!(sway_m(bow, 18_000.0, 18_000.0).length() < 1e-3);
        let mid = sway_m(bow, 9_000.0, 18_000.0);
        assert!((mid.x - 40.0).abs() < 1e-3 && mid.y == 0.0 && (mid.z + 10.0).abs() < 1e-3, "{mid:?}");
        // A shorter span bows by the square of its length share.
        assert!((sway_m(bow, 4_500.0, 9_000.0).x - 10.0).abs() < 1e-3);
    }

    #[test]
    fn the_deck_surrounds_the_shaft_and_runs_out_along_the_gallery() {
        let hub = hub_half_m(220.0);
        assert!((hub - 18.0).abs() < 1e-6);
        assert!((hub_half_m(38.0) - (HALF_WIDTH_M + 4.0)).abs() < 1e-6);
        let v = glam::Vec2::new;
        assert!(!deck_walkable(v(0.0, 0.0), hub), "shaft");
        assert!(deck_walkable(v(6.0, 6.0), hub));
        assert!(deck_walkable(v(-17.0, 17.0), hub));
        assert!(!deck_walkable(v(0.0, 19.0), hub), "outside the concourse");
        assert!(deck_walkable(v(hub + GALLERY_LEN_M - 1.0, 0.0), hub), "gallery end");
        assert!(!deck_walkable(v(hub + GALLERY_LEN_M + 1.0, 0.0), hub));
        assert!(!deck_walkable(v(hub + 5.0, GALLERY_HALF_M + 0.5), hub), "gallery side");
    }

    #[test]
    fn the_door_only_opens_docked_at_the_crown() {
        let push = Vec3::new(HALF_WIDTH_M + 0.3, 0.0, 0.0);
        let mut moving = ride(true, 60.0);
        moving.local_m = push;
        walls(&mut moving, STEP_SECONDS);
        assert!(!moving.deck && moving.local_m.x <= HALF_WIDTH_M - DECK_MARGIN_M);
        let mut docked = ride(true, 125.0);
        docked.local_m = push;
        walls(&mut docked, STEP_SECONDS);
        assert!(docked.deck, "through the open door");
        // Along the gallery, held by its side wall, then back through the door.
        docked.local_m = Vec3::new(docked.hub_m + 10.0, 0.0, GALLERY_HALF_M + 1.0);
        docked.local_vel = Vec3::new(0.0, 0.0, 2.0 / STEP_SECONDS);
        walls(&mut docked, STEP_SECONDS);
        assert!(docked.local_m.z.abs() < GALLERY_HALF_M && docked.local_vel.z == 0.0, "{:?}", docked.local_m);
        docked.local_m = Vec3::new(HALF_WIDTH_M - 0.1, 0.0, 0.0);
        docked.local_vel = Vec3::ZERO;
        walls(&mut docked, STEP_SECONDS);
        assert!(!docked.deck, "back aboard");
    }

    #[test]
    fn the_board_counts_down_to_the_next_light_change() {
        assert_eq!(next_light(0.0), (0.25, true));
        let (to_sunset, sunrise) = next_light(0.5);
        assert!((to_sunset - 0.25).abs() < 1e-9 && !sunrise);
        let (to_sunrise, sunrise) = next_light(0.9);
        assert!((to_sunrise - 0.35).abs() < 1e-9 && sunrise);
        // 0.3 g at 220 m: about 1.1 rpm, slow enough to walk against.
        assert!((rim_rpm(220.0) - 1.104).abs() < 0.01, "{}", rim_rpm(220.0));
    }

    #[test]
    fn docked_cabins_only_hum() {
        let mut orbital = Orbital::default();
        orbital.express.ride = Some(ride(true, 125.0));
        assert_eq!(drive_cue(&orbital), (0.06, 0.0));
    }

    #[test]
    fn the_cabin_climbs_the_guide_and_returns_down_it() {
        assert_eq!(ride(true, 0.0).cabin_m(), DVec3::new(10.0, 100.0, -20.0));
        let top = ride(true, 125.0).cabin_m();
        assert!((top.y - 18_100.0).abs() < 1e-6
            && (top.x - (10.0 - f64::from(DOCK_INSET_M))).abs() < 1e-6 && top.z == -20.0);
        assert!((ride(false, 0.0).cabin_m().y - 18_100.0).abs() < 1e-6);
        assert!((ride(false, 125.0).cabin_m().y - 100.0).abs() < 1e-6);
        let mid = ride(false, 60.0).cabin_m().y;
        // Ticks quantise the trip to 1/120 s; at 327 m/s that is under 3 m.
        assert!((mid - 9_100.0).abs() < 3.0, "{mid}");
    }

    #[test]
    fn residency_plans_for_the_terminal_only_on_final_approach() {
        let mut orbital = Orbital::default();
        assert_eq!(residency_altitude_m(&orbital, 9_000.0), 9_000.0);
        orbital.express.ride = Some(ride(true, 100.0));
        assert_eq!(residency_altitude_m(&orbital, 9_000.0), 9_000.0);
        orbital.express.ride = Some(ride(false, 60.0));
        assert_eq!(residency_altitude_m(&orbital, 9_000.0), 9_000.0);
        orbital.express.ride = Some(ride(false, 85.0));
        assert_eq!(residency_altitude_m(&orbital, 9_000.0), 0.0);
    }

    #[test]
    fn drive_cue_peaks_at_the_midpoint_and_flips_under_braking() {
        let mut orbital = Orbital::default();
        assert_eq!(drive_cue(&orbital), (0.0, 0.0));
        orbital.express.ride = Some(ride(true, 60.0));
        let (speed, load) = drive_cue(&orbital);
        assert!((speed - 1.0).abs() < 1e-5 && load.abs() < 1e-5, "{speed} {load}");
        orbital.express.ride = Some(ride(true, 85.0));
        assert!((drive_cue(&orbital).1 + 1.0).abs() < 1e-5);
    }
}
