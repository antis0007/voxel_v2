mod boundary;
mod memory;
pub mod models;
pub use memory::{MeshPlan, PLANNING_BYTES};
mod ground_detail;
mod solid;
use glam::{IVec3, Vec3};

use crate::world::{BlockId, CHUNK_SIZE};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex3 {
    pub pos: [f32; 3],
    pub color: [f32; 3],
    pub normal: [f32; 3],
    pub surface: [f32; 4],
    pub ao_strength: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub color: [f32; 3],
}

pub struct ChunkMesh {
    pub vertices: Vec<Vertex3>,
    pub indices: Vec<u32>,
}

pub const CHUNK_PAD: i32 = 1;
pub const PADDED_CHUNK_SIZE: i32 = CHUNK_SIZE + 2; // 34
/// Vertical inset (in voxel units) keeping free water surfaces recessed below
/// the top edge of surrounding bank and shoreline blocks.
pub const WATER_SURFACE_INSET: f32 = 0.135;

impl ChunkMesh {
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }
}

/// Cubic chunk plus an exact one-voxel halo shared by every mesh LOD.
#[derive(Clone)]
pub struct GeneratedChunk {
    pub voxel_meters: f32,
    pub water_levels: std::collections::HashMap<IVec3, u8>,
    pub chunk: IVec3,
    pub blocks: Vec<BlockId>,
    /// Highest opaque cell per padded column; generated terrain supplies roofs
    /// above this chunk too. Lighting metadata, never collision geometry.
    pub sky_ceiling: Vec<i32>,
}

impl GeneratedChunk {
    /// Conservative allocation estimate including hash buckets and control bytes.
    pub fn allocation_bytes(&self) -> usize {
        self.blocks.capacity() * std::mem::size_of::<BlockId>()
            + self.sky_ceiling.capacity() * std::mem::size_of::<i32>()
            + self.water_levels.capacity() * (std::mem::size_of::<(IVec3, u8)>() + 1) * 2
    }

    pub fn new(chunk: IVec3, blocks: Vec<BlockId>) -> Self {
        Self {
            voxel_meters: 1.0,
            chunk,
            blocks,
            sky_ceiling: Vec::new(),
            water_levels: Default::default(),
        }
    }

    pub fn origin(&self) -> IVec3 {
        self.chunk * CHUNK_SIZE - IVec3::splat(CHUNK_PAD)
    }

    fn local_index(x: i32, y: i32, z: i32) -> usize {
        ((y * PADDED_CHUNK_SIZE + z) * PADDED_CHUNK_SIZE + x) as usize
    }

    pub fn get_world(&self, p: IVec3) -> BlockId {
        let local = p - self.origin();
        if local.min_element() < 0 || local.max_element() >= PADDED_CHUNK_SIZE {
            return BlockId::Air;
        }
        self.blocks[Self::local_index(local.x, local.y, local.z)]
    }

    pub fn set_world(&mut self, p: IVec3, block: BlockId) {
        let local = p - self.origin();
        if local.min_element() >= 0 && local.max_element() < PADDED_CHUNK_SIZE {
            self.blocks[Self::local_index(local.x, local.y, local.z)] = block;
            if !self.sky_ceiling.is_empty() {
                let column = (local.z * PADDED_CHUNK_SIZE + local.x) as usize;
                if Self::sky_occluder(block) {
                    self.sky_ceiling[column] = self.sky_ceiling[column].max(p.y);
                } else if self.sky_ceiling[column] == p.y {
                    self.sky_ceiling[column] = self.local_roof(local.x, local.z);
                }
            }
        }
    }

    pub fn local_roof(&self, x: i32, z: i32) -> i32 {
        (0..PADDED_CHUNK_SIZE)
            .rev()
            .find(|&y| Self::sky_occluder(self.blocks[Self::local_index(x, y, z)]))
            .map_or(self.origin().y - 1, |y| self.origin().y + y)
    }

    fn sky_occluder(block: BlockId) -> bool {
        block.is_opaque() && !matches!(block, BlockId::Leaves | BlockId::PineLeaves | BlockId::Bush)
    }

    pub fn sky_visibility(&self, pos: Vec3, normal: Vec3) -> f32 {
        if self.sky_ceiling.is_empty() {
            return 1.0;
        }
        let air = pos + normal * 0.55;
        let p = (air.floor().as_ivec3() - self.origin())
            .clamp(IVec3::ZERO, IVec3::splat(PADDED_CHUNK_SIZE - 1));
        let roof = self.sky_ceiling[(p.z * PADDED_CHUNK_SIZE + p.x) as usize] as f32;
        let cover_m = ((roof + 0.5 - air.y) * self.voxel_meters).max(0.0);
        // Approximate diffuse light penetration, independent of voxel scale.
        (-(cover_m * 0.48)).exp().max(0.006)
    }

