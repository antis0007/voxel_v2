//! Ground handoff to a foreign body whose sphere of influence holds the player.
//!
//! The home atlas turns with home's spin, so another body sweeps through it at
//! hundreds of metres per second. From twice that body's sphere of influence
//! the player's state moves into the body's own frame (`Anchor`), and the atlas
//! position is derived from it every step. Across that band the reference
//! velocity and the upright blend from home's to the body's.
use engine_core::cube_sphere::atlas;
use engine_space::{BodyId, BodyKind, System};
use glam::{DQuat, DVec3, Quat};
#[path = "approach.rs"]
mod approach;
pub(crate) use approach::approached_body;
use approach::{APPROACH_BAND, CARRY_RAD, FORCE_RELEASE, RELEASE_ALIGN_RAD, TICK_SECONDS, aim, body_weight, view_dir};

/// Player state in a foreign body's rotating frame. Hell saves retain this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub body: BodyId,
    /// Metres from the body centre, body axes.
    pub pos: DVec3,
    /// Metres per second, body axes.
    pub vel: DVec3,
    /// Player upright frame in body axes; turns toward the radial up.
    pub upright: DQuat,
}

/// Rigid placement of home and one body at a tick.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    home_pos: DVec3,
    home_rot: DQuat,
    body_pos: DVec3,
    body_rot: DQuat,
}

impl Frame {
    pub fn for_world(world: &crate::world::World, body: BodyId, tick: u64) -> Option<Self> {
        if world.sample_config().is_hell() {
            let moon = super::hell_moons::moon(world.seed, body, tick)?;
            return Some(Self { home_pos: DVec3::ZERO, home_rot: DQuat::IDENTITY,
                body_pos: moon.centre, body_rot: DQuat::IDENTITY });
        }
        if world.world_type == crate::world::WorldType::Planet && !super::hell_moons::reserved(body) {
            return Self::at(body, tick);
        }
        None
    }
    pub fn at(body: BodyId, tick: u64) -> Option<Self> {
        let system = system()?;
        let home = system.id("home")?;
        let (home_pos, body_pos) = super::rails::with_snapshot(system, tick, |s| (s.position(home), s.position(body)));
        Some(Self {
            home_pos,
            home_rot: system.orientation(home, tick),
            body_pos,
            body_rot: system.orientation(body, tick),
        })
    }

    /// Body-axes metres of an atlas point.
    pub fn to_body(&self, atlas_m: DVec3) -> DVec3 {
        self.body_rot.inverse() * (self.home_pos + self.home_rot * atlas::to_body(atlas_m) - self.body_pos)
    }

    /// Atlas point of body-axes metres.
    pub fn to_atlas(&self, local: DVec3) -> DVec3 {
        atlas::from_body(self.home_rot.inverse() * (self.body_pos + self.body_rot * local - self.home_pos))
    }

    /// Rotation from body axes to the face-local axes at `atlas_m`.
    pub fn axes(&self, atlas_m: DVec3) -> DQuat {
        let face = DQuat::from_mat3(&atlas::frame(atlas_m).transpose());
        (face * self.home_rot.inverse() * self.body_rot).normalize()
    }
}

/// Upright turn rate toward the radial up, radians per second.
const TILT_RATE: f64 = 1.2;
const WALK_MPS: f64 = 4.5;
const JUMP_MPS: f64 = 3.0;
/// Downhill drop the feet follow without leaving the ground, metres.
const STEP_DOWN_M: f64 = 0.4;
/// The sphere is left only this far past its radius, so its edge cannot flicker.
const LEAVE_MARGIN: f64 = 1.05;
const DOUBLE_TAP_SECONDS: f32 = 0.3;
/// Whether an anchor is inside its body's sphere, where the body's ground is drawn.
pub fn within_sphere(world: &crate::world::World, anchor: &Anchor) -> bool {
    if world.sample_config().is_hell() {
        return true;
    }
    system().and_then(|s| s.soi(anchor.body)).is_some_and(|soi| anchor.pos.length() <= soi * LEAVE_MARGIN)
}

pub(crate) fn system() -> Option<&'static System> {
    static SYSTEM: std::sync::OnceLock<Option<System>> = std::sync::OnceLock::new();
    SYSTEM
        .get_or_init(|| {
            let file = engine_space::parse(include_str!("../../../../assets/star_systems.ron")).ok()?;
            System::compile(file.systems.first()?, 1.0 / 120.0).ok()
        })
        .as_ref()
}

/// Foreign body whose sphere of influence holds an atlas point; `None` for home and the star.
pub fn foreign_body(atlas_m: DVec3, tick: u64) -> Option<BodyId> {
    let system = system()?;
    let home = system.id("home")?;
    let body = super::rails::with_snapshot(system, tick, |snapshot| {
        let sample = snapshot.position(home) + system.orientation(home, tick) * atlas::to_body(atlas_m);
        system.dominant_body(snapshot, sample)
    });
    (body != home && system.parent(body).is_some()).then_some(body)
}

