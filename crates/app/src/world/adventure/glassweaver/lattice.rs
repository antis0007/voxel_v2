//! Lattice routing, node damage and bolt travel. Pure state; no rendering.
use super::*;

/// Entry at the intact node nearest `from`, exit at the reachable intact node
/// nearest `aim`; the path follows intact strands only. Pure and bounded.
pub(in crate::world::adventure) fn route(s: &State, from: Vec3, aim: Vec3) -> Option<([u8; MAX_NODES], u8)> {
    let n = s.count as usize;
    let nearest = |p: Vec3, ok: &dyn Fn(usize) -> bool| {
        (0..n)
            .filter(|&i| ok(i))
            .min_by(|&a, &b| s.nodes[a].pos.distance_squared(p).total_cmp(&s.nodes[b].pos.distance_squared(p)))
    };
    let entry = nearest(from, &|i| s.intact(i))?;
    let mut parent = [u8::MAX; MAX_NODES];
    let mut seen = [false; MAX_NODES];
    let mut queue = [0u8; MAX_NODES];
    let (mut head, mut tail) = (0, 1);
    queue[0] = entry as u8;
    seen[entry] = true;
    while head < tail {
        let a = queue[head] as usize;
        head += 1;
        for b in 0..n {
            if !seen[b] && s.strand(a, b) {
                seen[b] = true;
                parent[b] = a as u8;
                queue[tail] = b as u8;
                tail += 1;
            }
        }
    }
    let exit = nearest(aim, &|i| seen[i])?;
    let mut reversed = [0u8; MAX_NODES];
    let mut len = 0;
    let mut at = exit;
    loop {
        reversed[len] = at as u8;
        len += 1;
        if at == entry || len == MAX_NODES {
            break;
        }
        at = parent[at] as usize;
    }
    let mut path = [0u8; MAX_NODES];
    for i in 0..len {
        path[i] = reversed[len - 1 - i];
    }
    Some((path, len as u8))
}

/// Player shots that cross an intact node chip it and are consumed.
pub(in crate::world::adventure) fn strike_nodes(e: &mut Enemy, world: &mut World, shots: &mut [Projectile], dt: f32, sounds: &mut Vec<Sound>) {
    for shot in shots.iter_mut() {
        if shot.hostile || shot.farm.is_some() || shot.life <= 0.0 {
            continue;
        }
        let a = shot.pos - shot.velocity * dt;
        for i in 0..e.glass.count as usize {
            if !e.glass.intact(i) {
                continue;
            }
            let c = e.glass.nodes[i].pos;
            let ab = shot.pos - a;
            let t = ((c - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            if (a + ab * t).distance(c) > NODE_RADIUS {
                continue;
            }
            shot.life = 0.0;
            e.glass.nodes[i].health -= 12.0 * shot.mods.damage_mul.max(0.5);
            if e.glass.nodes[i].health <= 0.0 {
                e.glass.nodes[i].health = 0.0;
                e.glass.breaks += 1;
                sound(sounds, Cue::Shatter, Some(c), 0.9);
                world.expedition.say("A prism shatters; its strands go dark.");
            } else {
                sound(sounds, Cue::Clang, Some(c), 0.5);
            }
            break;
        }
    }
}

fn discharge(e: &mut Enemy, bolt: Bolt, shots: &mut Vec<Projectile>, sounds: &mut Vec<Sound>) {
    let from = e.glass.nodes[bolt.path[bolt.len as usize - 1] as usize].pos;
    let aim = (bolt.aim - from).normalize_or_zero();
    let fan: &[f32] = match bolt.payload {
        Payload::Shard => &[0.0],
        Payload::Fan => &[-0.2, 0.0, 0.2],
    };
    for &a in fan {
        if shots.len() > 90 {
            break;
        }
        shots.push(Projectile {
            farm: None,
            pos: from,
            velocity: Quat::from_rotation_y(a) * aim * 11.0,
            hostile: true,
            payload: None,
            payload_mods: Default::default(),
            spell: 0,
            life: 3.0,
            mods: wands::ShotMods { damage_mul: 1.3, ..Default::default() },
            bore_budget: 0,
            trail_timer: 0.0,
            splashed: false,
            follow: None,
        });
    }
    e.glass.fired += 1;
    sound(sounds, Cue::ArchiveRelease, Some(from), 0.85);
}

/// Advance a bolt along its strands. A strand cut under it ends the bolt there.
pub(in crate::world::adventure) fn carry(e: &mut Enemy, dt: f32, shots: &mut Vec<Projectile>, sounds: &mut Vec<Sound>) {
    let Some(mut bolt) = e.glass.bolt.take() else {
        return;
    };
    let speed = BOLT_SPEED * [1.0, 0.9, 1.25][boss_look::index(e)];
    let mut left = dt;
    while left > 0.0 {
        let here = bolt.path[bolt.at as usize] as usize;
        if !e.glass.intact(here) {
            sound(sounds, Cue::Shatter, Some(e.glass.nodes[here].pos), 0.4);
            return;
        }
        if bolt.at + 1 >= bolt.len {
            discharge(e, bolt, shots, sounds);
            return;
        }
        let next = bolt.path[bolt.at as usize + 1] as usize;
        if !e.glass.strand(here, next) {
            sound(sounds, Cue::Shatter, Some(e.glass.nodes[here].pos), 0.4);
            return;
        }
        let length = e.glass.nodes[here].pos.distance(e.glass.nodes[next].pos).max(0.1);
        let step = left * speed / length;
        if bolt.t + step < 1.0 {
            bolt.t += step;
            break;
        }
        left -= (1.0 - bolt.t) * length / speed;
        bolt.t = 0.0;
        bolt.at += 1;
    }
    e.glass.bolt = Some(bolt);
}
