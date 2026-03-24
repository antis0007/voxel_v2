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

pub const LOD_SEAM_WEST: u8 = 1 << 0;
pub const LOD_SEAM_EAST: u8 = 1 << 1;
pub const LOD_SEAM_NORTH: u8 = 1 << 2;
pub const LOD_SEAM_SOUTH: u8 = 1 << 3;

impl ChunkMesh {
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
struct ColumnTop {
    height: i16,
    block: BlockId,
}

impl ColumnTop {
    const EMPTY: Self = Self {
        height: i16::MIN,
        block: BlockId::Stone,
    };

    fn into_pair(self) -> (i32, BlockId) {
        (self.height as i32, self.block)
    }
}

/// Cubic 32x32x32 chunk with 1-block padding on all sides (34x34x34 total).
/// `chunk` is the chunk coordinate in all 3 axes (each axis uses CHUNK_SIZE=32).
#[derive(Clone)]
pub struct GeneratedChunk {
    pub chunk: IVec3,
    pub water_level: i32,
    pub blocks: Vec<BlockId>,
    top_cache: Box<[ColumnTop]>,
}

impl GeneratedChunk {
    pub fn new(chunk: IVec3, water_level: i32, blocks: Vec<BlockId>) -> Self {
        let mut out = Self {
            chunk,
            water_level,
            blocks,
            top_cache: vec![ColumnTop::EMPTY; (CHUNK_SIZE * CHUNK_SIZE) as usize]
                .into_boxed_slice(),
        };
        out.rebuild_top_cache();
        out
    }

    /// World-space origin of the padded block at local (0,0,0).
    pub fn origin(&self) -> IVec3 {
        IVec3::new(
            self.chunk.x * CHUNK_SIZE - CHUNK_PAD,
            self.chunk.y * CHUNK_SIZE - CHUNK_PAD,
            self.chunk.z * CHUNK_SIZE - CHUNK_PAD,
        )
    }

    fn local_index(x: i32, y: i32, z: i32) -> usize {
        ((y * PADDED_CHUNK_SIZE + z) * PADDED_CHUNK_SIZE + x) as usize
    }

    /// Returns a column-top cache index for XZ relative to the chunk interior.
    fn column_top_index(x: i32, z: i32) -> Option<usize> {
        if (0..CHUNK_SIZE).contains(&x) && (0..CHUNK_SIZE).contains(&z) {
            Some((z * CHUNK_SIZE + x) as usize)
        } else {
            None
        }
    }

    fn contains_world(&self, p: IVec3) -> bool {
        let origin = self.origin();
        p.x >= origin.x
            && p.x < origin.x + PADDED_CHUNK_SIZE
            && p.y >= origin.y
            && p.y < origin.y + PADDED_CHUNK_SIZE
            && p.z >= origin.z
            && p.z < origin.z + PADDED_CHUNK_SIZE
    }

    fn scan_top_solid_block(&self, x: i32, z: i32) -> ColumnTop {
        let wx = self.chunk.x * CHUNK_SIZE + x;
        let wz = self.chunk.z * CHUNK_SIZE + z;
        let y_start = self.chunk.y * CHUNK_SIZE + CHUNK_SIZE - 1;
        let y_end = self.chunk.y * CHUNK_SIZE;
        for y in (y_end..=y_start).rev() {
            let block = self.get_world(IVec3::new(wx, y, wz));
            if block.contributes_to_heightmap() {
                return ColumnTop {
                    height: y as i16,
                    block,
                };
            }
        }
        ColumnTop::EMPTY
    }

    fn rebuild_top_cache(&mut self) {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                self.recompute_column_top(x, z);
            }
        }
    }

    fn recompute_column_top(&mut self, x: i32, z: i32) {
        let Some(index) = Self::column_top_index(x, z) else {
            return;
        };
        self.top_cache[index] = self.scan_top_solid_block(x, z);
    }

    pub fn get_world(&self, p: IVec3) -> BlockId {
        if !self.contains_world(p) {
            return BlockId::Air;
        }
        let origin = self.origin();
        let lx = p.x - origin.x;
        let ly = p.y - origin.y;
        let lz = p.z - origin.z;
        self.blocks[Self::local_index(lx, ly, lz)]
    }

    pub fn set_world(&mut self, p: IVec3, block: BlockId) {
        if !self.contains_world(p) {
            return;
        }
        let origin = self.origin();
        let lx = p.x - origin.x;
        let ly = p.y - origin.y;
        let lz = p.z - origin.z;
        let index = Self::local_index(lx, ly, lz);
        self.blocks[index] = block;

        let local_x = p.x - self.chunk.x * CHUNK_SIZE;
        let local_z = p.z - self.chunk.z * CHUNK_SIZE;
        self.recompute_column_top(local_x, local_z);
    }

    fn top_solid_block(&self, x: i32, z: i32) -> (i32, BlockId) {
        if let Some(index) = Self::column_top_index(x, z) {
            self.top_cache[index].into_pair()
        } else {
            self.scan_top_solid_block(x, z).into_pair()
        }
    }
}

