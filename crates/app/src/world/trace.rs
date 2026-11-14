//! Segment/voxel traversal in metres. Thin voxels and narrow conduits cannot be skipped.
//! This lives at the world level so combat, mechanisms and future gameplay share
//! exactly the same line-of-sight rules.
use super::*;
use glam::DVec3;
fn box_entry(a: DVec3, d: DVec3, min: DVec3, max: DVec3) -> Option<(f64, Vec3)> {
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    let mut normal = Vec3::ZERO;
    for axis in 0..3 {
        if d[axis].abs() < 1e-8 {
            if a[axis] < min[axis] || a[axis] > max[axis] {
                return None;
            }
        } else {
            let x = (min[axis] - a[axis]) / d[axis];
            let y = (max[axis] - a[axis]) / d[axis];
            let entry = x.min(y);
            if entry > lo || (entry == lo && normal == Vec3::ZERO) {
                lo = entry;
                normal = Vec3::ZERO;
                normal[axis] = -d[axis].signum() as f32;
            }
            hi = hi.min(x.max(y));
        }
    }
    (hi >= lo).then_some((lo, normal))
}
pub(crate) fn opaque_between(world: &World, a: Vec3, b: Vec3) -> bool {
    let scale = world.voxel_scale.meters().max(1e-6);
    if !a.is_finite() || !b.is_finite() {
        return true;
    }
    // The cylinder is opaque. A stair cube in the hollow is not.
    if shell_hit(world, a, b).is_some() {
        return true;
    }
    let start = a / scale;
    let end = b / scale;
    let d = end - start;
    if d.length() < 0.5 {
        return false;
    }
    let start_cell = start.floor().as_ivec3();
    let end_cell = end.floor().as_ivec3();
    if start_cell == end_cell {
        return false;
    }
    let mut cell = start_cell;
    let mut next = Vec3::splat(f32::INFINITY);
    let mut stride = next;
    let mut step = IVec3::ZERO;
    for axis in 0..3 {
        if d[axis].abs() > 1e-8 {
            step[axis] = if d[axis] > 0.0 { 1 } else { -1 };
            let edge = cell[axis] as f32 + if step[axis] > 0 { 1.0 } else { 0.0 };
            next[axis] = (edge - start[axis]) / d[axis];
            stride[axis] = d[axis].abs().recip();
        }
    }
    let count = (d.abs().element_sum().ceil() as usize + 4).min(96);
    for _ in 0..count {
        let axis = if next.x <= next.y && next.x <= next.z {
            0
        } else if next.y <= next.z {
            1
        } else {
            2
        };
        if next[axis] > 1.0 {
            break;
        }
        cell[axis] += step[axis];
        next[axis] += stride[axis];
        if cell == end_cell {
            return false;
        }
        if !shell_poke(world, a, b, cell, false) && world.get(cell).is_opaque() {
            return true;
        }
    }
    false
}

pub(crate) fn solid(world: &World, a: Vec3, b: Vec3) -> Option<(Vec3, IVec3)> {
    solid_contact(world, a, b).map(|hit| (hit.pos, hit.cell))
}

pub(crate) struct SolidContact {
    pub pos: Vec3,
    pub cell: IVec3,
    pub normal: Vec3,
}

pub(crate) fn solid_contact(world: &World, a: Vec3, b: Vec3) -> Option<SolidContact> {
    solid_contact_precise(world, a.as_dvec3(), b.as_dvec3())
}

