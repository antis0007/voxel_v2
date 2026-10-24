//! Grove dressing: the warm mushroom grove, a fallen giant, ground fungi and
//! ferns, and the hollow tree's stair down to the glowcap undercroft.
use super::plan::{HOLLOW, Plan};
use crate::{BlockId as B, WorldSampleConfig, hash2, hash3, value_noise2};
use glam::{IVec3, Vec2, Vec3};

const STAIR_RUN: f32 = 12.0;
const STAIR_FALL: f32 = 0.72;
const TUNNEL_HALF: f32 = 1.3;
const TUNNEL_HEAD: f32 = 3.2;
const CHAMBER_R: f32 = 6.5;
const CHAMBER_H: f32 = 4.4;

/// Stair frame: hollow centre, ground there, and the stair direction (away from the door).
pub(super) fn stair(plan: &Plan) -> Option<(Vec2, f32, Vec2)> {
    let g = plan.giants.get(HOLLOW)?;
    Some((Vec2::new(g.x, g.z), g.base, -g.hollow?))
}

pub(super) fn chamber(plan: &Plan) -> Option<(Vec2, f32)> {
    let (c, g0, dir) = stair(plan)?;
    Some((c + dir * (STAIR_RUN + 3.5), g0 - STAIR_RUN * STAIR_FALL - 0.4))
}

pub(super) fn in_chamber(plan: &Plan, m: Vec3) -> bool {
    chamber(plan).is_some_and(|(c, floor)| {
        let e = ((Vec2::new(m.x, m.z) - c).length() / CHAMBER_R).powi(2) + ((m.y - floor) / CHAMBER_H).powi(2);
        e < 1.0 && m.y >= floor - 1.0
    })
}

/// The stair tunnel and the undercroft: carved air, glowcaps and amber lights.
pub(super) fn underground(plan: &Plan, m: Vec3, vm: f32) -> Option<B> {
    let (c, g0, dir) = stair(plan)?;
    let rel = Vec2::new(m.x, m.z) - c;
    let along = rel.dot(dir);
    if (-2.0..STAIR_RUN + 4.0).contains(&along) {
        let lateral = rel.x * dir.y - rel.y * dir.x;
        let floor = g0 - along.clamp(0.0, STAIR_RUN) * STAIR_FALL;
        if lateral.abs() < TUNNEL_HALF && m.y >= floor && m.y < floor + TUNNEL_HEAD {
            return Some(B::Air);
        }
        // Amber lamps on alternating walls light the way down.
        for (k, at) in [3.5_f32, 7.5, 11.0].into_iter().enumerate() {
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let off = lateral * side - TUNNEL_HALF;
            if (along - at).abs() < 0.6 && (0.0..vm).contains(&off) && (m.y - floor - 1.9).abs() < 0.5 {
                return Some(B::AmberCap);
            }
        }
    }
    let (cc, floor) = chamber(plan)?;
    let hd = (Vec2::new(m.x, m.z) - cc).length();
    let e = (hd / CHAMBER_R).powi(2) + ((m.y - floor) / CHAMBER_H).powi(2);
    if e >= 1.35 || m.y < floor - vm {
        return None;
    }
    if m.y >= floor && e < 1.0 {
        return Some(B::Air);
    }
    if m.y < floor {
        return (hd < CHAMBER_R).then_some(B::FungalSoil);
    }
    // Three warm clumps; the rest of the shell is spotted with glowcaps.
    for k in 0..3 {
        let a = plan.phase + k as f32 * 2.1;
        let lamp = Vec3::new(cc.x + a.cos() * CHAMBER_R * 0.9, floor + 2.2, cc.y + a.sin() * CHAMBER_R * 0.9);
        if (m - lamp).length() < 1.2 {
            return Some(B::AmberCap);
        }
    }
    // Glowcaps colonise the lower walls in a few clumps; the vault stays dark.
    let colony = crate::value_noise3(m.x / 2.6, m.y / 2.6, m.z / 2.6, plan.seed.wrapping_add(41));
    let cell = (m / vm.max(0.5)).floor().as_ivec3();
    let wall = m.y < floor + CHAMBER_H * 0.55;
    (wall && colony > 0.68 && hash3(cell.x, cell.y, cell.z, plan.seed.wrapping_add(42)) > 0.4).then_some(B::GlowCap)
}

