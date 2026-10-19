mod boundary;
pub mod models;
mod memory;
pub use memory::{MeshPlan, PLANNING_BYTES};
mod solid;
mod ground_detail;
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
                if Self::sky_occluder(block) { self.sky_ceiling[column] = self.sky_ceiling[column].max(p.y); }
                else if self.sky_ceiling[column] == p.y {
                    self.sky_ceiling[column] = self.local_roof(local.x, local.z);
                }
            }
        }
    }

    pub fn local_roof(&self, x: i32, z: i32) -> i32 {
        (0..PADDED_CHUNK_SIZE).rev()
            .find(|&y| Self::sky_occluder(self.blocks[Self::local_index(x,y,z)]))
            .map_or(self.origin().y - 1, |y| self.origin().y + y)
    }

    fn sky_occluder(block: BlockId) -> bool {
        block.is_opaque() && !matches!(block, BlockId::Leaves | BlockId::PineLeaves | BlockId::Bush)
    }

    pub fn sky_visibility(&self, pos: Vec3, normal: Vec3) -> f32 {
        if self.sky_ceiling.is_empty() { return 1.0; }
        let air = pos + normal * 0.55;
        let p = (air.floor().as_ivec3() - self.origin())
            .clamp(IVec3::ZERO, IVec3::splat(PADDED_CHUNK_SIZE-1));
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

                emit_oriented_box(mesh, shaft_start, shaft_end, 0.045, wood_color, surface);
                // Dense player builds must fit the minimum worker allowance.
                // Retain every handle and light position, replacing collar/ring
                // detail with a bounded emissive volume above 1024 lights/chunk.
                if compact {
                    let mut flame = surface;
                    flame.detail_strength = -2.0;
                    emit_oriented_box(mesh, flame_center-Vec3::Y*0.05,
                        flame_center+Vec3::Y*0.20, 0.065, flame_color, flame);
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
                        emit_oriented_box(mesh, grip - axis * 0.022, grip + axis * 0.022,
                            0.051, [0.24, 0.13, 0.075], surface);
                    }
                    if block != BlockId::Torch {
                        let wall = block.torch_support_offset().unwrap().as_vec3();
                        let plate = base + Vec3::new(0.5, 0.38, 0.5) + wall * 0.475;
                        emit_oriented_box(mesh, plate - wall * 0.015, plate + wall * 0.015,
                            0.105, [0.18, 0.16, 0.14], metal);
                        emit_oriented_box(mesh, plate - wall * 0.02, shaft_start + axis * 0.10,
                            0.028, [0.28, 0.23, 0.17], metal);
                    }
                }
                for distance in [0.07, 0.19] {
                    if lod >= 2 { continue; }
                    let collar = shaft_end - axis * distance;
                    emit_oriented_box(mesh, collar - axis * 0.018, collar + axis * 0.018,
                        0.061, [0.21, 0.18, 0.14], metal);
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
                let rings = [(-0.08, 0.045, 0.0), (0.01, 0.10, 0.0),
                    (0.13, 0.055, 0.018), (0.27, 0.003, 0.045)];
                // Keep the emissive silhouette stable across chunk LOD swaps.
                // Small non-emissive fittings still disappear in the far mesh.
                let sides = 6;
                for band in 0..3 {
                    let mut flame = surface;
                    flame.detail_strength = -2.0;
                    for side in 0..sides {
                        let point = |ring: usize, side: usize| {
                            let (height, radius, bend) = rings[ring];
                            let angle = side as f32 * std::f32::consts::TAU / sides as f32;
                            (flame_center + Vec3::new(angle.cos() * radius + bend,
                                height, angle.sin() * radius)).to_array()
                        };
                        let quad = [point(band, side), point(band + 1, side),
                            point(band + 1, side + 1), point(band, side + 1)];
                        let color = if band == 0 { [1.0, 0.88, 0.47] }
                            else if band == 1 { flame_color } else { [1.0, 0.29, 0.055] };
                        push_face(&mut mesh.vertices, &mut mesh.indices, quad, color,
                            normal_from_quad(quad), flame, [1.0; 4]);
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
                            [foot + va * 0.025, belly + va * 0.035, belly + vb * 0.035, foot + vb * 0.025],
                            [belly + va * 0.035, tip + va * 0.002, tip + vb * 0.002, belly + vb * 0.035],
                        ] {
                            let quad = quad.map(|v| v.to_array());
                            push_face(&mut mesh.vertices, &mut mesh.indices, quad,
                                [1.0, 0.42, 0.09], normal_from_quad(quad), flame, [1.0; 4]);
                        }
                    }
                }
                scale_torch_vertices(mesh, first_vertex, base, block, chunk_data.voxel_meters);
            }
        }
    }
}

fn scale_torch_vertices(mesh: &mut ChunkMesh, first: usize, cell: Vec3, block: BlockId, scale: f32) {
    for vertex in &mut mesh.vertices[first..] {
        let local = Vec3::from_array(vertex.pos) - cell;
        vertex.pos = (block.torch_point_meters(cell, local, scale) / scale).to_array();
    }
}

fn append_water_mesh(mesh: &mut ChunkMesh, chunk: &GeneratedChunk) {
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
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let p = chunk.chunk * CHUNK_SIZE + IVec3::new(x, y, z);
                if chunk.get_world(p) != BlockId::Water {
                    continue;
                }
                let h = volume(p);
                let base = p.as_vec3();
                for axis in 0..3 {
                    for positive in [false, true] {
                        let mut offset = IVec3::ZERO;
                        offset[axis] = if positive { 1 } else { -1 };
                        let neighbor = chunk.get_world(p + offset);
                        let mut low = 0.0;
                        if axis == 1 {
                            if positive && h >= 1.0 && neighbor == BlockId::Water {
                                continue;
                            }
                            if !positive && !neighbor.is_empty() {
                                continue;
                            }
                            if positive && neighbor.is_opaque() && h >= 1.0 {
                                continue;
                            }
                        } else {
                            if neighbor.is_opaque() {
                                continue;
                            }
                            low = volume(p + offset);
                            if low >= h {
                                continue;
                            }
                        }
                        let u = (axis + 1) % 3;
                        let v = (axis + 2) % 3;
                        let mut a = Vec3::ZERO;
                        a[axis] = if positive {
                            if axis == 1 { h } else { 1.0 }
                        } else {
                            0.0
                        };
                        if axis != 1 {
                            a.y = low;
                        }
                        let mut du = Vec3::ZERO;
                        du[u] = if u == 1 { h - low } else { 1.0 };
                        let mut dv = Vec3::ZERO;
                        dv[v] = if v == 1 { h - low } else { 1.0 };
                        let corners = [base + a, base + a + du, base + a + du + dv, base + a + dv];
                        let quad = if positive {
                            corners
                        } else {
                            [corners[3], corners[2], corners[1], corners[0]]
                        };
                        push_face(
                            &mut mesh.vertices,
                            &mut mesh.indices,
                            quad.map(|p| p.to_array()),
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
