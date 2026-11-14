//! Gravity-relative capsule motion for the finite dry Manyfold district.
//! Velocity remains in world coordinates, including during airborne alignment.
use super::{Input, Player};
use crate::world::{World, player_height_blocks, player_width_blocks};
use glam::{IVec3, Quat, Vec3};

/// Exact minimum squared distance between a segment and an axis-aligned box.
/// Slab crossings partition the convex distance into quadratic intervals.
fn segment_box(a: Vec3, b: Vec3, lo: Vec3, hi: Vec3) -> f32 {
    let d = b - a;
    let mut cuts = [0.0f32; 8];
    cuts[1] = 1.0;
    let mut count = 2;
    for axis in 0..3 {
        if d[axis].abs() < 1e-8 { continue; }
        for edge in [lo[axis], hi[axis]] {
            let t = (edge - a[axis]) / d[axis];
            if t > 0.0 && t < 1.0 { cuts[count] = t; count += 1; }
        }
    }
    cuts[..count].sort_by(f32::total_cmp);
    let distance = |t: f32| { let p = a + d * t; (p - p.clamp(lo, hi)).length_squared() };
    let mut best = distance(0.0).min(distance(1.0));
    for pair in cuts[..count].windows(2) {
        let mid = a + d * ((pair[0] + pair[1]) * 0.5);
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        for axis in 0..3 {
            let edge = if mid[axis] < lo[axis] { lo[axis] }
                else if mid[axis] > hi[axis] { hi[axis] } else { continue; };
            numerator += d[axis] * (a[axis] - edge);
            denominator += d[axis] * d[axis];
        }
        let t = if denominator > 0.0 { (-numerator / denominator).clamp(pair[0], pair[1]) } else { pair[0] };
        best = best.min(distance(t)).min(distance(pair[1]));
    }
    best
}

fn blocked(world: &World, player: &Player, pos: Vec3, up: Vec3, crouch: f32, margin: f32) -> bool {
    blocked_path(world, player, pos, up, pos, up, crouch, margin)
}

fn blocked_path(world: &World, player: &Player, pos: Vec3, up: Vec3,
    end: Vec3, end_up: Vec3, crouch: f32, margin: f32) -> bool {
    let curve = (world.world_type == crate::world::WorldType::Manyfold)
        .then(|| engine_worldgen::manyfold::shell_curve(world.generator_version)).flatten();
    let radius = player_width_blocks(world.voxel_scale, player.scale) * 0.5;
    let height = player_height_blocks(world.voxel_scale, player.scale)
        - 0.3 * player.scale * crouch / world.voxel_scale.meters();
    let a = pos + up * radius;
    let b = pos + up * (height - radius).max(radius);
    let da = end + end_up * radius - a;
    let db = end + end_up * (height - radius).max(radius) - b;
    let r = radius + margin;
    let lo = (a.min(b).min(a + da).min(b + db) - Vec3::splat(r)).floor().as_ivec3();
    let hi = (a.max(b).max(a + da).max(b + db) + Vec3::splat(r)).floor().as_ivec3();
    for z in lo.z..=hi.z { for y in lo.y..=hi.y { for x in lo.x..=hi.x {
        let cell = IVec3::new(x, y, z);
        if !world.solid(cell) && !world.ca_reservations.contains(&cell) { continue; }
        if world.solid(cell)
            && let Some(curve) = curve
            && let Some(blocks) = shell_blocks(curve, world.voxel_scale.meters(), pos, end, cell)
        {
            if blocks { return true; }
            if !world.ca_reservations.contains(&cell) { continue; }
        }
        let hit = |lo, hi| {
            let distance = |t| segment_box(a + da * t, b + db * t, lo, hi);
            let ends = distance(0.0).min(distance(1.0));
            if ends < r * r { return true; }
            if da.length_squared() + db.length_squared() < 1e-12 { return false; }
            // The translated segment and the fixed-pivot rotational chord each
            // sweep a convex shape. Minimize distance along that short sweep.
            let (mut left, mut right) = (0.0, 1.0);
            for _ in 0..20 {
                let a = (2.0 * left + right) / 3.0;
                let b = (left + 2.0 * right) / 3.0;
                if distance(a) < distance(b) { right = b; } else { left = a; }
            }
            distance((left + right) * 0.5) < r * r
        };
        let block = world.get(cell);
        if world.ca_reservations.contains(&cell) {
            if hit(cell.as_vec3(), cell.as_vec3() + Vec3::ONE) { return true; }
        } else if engine_content::fittings::state(block).is_some() {
            if engine_content::fittings::boxes(block, cell, world.voxel_scale.meters()).iter().any(|&(a,b)| hit(a,b)) { return true; }
        } else if engine_content::shaped::is_shaped(block) {
            let links = engine_content::shaped::links(block, cell, |p| world.get(p));
            if engine_content::shaped::boxes(block, cell, links).iter().any(|&(a,b)| hit(a,b)) { return true; }
        } else if let Some(boxes) = crate::world::factory::conduit_bounds(world, cell) {
            if boxes.iter().any(|&(a,b)| hit(a,b)) { return true; }
        } else if hit(cell.as_vec3(), cell.as_vec3() + Vec3::ONE) { return true; }
    } } }
    false
}

