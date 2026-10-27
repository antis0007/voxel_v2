use super::*;

/// Rough slope: the median and any-voxel rules disagree in many cells.
fn height(p: IVec3) -> i32 {
    8 + p.x.rem_euclid(5) * 3 + (p.z * 7).rem_euclid(11) + p.x.rem_euclid(64) / 3
}

fn terrain(coord: IVec3) -> GeneratedChunk {
    let mut chunk =
        GeneratedChunk::new(coord, vec![BlockId::Air; PADDED_CHUNK_SIZE.pow(3) as usize]);
    let origin = chunk.origin();
    for z in 0..PADDED_CHUNK_SIZE {
        for x in 0..PADDED_CHUNK_SIZE {
            for y in 0..PADDED_CHUNK_SIZE {
                let p = origin + IVec3::new(x, y, z);
                if p.y < height(p) {
                    chunk.set_world(p, BlockId::Stone);
                }
            }
        }
    }
    chunk
}

/// The chunk's own classification of one of its coarse cells.
fn own_solid(chunk: &GeneratedChunk, cell: IVec3, step: i32) -> bool {
    let tops = column_tops(chunk);
    let origin = chunk.chunk * CHUNK_SIZE;
    let n = CHUNK_SIZE / step;
    coarse_cell(chunk, origin + cell * step, step).is_some()
        && (terrain_reaches(&tops, cell.x * step, cell.z * step, step, cell.y * step)
            || (0..3).any(|axis| {
                let layer = match cell[axis] {
                    0 => 0,
                    c if c == n - 1 => CHUNK_SIZE - 1,
                    _ => return false,
                };
                seam_slab_sealed(chunk, origin + cell * step, step, axis, origin[axis] + layer)
            }))
}

/// Whether `mesh` has a face with this outward normal through `point`.
fn covers(mesh: &ChunkMesh, normal: Vec3, point: Vec3) -> bool {
    let axis = normal.abs().max_position();
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    mesh.indices.chunks_exact(3).any(|tri| {
        let p = [0, 1, 2].map(|k| Vec3::from_array(mesh.vertices[tri[k] as usize].pos));
        if Vec3::from_array(mesh.vertices[tri[0] as usize].normal).dot(normal) < 0.99
            || (p[0][axis] - point[axis]).abs() > 0.5
        {
            return false;
        }
        let cross = |a: Vec3, b: Vec3| {
            (b[u] - a[u]) * (point[v] - a[v]) - (b[v] - a[v]) * (point[u] - a[u])
        };
        let s = [cross(p[0], p[1]), cross(p[1], p[2]), cross(p[2], p[0])];
        s.iter().all(|&d| d >= 0.0) || s.iter().all(|&d| d <= 0.0)
    })
}

#[test]
fn coarse_seams_never_drop_a_wall_both_sides_skip() {
    for base in [IVec3::ZERO, IVec3::new(-3, 0, -5)] {
        for axis in 0..3 {
            let low = terrain(base);
            let high = terrain(base + IVec3::AXES[axis]);
            for lod in 1..=3u8 {
                let step = 1i32 << lod;
                let n = CHUNK_SIZE / step;
                let (low_mesh, high_mesh) = (coarsened_mesh(&low, lod), coarsened_mesh(&high, lod));
                let normal = Vec3::AXES[axis];
                let mut holes = Vec::new();
                for a in 0..n {
                    for b in 0..n {
                        let mut cell = IVec3::ZERO;
                        cell[(axis + 1) % 3] = a;
                        cell[(axis + 2) % 3] = b;
                        let mut low_cell = cell;
                        low_cell[axis] = n - 1;
                        let below = own_solid(&low, low_cell, step);
                        let above = own_solid(&high, cell, step);
                        let mut point = ((high.chunk * CHUNK_SIZE + cell * step).as_vec3()
                            + Vec3::splat(step as f32 * 0.5))
                            .to_array();
                        point[axis] = (high.chunk[axis] * CHUNK_SIZE) as f32;
                        let point = Vec3::from_array(point);
                        let sealed = match (below, above) {
                            (true, false) => covers(&low_mesh, normal, point),
                            (false, true) => covers(&high_mesh, -normal, point),
                            _ => true,
                        };
                        if !sealed {
                            holes.push(point);
                        }
                    }
                }
                assert!(
                    holes.is_empty(),
                    "base {base} axis {axis} lod {lod}: {} open seam cells, first {:?}",
                    holes.len(),
                    holes.first()
                );
            }
        }
    }
}