/// Above-ground dressing once the giants have had their say.
pub(super) fn above(plan: &Plan, cfg: &WorldSampleConfig, p: IVec3, m: Vec3, ground: f32, height: i32, clear: bool) -> Option<B> {
    let vm = cfg.voxel_meters();
    for s in &plan.shrooms {
        if let Some(b) = shroom(s, m, vm) {
            return Some(b);
        }
    }
    if let Some(b) = fallen_log(plan, m) {
        return Some(b);
    }
    if clear || p.y > height + cfg.blocks_for_span(1.0) + 1 {
        return None;
    }
    let first = p.y == height + 1;
    // Amber clumps gather at the giants' feet, between the roots.
    if first {
        let cell = 2.5;
        let (cx, cz) = ((m.x / cell).floor() as i32, (m.z / cell).floor() as i32);
        if hash2(cx, cz, plan.seed.wrapping_add(51)) > 0.84 {
            let j = Vec2::new(hash2(cx, cz, plan.seed.wrapping_add(52)), hash2(cx, cz, plan.seed.wrapping_add(53)));
            let centre = (Vec2::new(cx as f32, cz as f32) + j) * cell;
            let near_giant = plan.giants.iter().any(|g| {
                let d = (centre - Vec2::new(g.x, g.z)).length();
                d > g.ground_radius() && d < g.ground_radius() + 7.0
            });
            if near_giant && (Vec2::new(m.x, m.z) - centre).length() < 0.9 {
                return Some(B::AmberCap);
            }
        }
    }
    // Sparse fern drifts only where roots or water shelter them; the floor
    // between stays open so the giants read as shapes, not clutter.
    let w = Vec2::new(m.x, m.z);
    let sheltered = plan.giants.iter().any(|g| (w - Vec2::new(g.x, g.z)).length() < g.ground_radius() + 9.0)
        || plan.pools.iter().any(|pool| {
            let c = plan.world(pool.c.x, pool.c.y);
            (w - c).length() < pool.r + 3.0
        });
    let patch = value_noise2(m.x / 9.0, m.z / 9.0, plan.seed.wrapping_add(61));
    let pick = hash3(p.x, 0, p.z, plan.seed.wrapping_add(62));
    let tall = m.y < ground + 2.0 && patch > 0.7 && pick > 0.9;
    (sheltered && (first || tall) && patch > 0.55 && pick > 0.72).then_some(B::Bush)
}

fn shroom(s: &super::plan::Shroom, m: Vec3, vm: f32) -> Option<B> {
    let reach = s.cap + 1.0 + s.lean.length() * s.height;
    if (m.x - s.x).abs() > reach || (m.z - s.z).abs() > reach {
        return None;
    }
    let y = m.y - s.base;
    if y < -1.0 || y > s.height + s.cap {
        return None;
    }
    let top = s.base + s.height;
    let off = s.lean * y.clamp(0.0, s.height);
    let rho = Vec2::new(m.x - s.x - off.x, m.z - s.z - off.y).length();
    if y < s.height && rho < s.stem.max(vm * 0.5) {
        return Some(B::MushroomStem);
    }
    if rho >= s.cap {
        return None;
    }
    if m.y >= top - vm && m.y < top {
        return Some(B::AmberCap);
    }
    let dome = vm + s.cap * 0.6 * (1.0 - (rho / s.cap).powi(2)).sqrt();
    (m.y >= top && m.y < top + dome).then_some(if s.red { B::RedCap } else { B::UmberCap })
}

/// A fallen giant lies across the far side of the hollow, moss on its back.
fn fallen_log(plan: &Plan, m: Vec3) -> Option<B> {
    let a = plan.world(-8.0, -54.0);
    let b = plan.world(15.0, -48.0);
    let ab = b - a;
    let t = ((Vec2::new(m.x, m.z) - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    let q = a + ab * t;
    if (Vec2::new(m.x, m.z) - q).length() > 1.5 {
        return None;
    }
    let (qu, qv) = plan.local(q.x, q.y);
    let axis_y = plan.floor_at(qu, qv) + 1.0;
    let r = 1.4 * (1.0 - 0.3 * t);
    let d = Vec2::new((Vec2::new(m.x, m.z) - q).length(), m.y - axis_y);
    if d.length() >= r {
        return None;
    }
    let moss = d.y > r * 0.55 && hash2(m.x.floor() as i32, m.z.floor() as i32, plan.seed.wrapping_add(71)) > 0.35;
    Some(if moss { B::Leaves } else { B::Log })
}
