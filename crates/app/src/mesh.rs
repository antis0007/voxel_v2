mod boundary;
mod connectivity;
pub use connectivity::ChunkConnectivity;
mod memory;
pub mod models;
pub use memory::{MeshPlan, PLANNING_BYTES, encode_water};
mod canopy;
mod leaf_sprays;
mod ground_detail;
pub mod mining;
mod solid;
pub mod water_motion;
pub(crate) mod prop_palette;
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
pub use engine_core::PADDED_CHUNK_SIZE;
#[cfg(test)]
mod coarse_seams;
#[cfg(test)]
mod terrain_contract;
/// Vertical inset (in voxel units) keeping free water surfaces recessed below
/// the top edge of surrounding bank and shoreline blocks.
pub const WATER_SURFACE_INSET: f32 = 0.135;
pub const LAVA_SURFACE_INSET: f32 = 0.08;
pub const ACID_SURFACE_INSET: f32 = 0.10;

pub(crate) fn free_water_height(fill: f32) -> f32 {
    fill.clamp(0.0, 1.0) * (1.0 - WATER_SURFACE_INSET)
}
/// `ao_strength` marking a water wall vertex that sits on the free surface. The
/// shader rides such vertices on the same waves as the top face. Water lighting
/// clamps ao to [0, 1], so the value is invisible to shading.
pub const WATER_SURFACE_RIM_AO: f32 = 1.5;

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
    /// World generator that built this chunk. Torch meshes read it for local up.
    pub generator_version: u32,
    pub(crate) flora_config: Option<engine_voxel::WorldSampleConfig>,
    /// Pin this chunk's stair vertices onto the shell cylinder.
    pub manyfold_shell: bool,
    pub water_levels: std::collections::HashMap<IVec3, u8>,
    pub water_deficit: std::collections::HashMap<IVec3, u64>,
    /// Fills the last mesh showed for cells patched since; drives the surface
    /// animation between simulation steps. Empty for freshly built sources.
    pub water_motion: water_motion::WaterMotion,
    pub chunk: IVec3,
    pub blocks: Vec<BlockId>,
    /// Highest opaque cell per padded column; generated terrain supplies roofs
    /// above this chunk too. Lighting metadata, never collision geometry.
    pub sky_ceiling: Vec<i32>,
    /// Terrain surface height per padded column from the generator, kept even
    /// when it lies below this chunk. Water that runs off the bottom of the
    /// chunk uses it to finish its depth, so neighbouring chunks of one ocean
    /// agree with each other and with the horizon mesh. Empty when unknown.
    pub bed_height: Vec<i32>,
    /// Generator 30+: the true seabed height above or below `bed_height`, in
    /// voxels (-0.5..=0.5), for padded columns that hold water. Whole-voxel beds
    /// make a gentle shelf step in one colour ring per voxel; adding this keeps
    /// the water depth continuous. Empty for older generators and dry chunks.
    pub bed_frac: Vec<f32>,
    /// Generated trees whose crowns can reach this chunk's columns. Ground
    /// cover reads root zones and canopy shade from it. Empty when unknown.
    pub trees: Vec<TreeFootprint>,
}

/// Trunk base and crown extent of one generated tree, in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeFootprint {
    pub x: f32,
    pub z: f32,
    pub base_y: f32,
    pub reach: f32,
    pub crown_low: f32,
    pub top: f32,
    pub rainforest: bool,
}

impl GeneratedChunk {
    /// Conservative allocation estimate including hash buckets and control bytes.
    pub fn allocation_bytes(&self) -> usize {
        self.blocks.capacity() * std::mem::size_of::<BlockId>()
            + self.sky_ceiling.capacity() * std::mem::size_of::<i32>()
            + self.bed_height.capacity() * std::mem::size_of::<i32>()
            + self.bed_frac.capacity() * std::mem::size_of::<f32>()
            + self.trees.capacity() * std::mem::size_of::<TreeFootprint>()
            + self.water_levels.capacity() * (std::mem::size_of::<(IVec3, u8)>() + 1) * 2
            + self.water_deficit.capacity() * (std::mem::size_of::<(IVec3, u64)>() + 1) * 2
            + self.water_motion.allocation_bytes()
    }

    pub fn new(chunk: IVec3, blocks: Vec<BlockId>) -> Self {
        Self {
            voxel_meters: 1.0,
            generator_version: 0,
            flora_config: None,
            manyfold_shell: false,
            chunk,
            blocks,
            sky_ceiling: Vec::new(),
            bed_height: Vec::new(),
            bed_frac: Vec::new(),
            trees: Vec::new(),
            water_levels: Default::default(),
            water_deficit: Default::default(),
            water_motion: Default::default(),
        }
    }

    pub fn origin(&self) -> IVec3 {
        self.chunk * CHUNK_SIZE - IVec3::splat(CHUNK_PAD)
    }

    pub fn water_fill(&self, p: IVec3) -> f32 {
        let volume = f64::from(self.voxel_meters).powi(3) * 1_000_000_000.0;
        (f64::from(self.water_levels.get(&p).copied().unwrap_or(8)) / 8.0
            - self.water_deficit.get(&p).copied().unwrap_or(0) as f64 / volume) as f32
    }

    /// Torch blocks in this chunk's own cells. The halo belongs to the neighbour,
    /// which reports those cells itself.
    pub fn torch_cells(&self) -> Vec<(IVec3, BlockId)> {
        let origin = self.origin();
        let mut out = Vec::new();
        for y in CHUNK_PAD..CHUNK_PAD + CHUNK_SIZE {
            for z in CHUNK_PAD..CHUNK_PAD + CHUNK_SIZE {
                for x in CHUNK_PAD..CHUNK_PAD + CHUNK_SIZE {
                    let block = self.blocks[Self::local_index(x, y, z)];
                    if block.is_light_source() {
                        out.push((origin + IVec3::new(x, y, z), block));
                    }
                }
            }
        }
        out
    }

    fn local_index(x: i32, y: i32, z: i32) -> usize {
        ((y * PADDED_CHUNK_SIZE + z) * PADDED_CHUNK_SIZE + x) as usize
    }

    pub fn get_world(&self, p: IVec3) -> BlockId {
        self.get_padded(p - self.origin())
    }