/// Shell crust blocks at the cylinder. Its stair voxel reaches into the hollow;
/// treating that box as solid walks the body back off the floor.
fn shell_blocks(
    curve: engine_worldgen::manyfold::ShellCurve,
    scale: f32,
    pos: Vec3,
    end: Vec3,
    cell: IVec3,
) -> Option<bool> {
    let centre = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
    let offset = curve.offset(centre);
    if !(offset > 0.0 && offset < curve.crust + 0.75) {
        return None;
    }
    let outside = |feet: Vec3| {
        let m = feet * scale;
        let rel = m - curve.center;
        let radial = Vec3::new(rel.x, rel.y, 0.0);
        let r = radial.length();
        if r < 0.2 {
            return false;
        }
        if r <= curve.radius(rel.z) - 0.02 {
            return false;
        }
        let dir = radial / r;
        let origin = Vec3::new(curve.center.x, curve.center.y, m.z);
        let lo = cell.as_vec3() * scale;
        ray_aabb(origin, dir, r + curve.crust + 2.0, lo, lo + Vec3::splat(scale))
    };
    Some(outside(pos) || outside(end))
}

fn ray_aabb(origin: Vec3, dir: Vec3, max_t: f32, lo: Vec3, hi: Vec3) -> bool {
    let mut t0: f32 = 0.0;
    let mut t1 = max_t;
    for axis in 0..3 {
        if dir[axis].abs() < 1e-8 {
            if origin[axis] < lo[axis] || origin[axis] > hi[axis] {
                return false;
            }
            continue;
        }
        let inv = 1.0 / dir[axis];
        let (a, b) = {
            let a = (lo[axis] - origin[axis]) * inv;
            let b = (hi[axis] - origin[axis]) * inv;
            if a > b { (b, a) } else { (a, b) }
        };
        t0 = t0.max(a);
        t1 = t1.min(b);
        if t0 > t1 {
            return false;
        }
    }
    t1 >= 0.0 && t0 <= max_t
}

/// Sweep the capsule through nearby voxel and fitting boxes, then stop just
/// before contact. Substeps bound the numerical minimization and work budget.
fn sweep(world: &World, p: &Player, start: Vec3, delta: Vec3, up: Vec3) -> (Vec3, bool) {
    let pieces = (delta.length() / 0.15).ceil().max(1.0) as usize;
    let step = delta / pieces as f32;
    let mut pos = start;
    for _ in 0..pieces {
        let clear = |fraction: f32| !blocked_path(world, p, pos, up, pos + step * fraction,
            up, p.crouch_t, 0.0);
        if !clear(1.0) {
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..10 { let t = (lo + hi) * 0.5; if clear(t) { lo = t; } else { hi = t; } }
            return (pos + step * (lo - 0.002 / step.length().max(0.002)).max(0.0), true);
        }
        pos += step;
    }
    (pos, false)
}