pub(crate) fn solid_contact_precise(world: &World, a: DVec3, b: DVec3) -> Option<SolidContact> {
    let scale = world.voxel_scale.meters();
    let delta = b - a;
    if !a.is_finite() || !b.is_finite() {
        return Some(SolidContact { pos: a.as_vec3(), cell: IVec3::ZERO, normal: Vec3::Y });
    }
    let shell = shell_hit(world, a.as_vec3(), b.as_vec3());
    let start = a / f64::from(scale);
    let d = delta / f64::from(scale);
    let mut cell = start.floor().as_ivec3();
    let mut next = DVec3::splat(f64::INFINITY);
    let mut stride = next;
    let mut step = IVec3::ZERO;
    for axis in 0..3 {
        if d[axis].abs() > 1e-8 {
            step[axis] = if d[axis] > 0.0 { 1 } else { -1 };
            let edge = f64::from(cell[axis]) + if step[axis] > 0 { 1.0 } else { 0.0 };
            next[axis] = (edge - start[axis]) / d[axis];
            stride[axis] = d[axis].abs().recip();
        }
    }
    let count = (d.abs().element_sum().ceil() as usize + 4).min(65536);
    let mut entered = 0.0f64;
    for _ in 0..count {
        if let Some((tc, cell_s, _)) = shell {
            if entered >= f64::from(tc) - 1e-5 {
                return Some(shell_contact(world, a.as_vec3(), delta.as_vec3(), tc, cell_s));
            }
        }
        if world.solid(cell) && !shell_poke(world, a.as_vec3(), b.as_vec3(), cell, shell.is_some()) {
            let fitting=engine_content::fittings::state(world.get(cell)).is_some();
            let boxes=if fitting { Some(engine_content::fittings::boxes(world.get(cell),cell,scale)) }
                else { factory::conduit_bounds(world,cell) };
            let entry = if let Some(boxes) = boxes {
                boxes
                    .into_iter()
                    .filter_map(|(min, max)| box_entry(start, d, min.as_dvec3(), max.as_dvec3()))
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            } else {
                box_entry(start, d, cell.as_dvec3(), cell.as_dvec3() + DVec3::ONE)
            };
            if let Some((t, normal)) = entry {
                if let Some((tc, cell_s, _)) = shell {
                    if f64::from(tc) <= t {
                        return Some(shell_contact(world, a.as_vec3(), delta.as_vec3(), tc, cell_s));
                    }
                }
                return Some(SolidContact {
                    pos: (a + delta * (t - 0.002 / delta.length().max(0.002)).max(0.0)).as_vec3(), cell,
                    normal: if normal == Vec3::ZERO { (-delta).as_vec3().normalize_or(Vec3::Y) } else { normal },
                });
            }
        }
        let axis = if next.x <= next.y && next.x <= next.z {
            0
        } else if next.y <= next.z {
            1
        } else {
            2
        };
        if next[axis] > 1.0 {
            break;
        }
        let boundary = next[axis];
        cell[axis] += step[axis];
        next[axis] += stride[axis];
        entered = boundary;
    }
    shell.map(|(tc, cell_s, _)| shell_contact(world, a.as_vec3(), delta.as_vec3(), tc, cell_s))
}

fn shell_contact(world: &World, a: Vec3, delta: Vec3, t: f32, cell: IVec3) -> SolidContact {
    let mut normal = (-delta).normalize_or(Vec3::Y);
    if let Some(curve) = active_curve(world) {
        let relative = a + delta * t - curve.center;
        let radial = Vec3::new(relative.x, relative.y, 0.0).normalize_or(Vec3::Y);
        let slope = (curve.radius(relative.z + 0.25) - curve.radius(relative.z - 0.25)) * 2.0;
        normal = (radial - Vec3::Z * slope).normalize_or(radial);
        if normal.dot(delta) > 0.0 {
            normal = -normal;
        }
    }
    SolidContact { pos: approach(a, delta, t), cell, normal }
}

fn approach(a: Vec3, delta: Vec3, t: f32) -> Vec3 {
    let safe = (t - 0.002 / delta.length().max(0.002)).max(0.0);
    a + delta * safe
}

fn active_curve(world: &World) -> Option<engine_worldgen::manyfold::ShellCurve> {
    (world.world_type == WorldType::Manyfold)
        .then(|| engine_worldgen::manyfold::shell_curve(world.generator_version))
        .flatten()
}

/// Analytic shell hit: fraction along `a`→`b`, the crust cell, and the inward neighbour.
pub(crate) fn shell_hit(world: &World, a: Vec3, b: Vec3) -> Option<(f32, IVec3, IVec3)> {
    let curve = active_curve(world)?;
    let t = curve.crossing(a, b)?;
    let point = a + (b - a) * t;
    let cell = shell_owner(world, curve, point)?;
    let face = inward_face(curve, cell, world.voxel_scale.meters());
    Some((t, cell, face))
}