/// Surface radius in metres along a body-axes direction.
pub fn ground_radius(seed: u32, body: BodyId, dir: DVec3) -> f64 {
    if super::hell_moons::reserved(body) {
        let radius = body_radius(seed, body);
        return radius + engine_worldgen::moon::height(seed ^ u32::from(body.0), radius, dir);
    }
    let Some(system) = system() else { return 0.0 };
    let radius = system.radius(body);
    match system.kind(body) {
        BodyKind::Moon | BodyKind::Planet => radius + engine_worldgen::moon::height(seed ^ u32::from(body.0), radius, dir),
        _ => radius,
    }
}

pub(super) fn body_radius(seed: u32, body: BodyId) -> f64 {
    if super::hell_moons::reserved(body) {
        return super::hell_moons::moon(seed, body, 0).map_or(1.0, |m| m.radius);
    }
    system().map_or(1.0, |s| s.radius(body))
}

pub(crate) fn valid_hell_anchor(world: &crate::world::World, a: Anchor) -> bool {
    world.sample_config().is_hell()
        && super::hell_moons::moon(world.seed, a.body, 0).is_some_and(|m| {
            a.pos.is_finite() && a.pos.length() >= m.radius * 0.5
                && a.pos.length() <= m.influence() * LEAVE_MARGIN + 30.0
        })
        && a.vel.is_finite() && a.vel.length() <= 3_500.0
        && a.upright.is_finite() && (a.upright.length_squared() - 1.0).abs() < 0.001
}

pub(crate) fn restore_hell_anchor(world: &crate::world::World, player: &mut crate::player::Player, a: Anchor) {
    if let Some(frame) = Frame::for_world(world, a.body, world.orbital.planet_tick) {
        player.anchor = Some(a);
        place(player, &frame, a, a.vel, f64::from(world.voxel_scale.meters()));
    }
}

pub(crate) fn sync_hell_anchor(world: &crate::world::World, player: &mut crate::player::Player) {
    if let Some(a) = player.anchor
        && super::hell_moons::reserved(a.body)
        && player.precise_pos.as_vec3() == player.pos
    {
        restore_hell_anchor(world, player, a);
    }
}

fn in_space(player: &crate::player::Player, scale: f64) -> bool {
    player.precise_pos.y * scale > engine_voxel::WORLD_MAX_Y_METERS as f64 + 8.0
}

