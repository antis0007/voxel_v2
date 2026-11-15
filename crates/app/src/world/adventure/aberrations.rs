//! Deep encounters. Append-only kinds; existing site/slot records own victories.
use super::{Enemy, World, line};
use crate::{mesh::models::Model, player::Player};
use glam::{Quat, Vec3};

pub(super) const SPECTATOR: u32 = 26;
pub(super) const BEHOLDER: u32 = 27;
pub(super) const ROPER: u32 = 28;
pub(super) const CHOKER: u32 = 29;
/// Roper rig hung from a ceiling; spire points down at the floor.
pub(super) const STALACTITE: u32 = 30;
/// Roper rig standing on the floor, identical to a roper's body.
pub(super) const STALAGMITE: u32 = 31;
/// Roper rig grown as a pine on the surface; roots are its tendrils.
pub(super) const PINE_ROPER: u32 = 66;
/// Roper rig grown as a palm on the surface.
pub(super) const PALM_ROPER: u32 = 67;
pub(super) fn is_kind(kind: u32) -> bool { (SPECTATOR..=STALAGMITE).contains(&kind) || tree_roper(kind) }
/// Ropers, both spires and both trees: rooted, tendril-casting, one weak-point rule.
pub(super) fn roper_like(kind: u32) -> bool { matches!(kind, ROPER | STALACTITE | STALAGMITE) || tree_roper(kind) }
/// Tree ropers keep the disguise whole until they act: no eye, no gape.
pub(super) fn tree_roper(kind: u32) -> bool { matches!(kind, PINE_ROPER | PALM_ROPER) }
fn tendrils(kind: u32) -> bool { roper_like(kind) || kind == CHOKER }
/// Crown height above a stalactite's floor anchor; spawns need rock here.
const HANG_M: f32 = 2.7;

pub(super) fn at_depth(cfg: engine_worldgen::WorldSampleConfig, pos: Vec3, original: u32, variation: u32, slot: u32) -> u32 {
    if super::is_boss(original) { return original; }
    let roll = variation.wrapping_add(slot.wrapping_mul(13));
    if !(slot == 0 && roll.is_multiple_of(7)
        || roll.is_multiple_of(4)
        || roll.is_multiple_of(5)
        || roll.is_multiple_of(6))
    {
        return original;
    }
    let scale = cfg.voxel_meters();
    let surface = cfg.world_y(engine_worldgen::terrain_height(&cfg, (pos.x / scale).floor() as i32, (pos.z / scale).floor() as i32));
    let depth = surface - pos.y;
    // Real metres below local ground, independent of room id and voxel scale.
    if depth >= 150.0 && slot == 0 && roll.is_multiple_of(7) { BEHOLDER }
    else if depth >= 80.0 && roll.is_multiple_of(4) { ROPER }
    else if depth >= 45.0 && roll.is_multiple_of(5) { SPECTATOR }
    else if depth >= 30.0 && roll.is_multiple_of(6) { CHOKER }
    // New spires only take rolls the older families left as the original kind.
    else if depth >= 80.0 && roll.is_multiple_of(6) { STALACTITE }
    else if depth >= 80.0 && roll % 10 == 5 { STALAGMITE }
    else { original }
}

/// A stalactite needs rock overhead; without it the spawn stands as a stalagmite.
pub(super) fn orient(p: Vec3, kind: u32, scale: f32, solid: impl FnOnce(glam::IVec3) -> bool) -> u32 {
    if kind == STALACTITE && !solid(((p + Vec3::Y * HANG_M) / scale).floor().as_ivec3()) {
        STALAGMITE
    } else {
        kind
    }
}

fn reach(e: &Enemy, wind: f32) -> f32 {
    wind.max(super::enemy_presentation::strike(e) * 0.85)
        .max(if tendrils(e.kind) && e.encounter.exposed > 0.0 { 0.85 } else { 0.0 })
}

