//! A corkscrewing tube whose radius swells and narrows and that sends off one
//! tangential branch, like a strand in space. Gravity points outward from the
//! nearest point on the centerline: zero on the axis, g_wall at the tube floor.
//! Dormant behind its generator gate; the app-side controller and native
//! validation are separate live-enable steps.
use crate::{BlockId, ColumnSample, WorldSampleConfig, hash_u32};
use glam::{IVec3, Vec3};

pub const VERSION: u32 = 102;

const G: f32 = 28.0;
const HELIX_RADIUS: f32 = 40.0;
const TURNS: f32 = 2.0;
const PITCH: f32 = 40.0;
const R_MIN: f32 = 12.0;
const R_MAX: f32 = 32.0;
const WALL: f32 = 2.0;
const SPINE_Z0: f32 = 64.0;
const N: usize = 32;
/// 21 / N, so the junction lands exactly on polyline vertex 21. At 0.66 the
/// branch started about 1.9 m off the trunk's shared vertex.
const BRANCH_T: f32 = 0.65625;
const BRANCH_LEN: f32 = 44.0;

/// Centreline of the trunk corkscrew at parameter t in [0, 1].
fn trunk(t: f32) -> Vec3 {
    let ang = std::f32::consts::TAU * TURNS * t;
    Vec3::new(HELIX_RADIUS * ang.cos(), HELIX_RADIUS * ang.sin(), SPINE_Z0 + t * TURNS * PITCH)
}

/// Tangent of the trunk at t, unit length, for the branch's take-off ray.
fn trunk_tangent(t: f32) -> Vec3 {
    let ang = std::f32::consts::TAU * TURNS * t;
    let d = Vec3::new(-HELIX_RADIUS * ang.sin(), HELIX_RADIUS * ang.cos(), PITCH * TURNS);
    d.normalize_or_zero()
}

/// A ray peeling off tangentially from the branch point; shares point + tangent
/// with the trunk so the field stays continuous across the junction.
fn branch(t: f32) -> Vec3 {
    let start = trunk(BRANCH_T);
    let dir = trunk_tangent(BRANCH_T);
    start + dir * ((t - BRANCH_T) / (1.0 - BRANCH_T)) * BRANCH_LEN
}

/// Polyline points for each strand. Strand A is the whole trunk; strand B is
/// the trunk up to the branch point and then the tangential ray.
fn strands() -> [Vec<Vec3>; 2] {
    let mut a = Vec::with_capacity(N + 1);
    for i in 0..=N {
        a.push(trunk(i as f32 / N as f32));
    }
    let ib = (BRANCH_T * N as f32).round() as usize;
    let mut b = a[..=ib].to_vec();
    // Vertex ib is already the junction; starting the ray there would repeat it.
    for i in ib + 1..=N {
        b.push(branch(i as f32 / N as f32));
    }
    [a, b]
}

/// A straight centerline segment with the arc length at its start.
struct Seg {
    a: Vec3,
    b: Vec3,
    s0: f32,
}

fn segments() -> Vec<Seg> {
    let mut out = Vec::new();
    for strand in strands() {
        let mut s0 = 0.0f32;
        for pair in strand.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            out.push(Seg { a, b, s0 });
            s0 += (b - a).length();
        }
    }
    out
}

/// Arc length of the trunk polyline, the reference for `t` lookups by arc length.
fn trunk_length() -> f32 {
    strands()[0].windows(2).map(|p| (p[1] - p[0]).length()).sum()
}

/// Tube radius at an arc length; the strand swells and narrows like a thread.
/// Seed-independent so the gravity field and the geometry always agree.
fn radius(s: f32) -> f32 {
    let r = 20.0 + 8.0 * (s * 0.1).sin();
    r.clamp(R_MIN, R_MAX)
}

/// Nearest centerline point, distance, arc length, radius and unit tangent.
fn nearest(p: Vec3) -> (Vec3, f32, f32, f32, Vec3) {
    let segs = segments();
    let mut best = (Vec3::ZERO, f32::INFINITY, 0.0f32, R_MAX, Vec3::Z);
    for seg in &segs {
        let ab = seg.b - seg.a;
        let len2 = ab.length_squared();
        let t = if len2 == 0.0 { 0.0 } else { ((p - seg.a).dot(ab) / len2).clamp(0.0, 1.0) };
        let c = seg.a + ab * t;
        let d = (p - c).length();
        if d < best.1 {
            let s = seg.s0 + (ab * t).length();
            best = (c, d, s, radius(s), (ab / len2.sqrt()).normalize_or_zero());
        }
    }
    best
}