/// Moves an anchored player one step; true when it did, so the atlas step is skipped.
pub fn step(player: &mut crate::player::Player, input: &crate::player::Input, world: &crate::world::World, dt: f32) -> bool {
    let hell = world.sample_config().is_hell();
    if !hell && world.world_type != crate::world::WorldType::Planet {
        player.anchor = None;
        return false;
    }
    let scale = world.voxel_scale.meters() as f64;
    let tick = world.orbital.planet_tick;
    let band = if hell { 1.0 } else { APPROACH_BAND };
    let mut anchor = match player.anchor {
        Some(anchor) => anchor,
        None => {
            if !in_space(player, scale) {
                return false;
            }
            let point = player.precise_pos * scale;
            let body = if hell { super::hell_moons::nearby(world.seed, point, tick) } else { approached_body(point, tick) };
            let Some(body) = body else { return false };
            let Some(frame) = Frame::for_world(world, body, tick) else { return false };
            // Keep the view. Hell keeps the felt velocity. On a planet the
            // state converts exactly at the band edge, so nothing in view
            // changes speed; a teleport deeper in keeps its felt velocity.
            // The atlas point was reached last tick, so the conversion spans
            // the previous frame to this one.
            let to_body = frame.axes(point).inverse();
            let chart = player.vel.as_dvec3() * scale;
            let here = frame.to_body(point);
            let prev = tick.checked_sub(1).and_then(|t| Frame::for_world(world, body, t));
            let (pos, vel) = match (prev, system().and_then(|s| s.soi(body))) {
                (Some(prev), Some(soi)) if !hell => {
                    let sweep = (here - prev.to_body(point)) / TICK_SECONDS;
                    let w = body_weight(here.length(), soi, band);
                    let felt = (frame.to_body(point + chart * TICK_SECONDS) - here) / TICK_SECONDS;
                    (here - sweep * ((1.0 - w) * TICK_SECONDS), felt + sweep * (1.0 - w))
                }
                _ => (here, to_body * chart),
            };
            Anchor {
                body,
                pos,
                vel,
                upright: to_body * player.gravity_frame.as_dquat().normalize(),
            }
        }
    };
    let Some(frame) = Frame::for_world(world, anchor.body, tick) else { player.anchor = None; return false };
    let (soi, surface_g) = if hell {
        let Some(m) = super::hell_moons::moon(world.seed, anchor.body, tick) else { return false };
        (m.influence(), 1.6)
    } else {
        let Some(system) = system() else { return false };
        (system.soi(anchor.body).unwrap_or(0.0), system.surface_gravity(anchor.body))
    };
    let prev = if band > 1.0 { tick.checked_sub(1).and_then(|t| Frame::for_world(world, anchor.body, t)) } else { None };
    let distance = anchor.pos.length();
    let point = frame.to_atlas(anchor.pos);
    let home_up = frame.axes(point).inverse() * DVec3::Y;
    let current = anchor.upright * DVec3::Y;
    if distance > soi * band * LEAVE_MARGIN
        && (band <= 1.0 || distance > soi * FORCE_RELEASE || current.angle_between(home_up) < RELEASE_ALIGN_RAD)
    {
        release(player, &frame, prev.as_ref(), anchor, scale);
        return false;
    }
    let w = body_weight(distance, soi, band);
    // Body-axes velocity of the atlas point here: home's frame sweeping past.
    let sweep = prev.map_or(DVec3::ZERO, |prev| (anchor.pos - prev.to_body(point)) / TICK_SECONDS);
    let reference = sweep * (1.0 - w);
    let mut vel = anchor.vel - reference;
    let dt64 = f64::from(dt);
    let up = anchor.pos.normalize_or(DVec3::Y);
    // Mid-band the nearer up wins, so hovering there cannot flip-flop.
    let target = if w >= 0.6 || w > 0.4 && current.dot(up) >= current.dot(home_up) { up } else { home_up };
    let look = anchor.upright * view_dir(player);
    let (axis, angle) = DQuat::from_rotation_arc(current, target).to_axis_angle();
    let turn = angle.min(TILT_RATE * dt64);
    anchor.upright = (DQuat::from_axis_angle(axis, turn) * anchor.upright).normalize();
    if turn > CARRY_RAD {
        // Ground curvature carries the view; past that, a frame handoff turns
        // the horizon about a nearly fixed line of sight.
        aim(player, anchor.upright.inverse() * (DQuat::from_axis_angle(axis, CARRY_RAD) * look));
    }

    let radius = body_radius(world.seed, anchor.body).max(1.0);
    let ground = ground_radius(world.seed, anchor.body, up);
    let height = distance - ground;
    let g = surface_g * (radius / distance.max(radius)).powi(2) * w;
    let home_g = (1.0 - w) * 9.81 * (atlas::RADIUS_M / (atlas::RADIUS_M + point.y.max(0.0))).powi(2);

    flight_toggle(player, input, world, dt);
    let creative = world.rules.mode.creative_tools() || hell && world.rules.mode.flight();
    if creative { crate::player::creative_flight::migrate(&mut player.flight); }
    let (sy, cy) = (player.yaw as f64).sin_cos();
    let forward = if creative && (player.flight.camera_direction || matches!(player.fly_mode, crate::player::FlyMode::Noclip)) {
        let (sp, cp) = (player.pitch as f64).sin_cos();
        anchor.upright * DVec3::new(sy * cp, sp, -cy * cp)
    } else { anchor.upright * DVec3::new(sy, 0.0, -cy) };
    let right = anchor.upright * DVec3::new(cy, 0.0, sy);
    let axis = |plus: bool, minus: bool| f64::from(plus as i8 - minus as i8);
    let mut wish = forward * axis(input.forward, input.back) + right * axis(input.right, input.left);
    let grounded = height <= 0.05;
    if player.flying {
        wish += anchor.upright * DVec3::Y * axis(input.jump, input.sneak);
        let body_speed = if creative {
            f64::from(crate::player::creative_flight::speed(&player.flight, input, height as f32))
        } else {
            let boost = if input.sprint { f64::from(player.flight.boost.clamp(1.0, 4.0)) } else { 1.0 };
            // Far from the ground the same keys cross the whole sphere in seconds.
            let cruise = (1.0 + height.max(0.0) / 60.0).min(60.0);
            f64::from(player.flight.speed.clamp(4.0, 64.0)) * boost * cruise
        };
        // Home's cruise at the band edge, so crossing it keeps the speed.
        let home_speed = f64::from(crate::player::creative_flight::speed(&player.flight, input, point.y as f32));
        let speed = home_speed + (body_speed - home_speed) * w;
        if !creative { vel -= up * g * dt64; }
        let target = wish.normalize_or_zero() * speed;
        vel = vel.lerp(target, (f64::from(crate::world::FLY_ACCEL) * dt64).min(1.0));
        if creative {
            vel = crate::player::creative_flight::approach(vel.as_vec3(), up.as_vec3(),
                height as f32, &player.flight, input).as_dvec3();
        }
    } else {
        vel -= home_up * home_g * dt64;
        let radial = vel.dot(up);
        let tangent = vel - up * radial;
        let walk = wish.reject_from(up).normalize_or_zero() * WALK_MPS * if input.sprint { 1.8 } else { 1.0 };
        let tangent = tangent.lerp(walk, (if grounded { 12.0 } else { 0.6 } * dt64).min(1.0));
        let mut radial = radial - g * dt64;
        if grounded && input.jump {
            radial = JUMP_MPS;
        }
        vel = tangent + up * radial;
    }
    anchor.vel = vel + reference;
    anchor.pos += anchor.vel * dt64;
    let up = anchor.pos.normalize_or(DVec3::Y);
    let ground = ground_radius(world.seed, anchor.body, up);
    let height = anchor.pos.length() - ground;
    let follow = grounded && !player.flying && anchor.vel.dot(up) <= 0.0 && height < STEP_DOWN_M;
    player.on_ground = height <= 0.0 || follow;
    if player.on_ground {
        anchor.pos = up * ground;
        anchor.vel -= up * anchor.vel.dot(up).min(0.0);
        if player.flying && player.flight.land_on_descent && input.sneak {
            player.flying = false;
        }
    }
    player.anchor = Some(anchor);
    place(player, &frame, anchor, anchor.vel - reference, scale);
    true
}