fn unstick(world: &World, p: &mut Player, acceleration: Vec3, scale: f32) {
    if p.flying && p.fly_mode == super::FlyMode::Noclip {
        return;
    }
    if !blocked(world, p, p.pos, p.up_dir(), p.crouch_t, 0.0) {
        return;
    }
    let out = (-acceleration).try_normalize().unwrap_or(p.up_dir());
    for step in 1..=32 {
        let candidate = p.pos + out * (step as f32 * 0.125 / scale);
        if !blocked(world, p, candidate, p.up_dir(), p.crouch_t, 0.0) {
            p.pos = candidate;
            break;
        }
    }
}

fn align(world: &World, p: &mut Player, desired: Vec3) {
    let old = p.up_dir();
    let rotation = Quat::from_rotation_arc(old, desired);
    if p.flying && p.fly_mode == super::FlyMode::Noclip {
        p.gravity_frame = (rotation * p.gravity_frame).normalize();
        return;
    }
    let angle = old.dot(desired).clamp(-1.0, 1.0).acos();
    if angle < 1e-5 { return; }
    let height = player_height_blocks(world.voxel_scale, p.scale);
    let radius = player_width_blocks(world.voxel_scale, p.scale) * 0.5;
    let pivot = p.pos + old * radius;
    let pieces = (angle / 0.02).ceil().max(1.0) as usize;
    // Rotate about the lower cap centre. The swept chord plus its sagitta
    // covers the arc; a small contact skin avoids floor-locking this sweep.
    let mut accepted = Quat::IDENTITY;
    for i in 0..pieces {
        let next = Quat::IDENTITY.slerp(rotation, (i + 1) as f32 / pieces as f32);
        let from_up = accepted * old;
        let to_up = next * old;
        let padding = height * (1.0 - (angle / (2.0 * pieces as f32)).cos());
        if blocked_path(world, p, pivot - from_up * radius, from_up,
            pivot - to_up * radius, to_up, p.crouch_t, padding) { break; }
        accepted = next;
    }
    p.gravity_frame = (accepted * p.gravity_frame).normalize();
    p.pos = pivot - p.up_dir() * radius;
}

