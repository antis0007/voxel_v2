//! Conservative volumetric LOD. Only identical uniform cells collapse; mixed
//! cells recurse to voxels. Exposed coverage and material boundaries stay exact.
//! Distance reduces coplanar tint/AO tessellation, never solid/air topology.
use super::{
    ChunkMesh, FaceMaterial, GeneratedChunk, append_torch_mesh, emit_greedy_face, face_visible,
    mesh_block_at,
};
use crate::material::ALL_BLOCK_IDS;
use crate::world::{BlockId, CHUNK_SIZE};
use glam::IVec3;

const SIZE: usize = CHUNK_SIZE as usize;

#[derive(Clone, Copy)]
struct Cell {
    origin: [usize; 3],
    size: usize,
    block: BlockId,
}

struct Hierarchy {
    // None means mixed, not empty. Air is an explicit uniform value.
    levels: Vec<Vec<Option<BlockId>>>,
}

fn index(p: [usize; 3], size: usize) -> usize {
    (p[1] * size + p[2]) * size + p[0]
}

impl Hierarchy {
    fn new(chunk: &GeneratedChunk, max_level: usize) -> Self {
        let mut base = Vec::with_capacity(SIZE * SIZE * SIZE);
        for y in 0..SIZE {
            for z in 0..SIZE {
                for x in 0..SIZE {
                    base.push(Some(mesh_block_at(chunk, [x as i32, y as i32, z as i32])));
                }
            }
        }
        let mut levels = vec![base];
        for level in 1..=max_level {
            let size = SIZE >> level;
            let previous = &levels[level - 1];
            let mut cells = Vec::with_capacity(size * size * size);
            for y in 0..size {
                for z in 0..size {
                    for x in 0..size {
                        let first = previous[index([x * 2, y * 2, z * 2], size * 2)];
                        let mergeable = first.is_some_and(|b| {
                            !b.is_torch() && (b.is_empty() || b.is_opaque() || b.is_liquid())
                        });
                        let same = mergeable
                            && (0..2).all(|dy| {
                                (0..2).all(|dz| {
                                    (0..2).all(|dx| {
                                        previous
                                            [index([x * 2 + dx, y * 2 + dy, z * 2 + dz], size * 2)]
                                            == first
                                    })
                                })
                            });
                        cells.push(if same { first } else { None });
                    }
                }
            }
            levels.push(cells);
        }
        Self { levels }
    }

    fn visit(&self, level: usize, p: [usize; 3], out: &mut Vec<Cell>, torches: &mut bool) {
        if let Some(block) = self.levels[level][index(p, SIZE >> level)] {
            if block.is_torch() {
                *torches = true;
            } else if !block.is_empty() && !block.is_model() {
                let size = 1 << level;
                out.push(Cell {
                    origin: p.map(|v| v * size),
                    size,
                    block,
                });
            }
            return;
        }
        // Level zero is always a concrete source voxel.
        for y in 0..2 {
            for z in 0..2 {
                for x in 0..2 {
                    self.visit(
                        level - 1,
                        [p[0] * 2 + x, p[1] * 2 + y, p[2] * 2 + z],
                        out,
                        torches,
                    );
                }
            }
        }
    }
}