fn eye_body(e: &Enemy, time: f32) -> (Vec3, f32, u32) {
    let large = e.kind == BEHOLDER;
    let r = if large { 0.83 } else { 0.48 };
    (e.pos + Vec3::Y * (r + 0.2 + (time * 1.5 + e.slot as f32).sin() * 0.06), r, if large { 10 } else { 4 })
}

fn stalk(e: &Enemy, time: f32, wind: f32, i: u32) -> (Vec3, Vec3, Vec3) {
    let (c, r, count) = eye_body(e, time);
    let a = (i as f32 + 0.5) * std::f32::consts::TAU / count as f32;
    let root = c + Vec3::new(a.cos() * r * 0.7, a.sin().abs() * r * 0.6, a.sin() * r * 0.4);
    let bend = root + Vec3::new(a.cos() * r * 0.4, r * 0.5, a.sin() * r * 0.3);
    let tip = bend + Vec3::new((time * 1.3 + a).sin() * 0.1, r * (0.4 + wind * 0.25), -r * 0.25);
    (root, bend, tip)
}

fn tendril_end(e: &Enemy, wind: f32, i: u32) -> Vec3 {
    let a = i as f32 * std::f32::consts::TAU / 6.0;
    e.pos + Vec3::new(a.cos() * (1.1 + wind * 0.8), 0.2, a.sin() * (1.1 + wind * 0.8))
}

fn hand(e: &Enemy, wind: f32, side: f32) -> Vec3 {
    e.pos + Vec3::Y * (0.8 - wind * 0.18)
        + Vec3::new(side * (0.35 - wind * 0.2), -0.45 + wind * 0.5, -0.42 - wind * 0.6)
}

fn emitter(e: &Enemy, time: f32, central: bool) -> Vec3 {
    let wind = super::enemy_presentation::wind(e);
    let extension = reach(e, wind);
    let point = if matches!(e.kind, SPECTATOR | BEHOLDER) {
        let (c, r, count) = eye_body(e, time);
        if central { c - Vec3::Z * r * 1.04 }
        else { stalk(e, time, extension, e.attacks % count).2 - Vec3::Z * r * 0.2 }
    } else if roper_like(e.kind) { tendril_end(e, extension, 4) }
    else { hand(e, extension, if e.attacks % 2 == 0 { -1.0 } else { 1.0 }) };
    super::enemy_presentation::posed_point(e, point, time, wind)
}

pub(super) fn antimagic(enemies: &[Enemy], world: &World, eye: Vec3, time: f32) -> bool {
    enemies.iter().any(|e| {
        if e.kind != BEHOLDER { return false; }
        let origin = emitter(e, time, true);
        let delta = eye - origin;
        e.kind == BEHOLDER && e.health > 0.0 && e.stagger <= 0.0
            // Central eye closes while stalks cast, leaving a counterattack window.
            && e.tell <= 0.0 && e.recovery <= 0.0 && delta.length() < 12.0
            && delta.normalize_or_zero().dot(Vec3::new(-e.yaw.sin(), 0.0, -e.yaw.cos())) > 0.86
            && line(world, origin, eye)
    })
}

fn ray_color(e: &Enemy) -> [f32; 3] {
    match e.attacks % 3 { 0 => [0.86, 0.32, 0.19], 1 => [0.48, 0.68, 0.83], _ => [0.65, 0.36, 0.72] }
}