pub(super) fn update(p: &mut Player, input: &mut Input, world: &World, dt: f32) {
    let fold_start = p.pos;
    p.manyfold_feedback_cooldown = (p.manyfold_feedback_cooldown - dt).max(0.0);
    let scale = world.voxel_scale.meters();
    let jump = input.jump && !p.jump_was_down;
    if world.rules.mode.flight() { super::update_flight_toggle(p, input, dt); }
    let acceleration = crate::world::orbital::habitat::acceleration(world, p.pos * scale)
        .unwrap_or_else(|| engine_worldgen::manyfold::acceleration_for(world.generator_version, p.pos * scale)) / scale;
    unstick(world, p, acceleration, scale);
    if acceleration.length_squared() > 1e-8 { align(world, p, -acceleration.normalize()); }
    unstick(world, p, acceleration, scale);
    let up = p.up_dir();
    let grounded = blocked(world, p, p.pos - up * (0.06 / scale), up, p.crouch_t, 0.0);
    p.movement.sneak_was_down = input.sneak;
    let crouch = if input.sneak { 1.0 } else { 0.0 };
    if crouch > p.crouch_t || !blocked(world, p, p.pos, up, crouch, 0.0) { p.crouch_t = crouch; }
    p.jump_was_down = input.jump;
    p.swimming = false;
    p.underwater = false;
    p.step_camera_offset = 0.0;
    p.walk_bob_meters = 0.0;
    if super::traversal::octo_velocity(p, input, world, dt, IVec3::ZERO, jump) {
        let right = p.right_dir();
        let ahead = p.flat_forward();
        for axis in [right, ahead, up] {
            let (next, _) = sweep(world, p, p.pos, axis * p.vel.dot(axis) * dt, up);
            p.pos = next;
        }
        p.on_ground = false;
        p.precise_pos = p.pos.as_dvec3();
        crate::manyfold::weave::step(world, p, fold_start);
        crate::manyfold::maze_portal::step(world, p, fold_start);
        return;
    }
    let mut wish = p.flat_forward() * (i32::from(input.forward) - i32::from(input.back)) as f32
        + p.right_dir() * (i32::from(input.right) - i32::from(input.left)) as f32;
    wish = wish.normalize_or_zero();
    let speed = if input.sneak { 2.0 } else if input.sprint { 8.5 } else { 5.5 } / scale;
    let normal_speed = p.vel.dot(up);
    if !p.glide.available || grounded || p.flying {
        p.glide.deployed=false;
    } else if jump && (p.glide.deployed || normal_speed < 0.0) {
        p.glide.deployed=!p.glide.deployed;
    }
    let tangent = p.vel - up * normal_speed;
    // Air control adds bounded acceleration; alignment never rotates momentum.
    let difference = wish * speed - tangent;
    let limit = if grounded { 55.0 } else { 8.0 } * dt / scale;
    if !p.glide.deployed {
        // Slime launches keep their speed in the air; steering changes only
        // the unclaimed wish speed, rather than braking toward walking pace.
        let carry = !grounded && (p.traversal.slime_boots || p.traversal.sling);
        let steering = if carry {
            wish * (speed - tangent.dot(wish)).max(0.0)
        } else { difference };
        p.vel += steering.clamp_length_max(limit);
        if carry {
            let normal=up*p.vel.dot(up);
            p.vel=normal+(p.vel-normal).clamp_length_max(super::movement::MOMENTUM_SPEED_CAP/scale);
        }
    }
    if grounded && normal_speed < 0.0 { p.vel -= up * normal_speed; }
    if jump && grounded { p.vel += up * (8.4 / scale); }
    if p.glide.deployed {
        let frame=p.gravity_frame;
        p.vel=frame*super::glide::step(frame.inverse()*p.vel*scale,
            frame.inverse()*super::glide::glide_air(p.glide.wind),p.yaw,p.pitch,acceleration.length()*scale,dt)/scale;
        p.movement.fall_speed=(-p.vel.dot(up)*scale).max(0.0);
    } else { p.vel += acceleration * dt; }
    if !p.flying { super::traversal::propel(p, input, world, dt); }
    if p.flying {
        let vertical = (i32::from(input.jump) - i32::from(input.sneak)) as f32;
        if world.rules.mode.creative_tools() && world.world_type == crate::world::WorldType::Planet {
            super::creative_flight::migrate(&mut p.flight);
            let forward = if p.flight.camera_direction || p.fly_mode == super::FlyMode::Noclip {
                p.forward_dir()
            } else { p.flat_forward() };
            let wish = forward * (i32::from(input.forward) - i32::from(input.back)) as f32
                + p.right_dir() * (i32::from(input.right) - i32::from(input.left)) as f32 + up * vertical;
            // Inside a habitat, cruise height is local to its floor.
            p.vel = wish.normalize_or_zero() * super::creative_flight::speed(&p.flight, input, 0.0) / scale;
        } else {
            p.vel = (wish + up * vertical).normalize_or_zero() * p.flight.speed / scale;
        }
        if p.fly_mode == super::FlyMode::Noclip {
            p.pos += p.vel * dt;
            p.precise_pos = p.pos.as_dvec3();
            p.on_ground = false;
            return;
        }
    }
    let vertical = up * p.vel.dot(up);
    let was_on_ground = p.on_ground;
    if !p.flying {
        p.movement.fall_speed = p.movement.fall_speed.max((-p.vel.dot(up) * scale).max(0.0));
    }
    let sideways = (p.vel - vertical) * dt;
    let start = p.pos;
    // One swept vector stops on the first voxel corner of a curved floor.
    // The two tangent axes slide, so a lip only blocks the axis that hits it.
    let right = p.right_dir();
    let ahead = p.flat_forward();
    let (slid, hit_right) = sweep(world, p, start, right * sideways.dot(right), up);
    let (mut next, hit_ahead) = sweep(world, p, slid, ahead * sideways.dot(ahead), up);
    let hit = hit_right || hit_ahead;
    if hit && grounded && !jump {
        // One body step. A taller lift hits the low corridor ceiling and cancels the step.
        let step = crate::world::player_step_height_blocks(world.voxel_scale, p.scale);
        let lift = up * (step + 0.05);
        let (raised, ceiling) = sweep(world, p, start, lift, up);
        if !ceiling {
            let (across_r, _) = sweep(world, p, raised, right * sideways.dot(right), up);
            let (across, _) = sweep(world, p, across_r, ahead * sideways.dot(ahead), up);
            if (across - raised).length() > 0.02 {
                let (land, supported) = sweep(world, p, across, -lift - up * (0.08 / scale), up);
                if supported { next = land; }
            }
        }
    }
    if !input.sneak || !grounded || blocked(world, p, next - up * (0.15 / scale), up, p.crouch_t, 0.0) { p.pos = next; }
    let (next, contact) = sweep(world, p, p.pos, vertical * dt, up);
    p.pos = next;
    p.on_ground = contact && vertical.dot(up) <= 0.0;
    if contact { p.vel -= vertical; }
    if p.on_ground && !was_on_ground {
        if p.movement.fall_speed >= super::movement::HARD_LANDING_SPEED {
            p.movement.raise(super::movement::EVENT_LAND);
        }
        if !world.adventure.creative && !p.flying && !p.traversal.slime_boots {
            p.harm(super::hazards::fall_damage(p.movement.fall_speed), crate::world::adventure::death::Cause::Fall);
        }
        super::traversal::bounce(p, input, world);
        p.movement.fall_speed = 0.0;
    }
    if p.on_ground || p.flying { p.movement.fall_speed = 0.0; p.glide.deployed=false; }
    if p.on_ground { p.walk_phase = (p.walk_phase + sideways.length() * scale * 2.3).rem_euclid(std::f32::consts::TAU); }
    p.precise_pos = p.pos.as_dvec3();
    crate::manyfold::weave::step(world, p, fold_start);
    crate::manyfold::maze_portal::step(world, p, fold_start);
}