/// Double-tap Jump toggles flight, as on home ground.
fn flight_toggle(player: &mut crate::player::Player, input: &crate::player::Input, world: &crate::world::World, dt: f32) {
    let pressed = input.jump && !player.jump_was_down;
    player.jump_was_down = input.jump;
    player.jump_tap_timer = (player.jump_tap_timer - dt).max(0.0);
    if !pressed {
        return;
    }
    if player.jump_tap_timer > 0.0 && world.rules.mode.flight() {
        player.flying = !player.flying;
        player.jump_tap_timer = 0.0;
    } else {
        player.jump_tap_timer = DOUBLE_TAP_SECONDS;
    }
}

/// Writes the atlas position, felt velocity and upright frame of an anchored player.
fn place(player: &mut crate::player::Player, frame: &Frame, anchor: Anchor, felt: DVec3, scale: f64) {
    let point = frame.to_atlas(anchor.pos);
    let axes = frame.axes(point);
    player.precise_pos = point / scale;
    player.pos = player.precise_pos.as_vec3();
    player.vel = (axes * felt / scale).as_vec3();
    player.gravity_frame = (axes * anchor.upright).as_quat().normalize();
}

/// Hands the player back to the home atlas, keeping where they look.
// On a planet the body state is last tick's, so it is placed through the
// previous frame and the atlas carries on from there.
fn release(player: &mut crate::player::Player, frame: &Frame, prev: Option<&Frame>, anchor: Anchor, scale: f64) {
    place(player, prev.unwrap_or(frame), anchor, anchor.vel, scale);
    let look = player.forward_dir();
    player.yaw = look.x.atan2(-look.z);
    player.pitch = look.y.clamp(-1.0, 1.0).asin();
    player.gravity_frame = Quat::IDENTITY;
    player.anchor = None;
    // The exact chart velocity, so the atlas carries on at the same physical speed.
    if let Some(prev) = prev {
        let point = prev.to_atlas(anchor.pos);
        let ahead = frame.to_atlas(anchor.pos + anchor.vel * TICK_SECONDS);
        if atlas::split(point).0 == atlas::split(ahead).0 {
            player.vel = ((ahead - point) / (TICK_SECONDS * scale)).as_vec3();
        }
    }
}

/// Atlas point in metres `distance` from the moon's centre on the side facing away from home.
pub fn moon_point(tick: u64, distance: f64) -> Option<DVec3> {
    let system = system()?;
    let moon = system.id("moon")?;
    let frame = Frame::at(moon, tick)?;
    let away = frame.body_rot.inverse() * (frame.body_pos - frame.home_pos).normalize();
    Some(frame.to_atlas(away * distance))
}

/// Atlas point `height` metres above the moon's ground on its side facing away from home.
pub fn moon_ground_point(tick: u64, seed: u32, height: f64) -> Option<DVec3> {
    let system = system()?;
    let moon = system.id("moon")?;
    let frame = Frame::at(moon, tick)?;
    let away = (frame.body_rot.inverse() * (frame.body_pos - frame.home_pos)).normalize();
    Some(frame.to_atlas(away * (ground_radius(seed, moon, away) + height)))
}

#[cfg(test)]
#[path = "influence_tests.rs"]
mod tests;