/// All attacks commit to the windup's aim. Moving sideways or breaking sight
/// defeats the ray; no tracking hitscan and no damage through a wall.
pub(super) fn release(e: &mut Enemy, world: &World, player: &mut Player, time: f32) -> Option<f32> {
    if !is_kind(e.kind) { return None; }
    let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
    let origin = emitter(e, time, false);
    let delta = e.aim - origin;
    let ray = delta.normalize_or_zero();
    let along = (eye - origin).dot(ray);
    let hit = along > 0.0 && along < delta.length() + 0.8
        && (eye - (origin + ray * along)).length() < if roper_like(e.kind) { 0.65 } else { 0.48 }
        && line(world, origin, eye) && line(world, origin, e.aim);
    if std::env::var_os("VOXEL_REVIEW_ENEMIES").is_some() {
        println!("[aberration-cast] kind={} ray={} hit={hit} miss_m={:.3} origin_m={origin:?}",e.kind,e.attacks%3,(eye-(origin+ray*along)).length());
    }
    if !hit { return Some(0.0); }
    if tendrils(e.kind) {
        e.encounter.exposed = if roper_like(e.kind) { 2.4 } else { 0.9 };
        return Some(if e.kind == CHOKER { 7.0 } else { 0.0 });
    }
    let damage = match e.attacks % 3 {
        0 => if e.kind == BEHOLDER { 26.0 } else { 14.0 },
        1 => { e.encounter.exposed = 0.8; 5.0 },
        _ => { e.encounter.exposed = 1.0; 8.0 },
    };
    Some(damage * (1.0 + e.menace as f32 * 0.2))
}

pub(super) fn tether(e: &mut Enemy, world: &World, player: &mut Player, dt: f32, time: f32) -> f32 {
    if !is_kind(e.kind) || e.encounter.exposed <= 0.0 { return 0.0; }
    e.encounter.exposed = (e.encounter.exposed - dt).max(0.0);
    let scale = world.voxel_scale.meters();
    let eye = player.eye(world.voxel_scale) * scale;
    let delta = e.center() - eye;
    if !line(world, emitter(e, time, false), eye) || delta.length() > 13.0 || e.stagger > 0.0 {
        e.encounter.exposed = 0.0;
        return 0.0;
    }
    if tendrils(e.kind) {
        e.aim = eye;
        // Sweep a short physical pull; a corner releases the tendril rather
        // than pulling the player through terrain. Melee stagger also severs it.
        let movement = delta * Vec3::new(1.0, 0.0, 1.0);
        let q = player.pos + movement.normalize_or_zero() * dt * 1.7 / scale;
        let (min, max) = super::super::player_aabb(q, world.voxel_scale, player.scale, player.crouch_t);
        if super::collides_world(world, min, max) { e.encounter.exposed = 0.0; }
        else { player.pos = q; }
        if delta.length() < 1.9 {
            e.encounter.exposed = 0.0;
            return if roper_like(e.kind) { 24.0 } else { 12.0 };
        }
    } else if e.attacks % 3 == 1 {
        player.vel.x *= (-dt * 5.0).exp();
        player.vel.z *= (-dt * 5.0).exp();
    } else {
        player.vel += delta.normalize_or_zero() * dt * 2.0 / scale;
    }
    0.0
}

pub(super) fn telegraph(e: &Enemy, m: &mut Model, time: f32) {
    if !is_kind(e.kind) || e.health <= 0.0 { return; }
    if e.tell > 0.0 || e.recovery > 0.12 || (tendrils(e.kind) && e.encounter.exposed > 0.0) {
        let tether = tendrils(e.kind);
        m.beam(emitter(e, time, false), e.aim, if tether { 0.045 } else { 0.025 },
            if tether { [0.46, 0.39, 0.30] } else { ray_color(e) }, if tether { 0.0 } else { 0.65 });
    }
    if e.kind == BEHOLDER && e.tell <= 0.0 && e.recovery <= 0.0 && e.stagger <= 0.0 {
        let forward = Vec3::new(-e.yaw.sin(), 0.0, -e.yaw.cos());
        let origin = emitter(e, time, true);
        for a in [-0.53_f32, 0.53] {
            m.beam(origin, origin + Quat::from_rotation_y(a) * forward * 12.0,
                0.018, [0.38, 0.57, 0.33], 0.22);
        }
    }
}