fn shell_owner(
    world: &World,
    curve: engine_worldgen::manyfold::ShellCurve,
    point: Vec3,
) -> Option<IVec3> {
    let scale = world.voxel_scale.meters();
    let rel = point - curve.center;
    let radial = Vec3::new(rel.x, rel.y, 0.0);
    let r = radial.length();
    if r < 0.2 {
        return None;
    }
    let dir = radial / r;
    let origin = Vec3::new(curve.center.x, curve.center.y, point.z);
    let mut dist = r - 0.05;
    let end = r + curve.crust + 0.75;
    while dist <= end {
        let q = origin + dir * dist;
        let cell = (q / scale).floor().as_ivec3();
        if world.solid(cell) {
            let centre = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
            if curve.crust(centre) {
                return Some(cell);
            }
            if curve.offset(centre) > 0.0 {
                return None;
            }
        }
        dist += (scale * 0.5).max(0.05);
    }
    None
}

fn inward_face(curve: engine_worldgen::manyfold::ShellCurve, cell: IVec3, scale: f32) -> IVec3 {
    let centre = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
    let rel = Vec3::new(centre.x - curve.center.x, centre.y - curve.center.y, 0.0);
    let mut face = cell;
    if rel.x.abs() >= rel.y.abs() {
        if rel.x > 0.0 {
            face.x -= 1;
        } else if rel.x < 0.0 {
            face.x += 1;
        }
    } else if rel.y > 0.0 {
        face.y -= 1;
    } else if rel.y < 0.0 {
        face.y += 1;
    }
    face
}

/// Crust stair in the hollow. `analytic` means the cylinder already owns this ray.
pub(crate) fn shell_poke(world: &World, a: Vec3, b: Vec3, cell: IVec3, analytic: bool) -> bool {
    let Some(curve) = active_curve(world) else {
        return false;
    };
    let scale = world.voxel_scale.meters();
    let centre = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
    if !curve.crust(centre) {
        return false;
    }
    if analytic {
        return true;
    }
    let lo = cell.as_vec3() * scale;
    let Some((t0, t1)) = segment_box_t(a, b, lo, lo + Vec3::splat(scale)) else {
        return false;
    };
    let mut peak = f32::MIN;
    for step in 0..5 {
        let t = t0 + (t1 - t0) * (step as f32 / 4.0);
        peak = peak.max(curve.offset(a + (b - a) * t));
    }
    peak < -0.05
}

fn segment_box_t(a: Vec3, b: Vec3, lo: Vec3, hi: Vec3) -> Option<(f32, f32)> {
    let d = b - a;
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;
    for axis in 0..3 {
        if d[axis].abs() < 1e-8 {
            if a[axis] < lo[axis] || a[axis] > hi[axis] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d[axis];
        let (s0, s1) = {
            let s0 = (lo[axis] - a[axis]) * inv;
            let s1 = (hi[axis] - a[axis]) * inv;
            if s0 > s1 { (s1, s0) } else { (s0, s1) }
        };
        t0 = t0.max(s0);
        t1 = t1.min(s1);
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, t1))
}

#[cfg(test)]
mod acceptance {
    use super::*;
    use crate::{
        fluid::VoxelScalePreset,
        world::{BlockId, WorldType},
    };
    #[test]
    fn wand_trace_hits_single_voxel_walls_from_both_sides_at_every_scale() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let mut w = World::with_seed(WorldType::Superflat, scale, 17);
            let cell = IVec3::new(0, (150.0 / scale.meters()) as i32, 0);
            w.set(cell, BlockId::Stone);
            let center = (cell.as_vec3() + Vec3::splat(0.5)) * scale.meters();
            for sign in [-1.0, 1.0] {
                let a = center + Vec3::X * sign * (0.5 * scale.meters() + 0.005);
                let b = center - Vec3::X * sign * 3.0;
                let (impact, hit) = solid(&w, a, b).expect("thin wall must stop bolt");
                assert_eq!(hit, cell);
                assert!((impact.x - center.x) * sign >= 0.5 * scale.meters());
            }
        }
    }
}