    #[inline]
    fn get_padded(&self, local: IVec3) -> BlockId {
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
        let base = Self::local_index(x, 0, z);
        let plane = (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize;
        let bottom = self.origin().y;
        (0..PADDED_CHUNK_SIZE)
            .rev()
            .find(|&y| Self::sky_occluder(self.blocks[base + y as usize * plane]))
            .map_or(bottom - 1, |y| bottom + y)
    }

    fn sky_occluder(block: BlockId) -> bool {
        block.is_opaque() && !matches!(block, BlockId::Leaves | BlockId::PineLeaves | BlockId::Bush)
    }

    pub fn sky_visibility(&self, pos: Vec3, normal: Vec3) -> f32 {
        if self.sky_ceiling.is_empty() {
            return 1.0;
        }
        let air = pos + normal * 0.55;
        // Columns describe cell centres. Reconstruct visibility at the shared
        // vertex, rather than selecting one side of a roof edge with floor().
        // Average transmitted light, not roof heights (which invents a roof).
        let local = air - self.origin().as_vec3() - Vec3::new(0.5, 0.0, 0.5);
        let cell = local.floor().as_ivec3();
        let fraction = local - local.floor();
        let sample = |dx: i32, dz: i32| {
            let x = (cell.x + dx).clamp(0, PADDED_CHUNK_SIZE - 1);
            let z = (cell.z + dz).clamp(0, PADDED_CHUNK_SIZE - 1);
            let roof = self.sky_ceiling[(z * PADDED_CHUNK_SIZE + x) as usize] as f32;
            let cover_m = ((roof + 0.5 - air.y) * self.voxel_meters).max(0.0);
            if cover_m == 0.0 {
                1.0
            } else {
                (-(cover_m * 0.48)).exp().max(0.006)
            }
        };
        let low = sample(0, 0) * (1.0 - fraction.x) + sample(1, 0) * fraction.x;
        let high = sample(0, 1) * (1.0 - fraction.x) + sample(1, 1) * fraction.x;
        low * (1.0 - fraction.z) + high * fraction.z
    }

    /// Depth in metres of a water column that is still wet at the chunk's bottom
    /// row, `cells` voxels deep so far, finished down to the generated seabed.
    /// `None` when the bed is unknown or inside this chunk (a cave or lake that
    /// continues below), where the caller keeps its conservative floor.
    fn depth_below_chunk(&self, p: IVec3, cells: f32) -> Option<f32> {
        let local = p - self.origin();
        let in_column = (0..PADDED_CHUNK_SIZE).contains(&local.x)
            && (0..PADDED_CHUNK_SIZE).contains(&local.z);
        let bed = *self
            .bed_height
            .get((local.z * PADDED_CHUNK_SIZE + local.x) as usize)
            .filter(|_| in_column)?;
        // `p.y` is the first row below the chunk; the bed block is at `bed`.
        (bed < p.y).then(|| {
            ((cells + (p.y - bed) as f32) * self.voxel_meters - self.bed_correction(p, None))
                .max(0.02 * self.voxel_meters)
        })
    }

    /// Metres to take off a depth that ends on the generated bed, so the bed sits
    /// at its true fractional height. Zero when the column was edited (the bed row
    /// no longer matches the generated one) or carries no fraction.
    fn bed_correction(&self, p: IVec3, bed_row: Option<i32>) -> f32 {
        let local = p - self.origin();
        if self.bed_frac.is_empty()
            || !(0..PADDED_CHUNK_SIZE).contains(&local.x)
            || !(0..PADDED_CHUNK_SIZE).contains(&local.z)
        {
            return 0.0;
        }
        let i = (local.z * PADDED_CHUNK_SIZE + local.x) as usize;
        match (self.bed_height.get(i), self.bed_frac.get(i)) {
            (Some(&bed), Some(&frac)) if bed_row.is_none_or(|row| row == bed) => {
                frac * self.voxel_meters
            }
            _ => 0.0,
        }
    }

    #[cfg(test)]
    pub fn water_optics(&self, pos: Vec3, normal: Vec3) -> (f32, [f32; 3]) {
        let (depth, color, _) = self.water_surface_optics(pos, normal);
        (depth, color)
    }

    fn water_surface_optics(&self, pos: Vec3, normal: Vec3) -> (f32, [f32; 3], u8) {
        // A centimetre probe falls through thin films into their substrate.
        let inside = pos - normal * 0.00001;
        let mut depth = 0.0;
        let mut water_samples = 0;
        let mut solid_shore_samples = 0;
        let mut bed_color = Vec3::ZERO;
        let mut bed_rise = glam::Vec2::ZERO;
        let floor_y = self.origin().y;
        // Sample all 4 cells around a shared vertex corner. Adjacent banks
        // contribute bed colour while depth comes only from actual water.
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
                // Match the rendered body: water with water above fills this
                // cell even when simulation retains a fractional volume.
                let mut cells = if self.get_world(p + IVec3::Y) == BlockId::Water {
                    1.0
                } else {
                    self.water_fill(p)
                };
                p.y -= 1;
                while p.y >= floor_y && self.get_world(p) == BlockId::Water {
                    cells += 1.0;
                    p.y -= 1;
                }
                let meters = cells * self.voxel_meters;
                let bed = self.get_world(p);
                // Reuse the existing depth walk. Unknown beds beneath this
                // source halo are deep enough that their tint is attenuated.
                let bed = if bed.is_opaque() { bed } else { BlockId::Sand };
                bed_color += Vec3::from_array(bed.color(p));
                let column_depth = if p.y < floor_y {
                    self.depth_below_chunk(p, cells).unwrap_or_else(|| meters.max(10.0))
                } else {
                    (meters - self.bed_correction(p, Some(p.y))).max(0.001 * self.voxel_meters)
                };
                depth += column_depth;
                bed_rise -= glam::Vec2::new(dx, dz) * column_depth;
                water_samples += 1;
            }
        }
        if water_samples == 0 {
            (10.0, BlockId::Sand.color(inside.floor().as_ivec3()), 0)
        } else {
            let total = (water_samples + solid_shore_samples) as f32;
            // Banks contribute substrate colour, never imaginary water depth.
            let mut avg_depth = depth / water_samples as f32;
            if solid_shore_samples > 0 {
                let shore_cap = (0.12 + 0.11 * water_samples as f32) * self.voxel_meters;
                avg_depth = avg_depth.min(shore_cap);
            }
            // Air beside an exposed water wall is not a shallow bed. Clamping
            // incomplete corners to 18 cm erased the real column depth even
            // on two-metre flow steps, tinting their whole side like the bed.
            let shore_direction = if avg_depth > 1.5 || bed_rise.abs().max_element() < 0.01 {
                0
            } else if bed_rise.x.abs() >= bed_rise.y.abs() {
                if bed_rise.x > 0.0 { 1 } else { 2 }
            } else if bed_rise.y > 0.0 {
                3
            } else {
                4
            };
            (avg_depth, (bed_color / total).to_array(), shore_direction)
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct FaceMaterial {
    block: BlockId,
    color_bits: [u32; 3],
    water_depth_bucket: u8,
    capped_turf: bool,
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
            capped_turf: false,
        }
    }

    fn surface(self) -> crate::material::SurfaceDetail {
        let mut surface = self.block.surface();
        if self.capped_turf {
            surface.texture_kind = crate::material::terrain_family::CAPPED_TURF;
        }
        surface
    }

    fn color(self) -> [f32; 3] {
        [
            f32::from_bits(self.color_bits[0]),
            f32::from_bits(self.color_bits[1]),
            f32::from_bits(self.color_bits[2]),
        ]
    }
}

/// Water for a submerged plant cell, so its faces mesh as the water it stands in.
fn water_for_plant(block: BlockId) -> BlockId {
    if engine_content::flora::is_submerged(block) { BlockId::Water } else { block }
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
    chunk_data.get_padded(IVec3::from_array(local) + IVec3::splat(CHUNK_PAD))
}