    pub fn water_optics(&self, pos: Vec3, normal: Vec3) -> (f32, [f32; 3]) {
        let inside = pos - normal * 0.01;
        let mut depth = 0.0;
        let mut water_samples = 0;
        let mut solid_shore_samples = 0;
        let mut bed_color = Vec3::ZERO;
        // Sample all 4 cells around a shared vertex corner. Including adjacent
        // solid beach cells as shallow shore margins lets shoreline vertices
        // smoothly approach 0.15m depth right at the water's edge.
        for dx in [-0.125, 0.125] {
            for dz in [-0.125, 0.125] {
                let mut p = (inside + Vec3::new(dx, 0.0, dz)).floor().as_ivec3();
                let top_block = self.get_world(p);
                if top_block != BlockId::Water {
                    if top_block.is_opaque() {
                        solid_shore_samples += 1;
                        bed_color += Vec3::from_array(top_block.color(p));
                    }
                    continue;
                }
                let mut cells = self.water_levels.get(&p).copied().unwrap_or(8) as f32 / 8.0;
                p.y -= 1;
                while p.y >= self.origin().y && self.get_world(p) == BlockId::Water {
                    cells += 1.0;
                    p.y -= 1;
                }
                let meters = cells * self.voxel_meters;
                let bed = self.get_world(p);
                // Reuse the existing depth walk. Unknown beds beneath this
                // source halo are deep enough that their tint is attenuated.
                let bed = if bed.is_opaque() { bed } else { BlockId::Sand };
                bed_color += Vec3::from_array(bed.color(p));
                depth += if p.y < self.origin().y {
                    meters.max(10.0)
                } else {
                    meters
                };
                water_samples += 1;
            }
        }
        if water_samples == 0 {
            (10.0, BlockId::Sand.color(inside.floor().as_ivec3()))
        } else {
            let total = (water_samples + solid_shore_samples) as f32;
            let mut avg_depth =
                (depth + solid_shore_samples as f32 * (0.08 * self.voxel_meters)) / total;
            if solid_shore_samples > 0 {
                let shore_cap = (0.12 + 0.11 * water_samples as f32) * self.voxel_meters;
                avg_depth = avg_depth.min(shore_cap);
            } else if water_samples < 4 {
                avg_depth = avg_depth.min(0.18 * self.voxel_meters);
            }
            (avg_depth, (bed_color / total).to_array())
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct FaceMaterial {
    block: BlockId,
    color_bits: [u32; 3],
    water_depth_bucket: u8,
}

impl FaceMaterial {
    fn from_block(block: BlockId, world_pos: IVec3) -> Self {
        Self::from_block_with_water_bucket(block, world_pos, 0)
    }

    fn from_block_with_water_bucket(
        block: BlockId,
        world_pos: IVec3,
        water_depth_bucket: u8,
    ) -> Self {
        // Keep base albedo identical across LODs. Period-two voxel tint used to
        // split every near face and then disappear abruptly in distant meshes.
        // World-space shader detail supplies the fine surface variation.
        static COLORS: std::sync::OnceLock<Vec<[f32; 3]>> = std::sync::OnceLock::new();
        let color = if block.is_opaque() || block.is_liquid() {
            COLORS.get_or_init(|| {
                crate::material::ALL_BLOCK_IDS
                    .iter()
                    .map(|&id| {
                        let mut mean = [0.0; 3];
                        for y in 0..2 {
                            for z in 0..2 {
                                for x in 0..2 {
                                    let sample = id.color(IVec3::new(x, y, z));
                                    for c in 0..3 {
                                        mean[c] += sample[c] * 0.125;
                                    }
                                }
                            }
                        }
                        mean
                    })
                    .collect()
            })[block.as_u32() as usize]
        } else {
            block.color(world_pos)
        };
        Self {
            block,
            color_bits: [color[0].to_bits(), color[1].to_bits(), color[2].to_bits()],
            water_depth_bucket,
        }
    }

    fn color(self) -> [f32; 3] {
        [
            f32::from_bits(self.color_bits[0]),
            f32::from_bits(self.color_bits[1]),
            f32::from_bits(self.color_bits[2]),
        ]
    }
}

fn face_visible(block: BlockId, neighbor: BlockId) -> bool {
    if block.is_empty() || block.is_model() {
        return false;
    }

    if block.is_liquid() {
        block != neighbor && !neighbor.is_solid()
    } else {
        !neighbor.is_opaque()
    }
}

fn world_pos_from_mesh_local(chunk_data: &GeneratedChunk, local: [i32; 3]) -> IVec3 {
    IVec3::new(
        chunk_data.chunk.x * CHUNK_SIZE + local[0],
        chunk_data.chunk.y * CHUNK_SIZE + local[1],
        chunk_data.chunk.z * CHUNK_SIZE + local[2],
    )
}

fn mesh_block_at(chunk_data: &GeneratedChunk, local: [i32; 3]) -> BlockId {
    chunk_data.get_world(world_pos_from_mesh_local(chunk_data, local))
}

fn add_axes(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn mesh_vertex(chunk_data: &GeneratedChunk, local: [i32; 3]) -> [f32; 3] {
    let p = world_pos_from_mesh_local(chunk_data, local);
    [p.x as f32, p.y as f32, p.z as f32]
}

fn face_normal_from_axis(axis: usize, positive: bool) -> [f32; 3] {
    let sign = if positive { 1.0 } else { -1.0 };
    match axis {
        0 => [sign, 0.0, 0.0],
        1 => [0.0, sign, 0.0],
        2 => [0.0, 0.0, sign],
        _ => unreachable!(),
    }
}

fn normal_from_quad(quad: [[f32; 3]; 4]) -> [f32; 3] {
    let a = Vec3::from_array(quad[0]);
    let b = Vec3::from_array(quad[1]);
    let c = Vec3::from_array(quad[2]);
    (b - a).cross(c - a).normalize_or_zero().to_array()
}

fn emit_greedy_face(
    mesh: &mut ChunkMesh,
    chunk_data: &GeneratedChunk,
    axis: usize,
    positive: bool,
    plane: i32,
    u0: i32,
    v0: i32,
    width: i32,
    height: i32,
    material: FaceMaterial,
) {
    let u_axis = (axis + 1) % 3;
    let v_axis = (axis + 2) % 3;

    let mut base = [0; 3];
    base[axis] = plane;
    base[u_axis] = u0;
    base[v_axis] = v0;

    let mut du = [0; 3];
    du[u_axis] = width;

    let mut dv = [0; 3];
    dv[v_axis] = height;

    let mut a = mesh_vertex(chunk_data, base);
    let mut b = mesh_vertex(chunk_data, add_axes(base, du));
    let mut c = mesh_vertex(chunk_data, add_axes(add_axes(base, du), dv));
    let mut d = mesh_vertex(chunk_data, add_axes(base, dv));

    if material.block == BlockId::Water {
        if axis == 1 && positive {
            for p in [&mut a, &mut b, &mut c, &mut d] {
                p[1] -= WATER_SURFACE_INSET;
            }
        } else if axis != 1 {
            let mut top_cell = base;
            top_cell[axis] = if positive { plane - 1 } else { plane };
            top_cell[1] += if u_axis == 1 { width - 1 } else { height - 1 };
            let mut above = top_cell;
            above[1] += 1;
            if mesh_block_at(chunk_data, above) != BlockId::Water {
                if u_axis == 1 {
                    b[1] -= WATER_SURFACE_INSET;
                    c[1] -= WATER_SURFACE_INSET;
                } else {
                    c[1] -= WATER_SURFACE_INSET;
                    d[1] -= WATER_SURFACE_INSET;
                }
            }
        }
    }

    let quad = if positive { [a, b, c, d] } else { [d, c, b, a] };
    let corner_ao = face_corner_ao(chunk_data, axis, positive, plane, u0, v0, width, height);

    let border = |coordinate: i32| coordinate == 0 || coordinate == CHUNK_SIZE;
    let steps = [
        if border(v0) { width } else { 1 },
        if border(u0 + width) { height } else { 1 },
        if border(v0 + height) { width } else { 1 },
        if border(u0) { height } else { 1 },
    ];
    let locals = [
        base,
        add_axes(base, du),
        add_axes(add_axes(base, du), dv),
        add_axes(base, dv),
    ];
    let (locals, steps) = if positive {
        (locals, steps)
    } else {
        (
            [locals[3], locals[2], locals[1], locals[0]],
            [steps[2], steps[1], steps[0], steps[3]],
        )
    };
    if boundary::emit_conforming_face(
        mesh,
        chunk_data,
        locals,
        steps,
        material,
        face_normal_from_axis(axis, positive),
        corner_ao,
    ) {
        return;
    }

    push_face(
        &mut mesh.vertices,
        &mut mesh.indices,
        quad,
        material.color(),
        face_normal_from_axis(axis, positive),
        material.block.surface(),
        corner_ao,
    );
}

fn face_corner_ao(
    chunk_data: &GeneratedChunk,
    axis: usize,
    positive: bool,
    plane: i32,
    u0: i32,
    v0: i32,
    width: i32,
    height: i32,
) -> [f32; 4] {
    let u_axis = (axis + 1) % 3;
    let v_axis = (axis + 2) % 3;
    // Occluders sit on the air side of the face. Sampling the solid layer
    // underneath a flat surface falsely occludes it and exposes greedy seams.
    let outside_axis = if positive { plane } else { plane - 1 };

    let sample = |u_positive: bool, v_positive: bool| {
        let inside_u = if u_positive { u0 + width - 1 } else { u0 };
        let inside_v = if v_positive { v0 + height - 1 } else { v0 };
        let outside_u = if u_positive { u0 + width } else { u0 - 1 };
        let outside_v = if v_positive { v0 + height } else { v0 - 1 };

        let mut side_u = [0; 3];
        side_u[axis] = outside_axis;
        side_u[u_axis] = outside_u;
        side_u[v_axis] = inside_v;

        let mut side_v = [0; 3];
        side_v[axis] = outside_axis;
        side_v[u_axis] = inside_u;
        side_v[v_axis] = outside_v;

        let mut corner = [0; 3];
        corner[axis] = outside_axis;
        corner[u_axis] = outside_u;
        corner[v_axis] = outside_v;

        let side_u_occ = mesh_block_at(chunk_data, side_u).is_opaque() as i32;
        let side_v_occ = mesh_block_at(chunk_data, side_v).is_opaque() as i32;
        let corner_occ = mesh_block_at(chunk_data, corner).is_opaque() as i32;
        let occ = if side_u_occ == 1 && side_v_occ == 1 {
            3
        } else {
            side_u_occ + side_v_occ + corner_occ
        };
        0.35 + 0.65 * (1.0 - occ as f32 / 3.0)
    };

    let ao = [
        sample(false, false),
        sample(true, false),
        sample(true, true),
        sample(false, true),
    ];

    if positive {
        ao
    } else {
        [ao[3], ao[2], ao[1], ao[0]]
    }
}

fn push_face(
    vertices: &mut Vec<Vertex3>,
    indices: &mut Vec<u32>,
    quad: [[f32; 3]; 4],
    color: [f32; 3],
    normal: [f32; 3],
    surface: crate::material::SurfaceDetail,
    corner_ao: [f32; 4],
) {
    let base = vertices.len() as u32;
    for (index, p) in quad.into_iter().enumerate() {
        let ao = 1.0 - (1.0 - corner_ao[index]) * surface.ao_strength;
        vertices.push(Vertex3 {
            pos: p,
            color,
            normal,
            surface: [
                surface.roughness,
                surface.specular,
                surface.shader_key(),
                surface.detail_strength,
            ],
            ao_strength: ao,
        });
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Leaves and wood are thin, porous volumes. Max-filtering them like terrain
/// turned a one-voxel trunk into an 8 m pillar and a crown into stacked cubes,
/// and the halves of a crown on either side of a LOD change no longer matched.
fn coarse_vegetation(block: BlockId) -> bool {
    matches!(
        block,
        BlockId::Leaves | BlockId::PineLeaves | BlockId::Bush | BlockId::Log
    )
}

/// Representative block for one coarse cell, or `None` when it reads as empty.
/// Terrain keeps the conservative max filter so cliffs stay sealed. Canopy needs
/// a real share of the cell, and wood needs about one voxel column, so trees keep
/// their silhouette instead of inflating by a whole cell in every direction.
/// Voxels outside the padded halo are unknown and excluded from the shares.
/// Chunk-local height (exclusive top) of the highest opaque terrain voxel in
/// each column, or `i32::MIN` when the column has none.
fn column_tops(chunk: &GeneratedChunk) -> Vec<i32> {
    let mut tops = vec![i32::MIN; (CHUNK_SIZE * CHUNK_SIZE) as usize];
    for z in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            let origin = chunk.chunk * CHUNK_SIZE;
            for y in (0..CHUNK_SIZE).rev() {
                let b = chunk.get_world(origin + IVec3::new(x, y, z));
                if b.is_opaque() && !coarse_vegetation(b) {
                    tops[(z * CHUNK_SIZE + x) as usize] = y + 1;
                    break;
                }
            }
        }
    }
    tops
}

/// Terrain in a coarse cell is solid when the cell's centre lies under the median
/// surface of its columns. "Any voxel solid" lifted every coarse surface to its
/// cell top, which read as stair-stepped terraces. The median keeps thin shells:
/// a surface one voxel thick still stands in the cells beneath it.
fn terrain_reaches(tops: &[i32], x0: i32, z0: i32, step: i32, bottom: i32) -> bool {
    let mut seen: Vec<i32> = Vec::with_capacity((step * step) as usize);
    for dz in 0..step {
        for dx in 0..step {
            let (x, z) = (x0 + dx, z0 + dz);
            if (0..CHUNK_SIZE).contains(&x) && (0..CHUNK_SIZE).contains(&z) {
                let top = tops[(z * CHUNK_SIZE + x) as usize];
                if top != i32::MIN {
                    seen.push(top);
                }
            }
        }
    }
    if seen.is_empty() {
        return true;
    }
    seen.sort_unstable();
    // The cell centre decides. A bottom test rounds every surface up to the next
    // cell boundary, which is the same bias as before, one cell tall.
    2 * bottom + step < 2 * seen[seen.len() / 2]
}

fn coarse_cell(chunk: &GeneratedChunk, min: IVec3, step: i32) -> Option<BlockId> {
    let padded = chunk.origin();
    let (mut terrain, mut liquid) = (None, None);
    let (mut known, mut canopy, mut wood) = (0u32, 0u32, 0u32);
    let mut canopy_block = BlockId::Leaves;
    for dy in 0..step {
        for dz in 0..step {
            for dx in 0..step {
                let p = min + IVec3::new(dx, dy, dz);
                let local = p - padded;
                if local.min_element() < 0 || local.max_element() >= PADDED_CHUNK_SIZE {
                    continue;
                }
                known += 1;
                let sample = chunk.get_world(p);
                if sample == BlockId::Log {
                    wood += 1;
                } else if coarse_vegetation(sample) {
                    canopy += 1;
                    canopy_block = sample;
                } else if sample.is_opaque() {
                    // Later samples are higher, so the surface material wins.
                    terrain = Some(sample);
                } else if sample.is_liquid() && liquid.is_none() {
                    liquid = Some(sample);
                }
            }
        }
    }
    if terrain.is_some() {
        return terrain;
    }
    if known > 0 {
        let share = |count: u32| count as f32 / known as f32;
        if share(canopy + wood) >= 0.375 && canopy > 0 {
            return Some(canopy_block);
        }
        if share(wood) >= 1.0 / (step * step) as f32 {
            return Some(BlockId::Log);
        }
    }
    liquid
}

/// Coarse cells for distant chunks. Terrain cells are solid if any voxel in them
/// is, so cliffs stay sealed and clouds cannot show through the gaps a heightfield
/// leaves; vegetation uses coverage instead (see `coarse_cell`). Boundary faces
/// overlap the neighbour by a fraction of a cell, which hides LOD cracks without
/// a second full-resolution mesh.
pub fn coarsened_mesh(chunk: &GeneratedChunk, lod: u8) -> ChunkMesh {
    let step = 1i32 << lod.clamp(1, 3);
    let n = CHUNK_SIZE / step;
    let origin = chunk.chunk * CHUNK_SIZE;
    let cells = (n * n * n) as usize;
    let index = |x: i32, y: i32, z: i32| ((y * n + z) * n + x) as usize;
    let mut solid = vec![false; cells];
    let mut block = vec![BlockId::Air; cells];
    let tops = column_tops(chunk);
    for cz in 0..n {
        for cx in 0..n {
            for cy in 0..n {
                let chosen = coarse_cell(chunk, origin + IVec3::new(cx, cy, cz) * step, step)
                    .filter(|b| {
                        b.is_liquid()
                            || coarse_vegetation(*b)
                            || terrain_reaches(&tops, cx * step, cz * step, step, cy * step)
                    });
                if let Some(chosen) = chosen {
                    let id = index(cx, cy, cz);
                    solid[id] = true;
                    block[id] = chosen;
                }
            }
        }
    }
    // Neighbour cells are classified by the same rule from the one-voxel halo,
    // so both sides of a seam agree on whether a crown continues across it.
    let occupied = |x: i32, y: i32, z: i32| -> bool {
        if (0..n).contains(&x) && (0..n).contains(&y) && (0..n).contains(&z) {
            return solid[index(x, y, z)];
        }
        coarse_cell(chunk, origin + IVec3::new(x, y, z) * step, step).is_some()
    };
    let mut mesh = ChunkMesh::empty();
    let emit = |mesh: &mut ChunkMesh,
                x0: i32,
                y0: i32,
                z0: i32,
                x1: i32,
                y1: i32,
                z1: i32,
                x2: i32,
                y2: i32,
                z2: i32,
                x3: i32,
                y3: i32,
                z3: i32,
                material: BlockId,
                normal: [f32; 3],
                push: f32| {
        let mut quad = [
            mesh_vertex(chunk, [x0, y0, z0]),
            mesh_vertex(chunk, [x1, y1, z1]),
            mesh_vertex(chunk, [x2, y2, z2]),
            mesh_vertex(chunk, [x3, y3, z3]),
        ];
        if push != 0.0 {
            for corner in &mut quad {
                corner[0] += normal[0] * push;
                corner[1] += normal[1] * push;
                corner[2] += normal[2] * push;
            }
        }
        let face = FaceMaterial::from_block(material, origin + IVec3::new(x0, y0, z0));
        push_face(
            &mut mesh.vertices,
            &mut mesh.indices,
            quad,
            face.color(),
            normal,
            material.surface(),
            [0.94; 4],
        );
    };
    for cz in 0..n {
        for cy in 0..n {
            for cx in 0..n {
                let id = index(cx, cy, cz);
                if !solid[id] {
                    continue;
                }
                let material = block[id];
                let x = cx * step;
                let y = cy * step;
                let z = cz * step;
                let s = step;
                let edge = |axis_min: bool, axis_max: bool| {
                    if axis_min || axis_max { 0.22 } else { 0.0 }
                };
                // Corner order is CCW when looking along the outward normal.
                // The previous lists were the reverse of that, so back-face
                // culling drew the inside of every coarse cell.
                if !occupied(cx, cy + 1, cz) {
                    emit(
                        &mut mesh,
                        x,
                        y + s,
                        z,
                        x,
                        y + s,
                        z + s,
                        x + s,
                        y + s,
                        z + s,
                        x + s,
                        y + s,
                        z,
                        material,
                        [0.0, 1.0, 0.0],
                        0.0,
                    );
                }
                if !occupied(cx, cy - 1, cz) {
                    emit(
                        &mut mesh,
                        x,
                        y,
                        z + s,
                        x,
                        y,
                        z,
                        x + s,
                        y,
                        z,
                        x + s,
                        y,
                        z + s,
                        material,
                        [0.0, -1.0, 0.0],
                        0.0,
                    );
                }
                if !occupied(cx + 1, cy, cz) {
                    let push = edge(false, cx + 1 == n);
                    emit(
                        &mut mesh,
                        x + s,
                        y,
                        z,
                        x + s,
                        y + s,
                        z,
                        x + s,
                        y + s,
                        z + s,
                        x + s,
                        y,
                        z + s,
                        material,
                        [1.0, 0.0, 0.0],
                        push,
                    );
                }
                if !occupied(cx - 1, cy, cz) {
                    let push = edge(cx == 0, false);
                    emit(
                        &mut mesh,
                        x,
                        y,
                        z + s,
                        x,
                        y + s,
                        z + s,
                        x,
                        y + s,
                        z,
                        x,
                        y,
                        z,
                        material,
                        [-1.0, 0.0, 0.0],
                        push,
                    );
                }
                if !occupied(cx, cy, cz + 1) {
                    let push = edge(false, cz + 1 == n);
                    emit(
                        &mut mesh,
                        x + s,
                        y,
                        z + s,
                        x + s,
                        y + s,
                        z + s,
                        x,
                        y + s,
                        z + s,
                        x,
                        y,
                        z + s,
                        material,
                        [0.0, 0.0, 1.0],
                        push,
                    );
                }
                if !occupied(cx, cy, cz - 1) {
                    let push = edge(cz == 0, false);
                    emit(
                        &mut mesh,
                        x,
                        y,
                        z,
                        x,
                        y + s,
                        z,
                        x + s,
                        y + s,
                        z,
                        x + s,
                        y,
                        z,
                        material,
                        [0.0, 0.0, -1.0],
                        push,
                    );
                }
                // Hang a lip below perimeter tops. A coarser neighbour can be a
                // cell shorter; the lip covers that crack without a second mesh.
                // Heightfield cracks only exist in terrain. On a crown the lip
                // is a flat sheet hanging on the seam, so vegetation skips it.
                if !occupied(cx, cy + 1, cz)
                    && !material.is_liquid()
                    && !coarse_vegetation(material)
                {
                    let drop = (s as f32 * 0.7).max(1.0);
                    let hang = |vertices: &mut [Vertex3], drop: f32| {
                        let count = vertices.len();
                        if count >= 4 {
                            vertices[count - 4].pos[1] -= drop;
                            vertices[count - 3].pos[1] -= drop;
                        }
                    };
                    // The first two corners are the ones `hang` drops. They are
                    // the bottom edge, ordered so the skirt faces outward.
                    if cx == 0 {
                        emit(
                            &mut mesh,
                            x,
                            y,
                            z,
                            x,
                            y,
                            z + s,
                            x,
                            y,
                            z + s,
                            x,
                            y,
                            z,
                            material,
                            [-1.0, 0.0, 0.0],
                            0.22,
                        );
                        hang(&mut mesh.vertices, drop);
                    }
                    if cx + 1 == n {
                        emit(
                            &mut mesh,
                            x + s,
                            y,
                            z + s,
                            x + s,
                            y,
                            z,
                            x + s,
                            y,
                            z,
                            x + s,
                            y,
                            z + s,
                            material,
                            [1.0, 0.0, 0.0],
                            0.22,
                        );
                        hang(&mut mesh.vertices, drop);
                    }
                    if cz == 0 {
                        emit(
                            &mut mesh,
                            x + s,
                            y,
                            z,
                            x,
                            y,
                            z,
                            x,
                            y,
                            z,
                            x + s,
                            y,
                            z,
                            material,
                            [0.0, 0.0, -1.0],
                            0.22,
                        );
                        hang(&mut mesh.vertices, drop);
                    }
                    if cz + 1 == n {
                        emit(
                            &mut mesh,
                            x,
                            y,
                            z + s,
                            x + s,
                            y,
                            z + s,
                            x + s,
                            y,
                            z + s,
                            x,
                            y,
                            z + s,
                            material,
                            [0.0, 0.0, 1.0],
                            0.22,
                        );
                        hang(&mut mesh.vertices, drop);
                    }
                }
            }
        }
    }
    mesh
}

fn emit_oriented_box(
    mesh: &mut ChunkMesh,
    start: Vec3,
    end: Vec3,
    half_extent: f32,
    color: [f32; 3],
    surface: crate::material::SurfaceDetail,
) {
    let axis = end - start;
    let forward = axis.normalize_or_zero();
    if forward.length_squared() <= 1e-6 {
        return;
    }
    let helper = if forward.y.abs() > 0.92 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let side_a = forward.cross(helper).normalize_or_zero() * half_extent;
    let side_b = forward.cross(side_a).normalize_or_zero() * half_extent;

    let s00 = start - side_a - side_b;
    let s10 = start + side_a - side_b;
    let s11 = start + side_a + side_b;
    let s01 = start - side_a + side_b;

    let e00 = end - side_a - side_b;
    let e10 = end + side_a - side_b;
    let e11 = end + side_a + side_b;
    let e01 = end - side_a + side_b;

    for quad in [
        [
            s00.to_array(),
            s10.to_array(),
            e10.to_array(),
            e00.to_array(),
        ],
        [
            s10.to_array(),
            s11.to_array(),
            e11.to_array(),
            e10.to_array(),
        ],
        [
            s11.to_array(),
            s01.to_array(),
            e01.to_array(),
            e11.to_array(),
        ],
        [
            s01.to_array(),
            s00.to_array(),
            e00.to_array(),
            e01.to_array(),
        ],
        [
            s01.to_array(),
            s11.to_array(),
            s10.to_array(),
            s00.to_array(),
        ],
        [
            e00.to_array(),
            e10.to_array(),
            e11.to_array(),
            e01.to_array(),
        ],
    ] {
        push_face(
            &mut mesh.vertices,
            &mut mesh.indices,
            quad,
            color,
            normal_from_quad(quad),
            surface,
            [1.0; 4],
        );
    }
}

fn append_torch_mesh(mesh: &mut ChunkMesh, chunk_data: &GeneratedChunk, lod: u8, compact: bool) {
    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            for ly in 0..CHUNK_SIZE {
                let local = [lx, ly, lz];
                let block = mesh_block_at(chunk_data, local);
                if !block.is_torch() {
                    continue;
                }

                let world = world_pos_from_mesh_local(chunk_data, local);
                let base = world.as_vec3();
                let first_vertex = mesh.vertices.len();
                let mut surface = block.surface();
                // Keep the handle non-emissive; only the fuel and flames glow.
                surface.detail_strength = 0.0;
                let wood_color = block.color(world);
                let ember_color = [1.00, 0.44, 0.16];
                let flame_color = [1.00, 0.64, 0.26];

                let (shaft_start, shaft_end, flame_center) = match block {
                    BlockId::Torch => (
                        base + Vec3::new(0.50, 0.06, 0.50),
                        base + Vec3::new(0.50, 0.72, 0.50),
                        base + Vec3::new(0.50, 0.80, 0.50),
                    ),
                    // Wall torches start almost flush with the supporting face and
                    // lean out only enough for the flame to clear the masonry. The
                    // previous mounts began 0.18 m from the wall, which read as a
                    // floating torch, especially beside thin voxel trims.
                    BlockId::TorchWallNorth => (
                        base + Vec3::new(0.50, 0.30, 0.92),
                        base + Vec3::new(0.50, 0.68, 0.60),
                        base + Vec3::new(0.50, 0.74, 0.56),
                    ),
                    BlockId::TorchWallSouth => (
                        base + Vec3::new(0.50, 0.30, 0.08),
                        base + Vec3::new(0.50, 0.68, 0.40),
                        base + Vec3::new(0.50, 0.74, 0.44),
                    ),
                    BlockId::TorchWallWest => (
                        base + Vec3::new(0.92, 0.30, 0.50),
                        base + Vec3::new(0.60, 0.68, 0.50),
                        base + Vec3::new(0.56, 0.74, 0.50),
                    ),
                    BlockId::TorchWallEast => (
                        base + Vec3::new(0.08, 0.30, 0.50),
                        base + Vec3::new(0.40, 0.68, 0.50),
                        base + Vec3::new(0.44, 0.74, 0.50),
                    ),
                    _ => continue,
                };

                emit_oriented_box(mesh, shaft_start, shaft_end, 0.045, wood_color, surface);
                // Dense player builds must fit the minimum worker allowance.
                // Retain every handle and light position, replacing collar/ring
                // detail with a bounded emissive volume above 1024 lights/chunk.
                if compact {
                    let mut flame = surface;
                    flame.detail_strength = -2.0;
                    emit_oriented_box(
                        mesh,
                        flame_center - Vec3::Y * 0.05,
                        flame_center + Vec3::Y * 0.20,
                        0.065,
                        flame_color,
                        flame,
                    );
                    scale_torch_vertices(mesh, first_vertex, base, block, chunk_data.voxel_meters);
                    continue;
                }
                let axis = (shaft_end - shaft_start).normalize_or_zero();
                let mut metal = surface;
                metal.roughness = 0.38;
                metal.specular = 0.65;
                if lod < 2 {
                    // A bound leather grip reads clearly even on the angled mounts.
                    for distance in [0.29, 0.35, 0.41] {
                        let grip = shaft_end - axis * distance;
                        emit_oriented_box(
                            mesh,
                            grip - axis * 0.022,
                            grip + axis * 0.022,
                            0.051,
                            [0.24, 0.13, 0.075],
                            surface,
                        );
                    }
                    if block != BlockId::Torch {
                        let wall = block.torch_support_offset().unwrap().as_vec3();
                        let plate = base + Vec3::new(0.5, 0.38, 0.5) + wall * 0.475;
                        emit_oriented_box(
                            mesh,
                            plate - wall * 0.015,
                            plate + wall * 0.015,
                            0.105,
                            [0.18, 0.16, 0.14],
                            metal,
                        );
                        emit_oriented_box(
                            mesh,
                            plate - wall * 0.02,
                            shaft_start + axis * 0.10,
                            0.028,
                            [0.28, 0.23, 0.17],
                            metal,
                        );
                    }
                }
                for distance in [0.07, 0.19] {
                    if lod >= 2 {
                        continue;
                    }
                    let collar = shaft_end - axis * distance;
                    emit_oriented_box(
                        mesh,
                        collar - axis * 0.018,
                        collar + axis * 0.018,
                        0.061,
                        [0.21, 0.18, 0.14],
                        metal,
                    );
                }
                let mut ember = surface;
                ember.detail_strength = -1.0;
                emit_oriented_box(
                    mesh,
                    shaft_end - (shaft_end - shaft_start).normalize_or_zero() * 0.05,
                    shaft_end + Vec3::new(0.0, 0.08, 0.0),
                    0.065,
                    ember_color,
                    ember,
                );

                // Six-sided tapered volume: a bright belly and an asymmetric tip.
                // Explicit materials keep wall-mounted flames aligned with their mesh.
                let rings = [
                    (-0.08, 0.045, 0.0),
                    (0.01, 0.10, 0.0),
                    (0.13, 0.055, 0.018),
                    (0.27, 0.003, 0.045),
                ];
                // Keep the emissive silhouette stable across chunk LOD swaps.
                // Small non-emissive fittings still disappear in the far mesh.
                let sides = 6;
                // One colour per ring; each band interpolates between its two
                // rings, so the flame runs yellow core -> orange -> red tip with
                // no flat stripes.
                let ring_colors = [
                    [1.0, 0.88, 0.47],
                    flame_color,
                    [1.0, 0.42, 0.09],
                    [1.0, 0.29, 0.055],
                ];
                for band in 0..3 {
                    let mut flame = surface;
                    flame.detail_strength = -2.0;
                    for side in 0..sides {
                        let point = |ring: usize, side: usize| {
                            let (height, radius, bend) = rings[ring];
                            let angle = side as f32 * std::f32::consts::TAU / sides as f32;
                            (flame_center
                                + Vec3::new(
                                    angle.cos() * radius + bend,
                                    height,
                                    angle.sin() * radius,
                                ))
                            .to_array()
                        };
                        let quad = [
                            point(band, side),
                            point(band + 1, side),
                            point(band + 1, side + 1),
                            point(band, side + 1),
                        ];
                        let first_vertex = mesh.vertices.len();
                        push_face(
                            &mut mesh.vertices,
                            &mut mesh.indices,
                            quad,
                            ring_colors[band],
                            normal_from_quad(quad),
                            flame,
                            [1.0; 4],
                        );
                        // quad order is [bottom, top, top, bottom] along the band.
                        let corners = [
                            ring_colors[band],
                            ring_colors[band + 1],
                            ring_colors[band + 1],
                            ring_colors[band],
                        ];
                        for (vertex, color) in mesh.vertices[first_vertex..].iter_mut().zip(corners) {
                            vertex.color = color;
                        }
                    }
                }
                {
                    // A smaller tongue breaks the single-cone silhouette.
                    let mut flame = surface;
                    flame.detail_strength = -2.0;
                    let foot = flame_center + Vec3::new(-0.065, 0.025, 0.0);
                    let belly = flame_center + Vec3::new(-0.105, 0.12, 0.0);
                    let tip = flame_center + Vec3::new(-0.08, 0.225, 0.015);
                    for side in 0..4 {
                        let a = side as f32 * std::f32::consts::FRAC_PI_2;
                        let b = a + std::f32::consts::FRAC_PI_2;
                        let va = Vec3::new(a.cos(), 0.0, a.sin());
                        let vb = Vec3::new(b.cos(), 0.0, b.sin());
                        for quad in [
                            [
                                foot + va * 0.025,
                                belly + va * 0.035,
                                belly + vb * 0.035,
                                foot + vb * 0.025,
                            ],
                            [
                                belly + va * 0.035,
                                tip + va * 0.002,
                                tip + vb * 0.002,
                                belly + vb * 0.035,
                            ],
                        ] {
                            let quad = quad.map(|v| v.to_array());
                            push_face(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                quad,
                                [1.0, 0.42, 0.09],
                                normal_from_quad(quad),
                                flame,
                                [1.0; 4],
                            );
                        }
                    }
                }
                scale_torch_vertices(mesh, first_vertex, base, block, chunk_data.voxel_meters);
            }
        }
    }
}

fn scale_torch_vertices(
    mesh: &mut ChunkMesh,
    first: usize,
    cell: Vec3,
    block: BlockId,
    scale: f32,
) {
    for vertex in &mut mesh.vertices[first..] {
        let local = Vec3::from_array(vertex.pos) - cell;
        vertex.pos = (block.torch_point_meters(cell, local, scale) / scale).to_array();
    }
}

/// Level of a free water surface at voxel `p`: water with air directly above.
/// Submerged water (water above) returns `None`; its surface is the one above it.
fn free_water_level(chunk: &GeneratedChunk, p: IVec3) -> Option<f32> {
    (chunk.get_world(p) == BlockId::Water && chunk.get_world(p + IVec3::Y) != BlockId::Water)
        .then(|| {
            chunk.water_levels.get(&p).copied().unwrap_or(8) as f32 / 8.0
                * (1.0 - WATER_SURFACE_INSET)
        })
}

/// Surface height above row `row`'s floor at lattice corner `(x, z)`: the mean
/// level of the free surfaces in the four voxel columns that touch the corner.
/// Every face sharing the corner computes the same value, so neighbouring
/// columns of different volume slope into each other instead of stepping in
/// 1/8 m terraces. Halo columns count, so chunks agree on their border corners.
fn water_corner_height(chunk: &GeneratedChunk, x: i32, z: i32, row: i32) -> f32 {
    let (mut sum, mut count) = (0.0, 0u32);
    for (cx, cz) in [(x - 1, z - 1), (x, z - 1), (x - 1, z), (x, z)] {
        if let Some(level) = free_water_level(chunk, IVec3::new(cx, row, cz)) {
            sum += level;
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f32
    }
}

fn append_water_mesh(mesh: &mut ChunkMesh, chunk: &GeneratedChunk) {
    // Height above the voxel floor for a water voxel: 1.0 when water sits
    // above it, otherwise its own free level.
    let volume = |p: IVec3| -> f32 {
        if chunk.get_world(p) != BlockId::Water {
            return 0.0;
        }
        if chunk.get_world(p + IVec3::Y) == BlockId::Water {
            return 1.0;
        }
        (chunk.water_levels.get(&p).copied().unwrap_or(8) as f32 / 8.0)
            * (1.0 - WATER_SURFACE_INSET)
    };
    // Corners of a unit cell in the order the top quad is emitted.
    const CELLS: [(i32, i32); 4] = [(0, 0), (0, 1), (1, 1), (1, 0)];
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let p = chunk.chunk * CHUNK_SIZE + IVec3::new(x, y, z);
                if chunk.get_world(p) != BlockId::Water {
                    continue;
                }
                let h = volume(p);
                // Surface height at a lattice corner. Submerged water is full height.
                let top_at = |cx: i32, cz: i32| {
                    if h >= 1.0 {
                        1.0
                    } else {
                        water_corner_height(chunk, cx, cz, p.y)
                    }
                };
                let base = p.as_vec3();
                for axis in 0..3 {
                    for positive in [false, true] {
                        let mut offset = IVec3::ZERO;
                        offset[axis] = if positive { 1 } else { -1 };
                        let neighbor = chunk.get_world(p + offset);
                        let rel: [Vec3; 4] = if axis == 1 {
                            if positive {
                                // Water above: this voxel has no free surface.
                                if h >= 1.0 {
                                    continue;
                                }
                                CELLS.map(|(i, j)| {
                                    Vec3::new(i as f32, top_at(p.x + i, p.z + j), j as f32)
                                })
                            } else {
                                if !neighbor.is_empty() {
                                    continue;
                                }
                                CELLS.map(|(i, j)| Vec3::new(i as f32, 0.0, j as f32))
                            }
                        } else {
                            if neighbor.is_opaque() {
                                continue;
                            }
                            // The wall's foot is the neighbour's surface at the shared
                            // corners: its free level if it has one, the full voxel if
                            // water sits above it, and the floor if it is dry.
                            let floor_at = |cx: i32, cz: i32| match neighbor {
                                BlockId::Water
                                    if chunk.get_world(p + offset + IVec3::Y)
                                        != BlockId::Water =>
                                {
                                    water_corner_height(chunk, cx, cz, p.y)
                                }
                                BlockId::Water => 1.0,
                                _ => 0.0,
                            };
                            let plane = if positive { 1 } else { 0 };
                            let (e0, e1) = if axis == 0 {
                                ((p.x + plane, p.z), (p.x + plane, p.z + 1))
                            } else {
                                ((p.x, p.z + plane), (p.x + 1, p.z + plane))
                            };
                            let (b0, b1) = (floor_at(e0.0, e0.1), floor_at(e1.0, e1.1));
                            let (t0, t1) = (top_at(e0.0, e0.1), top_at(e1.0, e1.1));
                            if t0 <= b0 && t1 <= b1 {
                                // Shared surface on both sides: no wall.
                                continue;
                            }
                            let pl = plane as f32;
                            if axis == 0 {
                                [
                                    Vec3::new(pl, b0, 0.0),
                                    Vec3::new(pl, t0, 0.0),
                                    Vec3::new(pl, t1, 1.0),
                                    Vec3::new(pl, b1, 1.0),
                                ]
                            } else {
                                [
                                    Vec3::new(0.0, b0, pl),
                                    Vec3::new(1.0, b1, pl),
                                    Vec3::new(1.0, t1, pl),
                                    Vec3::new(0.0, t0, pl),
                                ]
                            }
                        };
                        let quad = if positive {
                            rel
                        } else {
                            [rel[3], rel[2], rel[1], rel[0]]
                        };
                        push_face(
                            &mut mesh.vertices,
                            &mut mesh.indices,
                            quad.map(|c| (base + c).to_array()),
                            BlockId::Water.color(p),
                            face_normal_from_axis(axis, positive),
                            BlockId::Water.surface(),
                            [1.0; 4],
                        );
                    }
                }
            }
        }
    }
}