fn lod_cell_top(
    chunk_data: &GeneratedChunk,
    lx: i32,
    lz: i32,
    step: i32,
) -> Option<(i32, BlockId)> {
    let end_x = if lx >= CHUNK_SIZE {
        lx + 1
    } else {
        (lx + step).min(CHUNK_SIZE)
    };
    let end_z = if lz >= CHUNK_SIZE {
        lz + 1
    } else {
        (lz + step).min(CHUNK_SIZE)
    };
    let mut best: Option<(i32, BlockId)> = None;
    for z in lz..end_z {
        for x in lx..end_x {
            let sample = chunk_data.top_solid_block(x, z);
            // top_solid_block returns i16::MIN as height when no solid block exists
            if sample.0 == i16::MIN as i32 {
                continue;
            }
            if best.map_or(true, |(bh, _)| sample.0 > bh) {
                best = Some(sample);
            }
        }
    }
    best
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FaceMaterial {
    block: BlockId,
    color_bits: [u32; 3],
}

impl FaceMaterial {
    fn from_block(block: BlockId, world_pos: IVec3) -> Self {
        let color = block.color(world_pos);
        Self {
            block,
            color_bits: [color[0].to_bits(), color[1].to_bits(), color[2].to_bits()],
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
    if block.is_empty() {
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

    let a = mesh_vertex(chunk_data, base);
    let b = mesh_vertex(chunk_data, add_axes(base, du));
    let c = mesh_vertex(chunk_data, add_axes(add_axes(base, du), dv));
    let d = mesh_vertex(chunk_data, add_axes(base, dv));

    let quad = if positive { [a, b, c, d] } else { [d, c, b, a] };
    let corner_ao = face_corner_ao(chunk_data, axis, positive, plane, u0, v0, width, height);

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
    let inside_axis = if positive { plane - 1 } else { plane };

    let sample = |u_positive: bool, v_positive: bool| {
        let inside_u = if u_positive { u0 + width - 1 } else { u0 };
        let inside_v = if v_positive { v0 + height - 1 } else { v0 };
        let outside_u = if u_positive { u0 + width } else { u0 - 1 };
        let outside_v = if v_positive { v0 + height } else { v0 - 1 };

        let mut side_u = [0; 3];
        side_u[axis] = inside_axis;
        side_u[u_axis] = outside_u;
        side_u[v_axis] = inside_v;

        let mut side_v = [0; 3];
        side_v[axis] = inside_axis;
        side_v[u_axis] = inside_u;
        side_v[v_axis] = outside_v;

        let mut corner = [0; 3];
        corner[axis] = inside_axis;
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
                surface.detail_scale,
                surface.detail_strength,
            ],
            ao_strength: ao,
        });
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn push_height_quad(
    vertices: &mut Vec<Vertex3>,
    indices: &mut Vec<u32>,
    x0: f32,
    x1: f32,
    z0: f32,
    z1: f32,
    y: f32,
    color: [f32; 3],
    surface: BlockId,
) {
    push_face(
        vertices,
        indices,
        [[x0, y, z0], [x0, y, z1], [x1, y, z1], [x1, y, z0]],
        color,
        [0.0, 1.0, 0.0],
        surface.surface(),
        [1.0; 4],
    );
}

fn push_wall(
    vertices: &mut Vec<Vertex3>,
    indices: &mut Vec<u32>,
    quad: [[f32; 3]; 4],
    color: [f32; 3],
    surface: BlockId,
) {
    push_face(
        vertices,
        indices,
        quad,
        color,
        normal_from_quad(quad),
        surface.surface(),
        [0.94; 4],
    );
}

fn push_double_sided_face(
    vertices: &mut Vec<Vertex3>,
    indices: &mut Vec<u32>,
    quad: [[f32; 3]; 4],
    color: [f32; 3],
    surface: crate::material::SurfaceDetail,
) {
    push_face(
        vertices,
        indices,
        quad,
        color,
        normal_from_quad(quad),
        surface,
        [1.0; 4],
    );
    push_face(
        vertices,
        indices,
        [quad[3], quad[2], quad[1], quad[0]],
        color,
        normal_from_quad([quad[3], quad[2], quad[1], quad[0]]),
        surface,
        [1.0; 4],
    );
}

fn push_wall_lod(
    vertices: &mut Vec<Vertex3>,
    indices: &mut Vec<u32>,
    quad: [[f32; 3]; 4],
    color: [f32; 3],
    surface: BlockId,
    double_sided: bool,
) {
    if double_sided {
        push_double_sided_face(vertices, indices, quad, color, surface.surface());
    } else {
        push_wall(vertices, indices, quad, color, surface);
    }
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

fn append_torch_mesh(mesh: &mut ChunkMesh, chunk_data: &GeneratedChunk) {
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
                let surface = block.surface();
                let wood_color = block.color(world);
                let ember_color = [1.00, 0.44, 0.16];
                let flame_color = [1.00, 0.64, 0.26];

                let (shaft_start, shaft_end, flame_center) = match block {
                    BlockId::Torch => (
                        base + Vec3::new(0.50, 0.06, 0.50),
                        base + Vec3::new(0.50, 0.72, 0.50),
                        base + Vec3::new(0.50, 0.80, 0.50),
                    ),
                    BlockId::TorchWallNorth => (
                        base + Vec3::new(0.50, 0.36, 0.82),
                        base + Vec3::new(0.50, 0.76, 0.36),
                        base + Vec3::new(0.50, 0.82, 0.34),
                    ),
                    BlockId::TorchWallSouth => (
                        base + Vec3::new(0.50, 0.36, 0.18),
                        base + Vec3::new(0.50, 0.76, 0.64),
                        base + Vec3::new(0.50, 0.82, 0.66),
                    ),
                    BlockId::TorchWallWest => (
                        base + Vec3::new(0.82, 0.36, 0.50),
                        base + Vec3::new(0.36, 0.76, 0.50),
                        base + Vec3::new(0.34, 0.82, 0.50),
                    ),
                    BlockId::TorchWallEast => (
                        base + Vec3::new(0.18, 0.36, 0.50),
                        base + Vec3::new(0.64, 0.76, 0.50),
                        base + Vec3::new(0.66, 0.82, 0.50),
                    ),
                    _ => continue,
                };

                emit_oriented_box(mesh, shaft_start, shaft_end, 0.055, wood_color, surface);
                emit_oriented_box(
                    mesh,
                    shaft_end - (shaft_end - shaft_start).normalize_or_zero() * 0.05,
                    shaft_end + Vec3::new(0.0, 0.08, 0.0),
                    0.065,
                    ember_color,
                    surface,
                );

                let half_w = 0.09;
                let half_h = 0.15;
                for quad in [
                    [
                        (flame_center + Vec3::new(-half_w, -half_h, 0.0)).to_array(),
                        (flame_center + Vec3::new(half_w, -half_h, 0.0)).to_array(),
                        (flame_center + Vec3::new(half_w, half_h, 0.0)).to_array(),
                        (flame_center + Vec3::new(-half_w, half_h, 0.0)).to_array(),
                    ],
                    [
                        (flame_center + Vec3::new(0.0, -half_h, -half_w)).to_array(),
                        (flame_center + Vec3::new(0.0, -half_h, half_w)).to_array(),
                        (flame_center + Vec3::new(0.0, half_h, half_w)).to_array(),
                        (flame_center + Vec3::new(0.0, half_h, -half_w)).to_array(),
                    ],
                ] {
                    push_double_sided_face(
                        &mut mesh.vertices,
                        &mut mesh.indices,
                        quad,
                        flame_color,
                        surface,
                    );
                }
            }
        }
    }
}

pub fn build_chunk_mesh_lod_with_seams(
    chunk_data: &GeneratedChunk,
    lod: u8,
    seams: u8,
) -> ChunkMesh {
    match lod {
        0 => build_chunk_mesh_greedy(chunk_data),
        1 => build_chunk_mesh_lod_step(chunk_data, seams, 2, true),
        2 => build_chunk_mesh_lod_step(chunk_data, seams, 4, false),
        _ => build_chunk_mesh_lod_step(chunk_data, seams, 8, false),
    }
}

pub fn build_chunk_mesh_lod(chunk_data: &GeneratedChunk, lod: u8) -> ChunkMesh {
    build_chunk_mesh_lod_with_seams(chunk_data, lod, 0)
}

pub fn build_chunk_mesh_greedy(chunk_data: &GeneratedChunk) -> ChunkMesh {
    const DIMS: [i32; 3] = [CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE];

    let mut mesh = ChunkMesh::empty();
    // Maximum mask size is CHUNK_SIZE * CHUNK_SIZE for any face plane
    let mut mask = vec![None; (CHUNK_SIZE * CHUNK_SIZE) as usize];

    for axis in 0..3 {
        let u_axis = (axis + 1) % 3;
        let v_axis = (axis + 2) % 3;
        let row_len = DIMS[u_axis] as usize;
        let plane_len = (DIMS[u_axis] * DIMS[v_axis]) as usize;

        for positive in [false, true] {
            // Only emit faces for blocks INSIDE the chunk (0..CHUNK_SIZE).
            // `current[axis]` = plane-1 when positive, plane when negative.
            // Padding neighbours are used for visibility tests, but the face
            // itself must belong to an interior block — otherwise both adjacent
            // chunks emit the same boundary face, causing Z-fighting / doubled
            // geometry.
            let plane_start = if positive { 1 } else { 0 };
            let plane_end = if positive { DIMS[axis] } else { DIMS[axis] - 1 };
            for plane in plane_start..=plane_end {
                for v in 0..DIMS[v_axis] {
                    for u in 0..DIMS[u_axis] {
                        let mut current = [0; 3];
                        current[axis] = if positive { plane - 1 } else { plane };
                        current[u_axis] = u;
                        current[v_axis] = v;

                        let mut neighbor = current;
                        neighbor[axis] = if positive { plane } else { plane - 1 };

                        let block = mesh_block_at(chunk_data, current);
                        let neighbor_block = mesh_block_at(chunk_data, neighbor);
                        let index = (v as usize * row_len) + u as usize;
                        mask[index] = if block.is_torch() {
                            None
                        } else if face_visible(block, neighbor_block) {
                            Some(FaceMaterial::from_block(
                                block,
                                world_pos_from_mesh_local(chunk_data, current),
                            ))
                        } else {
                            None
                        };
                    }
                }

                let mut index = 0usize;
                while index < plane_len {
                    let Some(material) = mask[index] else {
                        index += 1;
                        continue;
                    };

                    let u0 = (index % row_len) as i32;
                    let v0 = (index / row_len) as i32;

                    let mut width = 1i32;
                    while u0 + width < DIMS[u_axis]
                        && mask[index + width as usize] == Some(material)
                    {
                        width += 1;
                    }

                    let mut height = 1i32;
                    'grow: while v0 + height < DIMS[v_axis] {
                        let row_start = (v0 + height) as usize * row_len + u0 as usize;
                        for offset in 0..width as usize {
                            if mask[row_start + offset] != Some(material) {
                                break 'grow;
                            }
                        }
                        height += 1;
                    }

                    emit_greedy_face(
                        &mut mesh, chunk_data, axis, positive, plane, u0, v0, width, height,
                        material,
                    );

                    for clear_v in v0..(v0 + height) {
                        let row_start = clear_v as usize * row_len + u0 as usize;
                        for clear_u in 0..width as usize {
                            mask[row_start + clear_u] = None;
                        }
                    }

                    index += width as usize;
                }
            }
        }
    }

    append_torch_mesh(&mut mesh, chunk_data);
    mesh
}

