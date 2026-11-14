//! Full source edges around adaptive tiles. Interiors retain their selected LOD.
use super::*;
use std::collections::HashMap;

fn average_color(values: [f32; 3]) -> f32 {
    let linear = values.map(|v| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    let mean = linear.into_iter().sum::<f32>() / 3.0;
    if mean <= 0.0031308 {
        12.92 * mean
    } else {
        1.055 * mean.powf(1.0 / 2.4) - 0.055
    }
}

pub(super) fn push(
    mesh: &mut ChunkMesh,
    cfg: &WorldSampleConfig,
    place: Place,
    center: (f32, f32),
    scale: f32,
    s: &Sampled,
    drawn: Drawn,
    half_edges: [bool; 4],
) {
    let Drawn {
        level,
        skirted,
        drop,
    } = drawn;
    let base = mesh.vertices.len();
    push_tile_vertices(mesh, place, center, scale, s, level);
    let q = 1usize << level;
    let m = tile::CELLS / q;
    let end = (2 * tile::CELLS) as i32;
    let common_normal = |at: [i32; 2]| -> Option<[f32; 3]> {
        let touches = [at[1] == 0, at[1] == end, at[0] == 0, at[0] == end];
        let outer = [
            place.tz == 0,
            place.tz + 1 == TILES,
            place.tx == 0,
            place.tx + 1 == TILES,
        ];
        let interface = (0..4).any(|side| {
            touches[side] && (half_edges[side] || (outer[side] && place.ring + 1 < RINGS.len()))
        });
        if !interface {
            return None;
        }
        let cell = RINGS[place.ring].1;
        let (x0, z0) = place.origin(center);
        let x = (x0 + at[0] as f32 * 0.5 * cell) / scale;
        let z = (z0 + at[1] as f32 * 0.5 * cell) / scale;
        let step = RINGS[0].1 / scale;
        let height = |x: f32, z: f32| distant_surface(cfg, x.floor() as i32, z.floor() as i32).0;
        let dx = (height(x + step, z) - height(x - step, z)) / (2.0 * step);
        let dz = (height(x, z + step) - height(x, z - step)) / (2.0 * step);
        Some(Vec3::new(-dx, 1.0, -dz).normalize_or_zero().to_array())
    };
    for z in 0..=m {
        for x in 0..=m {
            if x != 0 && x != m && z != 0 && z != m { continue; }
            let at = [(2 * x * q) as i32, (2 * z * q) as i32];
            let id = base + z * (m + 1) + x;
            if let Some(normal) = common_normal(at) {
                mesh.vertices[id].normal = normal;
            }
        }
    }
    // Only supplemental edge knots need a lookup. The tile vertices are
    // already laid out in row order at exact multiples of this stride.
    let mut knots = HashMap::new();
    let stride = (2 * q) as i32;
    let mut vertex = |mesh: &mut ChunkMesh, at: [i32; 2]| -> u32 {
        if at[0] % stride == 0 && at[1] % stride == 0 {
            return (base + (at[1] / stride) as usize * (m + 1) + (at[0] / stride) as usize) as u32;
        }
        if let Some(&id) = knots.get(&at) {
            return id;
        }
        let cell = RINGS[place.ring].1;
        let (x0, z0) = place.origin(center);
        let x = (x0 + at[0] as f32 * 0.5 * cell) / scale;
        let z = (z0 + at[1] as f32 * 0.5 * cell) / scale;
        let (i, k) = (at[0] as usize / 2, at[1] as usize / 2);
        let (h, block) = if at[0] % 2 == 0 && at[1] % 2 == 0 {
            (
                s.heights[(k + 1) * BORDERED + i + 1],
                s.blocks[k * tile::SIDE + i],
            )
        } else {
            // Only inner-ring interfaces need these additional source samples.
            // Both owners evaluate the identical world column, never an average
            // of different terrain heights or an averaged material identifier.
            distant_surface(cfg, x.floor() as i32, z.floor() as i32)
        };
        let material = block.surface();
        let id = mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex3 {
            pos: [x, h, z],
            color: block.color(IVec3::new(x.floor() as i32, h as i32, z.floor() as i32)),
            normal: common_normal(at)
                .unwrap_or_else(|| normal_at(&s.heights, i, k, cell / scale).to_array()),
            surface: [
                material.roughness,
                material.specular,
                0.0,
                material.shader_key(),
            ],
            ao_strength: -1.0,
        });
        knots.insert(at, id);
        id
    };
    let edge = |a: [i32; 2], b: [i32; 2]| {
        if a[1] == 0 && b[1] == 0 {
            Some((0, 0))
        } else if a[1] == end && b[1] == end {
            Some((1, 0))
        } else if a[0] == 0 && b[0] == 0 {
            Some((2, 1))
        } else if a[0] == end && b[0] == end {
            Some((3, 1))
        } else {
            None
        }
    };
    let mut polygon = Vec::new();
    for z in 0..m {
        for x in 0..m {
            if x > 0 && x + 1 < m && z > 0 && z + 1 < m {
                let a = (base + z * (m + 1) + x) as u32;
                let c = a + (m + 1) as u32;
                mesh.indices.extend_from_slice(&[a, c, c + 1, a, c + 1, a + 1]);
                continue;
            }
            let a = [(2 * x * q) as i32, (2 * z * q) as i32];
            let b = [a[0] + (2 * q) as i32, a[1]];
            let c = [a[0], a[1] + (2 * q) as i32];
            let d = [b[0], c[1]];
            for tri in [[a, c, d], [a, d, b]] {
                let original = tri.map(|p| vertex(mesh, p));
                polygon.clear();
                for n in 0..3 {
                    let a = tri[n];
                    let b = tri[(n + 1) % 3];
                    polygon.push(original[n]);
                    let Some((side, axis)) = edge(a, b) else {
                        continue;
                    };
                    let step = if half_edges[side] { 1 } else { 2 };
                    let sign = (b[axis] - a[axis]).signum();
                    let mut at = a;
                    at[axis] += sign * step;
                    while at[axis] != b[axis] {
                        polygon.push(vertex(mesh, at));
                        at[axis] += sign * step;
                    }
                }
                if polygon.len() == 3 {
                    mesh.indices.extend(original);
                    continue;
                }
                let parents = original.map(|i| mesh.vertices[i as usize]);
                let mut middle = parents[0];
                for axis in 0..3 {
                    middle.pos[axis] = parents.iter().map(|v| v.pos[axis]).sum::<f32>() / 3.0;
                    middle.color[axis] = average_color(parents.map(|v| v.color[axis]));
                    middle.normal[axis] = parents.iter().map(|v| v.normal[axis]).sum::<f32>();
                }
                middle.normal = Vec3::from_array(middle.normal)
                    .normalize_or_zero()
                    .to_array();
                let id = mesh.vertices.len() as u32;
                mesh.vertices.push(middle);
                for n in 0..polygon.len() {
                    mesh.indices
                        .extend([id, polygon[n], polygon[(n + 1) % polygon.len()]]);
                }
            }
        }
    }
    // Curtains remain only a guard at the outer/clipped domain, with their top
    // following the same stitched edge rather than a detached coarse chord.
    for (side, normal) in [-Vec3::Z, Vec3::Z, -Vec3::X, Vec3::X]
        .into_iter()
        .enumerate()
    {
        if !skirted[side] {
            continue;
        }
        let step = if half_edges[side] { 1 } else { 2 };
        let mut ids = Vec::new();
        for n in (0..=end).step_by(step) {
            let at = match side {
                0 => [n, 0],
                1 => [n, end],
                2 => [0, n],
                _ => [end, n],
            };
            ids.push(vertex(mesh, at));
        }
        for pair in ids.windows(2) {
            push_skirt(mesh, [pair[0], pair[1]], normal, drop);
        }
    }
}