pub(super) fn build_adaptive_mesh(
    chunk: &GeneratedChunk,
    level: u8,
    mut mesh: ChunkMesh,
) -> ChunkMesh {
    let hierarchy = Hierarchy::new(chunk, level as usize);
    let mut cells = Vec::new();
    let mut torches = false;
    let roots = SIZE >> level;
    for y in 0..roots {
        for z in 0..roots {
            for x in 0..roots {
                hierarchy.visit(level as usize, [x, y, z], &mut cells, &mut torches);
            }
        }
    }

    // Mean of the material's period-two tint, rather than choosing an arbitrary
    // dominant block or deleting rare materials from a coarse cell.
    let materials: Vec<_> = ALL_BLOCK_IDS
        .iter()
        .map(|&block| {
            let mut color = [0.0f32; 3];
            for y in 0..2 {
                for z in 0..2 {
                    for x in 0..2 {
                        let sample = block.color(IVec3::new(x, y, z));
                        for channel in 0..3 {
                            color[channel] += sample[channel] * 0.125;
                        }
                    }
                }
            }
            FaceMaterial {
                block,
                color_bits: color.map(f32::to_bits),
            }
        })
        .collect();

    let mut mask = vec![None; SIZE * SIZE];
    // Bucket indices, not mask-sized copies, so scratch space is bounded by the
    // cells for one face direction. Empty voxel planes require no mask scan.
    let mut planes: Vec<Vec<usize>> = (0..=SIZE).map(|_| Vec::new()).collect();
    for axis in 0..3 {
        let u_axis = (axis + 1) % 3;
        let v_axis = (axis + 2) % 3;
        for positive in [false, true] {
            for plane in &mut planes {
                plane.clear();
            }
            for (i, cell) in cells.iter().enumerate() {
                if cell.block == crate::world::BlockId::Water {
                    continue;
                }
                let plane = cell.origin[axis] + if positive { cell.size } else { 0 };
                planes[plane].push(i);
            }
            for (plane, indices) in planes.iter().enumerate() {
                if indices.is_empty() {
                    continue;
                }
                mask.fill(None);
                for &i in indices {
                    let cell = cells[i];
                    for v in cell.origin[v_axis]..cell.origin[v_axis] + cell.size {
                        for u in cell.origin[u_axis]..cell.origin[u_axis] + cell.size {
                            let mut neighbor = [0i32; 3];
                            neighbor[axis] = plane as i32 - i32::from(!positive);
                            neighbor[u_axis] = u as i32;
                            neighbor[v_axis] = v as i32;
                            if !face_visible(cell.block, mesh_block_at(chunk, neighbor)) {
                                continue;
                            }
                            let material = if cell.block.is_opaque() || cell.block.is_liquid() {
                                materials[cell.block.as_u32() as usize]
                            } else {
                                let mut current = neighbor;
                                current[axis] += if positive { -1 } else { 1 };
                                FaceMaterial::from_block(
                                    cell.block,
                                    chunk.chunk * CHUNK_SIZE + IVec3::from_array(current),
                                )
                            };
                            mask[v * SIZE + u] = Some(material);
                        }
                    }
                }
                merge_face_mask(
                    &mut mesh,
                    chunk,
                    axis,
                    positive,
                    plane as i32,
                    &mut mask,
                    level,
                );
            }
        }
    }
    if torches {
        append_torch_mesh(&mut mesh, chunk, level);
    }
    super::append_water_mesh(&mut mesh, chunk);
    mesh
}

pub(super) fn merge_face_mask(
    mesh: &mut ChunkMesh,
    chunk: &GeneratedChunk,
    axis: usize,
    positive: bool,
    plane: i32,
    mask: &mut [Option<FaceMaterial>],
    _level: u8,
) {
    let mut index = 0;
    while index < SIZE * SIZE {
        let Some(material) = mask[index] else {
            index += 1;
            continue;
        };
        let u0 = index % SIZE;
        let v0 = index / SIZE;
        // Shared albedo does not need LOD-dependent tint subdivisions.
        let u_limit = SIZE;
        let v_limit = SIZE;
        let mut width = 1;
        while u0 + width < u_limit && mask[index + width] == Some(material) {
            width += 1;
        }
        let mut height = 1;
        'grow: while v0 + height < v_limit {
            let row = (v0 + height) * SIZE + u0;
            for offset in 0..width {
                if mask[row + offset] != Some(material) {
                    break 'grow;
                }
            }
            height += 1;
        }
        emit_greedy_face(
            mesh,
            chunk,
            axis,
            positive,
            plane,
            u0 as i32,
            v0 as i32,
            width as i32,
            height as i32,
            material,
        );
        for v in v0..v0 + height {
            mask[v * SIZE + u0..v * SIZE + u0 + width].fill(None);
        }
        index += width;
    }
}