pub fn build_chunk_mesh_lod1(chunk_data: &GeneratedChunk, seams: u8) -> ChunkMesh {
    build_chunk_mesh_lod_step(chunk_data, seams, 2, true)
}

fn build_chunk_mesh_lod_step(
    chunk_data: &GeneratedChunk,
    seams: u8,
    step: i32,
    include_decor: bool,
) -> ChunkMesh {
    let mut mesh = build_chunk_shell(chunk_data, step.max(1), true, seams);
    append_torch_mesh(&mut mesh, chunk_data);
    if include_decor {
        append_foliage_mesh(&mut mesh, chunk_data);
    }
    mesh
}

fn build_chunk_shell(
    chunk_data: &GeneratedChunk,
    step: i32,
    include_walls: bool,
    seams: u8,
) -> ChunkMesh {
    let mut mesh = ChunkMesh::empty();
    let ox = chunk_data.chunk.x * CHUNK_SIZE;
    let oz = chunk_data.chunk.z * CHUNK_SIZE;

    let seam_west = seams & LOD_SEAM_WEST != 0;
    let seam_east = seams & LOD_SEAM_EAST != 0;
    let seam_north = seams & LOD_SEAM_NORTH != 0;
    let seam_south = seams & LOD_SEAM_SOUTH != 0;

    let chunk_floor_y = (chunk_data.chunk.y * CHUNK_SIZE) as f32;
    let double_sided_walls = true;

    for lz in (0..CHUNK_SIZE).step_by(step as usize) {
        for lx in (0..CHUNK_SIZE).step_by(step as usize) {
            let cell = lod_cell_top(chunk_data, lx, lz, step);

            let x0 = (ox + lx) as f32;
            let z0 = (oz + lz) as f32;
            let x1 = (ox + lx + step).min(ox + CHUNK_SIZE) as f32;
            let z1 = (oz + lz + step).min(oz + CHUNK_SIZE) as f32;

            if let Some((h, surface)) = cell {
                let color = surface.color(IVec3::new(ox + lx, h, oz + lz));
                let y = h as f32 + 1.0;

                push_height_quad(
                    &mut mesh.vertices,
                    &mut mesh.indices,
                    x0,
                    x1,
                    z0,
                    z1,
                    y,
                    color,
                    surface,
                );

                if include_walls {
                    let east_cell = lod_cell_top(chunk_data, (lx + step).min(CHUNK_SIZE), lz, step);
                    let south_cell =
                        lod_cell_top(chunk_data, lx, (lz + step).min(CHUNK_SIZE), step);
                    let chunk_floor = chunk_data.chunk.y * CHUNK_SIZE;

                    match east_cell {
                        Some((east_h, east_surface)) if east_h != h => {
                            let min_h = east_h.min(h) as f32 + 1.0;
                            let max_h = east_h.max(h) as f32 + 1.0;
                            let east_color = if h >= east_h {
                                color
                            } else {
                                east_surface.color(IVec3::new(ox + lx + step, east_h, oz + lz))
                            };
                            let east_quad = if h >= east_h {
                                [
                                    [x1, min_h, z0],
                                    [x1, max_h, z0],
                                    [x1, max_h, z1],
                                    [x1, min_h, z1],
                                ]
                            } else {
                                [
                                    [x1, min_h, z1],
                                    [x1, max_h, z1],
                                    [x1, max_h, z0],
                                    [x1, min_h, z0],
                                ]
                            };
                            push_wall_lod(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                east_quad,
                                east_color,
                                if h >= east_h { surface } else { east_surface },
                                double_sided_walls,
                            );
                        }
                        None => {
                            // East neighbor cell is empty — emit east-facing wall on this cell.
                            let min_h = ((h - CHUNK_SIZE).max(chunk_floor)) as f32;
                            let max_h = h as f32 + 1.0;
                            push_wall_lod(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                [
                                    [x1, min_h, z0],
                                    [x1, max_h, z0],
                                    [x1, max_h, z1],
                                    [x1, min_h, z1],
                                ],
                                color,
                                surface,
                                double_sided_walls,
                            );
                        }
                        _ => {}
                    }

                    match south_cell {
                        Some((south_h, south_surface)) if south_h != h => {
                            let min_h = south_h.min(h) as f32 + 1.0;
                            let max_h = south_h.max(h) as f32 + 1.0;
                            let south_color = if h >= south_h {
                                color
                            } else {
                                south_surface.color(IVec3::new(ox + lx, south_h, oz + lz + step))
                            };
                            let south_quad = if h >= south_h {
                                [
                                    [x0, min_h, z1],
                                    [x1, min_h, z1],
                                    [x1, max_h, z1],
                                    [x0, max_h, z1],
                                ]
                            } else {
                                [
                                    [x0, min_h, z1],
                                    [x0, max_h, z1],
                                    [x1, max_h, z1],
                                    [x1, min_h, z1],
                                ]
                            };
                            push_wall_lod(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                south_quad,
                                south_color,
                                if h >= south_h { surface } else { south_surface },
                                double_sided_walls,
                            );
                        }
                        None => {
                            // South neighbor cell is empty — emit south-facing wall on this cell.
                            let min_h = ((h - CHUNK_SIZE).max(chunk_floor)) as f32;
                            let max_h = h as f32 + 1.0;
                            push_wall_lod(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                [
                                    [x0, min_h, z1],
                                    [x1, min_h, z1],
                                    [x1, max_h, z1],
                                    [x0, max_h, z1],
                                ],
                                color,
                                surface,
                                double_sided_walls,
                            );
                        }
                        _ => {}
                    }
                }

                if h < chunk_data.water_level {
                    push_height_quad(
                        &mut mesh.vertices,
                        &mut mesh.indices,
                        x0,
                        x1,
                        z0,
                        z1,
                        chunk_data.water_level as f32 + 1.0,
                        BlockId::Water.color(IVec3::new(ox + lx, chunk_data.water_level, oz + lz)),
                        BlockId::Water,
                    );
                }
            } else {
                // Cell has no solid blocks. Still emit water surface if submerged,
                // and emit walls toward neighbors that have solid blocks above us.
                if chunk_data.water_level > chunk_data.chunk.y * CHUNK_SIZE {
                    push_height_quad(
                        &mut mesh.vertices,
                        &mut mesh.indices,
                        x0,
                        x1,
                        z0,
                        z1,
                        chunk_data.water_level as f32 + 1.0,
                        BlockId::Water.color(IVec3::new(ox + lx, chunk_data.water_level, oz + lz)),
                        BlockId::Water,
                    );
                }

                if include_walls {
                    // Emit walls for neighbors that overhang this empty cell.
                    let chunk_floor = chunk_data.chunk.y * CHUNK_SIZE;
                    let east_cell = lod_cell_top(chunk_data, (lx + step).min(CHUNK_SIZE), lz, step);
                    let south_cell =
                        lod_cell_top(chunk_data, lx, (lz + step).min(CHUNK_SIZE), step);

                    if let Some((east_h, east_surface)) = east_cell {
                        let min_h = ((east_h - CHUNK_SIZE).max(chunk_floor)) as f32;
                        let max_h = east_h as f32 + 1.0;
                        let east_color =
                            east_surface.color(IVec3::new(ox + lx + step, east_h, oz + lz));
                        // East cell is higher; wall faces west (-X) into our empty cell.
                        push_wall_lod(
                            &mut mesh.vertices,
                            &mut mesh.indices,
                            [
                                [x1, min_h, z1],
                                [x1, max_h, z1],
                                [x1, max_h, z0],
                                [x1, min_h, z0],
                            ],
                            east_color,
                            east_surface,
                            double_sided_walls,
                        );
                    }

                    if let Some((south_h, south_surface)) = south_cell {
                        let min_h = ((south_h - CHUNK_SIZE).max(chunk_floor)) as f32;
                        let max_h = south_h as f32 + 1.0;
                        let south_color =
                            south_surface.color(IVec3::new(ox + lx, south_h, oz + lz + step));
                        // South cell is higher; wall faces north (-Z) into our empty cell.
                        push_wall_lod(
                            &mut mesh.vertices,
                            &mut mesh.indices,
                            [
                                [x0, min_h, z1],
                                [x0, max_h, z1],
                                [x1, max_h, z1],
                                [x1, min_h, z1],
                            ],
                            south_color,
                            south_surface,
                            double_sided_walls,
                        );
                    }
                }
            }
        }
    }

    // West/north boundary walls: the main loop only emits east/south walls per cell,
    // relying on the western/northern neighbor to emit the matching east/south wall.
    // When no seam skirt is active (same-LOD neighbor), emit these explicitly using
    // the 1-block padding zone for height comparison.
    if include_walls {
        // West boundary (lx = 0 edge): compare interior cell vs padding column at x = -1
        if !seam_west {
            let chunk_floor = chunk_data.chunk.y * CHUNK_SIZE;
            for lz in (0..CHUNK_SIZE).step_by(step as usize) {
                let z0 = (oz + lz) as f32;
                let z1 = (oz + lz + step).min(oz + CHUNK_SIZE) as f32;
                let cell = lod_cell_top(chunk_data, 0, lz, step);
                // Sample a padding column at the center of the cell to represent the western neighbor height
                let pad_sample =
                    chunk_data.top_solid_block(-1, (lz + (step >> 1)).min(CHUNK_SIZE - 1));
                match cell {
                    Some((h, surface)) => {
                        if pad_sample.0 != i16::MIN as i32 && pad_sample.0 != h {
                            let min_h = pad_sample.0.min(h) as f32 + 1.0;
                            let max_h = pad_sample.0.max(h) as f32 + 1.0;
                            let color = if h < pad_sample.0 {
                                pad_sample
                                    .1
                                    .color(IVec3::new(ox - 1, pad_sample.0, oz + lz))
                            } else {
                                surface.color(IVec3::new(ox, h, oz + lz))
                            };
                            let surf = if h < pad_sample.0 {
                                pad_sample.1
                            } else {
                                surface
                            };
                            let quad = if pad_sample.0 > h {
                                [
                                    [ox as f32, min_h, z0],
                                    [ox as f32, max_h, z0],
                                    [ox as f32, max_h, z1],
                                    [ox as f32, min_h, z1],
                                ]
                            } else {
                                [
                                    [ox as f32, min_h, z0],
                                    [ox as f32, max_h, z0],
                                    [ox as f32, max_h, z1],
                                    [ox as f32, min_h, z1],
                                ]
                            };
                            push_wall_lod(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                quad,
                                color,
                                surf,
                                double_sided_walls,
                            );
                        } else if pad_sample.0 == i16::MIN as i32 {
                            // Western neighbor has no solid blocks — emit west-facing wall on this cell.
                            let min_h = ((h - CHUNK_SIZE).max(chunk_floor)) as f32;
                            let max_h = h as f32 + 1.0;
                            push_wall_lod(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                [
                                    [ox as f32, min_h, z0],
                                    [ox as f32, max_h, z0],
                                    [ox as f32, max_h, z1],
                                    [ox as f32, min_h, z1],
                                ],
                                surface.color(IVec3::new(ox, h, oz + lz)),
                                surface,
                                double_sided_walls,
                            );
                        }
                    }
                    None => {
                        if pad_sample.0 != i16::MIN as i32 {
                            // Interior is empty but western neighbor is solid — emit west-facing wall.
                            let min_h = ((pad_sample.0 - CHUNK_SIZE).max(chunk_floor)) as f32;
                            let max_h = pad_sample.0 as f32 + 1.0;
                            let color =
                                pad_sample
                                    .1
                                    .color(IVec3::new(ox - 1, pad_sample.0, oz + lz));
                            push_wall_lod(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                [
                                    [ox as f32, min_h, z0],
                                    [ox as f32, max_h, z0],
                                    [ox as f32, max_h, z1],
                                    [ox as f32, min_h, z1],
                                ],
                                color,
                                pad_sample.1,
                                double_sided_walls,
                            );
                        }
                    }
                }
            }
        }

        // North boundary (lz = 0 edge): compare interior cell vs padding row at z = -1
        if !seam_north {
            let chunk_floor = chunk_data.chunk.y * CHUNK_SIZE;
            for lx in (0..CHUNK_SIZE).step_by(step as usize) {
                let x0 = (ox + lx) as f32;
                let x1 = (ox + lx + step).min(ox + CHUNK_SIZE) as f32;
                let cell = lod_cell_top(chunk_data, lx, 0, step);
                let pad_sample =
                    chunk_data.top_solid_block((lx + (step >> 1)).min(CHUNK_SIZE - 1), -1);
                match cell {
                    Some((h, surface)) => {
                        if pad_sample.0 != i16::MIN as i32 && pad_sample.0 != h {
                            let min_h = pad_sample.0.min(h) as f32 + 1.0;
                            let max_h = pad_sample.0.max(h) as f32 + 1.0;
                            let color = if h < pad_sample.0 {
                                pad_sample
                                    .1
                                    .color(IVec3::new(ox + lx, pad_sample.0, oz - 1))
                            } else {
                                surface.color(IVec3::new(ox + lx, h, oz))
                            };
                            let surf = if h < pad_sample.0 {
                                pad_sample.1
                            } else {
                                surface
                            };
                            let quad = if pad_sample.0 > h {
                                [
                                    [x0, min_h, oz as f32],
                                    [x0, max_h, oz as f32],
                                    [x1, max_h, oz as f32],
                                    [x1, min_h, oz as f32],
                                ]
                            } else {
                                [
                                    [x0, min_h, oz as f32],
                                    [x0, max_h, oz as f32],
                                    [x1, max_h, oz as f32],
                                    [x1, min_h, oz as f32],
                                ]
                            };
                            push_wall_lod(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                quad,
                                color,
                                surf,
                                double_sided_walls,
                            );
                        } else if pad_sample.0 == i16::MIN as i32 {
                            // Northern neighbor has no solid blocks — emit north-facing wall on this cell.
                            let min_h = ((h - CHUNK_SIZE).max(chunk_floor)) as f32;
                            let max_h = h as f32 + 1.0;
                            push_wall_lod(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                [
                                    [x0, min_h, oz as f32],
                                    [x0, max_h, oz as f32],
                                    [x1, max_h, oz as f32],
                                    [x1, min_h, oz as f32],
                                ],
                                surface.color(IVec3::new(ox + lx, h, oz)),
                                surface,
                                double_sided_walls,
                            );
                        }
                    }
                    None => {
                        if pad_sample.0 != i16::MIN as i32 {
                            // Interior is empty but northern neighbor is solid — emit north-facing wall.
                            let min_h = ((pad_sample.0 - CHUNK_SIZE).max(chunk_floor)) as f32;
                            let max_h = pad_sample.0 as f32 + 1.0;
                            let color =
                                pad_sample
                                    .1
                                    .color(IVec3::new(ox + lx, pad_sample.0, oz - 1));
                            push_wall_lod(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                [
                                    [x0, min_h, oz as f32],
                                    [x0, max_h, oz as f32],
                                    [x1, max_h, oz as f32],
                                    [x1, min_h, oz as f32],
                                ],
                                color,
                                pad_sample.1,
                                double_sided_walls,
                            );
                        }
                    }
                }
            }
        }
    }

    // Seam skirts: vertical quads that drop from the LOD shell edge downward to
    // cover the crack between this coarser LOD chunk and the finer-LOD neighbor.
    // Only emitted when the seam flag is set (neighbor is higher-detail than us).
    if include_walls {
        let skirt_depth = (step * 4) as f32;

        // West skirt (lx = 0 edge, facing -X)
        if seam_west {
            for lz in (0..CHUNK_SIZE).step_by(step as usize) {
                let z0 = (oz + lz) as f32;
                let z1 = (oz + lz + step).min(oz + CHUNK_SIZE) as f32;
                if let Some((h, surface)) = lod_cell_top(chunk_data, 0, lz, step) {
                    let top_y = h as f32 + 1.0;
                    let bot_y = (top_y - skirt_depth).max(chunk_floor_y);
                    let color = surface.color(IVec3::new(ox, h, oz + lz));
                    push_wall_lod(
                        &mut mesh.vertices,
                        &mut mesh.indices,
                        [
                            [ox as f32, bot_y, z0],
                            [ox as f32, top_y, z0],
                            [ox as f32, top_y, z1],
                            [ox as f32, bot_y, z1],
                        ],
                        color,
                        surface,
                        double_sided_walls,
                    );
                }
            }
        }

        // East skirt (lx = CHUNK_SIZE edge, facing +X)
        if seam_east {
            let x_edge = (ox + CHUNK_SIZE) as f32;
            for lz in (0..CHUNK_SIZE).step_by(step as usize) {
                let z0 = (oz + lz) as f32;
                let z1 = (oz + lz + step).min(oz + CHUNK_SIZE) as f32;
                let lx_last = CHUNK_SIZE - step;
                if let Some((h, surface)) = lod_cell_top(chunk_data, lx_last.max(0), lz, step) {
                    let top_y = h as f32 + 1.0;
                    let bot_y = (top_y - skirt_depth).max(chunk_floor_y);
                    let color = surface.color(IVec3::new(ox + CHUNK_SIZE, h, oz + lz));
                    push_wall_lod(
                        &mut mesh.vertices,
                        &mut mesh.indices,
                        [
                            [x_edge, bot_y, z0],
                            [x_edge, top_y, z0],
                            [x_edge, top_y, z1],
                            [x_edge, bot_y, z1],
                        ],
                        color,
                        surface,
                        double_sided_walls,
                    );
                }
            }
        }

        // North skirt (lz = 0 edge, facing -Z)
        if seam_north {
            for lx in (0..CHUNK_SIZE).step_by(step as usize) {
                let x0 = (ox + lx) as f32;
                let x1 = (ox + lx + step).min(ox + CHUNK_SIZE) as f32;
                if let Some((h, surface)) = lod_cell_top(chunk_data, lx, 0, step) {
                    let top_y = h as f32 + 1.0;
                    let bot_y = (top_y - skirt_depth).max(chunk_floor_y);
                    let color = surface.color(IVec3::new(ox + lx, h, oz));
                    push_wall_lod(
                        &mut mesh.vertices,
                        &mut mesh.indices,
                        [
                            [x0, bot_y, oz as f32],
                            [x0, top_y, oz as f32],
                            [x1, top_y, oz as f32],
                            [x1, bot_y, oz as f32],
                        ],
                        color,
                        surface,
                        double_sided_walls,
                    );
                }
            }
        }

        // South skirt (lz = CHUNK_SIZE edge, facing +Z)
        if seam_south {
            let z_edge = (oz + CHUNK_SIZE) as f32;
            for lx in (0..CHUNK_SIZE).step_by(step as usize) {
                let x0 = (ox + lx) as f32;
                let x1 = (ox + lx + step).min(ox + CHUNK_SIZE) as f32;
                let lz_last = CHUNK_SIZE - step;
                if let Some((h, surface)) = lod_cell_top(chunk_data, lx, lz_last.max(0), step) {
                    let top_y = h as f32 + 1.0;
                    let bot_y = (top_y - skirt_depth).max(chunk_floor_y);
                    let color = surface.color(IVec3::new(ox + lx, h, oz + CHUNK_SIZE));
                    push_wall_lod(
                        &mut mesh.vertices,
                        &mut mesh.indices,
                        [
                            [x1, bot_y, z_edge],
                            [x1, top_y, z_edge],
                            [x0, top_y, z_edge],
                            [x0, bot_y, z_edge],
                        ],
                        color,
                        surface,
                        double_sided_walls,
                    );
                }
            }
        }
    }

    mesh
}