fn add_axes(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn mesh_vertex(chunk_data: &GeneratedChunk, local: [i32; 3]) -> [f32; 3] {
    let p = world_pos_from_mesh_local(chunk_data, local);
    let mut out = [p.x as f32, p.y as f32, p.z as f32];
    if chunk_data.manyfold_shell
        && let Some(curve) = engine_worldgen::manyfold::shell_curve(chunk_data.generator_version)
    {
        let metres = Vec3::from_array(out) * chunk_data.voxel_meters;
        out = (curve.project(metres) / chunk_data.voxel_meters).to_array();
    }
    out
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

    let surface_inset = match material.block {
        BlockId::Water => WATER_SURFACE_INSET,
        BlockId::Lava => LAVA_SURFACE_INSET,
        BlockId::SulfuricAcid => ACID_SURFACE_INSET,
        _ => 0.0,
    };
    if surface_inset > 0.0 {
        if axis == 1 && positive {
            for p in [&mut a, &mut b, &mut c, &mut d] {
                p[1] -= surface_inset;
            }
        } else if axis != 1 {
            let mut top_cell = base;
            top_cell[axis] = if positive { plane - 1 } else { plane };
            top_cell[1] += if u_axis == 1 { width - 1 } else { height - 1 };
            let mut above = top_cell;
            above[1] += 1;
            if mesh_block_at(chunk_data, above) != material.block {
                if u_axis == 1 {
                    b[1] -= surface_inset;
                    c[1] -= surface_inset;
                } else {
                    c[1] -= surface_inset;
                    d[1] -= surface_inset;
                }
            }
        }
    }

    let quad = if positive { [a, b, c, d] } else { [d, c, b, a] };
    let axis_normal = face_normal_from_axis(axis, positive);
    let face_normal = if chunk_data.manyfold_shell {
        let curved = normal_from_quad(quad);
        if Vec3::from_array(curved).length_squared() > 1e-8 { curved } else { axis_normal }
    } else {
        axis_normal
    };
    let corner_ao = face_corner_ao(chunk_data, axis, positive, plane, u0, v0, width, height);

    let water_top = material.block == BlockId::Water && axis == 1 && positive;
    let border = |coordinate: i32| coordinate == 0 || coordinate == CHUNK_SIZE;
    let steps = [
        if water_top || border(v0) { width } else { 1 },
        if water_top || border(u0 + width) { height } else { 1 },
        if water_top || border(v0 + height) { width } else { 1 },
        if water_top || border(u0) { height } else { 1 },
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
        face_normal,
        corner_ao,
        if axis == 1 && positive { surface_inset } else { 0.0 },
    ) {
        return;
    }

    push_face(
        &mut mesh.vertices,
        &mut mesh.indices,
        quad,
        material.color(),
        face_normal,
        material.surface(),
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

/// Sets colour and flame lift on the corners of a flame quad. Flame vertices
/// carry their height above the flame base as `surface[3] = -2 - lift` (lift
/// 0 at the base, 1 at the tip). The shader reads it back to sway the tip and
/// time the licks, and every `< -1.5` flame test still matches.
fn shade_flame_corners(
    mesh: &mut ChunkMesh,
    first: usize,
    colors: [[f32; 3]; 4],
    lifts: [f32; 4],
) {
    for ((vertex, color), lift) in mesh.vertices[first..].iter_mut().zip(colors).zip(lifts) {
        vertex.color = color;
        vertex.surface[3] = -2.0 - lift.clamp(0.0, 1.0);
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
    // Choose the AO diagonal instead of exposing a fixed triangular crease at
    // corners where two occluders meet.
    if corner_ao[0] + corner_ao[2] > corner_ao[1] + corner_ao[3] {
        indices.extend_from_slice(&[base, base + 1, base + 3, base + 1, base + 2, base + 3]);
    } else {
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
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

/// Chunk-local height (exclusive top) of the highest opaque terrain voxel in
/// each column, or `i32::MIN` when the column has none.
fn column_tops(chunk: &GeneratedChunk) -> [i32; (CHUNK_SIZE * CHUNK_SIZE) as usize] {
    let mut tops = [i32::MIN; (CHUNK_SIZE * CHUNK_SIZE) as usize];
    for z in 0..CHUNK_SIZE {
        let mut unfinished = (1u64 << CHUNK_SIZE) - 1;
        let row_start = (z * CHUNK_SIZE) as usize;
        for y in (0..CHUNK_SIZE).rev() {
            // Scan the contiguous X row and drop columns as soon as their
            // first terrain voxel is found.
            let start = GeneratedChunk::local_index(CHUNK_PAD, y + CHUNK_PAD, z + CHUNK_PAD);
            let row = &chunk.blocks[start..start + CHUNK_SIZE as usize];
            let mut pending = unfinished;
            while pending != 0 {
                let x = pending.trailing_zeros() as usize;
                let b = row[x];
                if b != BlockId::Air && b.is_opaque() && !coarse_vegetation(b) {
                    tops[row_start + x] = y + 1;
                    unfinished &= !(1 << x);
                }
                pending &= pending - 1;
            }
            if unfinished == 0 {
                break;
            }
        }
    }
    tops
}

/// Median surface of an XZ coarse column, shared by all of its vertical cells.
fn terrain_median(tops: &[i32], x0: i32, z0: i32, step: i32) -> i32 {
    let mut seen = [0; 8 * 8];
    let mut len = 0;
    for dz in 0..step {
        for dx in 0..step {
            let (x, z) = (x0 + dx, z0 + dz);
            if (0..CHUNK_SIZE).contains(&x) && (0..CHUNK_SIZE).contains(&z) {
                let top = tops[(z * CHUNK_SIZE + x) as usize];
                if top != i32::MIN {
                    seen[len] = top;
                    len += 1;
                }
            }
        }
    }
    if len == 0 {
        return i32::MAX;
    }
    let (_, median, _) = seen[..len].select_nth_unstable(len / 2);
    *median
}

/// Surface block of the median column in a coarse footprint.
///
/// A coarse cell takes the highest opaque sample it holds. Cells above the
/// median are dropped, so the exposed cap is often a cell lying wholly under
/// the one-voxel turf, and slopes drew bands of Dirt where the median crossed
/// each cell level.
fn footprint_surface(chunk: &GeneratedChunk, tops: &[i32], x0: i32, z0: i32, step: i32) -> Option<BlockId> {
    let mut seen = [(0, 0, 0); 8 * 8];
    let mut len = 0;
    for dz in 0..step {
        for dx in 0..step {
            let (x, z) = (x0 + dx, z0 + dz);
            if (0..CHUNK_SIZE).contains(&x) && (0..CHUNK_SIZE).contains(&z) {
                let top = tops[(z * CHUNK_SIZE + x) as usize];
                if top != i32::MIN {
                    seen[len] = (top, x, z);
                    len += 1;
                }
            }
        }
    }
    if len == 0 {
        return None;
    }
    let (_, &mut (top, x, z), _) = seen[..len].select_nth_unstable_by_key(len / 2, |s| s.0);
    let b = chunk.blocks[GeneratedChunk::local_index(x + CHUNK_PAD, top - 1 + CHUNK_PAD, z + CHUNK_PAD)];
    (b.is_opaque() && !coarse_vegetation(b)).then_some(b)
}

#[cfg(test)]
fn terrain_reaches(tops: &[i32], x0: i32, z0: i32, step: i32, bottom: i32) -> bool {
    // The cell centre decides. A bottom test rounds every surface up to the next
    // cell boundary, which is the same bias as before, one cell tall.
    2 * bottom + step < terrain_median(tops, x0, z0, step).saturating_mul(2)
}

// At the bottom coarse cell, only the single halo layer below it is known.
fn bottom_halo_supports(chunk: &GeneratedChunk, x: i32, z: i32, step: i32) -> bool {
    (z..z + step).any(|z| {
        let start = GeneratedChunk::local_index(x + CHUNK_PAD, CHUNK_PAD - 1, z + CHUNK_PAD);
        chunk.blocks[start..start + step as usize]
            .iter()
            .copied()
            .any(|b| b.is_opaque() && !coarse_vegetation(b))
    })
}

/// Whether one voxel layer of a coarse cell is all terrain.
fn seam_slab_sealed(chunk: &GeneratedChunk, min: IVec3, step: i32, axis: usize, layer: i32) -> bool {
    // The tangential coordinates are inside the chunk, and the normal
    // coordinate is either on its edge or in the single-voxel halo.
    let mut local = min - chunk.origin();
    local[axis] = layer - chunk.origin()[axis];
    let base = GeneratedChunk::local_index(local.x, local.y, local.z);
    let size = PADDED_CHUNK_SIZE as usize;
    let (u, v) = match axis {
        0 => (size * size, size),
        1 => (size, 1),
        _ => (1, size * size),
    };
    (0..step).all(|du| {
        let row = base + du as usize * u;
        (0..step).all(|dv| {
            let b = chunk.blocks[row + dv as usize * v];
            b.is_opaque() && !coarse_vegetation(b)
        })
    })
}

/// Terrain keeps the highest opaque sample. Canopy needs a share of the cell,
/// excluding any voxels outside the padded halo.
fn coarse_cell(chunk: &GeneratedChunk, min: IVec3, step: i32) -> Option<BlockId> {
    let local = min - chunk.origin();
    let padded_limit = IVec3::splat(PADDED_CHUNK_SIZE);
    let lo = local.clamp(IVec3::ZERO, padded_limit);
    let hi = (local + IVec3::splat(step)).clamp(IVec3::ZERO, padded_limit);
    let size = hi - lo;
    if size.min_element() == 0 {
        return None;
    }
    scan_coarse_cell(chunk, lo, hi, step)
}

fn scan_coarse_cell(chunk: &GeneratedChunk, lo: IVec3, hi: IVec3, step: i32) -> Option<BlockId> {
    let size = hi - lo;
    let known = (size.x * size.y * size.z) as u32;
    let mut liquid = None;
    let (mut canopy, mut wood) = (0u32, 0u32);
    let mut canopy_block = None;
    // The last opaque sample in forward order wins. Scan backward so a
    // solid coarse cell finishes as soon as its surface material is found.
    for y in (lo.y..hi.y).rev() {
        for z in (lo.z..hi.z).rev() {
            let start = GeneratedChunk::local_index(lo.x, y, z);
            let row = &chunk.blocks[start..start + size.x as usize];
            for &sample in row.iter().rev() {
                if sample == BlockId::Air {
                    continue;
                }
                if sample == BlockId::Log {
                    wood += 1;
                } else if coarse_vegetation(sample) {
                    canopy += 1;
                    canopy_block.get_or_insert(sample);
                } else if sample.is_opaque() {
                    return Some(sample);
                } else if sample.is_liquid() {
                    // Liquids keep the first sample in forward order.
                    liquid = Some(sample);
                }
            }
        }
    }
    // Coverage is at least 3/8 for canopy, or 1/step² for a trunk.
    if canopy > 0 && (canopy + wood) * 8 >= known * 3 {
        return canopy_block;
    }
    if wood * (step * step) as u32 >= known {
        return Some(BlockId::Log);
    }
    liquid
}

/// Coarse cells for distant chunks. Terrain cells are solid if any voxel in them
/// is, so cliffs stay sealed and clouds cannot show through the gaps a heightfield
/// leaves; vegetation uses coverage instead (see `coarse_cell`). Boundary faces
/// overlap the neighbour by a fraction of a cell, which hides LOD cracks without
/// a second full-resolution mesh.
#[cfg(test)]
pub fn coarsened_mesh(chunk: &GeneratedChunk, lod: u8) -> ChunkMesh {
    build_coarsened_mesh(chunk, lod, chunk.blocks.contains(&BlockId::Water))
}

pub(crate) fn coarsened_dry_mesh(chunk: &GeneratedChunk, lod: u8) -> ChunkMesh {
    build_coarsened_mesh(chunk, lod, false)
}

fn build_coarsened_mesh(chunk: &GeneratedChunk, lod: u8, has_water: bool) -> ChunkMesh {
    let step = 1i32 << lod.clamp(1, 3);
    let terrain = coarse_grid(chunk, step);
    let canopy = canopy::Canopy::new(chunk);
    let mut mesh = ChunkMesh::empty();
    append_coarsened_layer(chunk, step, &terrain, false, &canopy, &mut mesh);
    if has_water {
        append_water_mesh(&mut mesh, chunk);
    }
    // Terrain may simplify to eight-voxel cells. Applying that same grid to
    // thin trunks and crowns inflates trees into boxes and changes their
    // silhouette at every terrain LOD boundary. Keep one bounded forest grid.
    if step == 2 {
        append_coarsened_layer(chunk, 2, &terrain, true, &canopy, &mut mesh);
    } else if chunk.blocks.iter().copied().any(coarse_vegetation) {
        let foliage = coarse_grid(chunk, 2);
        append_coarsened_layer(chunk, 2, &foliage, true, &canopy, &mut mesh);
    }
    mesh
}

fn coarse_grid(chunk: &GeneratedChunk, step: i32) -> Vec<BlockId> {
    let n = CHUNK_SIZE / step;
    let mut cells = Vec::with_capacity((n * n * n) as usize);
    for y in 0..n {
        for z in 0..n {
            for x in 0..n {
                let lo = IVec3::new(x, y, z) * step + IVec3::splat(CHUNK_PAD);
                let hi = lo + IVec3::splat(step);
                cells.push(scan_coarse_cell(chunk, lo, hi, step).unwrap_or(BlockId::Air));
            }
        }
    }
    cells
}

fn append_coarsened_layer(
    chunk: &GeneratedChunk,
    step: i32,
    cells: &[BlockId],
    vegetation_only: bool,
    canopy: &canopy::Canopy,
    mesh: &mut ChunkMesh,
) {
    let in_layer = |b: BlockId| b != BlockId::Air && coarse_vegetation(b) == vegetation_only;
    if !cells.iter().copied().any(in_layer) {
        return;
    }
    let n = CHUNK_SIZE / step;
    let origin = chunk.chunk * CHUNK_SIZE;
    let index = |x: i32, y: i32, z: i32| ((y * n + z) * n + x) as usize;
    let mut block = vec![BlockId::Air; cells.len()];
    let tops = (!vegetation_only).then(|| column_tops(chunk));
    for cz in 0..n {
        for cx in 0..n {
            let mut median_twice = None;
            for cy in 0..n {
                let id = index(cx, cy, cz);
                let b = cells[id];
                if !in_layer(b) {
                    continue;
                }
                let cell = IVec3::new(cx, cy, cz);
                let min = origin + cell * step;
                if vegetation_only
                    || b.is_liquid()
                    || 2 * cy * step + step < *median_twice.get_or_insert_with(|| {
                        let tops = tops.as_ref().expect("solid layer has column tops");
                        terrain_median(tops, cx * step, cz * step, step).saturating_mul(2)
                    })
                    // The lower coarse cell already scanned this exact volume
                    // and prefers an opaque terrain block over every other sample.
                    || if cy == 0 {
                        !bottom_halo_supports(chunk, cx * step, cz * step, step)
                    } else {
                        let below = cells[index(cx, cy - 1, cz)];
                        !below.is_opaque() || coarse_vegetation(below)
                    }
                    || (0..3).any(|axis| {
                        let layer = match cell[axis] {
                            0 => 0,
                            c if c == n - 1 => CHUNK_SIZE - 1,
                            _ => return false,
                        };
                        seam_slab_sealed(chunk, min, step, axis, origin[axis] + layer)
                    })
                {
                    block[id] = b;
                }
            }
        }
    }
    if let Some(tops) = tops.as_ref() {
        for cz in 0..n {
            for cx in 0..n {
                let Some(cy) = (0..n).rev().find(|&cy| {
                    let b = block[index(cx, cy, cz)];
                    b != BlockId::Air && !b.is_liquid()
                }) else { continue };
                // A buried cap under a chunk-filling column belongs to the chunk above.
                let id = index(cx, cy, cz);
                if cy + 1 < n || terrain_median(tops, cx * step, cz * step, step) < CHUNK_SIZE {
                    if let Some(surface) = footprint_surface(chunk, tops, cx * step, cz * step, step) {
                        block[id] = surface;
                    }
                }
            }
        }
    }
    // The neighbour's median needs columns beyond the halo, so a seam face is
    // culled only when the halo slab is sealed: the neighbour then keeps that
    // cell by the rule above, and the two chunks cannot both skip the wall.
    // Crowns still continue across seams by coverage.
    let occupied = |own: BlockId, x: i32, y: i32, z: i32| -> bool {
        let cell = IVec3::new(x, y, z);
        if cell.cmpge(IVec3::ZERO).all() && cell.cmplt(IVec3::splat(n)).all() {
            let i = index(x, y, z);
            // The exact water sheet is transparent and may occupy only a
            // fraction of this cell. It cannot seal the ground below it.
            return !matches!(block[i], BlockId::Air | BlockId::Water);
        }
        let min = origin + cell * step;
        let outside = (0..3).find(|&axis| !(0..n).contains(&cell[axis]));
        let sealed = outside.is_some_and(|axis| {
            let layer = if cell[axis] < 0 { -1 } else { CHUNK_SIZE };
            seam_slab_sealed(chunk, min, step, axis, origin[axis] + layer)
        });
        sealed
            || (coarse_vegetation(own)
                && coarse_cell(chunk, min, step).is_some_and(coarse_vegetation))
    };
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
                normal: [f32; 3]| {
        // Keep coarse walls and tops on the same grid plane. Moving only the
        // perimeter wall outward separates its top edge from the cap.
        let quad = [
            mesh_vertex(chunk, [x0, y0, z0]),
            mesh_vertex(chunk, [x1, y1, z1]),
            mesh_vertex(chunk, [x2, y2, z2]),
            mesh_vertex(chunk, [x3, y3, z3]),
        ];
        let face = FaceMaterial::from_block(material, origin + IVec3::new(x0, y0, z0));
        let first = mesh.vertices.len();
        push_face(
            &mut mesh.vertices,
            &mut mesh.indices,
            quad,
            face.color(),
            normal,
            material.surface(),
            [0.94; 4],
        );
        // Use the near mesh's lighting metadata, sampled once per coarse face.
        // Sample in voxel coordinates before any seam overlap or projection.
        let center = origin.as_vec3()
            + Vec3::new((x0 + x2) as f32, (y0 + y2) as f32, (z0 + z2) as f32) * 0.5;
        let normal = Vec3::from_array(normal);
        let sky = 4.0 + chunk.sky_visibility(center, normal);
        let shade = if canopy.is_empty() {
            1.0
        } else {
            canopy.visibility(center + normal * 0.3)
        };
        for vertex in &mut mesh.vertices[first..] {
            vertex.surface[3] = sky;
            vertex.ao_strength *= shade;
        }
    };
    for cz in 0..n {
        for cy in 0..n {
            for cx in 0..n {
                let id = index(cx, cy, cz);
                if block[id] == BlockId::Air {
                    continue;
                }
                let material = block[id];
                // Coarse-cell tops round a free water level up by as much as
                // one cell and invent boundary walls. Keep water on the shared
                // fine source surface; the coarse occupancy still occludes
                // solid faces exactly as before.
                if material == BlockId::Water {
                    continue;
                }
                let x = cx * step;
                let y = cy * step;
                let z = cz * step;
                let s = step;
                // Corner order is CCW when looking along the outward normal.
                // The previous lists were the reverse of that, so back-face
                // culling drew the inside of every coarse cell.
                let top_exposed = !occupied(material, cx, cy + 1, cz);
                let west = occupied(material, cx - 1, cy, cz);
                let east = occupied(material, cx + 1, cy, cz);
                let north = occupied(material, cx, cy, cz - 1);
                let south = occupied(material, cx, cy, cz + 1);
                if top_exposed {
                    emit(
                        mesh,
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
                    );
                }
                if !occupied(material, cx, cy - 1, cz) {
                    emit(
                        mesh,
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
                    );
                }
                if !east {
                    emit(
                        mesh,
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
                    );
                }
                if !west {
                    emit(
                        mesh,
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
                    );
                }
                if !south {
                    emit(
                        mesh,
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
                    );
                }
                if !north {
                    emit(
                        mesh,
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
                    );
                }
                // Attach the lip to the actual top (y + s), on the shared
                // boundary plane. Starting at y or pushing it outward leaves
                // an open strip between the top and the curtain.
                // An emitted side already spans this exact plane. Add a lip
                // only where that side was culled. Crowns need no terrain lip.
                if top_exposed && !material.is_liquid() && !vegetation_only {
                    let drop = s as f32;
                    let hang = |vertices: &mut [Vertex3], drop: f32| {
                        let count = vertices.len();
                        if count >= 4 {
                            vertices[count - 4].pos[1] -= drop;
                            vertices[count - 3].pos[1] -= drop;
                        }
                    };
                    // The first two corners are the ones `hang` drops. They are
                    // the bottom edge, ordered so the skirt faces outward.
                    if cx == 0 && west {
                        emit(
                            mesh,
                            x,
                            y + s,
                            z,
                            x,
                            y + s,
                            z + s,
                            x,
                            y + s,
                            z + s,
                            x,
                            y + s,
                            z,
                            material,
                            [-1.0, 0.0, 0.0],
                        );
                        hang(&mut mesh.vertices, drop);
                    }
                    if cx + 1 == n && east {
                        emit(
                            mesh,
                            x + s,
                            y + s,
                            z + s,
                            x + s,
                            y + s,
                            z,
                            x + s,
                            y + s,
                            z,
                            x + s,
                            y + s,
                            z + s,
                            material,
                            [1.0, 0.0, 0.0],
                        );
                        hang(&mut mesh.vertices, drop);
                    }
                    if cz == 0 && north {
                        emit(
                            mesh,
                            x + s,
                            y + s,
                            z,
                            x,
                            y + s,
                            z,
                            x,
                            y + s,
                            z,
                            x + s,
                            y + s,
                            z,
                            material,
                            [0.0, 0.0, -1.0],
                        );
                        hang(&mut mesh.vertices, drop);
                    }
                    if cz + 1 == n && south {
                        emit(
                            mesh,
                            x,
                            y + s,
                            z + s,
                            x + s,
                            y + s,
                            z + s,
                            x + s,
                            y + s,
                            z + s,
                            x,
                            y + s,
                            z + s,
                            material,
                            [0.0, 0.0, 1.0],
                        );
                        hang(&mut mesh.vertices, drop);
                    }
                }
            }
        }
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

fn torch_local_up(chunk: &GeneratedChunk, at: Vec3) -> Vec3 {
    // The shell field is spatial only; planets and hills share its coordinates.
    if !chunk.manyfold_shell && engine_worldgen::manyfold::shell_curve(chunk.generator_version).is_some() {
        return Vec3::Y;
    }
    let g = engine_worldgen::manyfold::acceleration_for(chunk.generator_version, at * chunk.voxel_meters);
    (-g).try_normalize().unwrap_or(Vec3::Y)
}

/// Offset in the flame's own axes. World up keeps the authored (x, height, z).
fn flame_offset(axis: Vec3, x: f32, y: f32, z: f32) -> Vec3 {
    if axis.dot(Vec3::Y) > 0.995 {
        return Vec3::new(x, y, z);
    }
    let side = axis.cross(if axis.z.abs() < 0.9 { Vec3::Z } else { Vec3::X }).normalize_or_zero();
    let binormal = axis.cross(side).normalize_or_zero();
    side * x + axis * y + binormal * z
}

/// Shaft from the supporting face toward local up. `None` keeps the world-up mount.
fn gravity_torch_span(block: BlockId, up: Vec3) -> Option<(Vec3, Vec3, Vec3)> {
    let up = up.normalize_or_zero();
    if up.dot(Vec3::Y) > 0.995 {
        return None;
    }
    let support = block.torch_support_offset().map(|o| o.as_vec3()).unwrap_or(Vec3::NEG_Y);
    let floor = block == BlockId::Torch || support.normalize_or_zero().dot(-up) > 0.7;
    if floor {
        let foot = Vec3::splat(0.5) - up * 0.44;
        let head = foot + up * 0.66;
        return Some((foot, head, head + up * 0.08));
    }
    let wall = support.normalize_or_zero();
    let foot = Vec3::splat(0.5) + wall * 0.42 - up * 0.08;
    let head = foot - wall * 0.16 + up * 0.40;
    Some((foot, head, head + up * 0.06))
}

pub(crate) fn placed_torch_flame(cell: IVec3, block: BlockId, scale: f32, generator: u32, manyfold: bool) -> glam::DVec3 {
    let up = if !manyfold && engine_worldgen::manyfold::shell_curve(generator).is_some() { Vec3::Y } else { (-engine_worldgen::manyfold::acceleration_for(generator, (cell.as_vec3() + Vec3::splat(0.5)) * scale))
        .try_normalize().unwrap_or(Vec3::Y) };
    let point = gravity_torch_span(block, up).map(|(_, _, flame)| flame).unwrap_or_else(|| match block {
        BlockId::TorchWallNorth => Vec3::new(0.50, 0.77, 0.73),
        BlockId::TorchWallSouth => Vec3::new(0.50, 0.77, 0.27),
        BlockId::TorchWallWest => Vec3::new(0.73, 0.77, 0.50),
        BlockId::TorchWallEast => Vec3::new(0.27, 0.77, 0.50),
        _ => Vec3::new(0.50, 0.80, 0.50),
    });
    let anchor = block.torch_anchor();
    (cell.as_dvec3() + anchor.as_dvec3()) * f64::from(scale) + (point - anchor).as_dvec3()
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
                let wood_color = prop_palette::OAK;
                let ember_color = [1.00, 0.44, 0.16];
                let flame_color = [1.00, 0.64, 0.26];
                let local_up = torch_local_up(chunk_data, base + Vec3::splat(0.5));
                let flame_axis = local_up;

                let (shaft_start, shaft_end, flame_center) = if let Some((foot, head, flame)) = gravity_torch_span(block, local_up) {
                    (base + foot, base + head, base + flame)
                } else { match block {
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
                        base + Vec3::new(0.50, 0.70, 0.76),
                        base + Vec3::new(0.50, 0.77, 0.73),
                    ),
                    BlockId::TorchWallSouth => (
                        base + Vec3::new(0.50, 0.30, 0.08),
                        base + Vec3::new(0.50, 0.70, 0.24),
                        base + Vec3::new(0.50, 0.77, 0.27),
                    ),
                    BlockId::TorchWallWest => (
                        base + Vec3::new(0.92, 0.30, 0.50),
                        base + Vec3::new(0.76, 0.70, 0.50),
                        base + Vec3::new(0.73, 0.77, 0.50),
                    ),
                    BlockId::TorchWallEast => (
                        base + Vec3::new(0.08, 0.30, 0.50),
                        base + Vec3::new(0.24, 0.70, 0.50),
                        base + Vec3::new(0.27, 0.77, 0.50),
                    ),
                    _ => continue,
                } };

                emit_oriented_box(mesh, shaft_start, shaft_end, 0.045, wood_color, surface);
                // Dense player builds must fit the minimum worker allowance.
                // Retain every handle and light position, replacing collar/ring
                // detail with a bounded emissive volume above 1024 lights/chunk.
                if compact {
                    let mut flame = surface;
                    flame.detail_strength = -2.0;
                    let box_first = mesh.vertices.len();
                    emit_oriented_box(
                        mesh,
                        flame_center - flame_axis * 0.05,
                        flame_center + flame_axis * 0.20,
                        0.065,
                        flame_color,
                        flame,
                    );
                    // Lift the box by height so its top sways and its base stays put.
                    let box_bottom = flame_center - flame_axis * 0.05;
                    for vertex in &mut mesh.vertices[box_first..] {
                        let along = (Vec3::from_array(vertex.pos) - box_bottom).dot(flame_axis);
                        vertex.surface[3] = -2.0 - (along / 0.25).clamp(0.0, 1.0);
                    }
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
                            prop_palette::LEATHER,
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
                            prop_palette::IRON,
                            metal,
                        );
                        emit_oriented_box(
                            mesh,
                            plate - wall * 0.02,
                            shaft_start + axis * 0.10,
                            0.028,
                            prop_palette::IRON,
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
                        prop_palette::IRON,
                        metal,
                    );
                }
                let mut ember = surface;
                ember.detail_strength = -1.0;
                emit_oriented_box(
                    mesh,
                    shaft_end - (shaft_end - shaft_start).normalize_or_zero() * 0.05,
                    shaft_end + flame_axis * 0.08,
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
                // Height of each ring as a 0..1 lift, measured from the base ring.
                let ring_lift = rings.map(|ring| (ring.0 - rings[0].0) / (rings[3].0 - rings[0].0));
                for band in 0..3 {
                    let mut flame = surface;
                    flame.detail_strength = -2.0;
                    for side in 0..sides {
                        let point = |ring: usize, side: usize| {
                            let (height, radius, bend) = rings[ring];
                            let angle = side as f32 * std::f32::consts::TAU / sides as f32;
                            (flame_center
                                + flame_offset(
                                    flame_axis,
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
                        shade_flame_corners(
                            mesh,
                            first_vertex,
                            [
                                ring_colors[band],
                                ring_colors[band + 1],
                                ring_colors[band + 1],
                                ring_colors[band],
                            ],
                            [
                                ring_lift[band],
                                ring_lift[band + 1],
                                ring_lift[band + 1],
                                ring_lift[band],
                            ],
                        );
                    }
                }
                {
                    // A smaller tongue breaks the single-cone silhouette.
                    let mut flame = surface;
                    flame.detail_strength = -2.0;
                    let foot = flame_center + flame_offset(flame_axis, -0.065, 0.025, 0.0);
                    let belly = flame_center + flame_offset(flame_axis, -0.105, 0.12, 0.0);
                    let tip = flame_center + flame_offset(flame_axis, -0.08, 0.225, 0.015);
                    let lift_at = |height: f32| (height - rings[0].0) / (rings[3].0 - rings[0].0);
                    let (foot_lift, belly_lift, tip_lift) =
                        (lift_at(0.025), lift_at(0.12), lift_at(0.225));
                    for side in 0..4 {
                        let a = side as f32 * std::f32::consts::FRAC_PI_2;
                        let b = a + std::f32::consts::FRAC_PI_2;
                        let va = flame_offset(flame_axis, a.cos(), 0.0, a.sin());
                        let vb = flame_offset(flame_axis, b.cos(), 0.0, b.sin());
                        for (index, quad) in [
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
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            let quad = quad.map(|v| v.to_array());
                            let quad_first = mesh.vertices.len();
                            push_face(
                                &mut mesh.vertices,
                                &mut mesh.indices,
                                quad,
                                [1.0, 0.42, 0.09],
                                normal_from_quad(quad),
                                flame,
                                [1.0; 4],
                            );
                            let lifts = if index == 0 {
                                [foot_lift, belly_lift, belly_lift, foot_lift]
                            } else {
                                [belly_lift, tip_lift, tip_lift, belly_lift]
                            };
                            shade_flame_corners(mesh, quad_first, [[1.0, 0.42, 0.09]; 4], lifts);
                        }
                    }
                }
                // Two embers lift off the flame tip. Each is a pair of crossed
                // quads that the shader animates. ao_strength carries the corner
                // id ((quad * 4 + corner + 1) / 8) and surface[0] the spark seed;
                // surface[3] = -4 marks a spark, so it is not swayed as flame.
                let spark_hash = |spark: usize, k: f32| {
                    ((spark as f32 * 19.19 + k * 7.31 + base.x * 12.9898 + base.z * 78.233).sin()
                        * 43_758.547)
                        .fract()
                        .abs()
                };
                let mut spark_surface = surface;
                spark_surface.detail_strength = -4.0;
                for spark in 0..3 {
                    let center = flame_center
                        + flame_offset(
                            flame_axis,
                            (spark_hash(spark, 1.0) - 0.5) * 0.05,
                            // Spawn near the tip (flame top is about 0.27 m) and rise out.
                            0.20 + spark_hash(spark, 4.0) * 0.04,
                            (spark_hash(spark, 2.0) - 0.5) * 0.05,
                        );
                    let seed = spark_hash(spark, 3.0);
                    let half = 0.008;
                    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
                    for quad_kind in 0..2usize {
                        let quad = corners.map(|(cx, cy)| {
                            let offset = if quad_kind == 0 {
                                Vec3::new(cx * half, cy * half, 0.0)
                            } else {
                                Vec3::new(0.0, cy * half, cx * half)
                            };
                            (center + offset).to_array()
                        });
                        let quad_first = mesh.vertices.len();
                        push_face(
                            &mut mesh.vertices,
                            &mut mesh.indices,
                            quad,
                            [1.0, 0.85, 0.45],
                            normal_from_quad(quad),
                            spark_surface,
                            [1.0; 4],
                        );
                        for (corner, vertex) in mesh.vertices[quad_first..].iter_mut().enumerate() {
                            vertex.ao_strength = (quad_kind * 4 + corner + 1) as f32 / 8.0;
                            vertex.surface[0] = seed;
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
        if vertex.surface[3] < 0.0 {
            // Emissive parts can extend past their owning voxel at fine scales.
            // Store the offset back to the torch cell for the live flame lookup.
            let owner_offset = cell.as_ivec3() - Vec3::from_array(vertex.pos).floor().as_ivec3();
            let packed = owner_offset + IVec3::splat(8);
            vertex.surface[1] = -1024.0 - (packed.x + 16 * packed.y + 256 * packed.z) as f32;
        }
    }
}

/// Level of a free water surface at voxel `p`: water with air directly above.
/// Submerged water (water above) returns `None`; its surface is the one above it.
fn free_water_level(f: water_motion::Fills, p: IVec3) -> Option<f32> {
    (f.is_water(p) && !f.is_water(p + IVec3::Y))
        .then(|| f.level(p))
}

/// Surface height above row `row`'s floor at lattice corner `(x, z)`: the mean
/// level of the free surfaces in the four voxel columns that touch the corner.
/// Every face sharing the corner computes the same value, so neighbouring
/// columns of different volume slope into each other instead of stepping in
/// 1/8 m terraces. Halo columns count, so chunks agree on their border corners.
fn water_corner_height(f: water_motion::Fills, x: i32, z: i32, row: i32) -> f32 {
    if let Some(ramp) = water_ramp_corner_height(f, x, z, row) {
        return ramp;
    }
    let (mut sum, mut count) = (0.0, 0u32);
    for (cx, cz) in [(x - 1, z - 1), (x, z - 1), (x - 1, z), (x, z)] {
        if let Some(level) = free_water_level(f, IVec3::new(cx, row, cz)) {
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

/// Corner height above `row`'s floor where free surfaces one row apart meet,
/// so a one-voxel flow step meshes as a ramp instead of a vertical wall.
/// Columns are classified from rows `a..=a + 2` of the row pair alone, so both
/// cells at the corner compute the same height; at most one pair qualifies.
/// Deeper drops (waterfalls) return `None` and keep their wall.
fn water_ramp_corner_height(f: water_motion::Fills, x: i32, z: i32, row: i32) -> Option<f32> {
    let chunk = f.chunk;
    let water = |cx: i32, y: i32, cz: i32| f.is_water(IVec3::new(cx, y, cz));
    'pair: for a in [row - 1, row] {
        let (mut sum, mut count, mut low, mut high) = (0.0, 0u32, false, false);
        for (cx, cz) in [(x - 1, z - 1), (x, z - 1), (x - 1, z), (x, z)] {
            let top = if water(cx, a + 1, cz) {
                // A ramp through a solid ledge hides below the bed and tears
                // against the ground. Keep a waterfall face at solid steps.
                if chunk.get_world(IVec3::new(cx, a, cz)).is_opaque() {
                    continue 'pair;
                }
                if water(cx, a + 2, cz) {
                    continue 'pair;
                }
                high = true;
                a + 1
            } else if water(cx, a, cz) {
                low = true;
                a
            } else {
                continue;
            };
            let level = free_water_level(f, IVec3::new(cx, top, cz)).unwrap_or(0.0);
            sum += level + (top - row) as f32;
            count += 1;
        }
        if low && high {
            return Some(sum / count as f32);
        }
    }
    None
}

/// True when a free water surface sits one row above or below a neighbouring
/// free surface, which only the exact water mesher draws as a ramp. Both cells
/// test true, so a chunk whose partner cell lies in the halo still ramps.
pub(crate) fn has_water_flow_step(chunk: &GeneratedChunk, p: IVec3) -> bool {
    let water = |q: IVec3| chunk.get_world(q) == BlockId::Water;
    water(p)
        && !water(p + IVec3::Y)
        && [IVec3::X, -IVec3::X, IVec3::Z, -IVec3::Z].into_iter().any(|d| {
            let q = p + d;
            let lower = !water(q) && !chunk.get_world(q).is_opaque() && water(q - IVec3::Y);
            let upper = water(q) && water(q + IVec3::Y) && !water(q + IVec3::Y * 2);
            lower || upper
        })
}

/// Admission bound for exact water appended to a coarse solid mesh. A face
/// emits four vertices and six indices. Three times output bytes also covers
/// the old and new allocations overlapping during amortized Vec growth.
pub fn exact_water_mesh_bytes_upper_bound(chunk: &GeneratedChunk) -> usize {
    let row_stride = PADDED_CHUNK_SIZE as usize;
    let plane = row_stride * row_stride;
    let blocks = &chunk.blocks;
    let mut faces = 0usize;
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            let start = GeneratedChunk::local_index(CHUNK_PAD, y + CHUNK_PAD, z + CHUNK_PAD);
            for i in start..start + CHUNK_SIZE as usize {
                if blocks[i] != BlockId::Water { continue; }
                // Each neighbor, including the cell above a side neighbor,
                // is within the one-voxel halo of this interior cell.
                let submerged = blocks[i + plane] == BlockId::Water;
                faces += usize::from(!submerged);
                faces += usize::from(blocks[i - plane].is_empty());
                if !submerged {
                    // A ramped free surface can rise above its voxel and need a
                    // wall on any side, including toward an opaque neighbour.
                    faces += 4;
                    continue;
                }
                for side in [i + 1, i - 1, i + row_stride, i - row_stride] {
                    let neighbor = blocks[side];
                    if neighbor.is_opaque() { continue; }
                    // Two free surfaces share the exact same smoothed corner;
                    // a submerged neighbour reaches at least our top. Neither
                    // case emits an internal wall. Count possible flow steps.
                    if neighbor != BlockId::Water
                        || blocks[side + plane] != BlockId::Water
                    {
                        faces += 1;
                    }
                }
            }
        }
    }
    faces.saturating_mul(4 * std::mem::size_of::<Vertex3>() + 6 * std::mem::size_of::<u32>())
        .saturating_mul(3)
}

pub(crate) fn water_mesh(chunk: &GeneratedChunk) -> ChunkMesh {
    let mut mesh=ChunkMesh::empty();
    append_water_mesh(&mut mesh,chunk);
    mesh
}

fn append_water_mesh(mesh: &mut ChunkMesh, chunk: &GeneratedChunk) {
    const ROW: usize = PADDED_CHUNK_SIZE as usize;
    const PLANE: usize = ROW * ROW;
    // Corners of a unit cell in the order the top quad is emitted.
    const CELLS: [(i32, i32); 4] = [(0, 0), (0, 1), (1, 1), (1, 0)];
    let now = water_motion::Fills::now(chunk);
    let shown = water_motion::Fills::shown(chunk);
    let clock = chunk.water_motion.vertex_clock();
    let mut surface_corners = crate::fast_hash::FastMap::default();
    let origin = chunk.chunk * CHUNK_SIZE;
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            let start = GeneratedChunk::local_index(1, y + 1, z + 1);
            let row = &chunk.blocks[start..start + CHUNK_SIZE as usize];
            for (x, &block) in row.iter().enumerate() {
                if block != BlockId::Water {
                    continue;
                }
                let p = origin + IVec3::new(x as i32, y, z);
                let index = start + x;
                let h = if chunk.blocks[index + PLANE] == BlockId::Water {
                    1.0
                } else {
                    free_water_height(chunk.water_fill(p))
                };
                // Surface height at a lattice corner. Submerged water is full height.
                let top_at = |f: water_motion::Fills, cx: i32, cz: i32| {
                    if h >= 1.0 {
                        1.0
                    } else {
                        water_corner_height(f, cx, cz, p.y)
                    }
                };
                let base = p.as_vec3();
                for (axis, stride) in [1, PLANE, ROW].into_iter().enumerate() {
                    for positive in [false, true] {
                        let mut offset = IVec3::ZERO;
                        offset[axis] = if positive { 1 } else { -1 };
                        let neighbor_index = if positive { index + stride } else { index - stride };
                        let neighbor = chunk.blocks[neighbor_index];
                        // A generated channel can end at a ledge above another
                        // reach. Extend the bottom wall to the receiving pool.
                        let cascade_floor = if axis != 1 && neighbor == BlockId::Air
                            && chunk.blocks[index - PLANE].is_opaque()
                            && chunk.blocks[neighbor_index - PLANE] == BlockId::Water
                        {
                            let receiving = p + offset - IVec3::Y;
                            Some(free_water_height(chunk.water_fill(receiving)) - 1.0)
                        } else if axis != 1 && neighbor == BlockId::Air
                            && chunk.blocks[index - PLANE].is_opaque()
                            && let Some(cfg) = chunk.flora_config
                            && p.y > cfg.water_level()
                            && let Some((bed, head)) = engine_worldgen::steady_water_column(&cfg, p.x, p.z)
                            && p.y > bed && (p.y as f32) < head
                            && let Some((_, pool)) = engine_worldgen::steady_water_column(
                                &cfg, p.x + offset.x, p.z + offset.z,
                            )
                            && pool < p.y as f32
                            // The padded row belongs to the chunk below. Its
                            // live voxel and fill are available even at the seam.
                            && pool.ceil() as i32 - 1 >= chunk.origin().y
                            && (pool.ceil() as i32..p.y).all(|y| {
                                chunk.get_world(IVec3::new(p.x + offset.x, y, p.z + offset.z)) == BlockId::Air
                            })
                            && chunk.get_world(IVec3::new(
                                p.x + offset.x, pool.ceil() as i32 - 1, p.z + offset.z,
                            )) == BlockId::Water
                        {
                            let receiving = IVec3::new(
                                p.x + offset.x, pool.ceil() as i32 - 1, p.z + offset.z,
                            );
                            Some((receiving.y - p.y) as f32
                                + free_water_height(chunk.water_fill(receiving)))
                        } else {
                            None
                        };
                        // Corners now, and where the previous mesh showed them.
                        let (rel, was): ([Vec3; 4], [Vec3; 4]) = if axis == 1 {
                            if positive {
                                // Water above: this voxel has no free surface.
                                if h >= 1.0 {
                                    continue;
                                }
                                let top = |f| CELLS.map(|(i, j)| {
                                    Vec3::new(i as f32, top_at(f, p.x + i, p.z + j), j as f32)
                                });
                                let rel = top(now);
                                (rel, shown.map_or(rel, top))
                            } else {
                                if !neighbor.is_empty() {
                                    continue;
                                }
                                let floor = CELLS.map(|(i, j)| Vec3::new(i as f32, 0.0, j as f32));
                                (floor, floor)
                            }
                        } else {
                            let above = chunk.blocks[neighbor_index + PLANE];
                            if neighbor.is_opaque() && above.is_opaque() {
                                continue;
                            }
                            // The wall's foot is the neighbour's surface at the shared
                            // corners: its free level if it has one, the full voxel
                            // (or a ramp above it) if water sits above it, a lower
                            // neighbour's ramp where it rises into this row, and
                            // the floor if it is dry. Beside an opaque cell only a
                            // ramp risen above this voxel needs a wall.
                            let floor_at = |f: water_motion::Fills, cx: i32, cz: i32| match neighbor {
                                n if n.is_opaque() => 1.0,
                                BlockId::Water if above != BlockId::Water => {
                                    water_corner_height(f, cx, cz, p.y)
                                }
                                BlockId::Water
                                    if chunk.get_world(p + offset + IVec3::Y * 2)
                                        != BlockId::Water =>
                                {
                                    1.0 + water_corner_height(f, cx, cz, p.y + 1)
                                }
                                BlockId::Water => 1.0,
                                BlockId::Air if let Some(floor) = cascade_floor => floor,
                                _ if chunk.blocks[neighbor_index - PLANE] == BlockId::Water => {
                                    (water_corner_height(f, cx, cz, p.y - 1) - 1.0).max(0.0)
                                }
                                _ => 0.0,
                            };
                            let plane = if positive { 1 } else { 0 };
                            let (e0, e1) = if axis == 0 {
                                ((p.x + plane, p.z), (p.x + plane, p.z + 1))
                            } else {
                                ((p.x, p.z + plane), (p.x + 1, p.z + plane))
                            };
                            let wall = |f| {
                                let (b0, b1) = (floor_at(f, e0.0, e0.1), floor_at(f, e1.0, e1.1));
                                let (t0, t1) = (top_at(f, e0.0, e0.1), top_at(f, e1.0, e1.1));
                                let pl = plane as f32;
                                let quad = if axis == 0 {
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
                                };
                                (quad, t0 <= b0 && t1 <= b1)
                            };
                            let (rel, shared) = wall(now);
                            if shared {
                                // Shared surface on both sides: no wall.
                                continue;
                            }
                            (rel, shown.map_or(rel, |f| wall(f).0))
                        };
                        let (quad, was) = if positive {
                            (rel, was)
                        } else {
                            ([rel[3], rel[2], rel[1], rel[0]], [was[3], was[2], was[1], was[0]])
                        };
                        let first = mesh.vertices.len();
                        push_face(
                            &mut mesh.vertices,
                            &mut mesh.indices,
                            quad.map(|c| (base + c).to_array()),
                            BlockId::Water.color(p),
                            face_normal_from_axis(axis, positive),
                            BlockId::Water.surface(),
                            [1.0; 4],
                        );
                        if axis != 1 {
                            // Only wall vertices above their voxel floor are on the
                            // surface. A dry floor edge sits exactly at `base.y` and
                            // must stay put, while a wall top or a neighbour's
                            // water-level foot must follow the waves like the top.
                            for v in &mut mesh.vertices[first..] {
                                if v.pos[1] > base.y + 1e-4 {
                                    v.ao_strength = WATER_SURFACE_RIM_AO;
                                }
                            }
                        }
                        // A shoreline on solid ground has no downward current.
                        // Only a wall beside a lower receiving cell carries a fall.
                        let falling = axis != 1
                            && neighbor.is_empty()
                            && (cascade_floor.is_some()
                                || matches!(chunk.blocks[neighbor_index - PLANE], BlockId::Air | BlockId::Water));
                        for (v, (c, w)) in mesh.vertices[first..].iter_mut().zip(quad.iter().zip(&was)) {
                            let flow = if axis == 1 && positive {
                                let (cx, cz) = (p.x + c.x as i32, p.z + c.z as i32);
                                let (current, normal) = *surface_corners.entry((cx, cz, p.y)).or_insert_with(|| {
                                    let columns = [(cx - 1, cz - 1), (cx, cz - 1), (cx - 1, cz), (cx, cz)];
                                    let surface = columns.map(|(x, z)| now.column_surface(x, z, p.y));
                                    // Smooth the shared top, keeping large drops in the wall geometry.
                                    let rise = water_motion::corner_rise(surface, 1.0);
                                    let normal = Vec3::new(-rise.x, 1.0, -rise.y).normalize().to_array();
                                    if let Some(cfg) = chunk.flora_config
                                        && p.y > cfg.water_level()
                                    {
                                        let planned = columns.map(|(x, z)| {
                                            engine_worldgen::steady_water_column(&cfg, x, z)
                                                .filter(|(bed, head)| p.y > *bed && (*head - p.y as f32).abs() < 2.0)
                                                .map(|(_, head)| head)
                                        });
                                        let current = water_motion::corner_flow(planned);
                                        if current.length_squared() > 0.0 {
                                            return (current, normal);
                                        }
                                        if planned.iter().any(Option::is_some) {
                                            let current = engine_worldgen::geo::flat_river_current(
                                                &cfg, cfg.world_x(cx), cfg.world_z(cz),
                                            );
                                            if current.length_squared() > 0.0 {
                                                return (current, normal);
                                            }
                                        }
                                    }
                                    (water_motion::corner_flow(surface), normal)
                                });
                                v.normal = normal;
                                current
                            } else if falling {
                                glam::Vec2::new(0.0, -water_motion::FALL_SPEED)
                            } else {
                                glam::Vec2::ZERO
                            };
                            v.surface[0] = clock;
                            v.surface[1] = water_motion::pack(w.y - c.y, flow);
                        }
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

#[cfg(test)]
mod water_depth_probe;
#[cfg(test)]
mod water_ramp_probe;
