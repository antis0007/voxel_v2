//! Generator 36 passages: round arched roofs, rubble along the wall foot,
//! pillar rows down wide halls, and niches cut into the walls every eight
//! metres, half of them holding bones, some a lamp. Niches live in the
//! corridor's own shell, which room shells outrank, so a niche never opens
//! into a room or a seal.
use super::{Complex, Corr, kit::near_line};
use crate::{BlockId as B, hash2, hash3};
use glam::Vec3;

/// Spacing of wall niches, metres; they sit between the eight-metre sconces.
const NICHE: f32 = 8.0;

/// Roof height over the floor at `edge` (0 on the centre line, 1 at a wall):
/// a round arch springing a little over a metre below the crown.
pub fn roof(k: &Corr, edge: f32) -> f32 {
    let half = if k.along_x { (k.z1 - k.z0) * 0.5 } else { (k.x1 - k.x0) * 0.5 };
    let drop = (half * 0.8).min(1.2);
    k.h - drop + drop * (1.0 - edge * edge).max(0.0).sqrt()
}

/// Pillars and rubble inside a passage; `None` is air.
pub fn furnish(c: &Complex, k: &Corr, m: Vec3, along: f32, across: f32, half: f32, fy: f32, v: f32) -> Option<B> {
    let inner = along > k.a0 + 1.5 && along < k.a1 - 1.5;
    // Pillar rows a metre in from each wall of a wide, long passage.
    if half >= 2.0 && k.a1 - k.a0 > 9.0 && inner {
        let side = across.abs() - (half - 1.0);
        if (0.0..v.max(0.6)).contains(&side) && near_line(along, 4.0, v, 0.3) {
            return Some(if fy < 0.6 { c.pal.trim } else { c.pal.wall });
        }
    }
    // Rubble at the wall foot, only where it leaves a clear two-metre lane
    // and never on a ramp, where it would stack on a step.
    let p = (m / v).floor().as_ivec3();
    let flat = (k.fa - k.fb).abs() < 0.01;
    (flat && half >= 1.5 && half - across.abs() < 0.5 && fy < v && hash3(p.x, p.y, p.z, c.seed.wrapping_add(23)) > 0.82).then_some(B::Gravel)
}

/// The shell around a passage, with its niches.
pub fn shell(c: &Complex, k: &Corr, m: Vec3, v: f32) -> B {
    let stone = || super::kit::stone(c, m, v);
    let (along, across, half) = if k.along_x {
        (m.x, m.z - (k.z0 + k.z1) * 0.5, (k.z1 - k.z0) * 0.5)
    } else {
        (m.z, m.x - (k.x0 + k.x1) * 0.5, (k.x1 - k.x0) * 0.5)
    };
    let out = across.abs() - half;
    let fy = m.y - k.floor_at(m.x, m.z);
    if !(0.0..0.8).contains(&out) || k.a1 - k.a0 < 6.0 || along < k.a0 + 1.5 || along > k.a1 - 1.5 {
        return stone();
    }
    let slot = ((along - NICHE * 0.5) / NICHE).round();
    if (along - NICHE * 0.5 - slot * NICHE).abs() >= 0.5 || !(0.6..1.8).contains(&fy) {
        return stone();
    }
    let what = hash2(slot as i32, i32::from(across > 0.0), c.seed.wrapping_add(29));
    if fy < 0.6 + v {
        if what > 0.85 {
            return B::Torch;
        }
        if what > 0.4 {
            return B::Gravel;
        }
    }
    B::Air
}