fn append_foliage_mesh(mesh: &mut ChunkMesh, chunk_data: &GeneratedChunk) {
    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            for ly in 0..CHUNK_SIZE {
                let local = [lx, ly, lz];
                let block = mesh_block_at(chunk_data, local);
                if !block.render_in_lod_decor() {
                    continue;
                }

                let material =
                    FaceMaterial::from_block(block, world_pos_from_mesh_local(chunk_data, local));

                for (axis, positive, offset) in [
                    (0usize, false, [-1, 0, 0]),
                    (0usize, true, [1, 0, 0]),
                    (1usize, false, [0, -1, 0]),
                    (1usize, true, [0, 1, 0]),
                    (2usize, false, [0, 0, -1]),
                    (2usize, true, [0, 0, 1]),
                ] {
                    let neighbor = mesh_block_at(chunk_data, add_axes(local, offset));
                    if !face_visible(block, neighbor) {
                        continue;
                    }

                    let plane = local[axis] + if positive { 1 } else { 0 };
                    let u_axis = (axis + 1) % 3;
                    let v_axis = (axis + 2) % 3;
                    let mut base = [0; 3];
                    base[axis] = plane;
                    base[u_axis] = local[u_axis];
                    base[v_axis] = local[v_axis];

                    emit_greedy_face(
                        mesh,
                        chunk_data,
                        axis,
                        positive,
                        plane,
                        base[u_axis],
                        base[v_axis],
                        1,
                        1,
                        material,
                    );
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

pub fn build_outline_vertices_many(
    blocks: &[IVec3],
    color: [f32; 4],
    expand: f32,
) -> Vec<LineVertex> {
    let mut out = Vec::with_capacity(blocks.len() * 24);
    for &block in blocks {
        out.extend(build_outline_vertices(block, color, expand));
    }
    out
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32, color: [f32; 3], out: &mut Vec<UiVertex>) {
    out.extend_from_slice(&[
        UiVertex {
            pos: [x0, y0],
            color,
        },
        UiVertex {
            pos: [x1, y0],
            color,
        },
        UiVertex {
            pos: [x1, y1],
            color,
        },
        UiVertex {
            pos: [x0, y0],
            color,
        },
        UiVertex {
            pos: [x1, y1],
            color,
        },
        UiVertex {
            pos: [x0, y1],
            color,
        },
    ]);
}

fn glyph_bits(c: char) -> [u8; 7] {
    match c {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b01110, 0b00000, 0b00000, 0b00000,
        ],
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        _ => [0, 0, 0, 0, 0, 0, 0],
    }
}

fn draw_text(mut x: f32, y: f32, scale: f32, text: &str, color: [f32; 3], out: &mut Vec<UiVertex>) {
    let px = 0.010 * scale;
    for ch in text.chars() {
        let rows = glyph_bits(ch);
        for (row_i, row) in rows.iter().enumerate() {
            for col in 0..5 {
                if (row >> (4 - col)) & 1 == 1 {
                    let x0 = x + col as f32 * px;
                    let y0 = y - row_i as f32 * px;
                    rect(x0, y0, x0 + px, y0 - px, color, out);
                }
            }
        }
        x += px * 6.0;
    }
}

pub fn build_crosshair() -> Vec<UiVertex> {
    let mut out = Vec::new();
    rect(-0.014, -0.0015, 0.014, 0.0015, [0.95, 0.95, 0.95], &mut out);
    rect(-0.0015, -0.022, 0.0015, 0.022, [0.95, 0.95, 0.95], &mut out);
    rect(-0.010, -0.0008, 0.010, 0.0008, [0.15, 0.15, 0.15], &mut out);
    rect(-0.0008, -0.016, 0.0008, 0.016, [0.15, 0.15, 0.15], &mut out);
    out
}

pub fn build_hotbar(selected: BlockId) -> Vec<UiVertex> {
    let mut out = Vec::new();
    let y0 = -0.95;
    let y1 = -0.78;
    let blocks = BlockId::hotbar_defaults();
    let slot_w = 0.20;
    let slot_gap = 0.05;
    let total_w = slot_w * blocks.len() as f32 + slot_gap * blocks.len().saturating_sub(1) as f32;
    let start_x = -total_w * 0.5;

    for (i, block) in blocks.iter().enumerate() {
        let x0 = start_x + i as f32 * (slot_w + slot_gap);
        let x1 = x0 + slot_w;
        let frame = if *block == selected {
            [0.96, 0.92, 0.60]
        } else {
            [0.20, 0.20, 0.22]
        };
        rect(x0, y0, x1, y1, frame, &mut out);
        rect(
            x0 + 0.012,
            y0 + 0.012,
            x1 - 0.012,
            y1 - 0.012,
            block.color(IVec3::ZERO),
            &mut out,
        );
        draw_text(
            x0 + 0.05,
            y0 + 0.09,
            0.7,
            &format!("{}", i + 1),
            [0.0, 0.0, 0.0],
            &mut out,
        );
    }

    out
}

pub fn build_fps_overlay(fps: u32) -> Vec<UiVertex> {
    let mut out = Vec::new();
    draw_text(
        -0.94,
        0.93,
        0.9,
        &format!("FPS {}", fps),
        [1.0, 1.0, 0.55],
        &mut out,
    );
    out
}

pub fn build_pause_overlay() -> Vec<UiVertex> {
    let mut out = Vec::new();
    rect(-0.58, 0.34, 0.58, -0.34, [0.08, 0.08, 0.10], &mut out);
    rect(-0.56, 0.32, 0.56, -0.32, [0.20, 0.20, 0.24], &mut out);
    draw_text(-0.22, 0.28, 1.5, "PAUSED", [0.95, 0.95, 0.95], &mut out);
    draw_text(
        -0.37,
        0.10,
        0.8,
        "LEFT CLICK TO RESUME",
        [0.90, 0.90, 0.85],
        &mut out,
    );
    draw_text(
        -0.31,
        -0.02,
        0.8,
        "ESC TO TOGGLE MENU",
        [0.90, 0.90, 0.85],
        &mut out,
    );
    draw_text(
        -0.24,
        -0.14,
        0.8,
        "DOUBLE TAP JUMP: FLY",
        [0.75, 0.95, 1.0],
        &mut out,
    );
    out
}

pub fn build_world_select_overlay(selected: crate::world::WorldType) -> Vec<UiVertex> {
    let mut out = Vec::new();
    rect(-1.0, 1.0, 1.0, -1.0, [0.05, 0.08, 0.12], &mut out);
    rect(-0.72, 0.72, 0.72, -0.72, [0.08, 0.10, 0.14], &mut out);
    rect(-0.68, 0.68, 0.68, -0.68, [0.16, 0.19, 0.24], &mut out);
    draw_text(
        -0.40,
        0.62,
        1.6,
        "VOXEL FRONTIER",
        [0.92, 0.95, 1.0],
        &mut out,
    );
    draw_text(
        -0.35,
        0.48,
        1.0,
        "SELECT WORLD TYPE",
        [0.98, 0.98, 0.90],
        &mut out,
    );

    let options = [
        (crate::world::WorldType::Superflat, "1 SUPERFLAT CALM"),
        (crate::world::WorldType::Hills, "2 HILLS BALANCED"),
        (crate::world::WorldType::Ridged, "3 RIDGED HARSH"),
    ];

    for (i, (ty, label)) in options.iter().enumerate() {
        let y_top = 0.30 - i as f32 * 0.18;
        let frame = if *ty == selected {
            [0.97, 0.90, 0.56]
        } else {
            [0.24, 0.28, 0.34]
        };
        rect(-0.50, y_top, 0.50, y_top - 0.11, frame, &mut out);
        rect(
            -0.485,
            y_top - 0.01,
            0.485,
            y_top - 0.10,
            [0.12, 0.14, 0.18],
            &mut out,
        );
        draw_text(
            -0.46,
            y_top - 0.03,
            1.0,
            label,
            [0.95, 0.95, 0.95],
            &mut out,
        );
    }

    draw_text(
        -0.52,
        -0.20,
        0.8,
        "ARROW KEYS OR W S TO PICK",
        [0.82, 0.90, 1.0],
        &mut out,
    );
    draw_text(
        -0.52,
        -0.32,
        0.8,
        "PRESS ENTER OR LEFT CLICK TO START",
        [0.95, 0.92, 0.75],
        &mut out,
    );
    draw_text(
        -0.52,
        -0.44,
        0.8,
        "PRESS ESC TO QUIT",
        [0.90, 0.86, 0.80],
        &mut out,
    );
    out
}
