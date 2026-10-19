//! Shape boundaries, not sampled clouds of voxel wireframes. All positions are voxels.
use super::*;
use std::collections::HashMap;

/// Hovering a large brush never enumerates or samples its whole volume.
pub(super) fn volume(
    c: &CreativeState,
    scale: VoxelScalePreset,
    ray: RaycastTarget,
    mode: BrushMode,
    show_place: bool,
) -> PreviewResult {
    let center = if c.tool == ToolKind::ExplosiveWand {
        ray.hit
    } else if mode == BrushMode::Place && show_place {
        ray.place.or(ray.hit)
    } else {
        ray.hit.or(ray.place)
    };
    let color = if mode == BrushMode::Erase {
        [1.0, 0.38, 0.08, 1.0]
    } else {
        [0.3, 0.85, 1.0, 1.0]
    };
    let mut result = PreviewResult {
        blocks: Vec::new(),
        segments: Vec::new(),
        total_cells: 0,
        color,
        invert_blend: false,
        center_block: center,
    };
    let Some(center) = center else {
        return result;
    };
    let radius = c.radius_cells(
        match c.tool {
            ToolKind::ExplosiveWand => c.explosion_radius,
            ToolKind::AreaTool => c.area_tool.radius,
            _ => c.brush_radius,
        },
        scale,
    );
    let mut min = center - IVec3::splat(radius);
    let mut max = center + IVec3::splat(radius);
    let sphere = c.tool == ToolKind::ExplosiveWand
        || c.tool == ToolKind::Brush && c.brush_shape == BrushShape::Sphere;
    let mut count = 0;
    if c.tool == ToolKind::AreaTool {
        let (Some(hit), Some(place)) = (ray.hit, ray.place) else {
            return result;
        };
        let normal = place - hit;
        if normal == IVec3::ZERO {
            return result;
        }
        let axis = if normal.x != 0 {
            0
        } else if normal.y != 0 {
            1
        } else {
            2
        };
        let start = if mode == BrushMode::Place && show_place {
            place
        } else {
            hit
        };
        let depth = c.thickness_cells(c.area_tool.thickness, scale);
        let end = start + normal * (depth - 1) * if mode == BrushMode::Erase { -1 } else { 1 };
        min = start - IVec3::splat(radius);
        max = start + IVec3::splat(radius);
        min[axis] = start[axis].min(end[axis]);
        max[axis] = start[axis].max(end[axis]);
        for u in -radius..=radius {
            for v in -radius..=radius {
                if c.area_tool.shape == AreaFootprintShape::Square
                    || u * u + v * v <= radius * radius
                {
                    count += depth as usize;
                }
            }
        }
    } else if sphere {
        for x in -radius..=radius {
            for y in -radius..=radius {
                let n = radius * radius - x * x - y * y;
                if n >= 0 {
                    count += 2 * (n as f32).sqrt().floor() as usize + 1;
                }
            }
        }
    } else {
        count = (2 * radius + 1).pow(3) as usize;
    }
    result.total_cells = count;
    result.segments = boundary(c, scale, ray, &[min, max]);
    result
}

