//! Bounded presentation simulation. Contacts never decide combat damage.
use super::*;

pub(super) const REST: [Vec3; 4] = [
    Vec3::new(-0.8525, 0.02, -0.6525), Vec3::new(0.8525, 0.02, -0.6525),
    Vec3::new(-0.93, 0.02, 0.725), Vec3::new(0.93, 0.02, 0.725),
];

#[derive(Clone, Copy, Default)]
pub(super) struct Foot {
    pub at: Vec3,
    from: Vec3,
    target: Vec3,
    elapsed: f32,
    moving: bool,
}

#[derive(Clone, Default)]
pub(super) struct Motion {
    pub ready: bool,
    pub feet: [Foot; 4],
    last_body: Vec3,
    pub shards: Vec<Shard>,
}

#[derive(Clone)]
pub(super) struct Shard {
    pub at: Vec3,
    pub rotation: Quat,
    velocity: Vec3,
    spin: Vec3,
    pub life: f32,
    quiet: f32,
}

// Probe only a narrow neighbourhood: never plant a leg through an entire floor.
fn support(world: &World, p: Vec3) -> Option<Vec3> {
    let mut above = p + Vec3::Y * 0.6;
    for _ in 0..30 {
        let below = above - Vec3::Y * 0.05;
        if !factory::solid_point(world, above) && factory::solid_point(world, below) {
            return Some(above + Vec3::Y * 0.02);
        }
        above = below;
    }
    None
}

pub(super) fn fracture(e: &mut Enemy, at: Vec3, impulse: Vec3) {
    let shards = &mut e.glass.motion.shards;
    // Six bodies per break, at most 24; no unbounded debris accumulation.
    if shards.len() > 18 { shards.drain(..shards.len() - 18); }
    for i in 0..6 {
        let a = i as f32 * 2.399963;
        shards.push(Shard {
            at: at + Vec3::new(a.cos(), 0.0, a.sin()) * 0.12,
            rotation: Quat::from_rotation_y(a),
            velocity: Vec3::new(a.cos() * 1.8, 1.6 + i as f32 * 0.13, a.sin() * 1.8)
                + impulse.normalize_or_zero() * 1.2,
            spin: Vec3::new(2.0 + a.sin(), a.cos() * 3.0, 1.4),
            life: 8.0, quiet: 0.0,
        });
    }
}

pub(super) fn advance(e: &mut Enemy, world: &World, dt: f32, sounds: &mut Vec<Sound>) {
    if !dt.is_finite() || dt <= 0.0 { return; }
    let dt = dt.min(0.1);
    let k = boss_look::scale(e);
    let yaw = Quat::from_rotation_y(e.yaw);
    let motion = &mut e.glass.motion;
    let reset = !motion.ready || e.pos.distance_squared(motion.last_body) > 9.0;
    let velocity = if reset { Vec3::ZERO } else { (e.pos - motion.last_body) / dt };
    motion.last_body = e.pos;
    motion.ready = true;
    let pair = |i: usize| usize::from(i == 1 || i == 2);
    let mut swinging = motion.feet.iter().position(|f| f.moving).map(pair);
    for (i, foot) in motion.feet.iter_mut().enumerate() {
        let rest = e.pos + yaw * REST[i] * k;
        if reset {
            foot.at = support(world, rest).unwrap_or(rest);
            foot.moving = false;
            continue;
        }
        if foot.moving {
            foot.elapsed += dt;
            let t = (foot.elapsed / 0.22).min(1.0);
            foot.at = foot.from.lerp(foot.target, boss_look::ease(t))
                + Vec3::Y * (std::f32::consts::PI * t).sin() * 0.22 * k;
            if t >= 1.0 {
                foot.moving = false;
                if factory::solid_point(world, foot.at - Vec3::Y * 0.08) {
                    sound(sounds, Cue::Clang, Some(foot.at), 0.10);
                }
            }
        } else {
            let supported = factory::solid_point(world, foot.at - Vec3::Y * 0.08);
            if swinging.is_none_or(|active| active == pair(i)) && (foot.at.distance(rest) > 0.32 * k || !supported) {
                let lead = (velocity * 0.12).clamp_length_max(0.25);
                if let Some(target) = support(world, rest + lead) {
                    foot.from = foot.at;
                    foot.target = target;
                    foot.elapsed = 0.0;
                    foot.moving = true;
                    swinging = Some(pair(i));
                } else {
                    // Unsupported toes droop rather than snapping onto absent terrain.
                    foot.at = foot.at.lerp(rest - Vec3::Y * 0.25, 1.0 - (-12.0 * dt).exp());
                }
            }
        }
    }
    let mut impact_budget = 2;
    for shard in &mut motion.shards {
        shard.life -= dt;
        let supported = factory::solid_point(world, shard.at - Vec3::Y * 0.1);
        if shard.quiet > 0.3 && supported { continue; }
        if !supported { shard.quiet = 0.0; }
        // Conservative spherical collision proxy with bounded substeps. Visual
        // shards rotate independently; this is not a general convex-body solver.
        let steps = ((shard.velocity.length() * dt / 0.06).ceil() as usize).clamp(1, 24);
        let h = dt / steps as f32;
        for _ in 0..steps {
            shard.velocity.y -= 9.81 * h;
            for axis in 0..3 {
                let mut next = shard.at;
                next[axis] += shard.velocity[axis] * h;
                let mut edge = next;
                edge[axis] += shard.velocity[axis].signum() * 0.09;
                if factory::solid_point(world, edge) {
                    let speed = shard.velocity[axis].abs();
                    shard.velocity[axis] *= -0.28;
                    shard.velocity *= 0.76;
                    shard.spin *= 0.65;
                    if speed > 0.8 && impact_budget > 0 {
                        sound(sounds, Cue::Clang, Some(shard.at), (speed * 0.035).min(0.22));
                        impact_budget -= 1;
                    }
                } else { shard.at = next; }
            }
            shard.rotation = (Quat::from_scaled_axis(shard.spin * h) * shard.rotation).normalize();
        }
        if shard.velocity.length_squared() < 0.08 { shard.quiet += dt; }
        else { shard.quiet = 0.0; }
    }
    motion.shards.retain(|s| s.life > 0.0);
}
