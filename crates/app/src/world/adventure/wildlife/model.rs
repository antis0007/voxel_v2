//! Metre-authored livestock: wool masses over hide, planted two-link legs.
use super::*;
use crate::mesh::models::Model;
use crate::world::adventure::enemy_presentation::step;
use std::f32::consts::PI;

const WOOL: f32 = 69.0;
const HIDE: f32 = 70.0;
const HOOF: f32 = 71.0;
const DARK: [f32; 3] = [0.15, 0.14, 0.13];
const HORN: [f32; 3] = [0.70, 0.65, 0.51];

/// Species colours and head placement in metres. Forward is -z; the ground is y = 0.
struct Look {
    coat: [f32; 3],
    face: [f32; 3],
    shin: [f32; 3],
    hip_y: f32,
    head_at: Vec3,
    head: Vec3,
    fleece: bool,
}

fn look(goat: bool) -> Look {
    if goat {
        Look {
            coat: [0.44, 0.32, 0.22],
            face: [0.36, 0.27, 0.20],
            shin: [0.30, 0.24, 0.19],
            hip_y: 0.62,
            head_at: Vec3::new(0.0, 1.02, -0.66),
            head: Vec3::new(0.26, 0.30, 0.40),
            fleece: false,
        }
    } else {
        Look {
            coat: [0.88, 0.85, 0.74],
            face: [0.86, 0.77, 0.70],
            shin: [0.80, 0.70, 0.62],
            hip_y: 0.60,
            head_at: Vec3::new(0.0, 0.98, -0.66),
            head: Vec3::new(0.32, 0.34, 0.34),
            fleece: true,
        }
    }
}

fn part(m: &mut Model, e: &Enemy, offset: Vec3, size: Vec3, rotation: Quat, color: [f32; 3], material: f32) {
    m.material_box(e.pos + offset, size, rotation, color, material, 0.0);
}
fn limb(m: &mut Model, e: &Enemy, a: Vec3, b: Vec3, width: f32, color: [f32; 3], material: f32) {
    let d = b - a;
    part(m, e, (a + b) * 0.5, Vec3::new(width, d.length(), width),
        Quat::from_rotation_arc(Vec3::Y, d.normalize_or_zero()), color, material);
}