pub(super) fn boundary(
    c: &CreativeState,
    scale: VoxelScalePreset,
    ray: RaycastTarget,
    cells: &[IVec3],
) -> Vec<[Vec3; 2]> {
    if cells.is_empty() {
        return Vec::new();
    }
    if matches!(c.tool, ToolKind::BuildersWand | ToolKind::DestructorWand) {
        return contour(cells);
    }
    if c.tool == ToolKind::ExplosiveWand
        || c.tool == ToolKind::Brush && c.brush_shape == BrushShape::Sphere
    {
        let min = cells.iter().fold(cells[0], |a, &b| a.min(b));
        let max = cells.iter().fold(cells[0], |a, &b| a.max(b));
        let center = (min.as_vec3() + max.as_vec3() + Vec3::ONE) * 0.5;
        let r = (max.x - min.x + 1) as f32 * 0.5;
        let mut out = Vec::new();
        for axis in 0..3 {
            circle(&mut out, center, r, axis);
        }
        return out;
    }
    if c.tool == ToolKind::AreaTool && c.area_tool.shape == AreaFootprintShape::Circle {
        if let (Some(h), Some(p)) = (ray.hit, ray.place) {
            let n = p - h;
            let axis = if n.x != 0 {
                0
            } else if n.y != 0 {
                1
            } else {
                2
            };
            let min = cells.iter().fold(cells[0], |a, &b| a.min(b));
            let max = cells.iter().fold(cells[0], |a, &b| a.max(b));
            let center = (min.as_vec3() + max.as_vec3() + Vec3::ONE) * 0.5;
            let r = c.radius_cells(c.area_tool.radius, scale) as f32 + 0.5;
            let mut a = center;
            a[axis] = min[axis] as f32;
            let mut b = center;
            b[axis] = max[axis] as f32 + 1.0;
            let mut out = Vec::new();
            circle(&mut out, a, r, axis);
            circle(&mut out, b, r, axis);
            for j in 0..4 {
                let t = j as f32 * std::f32::consts::FRAC_PI_2;
                let mut d = Vec3::ZERO;
                d[(axis + 1) % 3] = t.cos() * r;
                d[(axis + 2) % 3] = t.sin() * r;
                out.push([a + d, b + d]);
            }
            return out;
        }
    }
    let min = cells.iter().fold(cells[0], |a, &b| a.min(b)).as_vec3();
    let max = cells.iter().fold(cells[0], |a, &b| a.max(b)).as_vec3() + Vec3::ONE;
    let mut out = Vec::new();
    for axis in 0..3 {
        for u in 0..2 {
            for v in 0..2 {
                let mut a = min;
                let b = (axis + 1) % 3;
                let d = (axis + 2) % 3;
                a[b] = if u == 0 { min[b] } else { max[b] };
                a[d] = if v == 0 { min[d] } else { max[d] };
                let mut end = a;
                end[axis] = max[axis];
                out.push([a, end]);
            }
        }
    }
    out
}
fn circle(out: &mut Vec<[Vec3; 2]>, center: Vec3, r: f32, axis: usize) {
    let point = |t: f32| {
        let mut p = center;
        p[(axis + 1) % 3] += t.cos() * r;
        p[(axis + 2) % 3] += t.sin() * r;
        p
    };
    for j in 0..64 {
        out.push([
            point(j as f32 * std::f32::consts::TAU / 64.0),
            point((j + 1) as f32 * std::f32::consts::TAU / 64.0),
        ]);
    }
}
fn contour(cells: &[IVec3]) -> Vec<[Vec3; 2]> {
    let occupied: HashSet<_> = cells.iter().copied().collect();
    let mut edges: HashMap<(IVec3, usize), u8> = HashMap::new();
    for &p in cells {
        for axis in 0..3 {
            for sign in [-1, 1] {
                let mut normal = IVec3::ZERO;
                normal[axis] = sign;
                if occupied.contains(&(p + normal)) {
                    continue;
                }
                let mut corner = p;
                if sign > 0 {
                    corner[axis] += 1;
                }
                for edge_axis in [(axis + 1) % 3, (axis + 2) % 3] {
                    let other = 3 - axis - edge_axis;
                    for side in 0..2 {
                        let mut origin = corner;
                        origin[other] += side;
                        *edges.entry((origin, edge_axis)).or_default() |=
                            1 << (axis * 2 + usize::from(sign > 0));
                    }
                }
            }
        }
    }
    // Coplanar shared edges have only one normal; silhouette corners have two.
    let mut lines: HashMap<(usize, i32, i32), Vec<i32>> = HashMap::new();
    for ((p, axis), normals) in edges {
        if normals.count_ones() > 1 {
            lines
                .entry((axis, p[(axis + 1) % 3], p[(axis + 2) % 3]))
                .or_default()
                .push(p[axis]);
        }
    }
    let mut out = Vec::new();
    for ((axis, u, v), mut runs) in lines {
        runs.sort_unstable();
        runs.dedup();
        let mut start = runs[0];
        let mut end = start + 1;
        let emit = |out: &mut Vec<[Vec3; 2]>, start: i32, end: i32| {
            let mut a = Vec3::ZERO;
            a[axis] = start as f32;
            a[(axis + 1) % 3] = u as f32;
            a[(axis + 2) % 3] = v as f32;
            let mut b = a;
            b[axis] = end as f32;
            out.push([a, b]);
        };
        for p in runs.into_iter().skip(1) {
            if p == end {
                end += 1;
            } else {
                emit(&mut out, start, end);
                start = p;
                end = p + 1;
            }
        }
        emit(&mut out, start, end);
    }
    out
}