pub fn build_outline_vertices(block: IVec3, color: [f32; 4], expand: f32) -> Vec<LineVertex> {
    let x = block.x as f32 - expand;
    let y = block.y as f32 - expand;
    let z = block.z as f32 - expand;
    let s = 1.0 + 2.0 * expand;
    let p = [
        [x, y, z],
        [x + s, y, z],
        [x + s, y, z + s],
        [x, y, z + s],
        [x, y + s, z],
        [x + s, y + s, z],
        [x + s, y + s, z + s],
        [x, y + s, z + s],
    ];
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let mut out = Vec::new();
    for (a, b) in edges {
        out.push(LineVertex { pos: p[a], color });
        out.push(LineVertex { pos: p[b], color });
    }
    out
}

#[cfg(test)]
mod coarsened_winding {
    use super::*;
    use glam::Vec3;

    fn solid_chunk() -> GeneratedChunk {
        let count = (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize;
        let mut chunk = GeneratedChunk::new(IVec3::ZERO, vec![BlockId::Air; count]);
        // Leave the one-voxel halo empty so the chunk shell is actually exposed.
        for z in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    chunk.set_world(IVec3::new(x, y, z), BlockId::Stone);
                }
            }
        }
        chunk
    }

    #[test]
    fn coarsened_lod_faces_point_outward() {
        for lod in 1..=3 {
            let mesh = coarsened_mesh(&solid_chunk(), lod);
            assert!(mesh.indices.len() >= 3, "lod {lod} produced no triangles");
            let mut checked = 0u32;
            for tri in mesh.indices.chunks_exact(3) {
                let position = |index: u32| Vec3::from_array(mesh.vertices[index as usize].pos);
                let geometric = (position(tri[1]) - position(tri[0]))
                    .cross(position(tri[2]) - position(tri[0]));
                if geometric.length_squared() < 1e-8 {
                    continue;
                }
                let stored = Vec3::from_array(mesh.vertices[tri[0] as usize].normal);
                let align = geometric.normalize().dot(stored.normalize_or_zero());
                assert!(
                    align > 0.85,
                    "lod {lod} triangle is wound against its normal ({align})"
                );
                checked += 1;
            }
            assert!(checked > 0, "lod {lod} had no triangles with area");
        }
    }
}