pub(crate) fn clear_landing(world: &World, p: &Player, at: Vec3, up: Vec3) -> bool {
    !blocked(world, p, at, up, 0.0, 0.0)
}

/// Capsule clearance on the destination side of a fold, with a small contact
/// skin so rotating a resting body does not mistake its floor for an obstacle.
pub(crate) fn clear_transit(world: &World, p: &Player, from: Vec3, to: Vec3, up: Vec3) -> bool {
    let height = player_height_blocks(world.voxel_scale, p.scale);
    let radius = Vec3::splat(player_width_blocks(world.voxel_scale, p.scale) * 0.5);
    let lo = from.min(to).min(from + up * height).min(to + up * height) - radius;
    let hi = from.max(to).max(from + up * height).max(to + up * height) + radius;
    let lo = crate::world::chunk_coord_from_world_block(lo.floor().as_ivec3());
    let hi = crate::world::chunk_coord_from_world_block(hi.floor().as_ivec3());
    for z in lo.z..=hi.z { for y in lo.y..=hi.y { for x in lo.x..=hi.x {
        let chunk = IVec3::new(x, y, z);
        if !world.chunks.contains_key(&chunk) && world.regions.view.records.contains_key(&chunk) {
            return false;
        }
    } } }
    let skin = (0.02 / world.voxel_scale.meters())
        .min(player_width_blocks(world.voxel_scale, p.scale) * 0.1);
    !blocked_path(world, p, from, up, to, up, p.crouch_t, -skin)
}

/// Host admission uses the same environment and shape as the walking client.
pub(crate) fn admit_move(world: &World, p: &Player, to: Vec3) -> bool {
    let steps = ((to - p.pos).length() / 0.2).ceil().clamp(1.0, 256.0) as u32;
    (1..=steps).all(|i| {
        let at = p.pos.lerp(to, i as f32 / steps as f32);
        let g = engine_worldgen::manyfold::acceleration_for(world.generator_version, at * world.voxel_scale.meters());
        let up = (-g).try_normalize().unwrap_or(p.up_dir());
        let skin = (0.1 / world.voxel_scale.meters())
            .min(player_width_blocks(world.voxel_scale, p.scale) * 0.25);
        !blocked(world, p, at, up, p.crouch_t, -skin)
    })
}