fn block(m: &mut Model, p: Vec3, size: Vec3, color: [f32; 3]) {
    m.material_box(p, size, Quat::IDENTITY, color, 70.0, 0.0);
}
fn eye(m: &mut Model, p: Vec3, r: f32, color: [f32; 3], closed: bool, energy: f32) {
    block(m, p, Vec3::new(r * 2.0, if closed { r * 0.18 } else { r * 1.6 }, r * 0.5), [0.77, 0.70, 0.48]);
    if !closed {
        m.material_box(p - Vec3::Z * r * 0.29, Vec3::new(r * 0.65, r * 1.15, r * 0.13), Quat::IDENTITY, color, 60.0, energy);
        block(m, p - Vec3::Z * r * 0.37, Vec3::new(r * 0.17, r * 0.8, r * 0.06), [0.045; 3]);
    }
}

pub(super) fn body(e: &Enemy, m: &mut Model, time: f32, wind: f32) {
    let wind = reach(e, wind);
    if matches!(e.kind, SPECTATOR | BEHOLDER) {
        let large = e.kind == BEHOLDER;
        let (c, r, count) = eye_body(e, time);
        let skin = if large { [0.35, 0.25, 0.31] } else { [0.28, 0.41, 0.33] };
        // Stepped barrel: broad middle and chamfered caps, never a cube head.
        for i in -2_i32..=2 {
            let width = r * (if i.abs() == 2 { 1.18 } else if i.abs() == 1 { 1.7 } else { 1.95 });
            block(m, c + Vec3::Y * i as f32 * r * 0.34, Vec3::new(width, r * 0.35, width * 0.85), skin);
        }
        eye(m, c - Vec3::Z * r * 0.89, r * 0.5, [0.49, 0.61, 0.30], large && wind > 0.0, 0.45);
        block(m, c + Vec3::new(0.0, -r * 0.52, -r * 0.77), Vec3::new(r * 1.08, r * 0.34, r * 0.28), [0.10, 0.055, 0.065]);
        for i in 0..5 {
            block(m, c + Vec3::new((i as f32 - 2.0) * r * 0.2, -r * 0.43, -r * 0.94), Vec3::new(r * 0.09, r * (0.18 + wind * 0.12), r * 0.07), [0.78, 0.68, 0.46]);
        }
        for i in 0..count {
            let (root, bend, tip) = stalk(e, time, wind, i);
            m.material_beam(root, bend, r * 0.09, skin, 70.0);
            m.material_beam(bend, tip, r * 0.07, skin, 70.0);
            let casting = i == e.attacks % count && (e.tell > 0.0 || e.recovery > 0.0);
            eye(m, tip, r * 0.15, if casting { ray_color(e) } else { [0.49, 0.61, 0.30] },
                false, if casting { 0.45 + wind * 0.65 } else { 0.20 });
        }
    } else if tree_roper(e.kind) {
        tree_body(e, m, time, wind);
    } else if roper_like(e.kind) {
        // A stalactite is the same stack hung from rock: root up, spire down.
        let hanging = e.kind == STALACTITE;
        let at = |p: Vec3| e.pos + Vec3::new(p.x, if hanging { HANG_M - p.y } else { p.y }, p.z);
        let stone = [0.32, 0.31, 0.27];
        for i in 0..7 {
            let width = 1.45 - i as f32 * 0.17;
            block(m, at(Vec3::Y * (0.16 + i as f32 * 0.33)), Vec3::new(width, 0.36, width * 0.85), stone);
        }
        eye(m, at(Vec3::new(0.0, 1.56, -0.43)), 0.19, [0.71, 0.38, 0.16], e.alert <= 0.0, 0.45);
        block(m, at(Vec3::new(0.0, 0.94, -0.54)), Vec3::new(0.66, 0.37 + wind * 0.15, 0.17), [0.12, 0.10, 0.085]);
        for i in 0..6 {
            let a = i as f32 * std::f32::consts::TAU / 6.0;
            let root = at(Vec3::new(a.cos() * 0.5, 0.8 + (i % 2) as f32 * 0.4, a.sin() * 0.4));
            let end = tendril_end(e, wind, i);
            let knee = root.lerp(end, 0.55) + Vec3::Y * (0.25 + (time + a).sin() * 0.08);
            m.material_beam(root, knee, 0.055, stone, 70.0);
            m.material_beam(knee, end, 0.027, stone, 70.0);
        }
    } else {
        let skin = [0.39, 0.34, 0.28];
        let c = e.pos + Vec3::Y * (0.8 - wind * 0.18);
        block(m, c, Vec3::new(0.30, 0.67, 0.25), skin);
        block(m, c + Vec3::new(0.0, 0.43, -0.08), Vec3::new(0.37, 0.32, 0.29), skin);
        eye(m, c + Vec3::new(0.0, 0.45, -0.24), 0.09, [0.68, 0.51, 0.31], false, 0.45);
        for side in [-1.0, 1.0] {
            let swing = (e.phase * 6.0 + side).sin() * 0.14 * e.gait;
            let hip = c + Vec3::new(side * 0.12, -0.30, 0.0);
            let knee = e.pos + Vec3::new(side * 0.23, 0.24, 0.15 + swing);
            let foot = e.pos + Vec3::new(side * 0.27, 0.06, -0.08 - swing);
            m.material_beam(hip, knee, 0.06, skin, 70.0);
            m.material_beam(knee, foot, 0.045, skin, 70.0);
            let shoulder = c + Vec3::new(side * 0.22, 0.20, 0.0);
            let elbow = c + Vec3::new(side * 0.48, -0.15, -0.16 - wind * 0.3);
            let hand = hand(e, wind, side);
            m.material_beam(shoulder, elbow, 0.052, skin, 70.0);
            m.material_beam(elbow, hand, 0.038, skin, 70.0);
            for j in 0..3 { m.material_beam(hand + Vec3::X * (j as f32 - 1.0) * 0.045, hand + Vec3::new((j as f32 - 1.0) * 0.06, -0.12, -0.12), 0.018, skin, 70.0); }
        }
    }
}