pub(in crate::world::adventure) fn draw(e: &Enemy, m: &mut Model, time: f32) {
    let mark = m.mark();
    let goat = e.kind == GOAT;
    let l = look(goat);
    let body = if l.fleece { WOOL } else { HIDE };
    let up = Vec3::Y * (time * 1.8 + e.slot as f32).sin() * 0.006 * (1.0 - e.gait);
    // Wool is stacked slabs: a core, side bulges, a crown, a hem and a chest, so the outline rounds off.
    if l.fleece {
        part(m, e, Vec3::new(0.0, 0.72, 0.04) + up, Vec3::new(0.66, 0.50, 1.04), Quat::IDENTITY, l.coat, WOOL);
        part(m, e, Vec3::new(0.0, 0.74, 0.04) + up, Vec3::new(0.80, 0.36, 0.90), Quat::IDENTITY, l.coat, WOOL);
        part(m, e, Vec3::new(0.0, 1.06, 0.04) + up, Vec3::new(0.64, 0.14, 0.98), Quat::IDENTITY, l.coat, WOOL);
        part(m, e, Vec3::new(0.0, 0.42, 0.04) + up, Vec3::new(0.70, 0.12, 0.98), Quat::IDENTITY, l.coat, WOOL);
        part(m, e, Vec3::new(0.0, 0.70, -0.46) + up, Vec3::new(0.72, 0.46, 0.16), Quat::IDENTITY, l.coat, WOOL);
        part(m, e, Vec3::new(0.0, 0.70, 0.56) + up, Vec3::new(0.68, 0.42, 0.14), Quat::IDENTITY, l.coat, WOOL);
        part(m, e, Vec3::new(0.0, 0.92, 0.66) + up, Vec3::new(0.12, 0.14, 0.12), Quat::IDENTITY, l.coat, WOOL);
    } else {
        part(m, e, Vec3::new(0.0, 0.74, 0.04) + up, Vec3::new(0.52, 0.50, 1.00), Quat::IDENTITY, l.coat, HIDE);
        part(m, e, Vec3::new(0.0, 0.96, 0.62) + up, Vec3::new(0.07, 0.16, 0.07), Quat::from_rotation_x(0.6), l.coat, HIDE);
    }
    // Legs: equal-length two-link IK. Hooves stay on the ground and limb lengths never change.
    let link = (l.hip_y - 0.075) * 0.5 + 0.025;
    for side in [-1.0_f32, 1.0] { for end in [-1.0_f32, 1.0] {
        let phase = e.phase * 3.0 + if side == end { 0.0 } else { PI };
        let (travel, lift) = step(phase, 0.16, 0.11, e.gait);
        let z = if end < 0.0 { -0.33 } else { 0.36 };
        let hip = Vec3::new(side * 0.20, l.hip_y, z) + up;
        let foot = Vec3::new(side * 0.20, 0.075 + lift, z + travel);
        let d = foot - hip;
        let bend = (link * link - d.length_squared() * 0.25).max(0.0).sqrt();
        // Front knees point forward and hocks point back.
        let knee = (hip + foot) * 0.5 - Vec3::new(0.0, -d.z, d.y).normalize_or_zero() * bend * end;
        let upper = if l.fleece { 0.18 } else { 0.15 };
        limb(m, e, hip, knee, upper, l.coat, body);
        limb(m, e, knee, foot, 0.12, l.shin, HIDE);
        part(m, e, foot + Vec3::new(0.0, -0.02, -0.01), Vec3::new(0.15, 0.11, 0.17), Quat::IDENTITY, DARK, HOOF);
    }}
    // The neck stays with the body; everything from the head up rotates for grazing.
    part(m, e, Vec3::new(0.0, l.head_at.y - 0.10, l.head_at.z + 0.16) + up, Vec3::new(0.22, 0.26, 0.20),
        Quat::IDENTITY, if l.fleece { l.coat } else { l.face }, if l.fleece { WOOL } else { HIDE });
    let head_mark = m.mark();
    let h = l.head_at + up;
    let front = h.z - l.head.z * 0.5 - 0.01;
    part(m, e, h, l.head, Quat::IDENTITY, l.face, HIDE);
    if l.fleece {
        part(m, e, h + Vec3::new(0.0, l.head.y * 0.5 + 0.02, 0.02), Vec3::new(l.head.x + 0.04, 0.10, l.head.z + 0.04),
            Quat::IDENTITY, l.coat, WOOL);
    }
    for side in [-1.0_f32, 1.0] {
        part(m, e, Vec3::new(side * 0.10, h.y + 0.04, front), Vec3::new(0.05, 0.05, 0.02), Quat::IDENTITY, [0.08, 0.08, 0.09], HOOF);
        if goat {
            part(m, e, Vec3::new(side * (l.head.x * 0.5 + 0.03), h.y + 0.14, h.z + 0.06), Vec3::new(0.06, 0.15, 0.09),
                Quat::from_rotation_z(-side * 0.55), l.face, HIDE);
            part(m, e, Vec3::new(side * 0.09, h.y + 0.26, h.z + 0.08), Vec3::new(0.08, 0.26, 0.08),
                Quat::from_rotation_x(0.55), HORN, HOOF);
        } else {
            part(m, e, Vec3::new(side * (l.head.x * 0.5 + 0.06), h.y + 0.06, h.z + 0.04), Vec3::new(0.15, 0.06, 0.09),
                Quat::from_rotation_z(-side * 0.35), [0.78, 0.60, 0.58], HIDE);
        }
    }
    if goat {
        part(m, e, Vec3::new(0.0, h.y - l.head.y * 0.5 - 0.05, h.z - 0.12), Vec3::new(0.09, 0.12, 0.08), Quat::IDENTITY, l.coat, HIDE);
    } else {
        part(m, e, Vec3::new(0.0, h.y - 0.09, front), Vec3::new(0.12, 0.06, 0.02), Quat::IDENTITY, [0.74, 0.54, 0.52], HIDE);
    }
    let grazing = (1.0 - e.alert.clamp(0.0, 1.0)) * (1.0 - e.gait.clamp(0.0, 1.0));
    let joint = e.pos + Vec3::new(0.0, h.y - l.head.y * 0.5, h.z + l.head.z * 0.5);
    m.rotate_since(head_mark, joint, Quat::from_rotation_x(-grazing * 1.0));
    let flash = (e.flash / 0.18).clamp(0.0, 1.0) * 0.5;
    if flash > 0.0 {
        for v in &mut m.vertices[mark..] {
            for (c, tint) in v.color.iter_mut().zip([0.95, 0.85, 0.76]) { *c = *c * (1.0 - flash) + tint * flash; }
        }
    }
    m.rotate_since(mark, e.pos, Quat::from_rotation_y(e.yaw));
    if e.youth > 0.0 {
        m.scale_since(mark, e.pos, Vec3::splat(1.0 - 0.45 * e.youth.min(1.0)));
    }
}