/// Outward-from-the-strand gravity; zero on the axis and outside the admitted
/// region. Magnitude grows linearly to g_wall at the tube floor.
pub fn acceleration(point_m: Vec3) -> Vec3 {
    let (c, d, _, r, _) = nearest(point_m);
    if d > r + WALL + 2.0 {
        return Vec3::ZERO;
    }
    if d < 1e-3 {
        return Vec3::ZERO;
    }
    (point_m - c) * (G / r)
}

pub fn spawn(cfg: &WorldSampleConfig) -> Vec3 {
    let c = trunk(0.0);
    let dir = (c - Vec3::new(0.0, 0.0, c.z)).normalize_or_zero();
    let r = radius(0.0);
    ((c + dir * (r - 1.5)) / cfg.voxel_meters()).round()
}

pub fn column(cfg: &WorldSampleConfig, _x: i32, _z: i32) -> ColumnSample {
    // Bounding heightfield: the strand lives inside this shell; outside it the
    // sampler returns air via block(), so streaming never loads endless stone.
    let height = cfg.blocks_for_height(SPINE_Z0 + TURNS * PITCH + R_MAX + 4.0);
    ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone,
        height,
        surface: BlockId::Stone,
        filler: BlockId::Stone,
        vegetation_density: 0.0,
        cave: Default::default(),
    }
}

/// Warm guides seeded by the strand's arc length and wrapping angle.
pub fn guide_lights(cfg: &WorldSampleConfig) -> Vec<IVec3> {
    let mut out = Vec::new();
    for i in 0..24 {
        let s = i as f32 * 4.0;
        let angle = s * 0.35;
        // Arc length along the trunk, so the centreline matches radius(s).
        let t = (s / trunk_length()).clamp(0.0, 1.0);
        let c = trunk(t);
        let tt = trunk_tangent(t);
        let world_up = Vec3::Y;
        let e1 = (world_up - tt * world_up.dot(tt)).normalize_or_zero();
        let e1 = if e1.length_squared() < 1e-6 { Vec3::X } else { e1.normalize() };
        let e2 = tt.cross(e1).normalize_or(Vec3::X);
        let r = radius(s);
        let outward = e1 * angle.cos() + e2 * angle.sin();
        let pos = c + outward * (r - 0.6);
        out.push((pos / cfg.voxel_meters()).floor().as_ivec3());
    }
    out
}

pub fn block(cfg: &WorldSampleConfig, cell: IVec3) -> BlockId {
    let m = cfg.voxel_meters();
    let p = (cell.as_vec3() + Vec3::splat(0.5)) * m;
    let (c, d, s, r, t) = nearest(p);

    // Outside the strand there is open space.
    if d > r + WALL {
        return BlockId::Air;
    }
    // Solid crust; the outer shell reads like a strand's rind.
    if d > r {
        return if d > r + WALL - 0.5 { BlockId::Bedrock } else { BlockId::Stone };
    }
    let h = r - d;

    // A frame around the tangent so a wrapping band stays continuous in angle.
    let world_up = Vec3::Y;
    let e1 = (world_up - t * world_up.dot(t)).normalize_or_zero();
    let e1 = if e1.length_squared() < 1e-6 { Vec3::X } else { e1.normalize() };
    let e2 = t.cross(e1).normalize_or(Vec3::X);
    let radial = (p - c) / d.max(1e-4);
    let angle = radial.dot(e2).atan2(radial.dot(e1));
    let band = (angle - s * 0.35).rem_euclid(std::f32::consts::TAU);

    if h < 0.4 {
        return if band < 1.1 {
            BlockId::Sandstone
        } else if band < 1.7 {
            BlockId::Leaves
        } else {
            BlockId::Grass
        };
    }

    // Low maze partitions; sparse and seed-stable, aisles stay open.
    if h < 2.6 {
        let sx = (s / 6.0).floor() as i32;
        let ax = (angle.rem_euclid(std::f32::consts::TAU) / 0.55).floor() as i32;
        let hashed = hash_u32(cfg.seed ^ (sx as u32).wrapping_mul(0x9e37_79b9) ^ (ax as u32).wrapping_mul(0x85eb_ca6b));
        if hashed % 5 == 0 {
            return BlockId::StoneBricks;
        }
    }

    // Guide stones follow the wrapping band.
    let gx = (s / 10.0).floor() as i32;
    if band < 0.6 && h < 1.2 && gx.rem_euclid(4) == 0 {
        return BlockId::Crystal;
    }

    BlockId::Air
}

#[cfg(test)]
mod tests;