#[cfg(test)]
mod coarse_vegetation_lod {
    use super::*;

    /// One broadleaf tree centred in an otherwise empty chunk on a stone floor.
    fn tree_chunk() -> GeneratedChunk {
        let count = (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize;
        let mut chunk = GeneratedChunk::new(IVec3::ZERO, vec![BlockId::Air; count]);
        for z in -1..=CHUNK_SIZE {
            for x in -1..=CHUNK_SIZE {
                chunk.set_world(IVec3::new(x, 0, z), BlockId::Stone);
                let d = Vec3::new(x as f32 - 16.0, 0.0, z as f32 - 16.0).length();
                for y in 1..=CHUNK_SIZE {
                    if x == 16 && z == 16 && y <= 18 {
                        chunk.set_world(IVec3::new(x, y, z), BlockId::Log);
                    } else if d.hypot(y as f32 - 20.0) <= 5.5 {
                        chunk.set_world(IVec3::new(x, y, z), BlockId::Leaves);
                    }
                }
            }
        }
        chunk
    }

    fn face_area(mesh: &ChunkMesh, block_color: [f32; 3]) -> f32 {
        mesh.vertices
            .chunks_exact(4)
            .filter(|q| q[0].color == block_color)
            .map(|q| {
                let p = |i: usize| Vec3::from_array(q[i].pos);
                (p(1) - p(0)).cross(p(3) - p(0)).length()
            })
            .sum()
    }

    #[test]
    fn crowns_keep_their_size_and_trunks_survive() {
        let chunk = tree_chunk();
        let leaf_color = FaceMaterial::from_block(BlockId::Leaves, IVec3::ZERO).color();
        let full = face_area(&coarsened_mesh(&chunk, 1), leaf_color);
        // Area of a 5.5-voxel sphere is ~380; the old max filter grew lod 3 to
        // several times that.
        for lod in 1..=3 {
            let mesh = coarsened_mesh(&chunk, lod);
            let leaves = face_area(&mesh, leaf_color);
            println!("lod {lod}: leaf face area {leaves:.0} (lod 1 {full:.0})");
            assert!(
                leaves > 150.0 && leaves < 900.0,
                "lod {lod}: crown area {leaves}"
            );
            let has_wood = mesh
                .vertices
                .iter()
                .any(|v| v.color == FaceMaterial::from_block(BlockId::Log, IVec3::ZERO).color());
            assert!(has_wood || lod == 3, "lod {lod}: trunk vanished");
        }
    }
}

#[cfg(test)]
mod lod_speed {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::{World, WorldType};
    use std::sync::atomic::AtomicBool;

