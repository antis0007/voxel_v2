//! Segment/voxel traversal in metres. Thin voxels and narrow conduits cannot be skipped.
use super::*;
fn box_entry(a: Vec3, d: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    for axis in 0..3 {
        if d[axis].abs() < 1e-8 {
            if a[axis] < min[axis] || a[axis] > max[axis] {
                return None;
            }
        } else {
            let x = (min[axis] - a[axis]) / d[axis];
            let y = (max[axis] - a[axis]) / d[axis];
            lo = lo.max(x.min(y));
            hi = hi.min(x.max(y));
        }
    }
    (hi >= lo).then_some(lo)
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
pub(super) fn solid(world: &World, a: Vec3, b: Vec3) -> Option<(Vec3, IVec3)> {
    let scale = world.voxel_scale.meters();
    let delta = b - a;
    if !a.is_finite() || !b.is_finite() {
        return Some((a, IVec3::ZERO));
    }
    let start = a / scale;
    let d = delta / scale;
    let mut cell = start.floor().as_ivec3();
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
    let count = (d.abs().element_sum().ceil() as usize + 4).min(65536);
    for _ in 0..count {
        if world.solid(cell) {
            let entry = if let Some(boxes) = factory::conduit_bounds(world, cell) {
                boxes
                    .into_iter()
                    .filter_map(|(min, max)| box_entry(start, d, min, max))
                    .min_by(f32::total_cmp)
            } else {
                box_entry(start, d, cell.as_vec3(), cell.as_vec3() + Vec3::ONE)
            };
            if let Some(t) = entry {
                let safe = (t - 0.002 / delta.length().max(0.002)).max(0.0);
                return Some((a + delta * safe, cell));
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
        cell[axis] += step[axis];
        next[axis] += stride[axis];
    }
    None
}