/// Shared reveal state for the face and the target HUD.
pub(super) fn tree_roused(e: &Enemy) -> bool {
    e.tell > 0.0 || e.recovery > 0.0 || e.encounter.exposed > 0.0
        || e.health < e.max_health() - 0.1
}

fn tree_body(e: &Enemy, m: &mut Model, time: f32, wind: f32) {
    let palm = e.kind == PALM_ROPER;
    let bark = if palm { [0.46, 0.37, 0.25] } else { [0.31, 0.22, 0.15] };
    let dark = if palm { [0.34, 0.27, 0.18] } else { [0.22, 0.15, 0.10] };
    let (leaf, tips) = if palm {
        ([0.27, 0.47, 0.19], [0.34, 0.52, 0.22])
    } else {
        ([0.13, 0.27, 0.16], [0.19, 0.34, 0.20])
    };
    let height = if palm { 4.4 } else { 3.6 };
    let segments = if palm { 10 } else { 7 };
    // Breathing: the trunk swells a little while it holds someone.
    let swell = 1.0 + wind * 0.08;
    let bow = |t: f32| if palm { Vec3::X * (t * std::f32::consts::PI * 0.8).sin() * 0.3 } else { Vec3::ZERO };
    for i in 0..segments {
        let t = (i as f32 + 0.5) / segments as f32;
        let width = (if palm { 0.42 } else { 0.55 }) * (1.0 - t * if palm { 0.2 } else { 0.4 }) * swell;
        let p = e.pos + Vec3::Y * (t * height) + bow(t);
        block(m, p, Vec3::new(width, height / segments as f32 + 0.02, width), bark);
        if palm { block(m, p + Vec3::Y * (height / segments as f32 * 0.5), Vec3::new(width + 0.06, 0.06, width + 0.06), dark); }
    }
    let top = e.pos + Vec3::Y * height + bow(1.0);
    if palm {
        for i in 0..7 {
            let a = i as f32 * std::f32::consts::TAU / 7.0 + e.slot as f32;
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            // Fronds twitch upward when it strikes.
            let lift = wind * 0.6 + (time * 0.8 + a).sin() * 0.05;
            let span = 0.9 + 0.16 * (a * 3.0).sin();
            let elbow = top + out * span + Vec3::Y * (0.15 + lift * 0.4);
            let tip = elbow + out * span + Vec3::Y * (-0.75 + lift);
            m.material_beam(top, elbow, 0.17, leaf, 70.0);
            m.material_beam(elbow, tip, 0.1, tips, 70.0);
            let side = out.cross(Vec3::Y);
            for side_sign in [-1.0, 1.0] {
                let outer = elbow + side * (side_sign * 0.24) + out * 0.26 - Vec3::Y * 0.19;
                m.material_beam(top.lerp(elbow, 0.65), outer, 0.09, leaf, 70.0);
            }
        }
        for i in 0..3 {
            let a = i as f32 * 2.1 + e.slot as f32;
            block(m, top + Vec3::new(a.cos() * 0.17, -0.2, a.sin() * 0.17), Vec3::splat(0.16), [0.36, 0.25, 0.13]);
        }
    } else {
        for i in 0..6 {
            let k = i as f32;
            let width = (2.3 - k * 0.34) * (1.0 + wind * 0.1);
            let y = 1.5 + k * 0.48;
            let tier = e.pos + Vec3::Y * y;
            let stagger = k * 0.65 + (e.slot % 31) as f32 + (time * 0.6 + k).sin() * 0.02;
            for branch in 0..5 {
                let a = stagger + branch as f32 * std::f32::consts::TAU / 5.0;
                let out = Vec3::new(a.cos(), 0.0, a.sin());
                let elbow = tier + out * (width * 0.26) - Vec3::Y * 0.09;
                let tip = tier + out * (width * 0.48) - Vec3::Y * 0.24;
                m.material_beam(tier, elbow, width * 0.23, leaf, 70.0);
                m.material_beam(elbow, tip, width * 0.14, tips, 70.0);
            }
        }
        block(m, e.pos + Vec3::Y * 4.08, Vec3::new(0.18, 0.38, 0.18), tips);
    }
    // Knot that is an eye; split that is a mouth.
    let face = e.pos + Vec3::Y * 1.3 + bow(1.3 / height);
    if tree_roused(e) {
        eye(m, face + Vec3::new(0.0, 0.0, -0.26 * swell), 0.14, [0.78, 0.55, 0.18], false, 0.55);
        block(m, face + Vec3::new(0.0, -0.5, -0.25 * swell), Vec3::new(0.34, 0.08 + wind * 0.22, 0.06), [0.07, 0.05, 0.04]);
    } else {
        block(m, face + Vec3::new(0.0, 0.0, -0.25), Vec3::new(0.16, 0.12, 0.05), dark);
    }
    // Roots: lying flat round the foot, they arch up and reach when it casts.
    for i in 0..6 {
        let a = i as f32 * std::f32::consts::TAU / 6.0;
        let root = e.pos + Vec3::new(a.cos() * 0.22, 0.25, a.sin() * 0.22);
        let end = tendril_end(e, wind, i);
        let knee = root.lerp(end, 0.5) + Vec3::Y * (0.05 + wind * 0.5 + (time + a).sin() * 0.03 * wind);
        m.material_beam(root, knee, 0.07, bark, 70.0);
        m.material_beam(knee, end, 0.04, bark, 70.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::adventure::bestiary;

    #[test]
    fn stone_spires_share_roper_rules() {
        let roper = bestiary::species(ROPER).unwrap();
        for kind in [STALACTITE, STALAGMITE] {
            let spire = bestiary::species(kind).unwrap();
            assert!(is_kind(kind) && roper_like(kind) && tendrils(kind));
            assert_eq!(spire.role, roper.role);
            assert_eq!(spire.speed, roper.speed);
            assert_eq!(spire.attack_range, roper.attack_range);
            assert_eq!(spire.body_height, roper.body_height);
        }
    }
}