    /// `cargo test --release lod_speed -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn coarsened_mesh_cost() {
        let cfg = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 7).sample_config();
        let never = AtomicBool::new(false);
        let chunks: Vec<_> = (-3..3)
            .flat_map(|x| (-3..3).flat_map(move |z| (0..3).map(move |y| IVec3::new(x, y, z))))
            .map(|c| crate::world::generate_padded(&cfg, c, &never).unwrap())
            .filter(|c| c.blocks.iter().any(|b| b.is_opaque()))
            .collect();
        println!("{} solid chunks", chunks.len());
        for lod in 1..=3u8 {
            let start = std::time::Instant::now();
            let mut tris = 0;
            for c in &chunks {
                tris += coarsened_mesh(c, lod).indices.len() / 3;
            }
            let ms = start.elapsed().as_secs_f64() * 1e3 / chunks.len() as f64;
            println!("lod {lod}: {ms:.2} ms/chunk, {} tris/chunk", tris / chunks.len());
        }
    }
}

#[cfg(test)]
mod terrace_metric {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::{World, WorldType};
    use std::sync::atomic::AtomicBool;

    /// `cargo test --release terrace_metric -- --ignored --nocapture`
    /// How far each LOD column's top sits from the true surface, in voxels.
    /// Stair-stepping shows up as a large mean and a bias above zero.
    #[test]
    #[ignore]
    fn lod_surface_error() {
        let cfg = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 7).sample_config();
        let never = AtomicBool::new(false);
        for lod in 1..=3u8 {
            let step = 1i32 << lod;
            let (mut sum, mut abs_sum, mut n) = (0.0f32, 0.0f32, 0usize);
            for cz in -4..4 {
                for cx in -4..4 {
                for cy in 0..2 {
                    let chunk = crate::world::generate_padded(&cfg, IVec3::new(cx, cy, cz), &never).unwrap();
                    let origin = chunk.chunk * CHUNK_SIZE;
                    for z in 0..CHUNK_SIZE {
                        for x in 0..CHUNK_SIZE {
                            // True surface: highest opaque terrain voxel in this column.
                            let mut true_top = None;
                            for y in (0..CHUNK_SIZE).rev() {
                                let b = chunk.get_world(origin + IVec3::new(x, y, z));
                                if b.is_opaque() && !coarse_vegetation(b) {
                                    true_top = Some(y + 1);
                                    break;
                                }
                            }
                            // Surfaces on the chunk floor are owned by the chunk below.
                            let Some(true_top) = true_top.filter(|&t| t > 4) else { continue };
                            // Coarse surface: top of the highest solid coarse cell.
                            let cx0 = x / step;
                            let cz0 = z / step;
                            let mut coarse_top = None;
                            for cy in (0..CHUNK_SIZE / step).rev() {
                                let min = origin + IVec3::new(cx0 * step, cy * step, cz0 * step);
                                let tops = column_tops(&chunk);
                                if coarse_cell(&chunk, min, step)
                                    .filter(|b| b.is_liquid() || coarse_vegetation(*b) || terrain_reaches(&tops, cx0 * step, cz0 * step, step, cy * step))
                                    .is_some_and(|b| !coarse_vegetation(b))
                                {
                                    coarse_top = Some((cy + 1) * step);
                                    break;
                                }
                            }
                            let Some(coarse_top) = coarse_top else { continue };
                            let err = (coarse_top - true_top) as f32;
                            sum += err;
                            abs_sum += err.abs();
                            n += 1;
                        }
                    }
                }
            }
            }
            println!(
                "lod {lod}: {n} columns, bias {:+.2} vox, mean |err| {:.2} vox",
                sum / n as f32,
                abs_sum / n as f32
            );
        }
    }
}
