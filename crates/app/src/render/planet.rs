//! Whole-planet cover using the same columns as streamed voxel terrain.
use crate::{mesh::Vertex3, world::WorldSampleConfig};
use engine_core::cube_sphere::{Face, atlas};
use glam::{DVec3, Vec2, Vec3};
use super::{coverage::ResidentCoverage, horizon::{collar, collar_set, parallel}};
use std::{collections::{HashMap, VecDeque}, sync::{Arc, atomic::{AtomicBool, Ordering}}};
use wgpu::util::DeviceExt;
mod shoreline;
pub(super) mod progress;

/// Grid cells per cube face edge.
const SIDE: usize = 128;
/// Grid cells along the solved spawn face, whose valleys are 100-300 m wide.
const SOLVED_SIDE: usize = 384;

fn side(cfg: &WorldSampleConfig, face: Face) -> usize {
    if face == Face::ALL[0] && engine_worldgen::basin::solved_face(cfg) { SOLVED_SIDE } else { SIDE }
}

/// Face and face-local centre in metres of the fine patch under the eye.
pub type Anchor = (Face, [i32; 2]);
/// Uncut face geometry and recent local patches for the current world recipe.
#[derive(Clone, Default)]
pub struct Shells {
    faces: [Option<Arc<crate::mesh::ChunkMesh>>; 6],
    patches: VecDeque<(Anchor, Arc<crate::mesh::ChunkMesh>)>,
}

const RECENT_PATCHES: usize = 3;

pub struct CoverJob {
    pub cfg: WorldSampleConfig,
    pub anchor: Option<Anchor>,
    pub cancel: Arc<AtomicBool>,
    pub started: std::time::Instant,
    pub terrain: std::sync::mpsc::Receiver<Mesh>,
    pub worker: std::thread::JoinHandle<Option<Mesh>>,
}

pub struct Mesh {
    pub cfg: WorldSampleConfig,
    pub refined: bool,
    /// Fine patch under the eye, on whichever face holds it.
    pub anchor: Option<Anchor>,
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub count: u32,
    pub backstop: Option<(wgpu::Buffer, u32)>,
    pub source: Arc<crate::mesh::ChunkMesh>,
    pub shells: Shells,
    collar: Option<Arc<collar_set::Set>>,
    variants: Option<collar::Ranges>,
}

/// CPU-side cover geometry, built off the render thread and uploaded later.
pub struct Cpu {
    pub cfg: WorldSampleConfig,
    pub refined: bool,
    pub anchor: Option<Anchor>,
    pub vertices: Vec<Vertex3>,
    pub indices: Vec<u32>,
    backstop: Vec<u32>,
    source: Option<Arc<crate::mesh::ChunkMesh>>,
    shells: Shells,
    collar: Option<Arc<collar_set::Set>>,
    variants: Option<collar::Ranges>,
}

impl Mesh {
    pub fn new(device: &wgpu::Device, cfg: WorldSampleConfig) -> Self {
        Self::upload(device, build_cpu(cfg, None))
    }

    pub fn upload(device: &wgpu::Device, cpu: Cpu) -> Self {
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Planet surface cover"), contents: bytemuck::cast_slice(&cpu.vertices), usage: wgpu::BufferUsages::VERTEX });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Planet surface indices"), contents: bytemuck::cast_slice(&cpu.indices), usage: wgpu::BufferUsages::INDEX });
        let backstop = (!cpu.backstop.is_empty()).then(|| {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Planet cover near the resident boundary"),
                contents: bytemuck::cast_slice(&cpu.backstop),
                usage: wgpu::BufferUsages::INDEX,
            });
            (buffer, cpu.backstop.len() as u32)
        });
        Self { cfg: cpu.cfg, refined: cpu.refined, anchor: cpu.anchor, count: cpu.indices.len() as u32,
            vertices, indices, backstop, shells: cpu.shells, collar: cpu.collar, variants: cpu.variants,
            source: cpu.source.unwrap_or_else(|| Arc::new(crate::mesh::ChunkMesh {
                vertices: cpu.vertices, indices: cpu.indices })),
        }
    }

    pub fn matches_collar(&self, current: Option<&Arc<collar_set::Set>>) -> bool {
        collar_set::same(self.collar.as_ref(), current)
    }

    pub fn index_range(&self, current: Option<&Arc<collar_set::Set>>,
        coverage: &ResidentCoverage) -> std::ops::Range<u32> {
        let valid = self.collar.as_ref().zip(current)
            .is_some_and(|(built, now)| built.valid_for(now, coverage));
        self.variants.as_ref().map_or(0..self.count, |ranges|
            if valid { ranges.collared.clone() } else { ranges.uncollared.clone() })
    }
}

/// Reuse the sampled planet when only installed edge profiles changed. Joins
/// and the original cover upload together; the draw selects a valid variant.
pub fn build_with_collar(cfg: WorldSampleConfig, anchor: Option<Anchor>,
    input: Option<Arc<collar_set::Set>>, source: Option<Arc<crate::mesh::ChunkMesh>>, shells: Shells) -> Cpu {
    build_with_progress(cfg, anchor, input, source, shells, None, None)
}

pub struct Streaming<'a> {
    pub terrain_ready: &'a dyn Fn(Cpu),
    pub cancel: &'a AtomicBool,
}

pub fn build_with_progress(cfg: WorldSampleConfig, anchor: Option<Anchor>,
    input: Option<Arc<collar_set::Set>>, source: Option<Arc<crate::mesh::ChunkMesh>>, shells: Shells,
    progress: Option<&progress::Progress>, streaming: Option<Streaming<'_>>) -> Cpu {
    if input.is_none() && source.is_none() {
        return build(cfg, anchor, true, shells, progress, streaming);
    }
    let (source, shells) = if let Some(source) = source { (source, shells) } else {
        let cpu = build(cfg, anchor, true, shells, progress, streaming);
        if !cpu.refined { return cpu; }
        (Arc::new(crate::mesh::ChunkMesh { vertices: cpu.vertices, indices: cpu.indices }), cpu.shells)
    };
    if let Some(p) = progress { p.begin(if input.is_some() { "Terrain Joins" } else { "Terrain Buffers" }, 0); }
    let mut mesh = crate::mesh::ChunkMesh {
        vertices: source.vertices.clone(), indices: source.indices.clone(),
    };
    let variants = input.as_ref().and_then(|set| {
        match collar_set::apply(&mut mesh, set, &AtomicBool::new(false), 256 * 1024 * 1024) {
            Some(collar::Outcome::Applied(ranges)) => Some(ranges),
            _ => None,
        }
    });
    let backstop = backstop_indices(&cfg, anchor, &source);
    Cpu { cfg, refined: true, anchor, vertices: mesh.vertices, indices: mesh.indices, backstop, shells,
        source: Some(source), collar: input, variants }
}

fn backstop_indices(cfg: &WorldSampleConfig, anchor: Option<Anchor>, mesh: &crate::mesh::ChunkMesh) -> Vec<u32> {
    let Some((face, [x, z])) = anchor else { return Vec::new(); };
    let scale = f64::from(cfg.voxel_meters());
    let centre = atlas::join(face, DVec3::new(f64::from(x), 0.0, f64::from(z))) / scale;
    // The eye can sit at the edge of a clamped patch. Include the widest
    // resident radius beyond the patch so its edge can still find cover.
    let reach = (PATCH_M * 0.5 / scale + 64.0 * f64::from(engine_core::CHUNK_SIZE)) as f32;
    let centre = centre.as_vec3();
    let water = crate::world::BlockId::Water.surface().shader_key();
    mesh.indices.chunks_exact(3).filter(|tri| {
        let vertices = [tri[0], tri[1], tri[2]].map(|i| &mesh.vertices[i as usize]);
        vertices.iter().all(|v| (v.surface[2] == 0.0 || v.surface[2] == water)
            && v.ao_strength < 0.0 && v.ao_strength > -1.5)
            // A coarse triangle can cross the near region with all three
            // corners outside it. Reject only disjoint bounds, not triangles
            // lacking an interior vertex; that left holes in the safety cover.
            && [0, 2].into_iter().all(|axis| {
                let low = vertices.iter().map(|v| v.pos[axis]).fold(f32::INFINITY, f32::min);
                let high = vertices.iter().map(|v| v.pos[axis]).fold(f32::NEG_INFINITY, f32::max);
                low <= centre[axis] + reach && high >= centre[axis] - reach
            })
    }).flatten().copied().collect()
}

/// Edge of the fine patch around the player, and its cells on the solved face.
const PATCH_M: f64 = 8192.0;
const PATCH_SIDE: usize = 410;
/// Cells of the patch on an unsolved face: 51 m against 375 m for the face grid.
/// Exact columns are sampled per vertex there, so the grid stays small.
const SHELL_PATCH_SIDE: usize = 160;

fn patch_side(cfg: &WorldSampleConfig, face: Face) -> usize {
    if face == Face::ALL[0] && engine_worldgen::basin::solved_face(cfg) { PATCH_SIDE } else { SHELL_PATCH_SIDE }
}
/// The patch recentres when the eye leaves the central quarter.
const ANCHOR_SNAP_M: f64 = 2048.0;

/// Face and patch centre for an eye in chart metres, snapped so small moves
/// reuse the mesh; `None` off a planet or between faces. The patch stays
/// inside its face. Faces 1 to 5 once had only the 375 m grid: with no horizon
/// rings on a planet, everything past the loaded chunks was that grid.
pub fn anchor_for(cfg: &WorldSampleConfig, eye_m: glam::Vec3) -> Option<Anchor> {
    if !cfg.is_planet() {
        return None;
    }
    let (face, local) = atlas::split(DVec3::new(f64::from(eye_m.x), 0.0, f64::from(eye_m.z)));
    if local.x.abs() >= atlas::HALF_M || local.z.abs() >= atlas::HALF_M {
        return None;
    }
    let limit = atlas::HALF_M - PATCH_M * 0.5;
    let snap = |v: f64| ((v / ANCHOR_SNAP_M).round() * ANCHOR_SNAP_M).clamp(-limit, limit) as i32;
    Some((face, [snap(local.x), snap(local.z)]))
}

/// One square sample grid on a face in local metres; quads whose centre falls
/// inside `hole` are left out for a finer grid to fill.
struct Grid {
    face: Face,
    min_m: [f64; 2],
    span_m: f64,
    side: usize,
    refined: bool,
}

type SplitCells = HashMap<usize, Vec<[u32; 3]>>;

fn mesh_grid(
    cfg: &WorldSampleConfig, g: &Grid, vertices: &mut Vec<Vertex3>, indices: &mut Vec<u32>,
    meander: &(impl Fn(f32, f32, f32) -> bool + Sync),
    relief: Option<engine_worldgen::planetary::review::catchment::Frame>,
    progress: Option<&progress::Progress>,
) -> SplitCells {
    let side = g.side;
    if let Some(p) = progress { p.begin("Surface Samples", ((side + 1) * (side + 1)) as u64); }
    let cell_m = g.span_m / side as f64;
    // Grid spacing in voxels; slopes come from neighbouring heights, not a flat up normal.
    let step = (g.span_m / side as f64 / cfg.voxel_meters() as f64) as f32;
    // Detail below 32 m builds an erosion tile per sample; the grid cannot afford it.
    let footprint = (step * cfg.voxel_meters()).max(if g.refined { 40.0 } else { 160.0 });
    let half = g.span_m * 0.5;
    let centre = atlas::join(g.face, DVec3::new(g.min_m[0] + half, 0.0, g.min_m[1] + half));
    let inland_patch = g.refined && cell_m <= 64.0 && meander(centre.x as f32, centre.z as f32, (half * 1.5) as f32);
    let inland = |p: Vec3, margin| {
        inland_patch && meander(p.x * cfg.voxel_meters(), p.z * cfg.voxel_meters(), margin)
    };
    let precise_scarps = engine_worldgen::planetary::reference_relief::active(cfg);
    let scarp = |p: Vec3| {
        relief.is_some_and(|frame| {
            let chart = glam::DVec2::new(f64::from(p.x), f64::from(p.z)) * f64::from(cfg.voxel_meters());
            let local = frame.local(chart);
            let edge = if precise_scarps { 0.0 } else { 240.0 };
            local.min_element() >= edge && local.max_element() <= 4000.0 - edge
        })
    };
    let base = vertices.len() as u32;
    let planetary = engine_worldgen::planetary::active(cfg);
    let cover = |x, z, footprint| {
        if planetary {
            let (column, head, bed) = engine_worldgen::planetary::cover_column(cfg, x, z, footprint);
            (column, head, bed, true)
        } else {
            let distant = engine_worldgen::basin::cover_column(cfg, x, z, footprint);
            let column = distant.unwrap_or_else(|| engine_worldgen::sample_column(cfg, x, z));
            (column, None, 0.0, distant.is_some())
        }
    };
    let bed_top = |column: &engine_worldgen::ColumnSample, correction: f32| {
        column.height as f32 + if planetary && column.surface != crate::world::BlockId::Lava {
            correction / cfg.voxel_meters()
        } else { 1.0 }
    };
    let patch_distance = |p: Vec3| (p.x * cfg.voxel_meters() - centre.x as f32).abs()
        .max((p.z * cfg.voxel_meters() - centre.z as f32).abs());
    let near_patch = |p: Vec3| g.span_m == PATCH_M && patch_distance(p) < 1536.0;
    let dry_relief = |p: Vec3| patch_distance(p) < 3300.0;
    let adaptive_relief = g.refined && g.span_m == PATCH_M && planetary && relief.is_none() && !precise_scarps;
    let vertex = |p: DVec3, column: &engine_worldgen::ColumnSample,
                  water_head: Option<f32>, bed_correction: f32, normal: Vec3| {
        let sea = cfg.world_type != crate::world::WorldType::Nether && column.height < cfg.water_level();
        let frozen = sea && planetary
            && engine_worldgen::planet::sea_ice_at(cfg, p.x.floor() as i32, p.z.floor() as i32);
        let ocean = (sea || water_head.is_some()) && !frozen;
        let sea_surface = (cfg.water_level() + 1) as f32 - crate::mesh::WATER_SURFACE_INSET;
        let inland_surface = water_head.map(|head| {
            let h = head / cfg.voxel_meters();
            if h.fract() > 0.0 { h.floor() + crate::mesh::free_water_height(h.fract()) }
            else { h - crate::mesh::WATER_SURFACE_INSET }
        });
        let bed = bed_top(column, bed_correction);
        let y = if frozen { sea_surface + crate::mesh::WATER_SURFACE_INSET } else if sea {
            inland_surface.map_or(sea_surface, |head| head.max(sea_surface))
        } else { inland_surface.unwrap_or(bed) };
        let normal = if ocean || frozen { Vec3::Y } else { normal };
        let at = glam::IVec3::new(p.x.floor() as i32, column.height, p.z.floor() as i32);
        let (color, surface) = if ocean {
            let water = crate::world::BlockId::Water.surface();
            let head = if planetary { y } else { cfg.water_level() as f32 };
            let depth = ((head - bed) * cfg.voxel_meters()).clamp(0.0, 48.0) / 16.0;
            (column.surface.color(at), [water.roughness, water.specular, water.shader_key(), depth])
        } else {
            let (block, color) = if frozen { (crate::world::BlockId::Ice, crate::world::BlockId::Ice.color(at)) }
                else { cover_material(column.surface, column.vegetation_density, at) };
            let material = block.surface();
            (color, [material.roughness, material.specular, 0.0, material.shader_key()])
        };
        Vertex3 { pos: [p.x as f32, y, p.z as f32], color,
            normal: normal.to_array(), surface, ao_strength: -1.0 }
    };
    let sample_row = |&z: &usize| {
        let mut row = Vec::with_capacity(side + 1);
        for x in 0..=side {
            let p = atlas::join(g.face, DVec3::new(
                g.min_m[0] + g.span_m * x as f64 / side as f64,
                0.0,
                g.min_m[1] + g.span_m * z as f64 / side as f64,
            )) / cfg.voxel_meters() as f64;
            let (cx, cz) = (p.x.floor() as i32, p.z.floor() as i32);
            let footprint = if inland(p.as_vec3(), cell_m as f32) { 8.0 } else { footprint };
            let (column, water_head, bed_correction, averaged) = cover(cx, cz, footprint);
            row.push((p, column, water_head, bed_correction, averaged));
        }
        if let Some(p) = progress { p.advance((side + 1) as u64); }
        Some(row)
    };
    let rows: Vec<_> = (0..=side).collect();
    let workers = if g.refined {
        parallel::workers(parallel::cap_from_env("VOXEL_HORIZON_WORKERS", 8))
    } else { 1 };
    let samples: Vec<_> = parallel::map_blocks(&rows, 2, workers, &AtomicBool::new(false), sample_row)
        .expect("planet cover row worker failed").into_iter().flatten().collect();
    if let Some(p) = progress { p.begin("Surface Mesh", 0); }
    let mut beds: Vec<_> = samples.iter().map(|(_, column, _, correction, _)| bed_top(column, *correction)).collect();
    let height = |x: usize, z: usize| beds[z * (side + 1) + x];
    for z in 0..=side {
        for x in 0..=side {
            let (p, column, water_head, bed_correction, _) = &samples[z * (side + 1) + x];
            let dx = height((x + 1).min(side), z) - height(x.saturating_sub(1), z);
            let dz = height(x, (z + 1).min(side)) - height(x, z.saturating_sub(1));
            let run = step * if x == 0 || x == side { 1.0 } else { 2.0 };
            let rise = step * if z == 0 || z == side { 1.0 } else { 2.0 };
            let normal = Vec3::new(-dx / run, 1.0, -dz / rise).normalize_or_zero();
            vertices.push(vertex(*p, column, *water_head, *bed_correction, normal));
        }
    }
    if g.refined {
        let grid = &mut vertices[base as usize..];
        let stride = side + 1;
        let colors: Vec<_> = grid.iter().map(|v| v.color).collect();
        // Filter with the samples already in the mesh. Keep coastlines and
        // steep contacts from picking up the color of the opposite surface.
        for i in 0..grid.len() {
            let v = grid[i];
            if !samples[i].4 || v.surface[2] != 0.0 { continue; }
            let (x, z) = (i % stride, i / stride);
            let neighbors = [i - usize::from(x > 0), i + usize::from(x < side),
                i - stride * usize::from(z > 0), i + stride * usize::from(z < side)];
            let mut color = Vec3::from_array(colors[i]) * 3.0;
            let mut weight = 3.0;
            for j in neighbors {
                if j == i || !samples[j].4 || grid[j].surface[2] != 0.0 { continue; }
                let contrast = if v.surface[3] == grid[j].surface[3] { 1.0 } else { 0.45 };
                let slope = (beds[i] - beds[j]) / (step * 0.35);
                let w = contrast / (1.0 + slope * slope);
                color += Vec3::from_array(colors[j]) * w;
                weight += w;
            }
            grid[i].color = (color / weight).to_array();
        }
    }
    let mut triangles = Vec::with_capacity(side * side * 2);
    for z in 0..side {
        for x in 0..side {
            let a = base + (z * (side + 1) + x) as u32;
            let b = a + 1;
            let c = a + side as u32 + 1;
            let d = c + 1;
            triangles.extend([[a, c, b], [b, c, d]]);
        }
    }
    if g.refined && cfg.is_planet() && (cell_m > 32.0 || inland_patch || relief.is_some() || adaptive_relief) {
        if let Some(p) = progress { p.begin("Terrain Refinement", 0); }
        shoreline::refine(
            vertices, &mut beds, &mut triangles, base, side, cfg.voxel_meters(),
            (cfg.water_level() + 1) as f32,
            cell_m > 32.0,
            |p| inland(p, cell_m as f32),
            near_patch,
            relief.as_ref().map(|_| &scarp as &dyn Fn(Vec3) -> bool),
            adaptive_relief.then_some(&dry_relief as &dyn Fn(Vec3) -> bool),
            precise_scarps,
            workers,
            |p, normal, footprint, dry| {
                let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
                let footprint = if dry { footprint.max(if near_patch(p) { 16.0 } else { 32.0 }) }
                    else if inland(p, cell_m as f32) || near_patch(p) || scarp(p) { footprint.max(4.0) }
                    else { footprint.max(40.0) };
                let (column, water_head, correction, _) = cover(x, z, footprint);
                let normal = if precise_scarps && scarp(p) {
                    let local = engine_worldgen::planetary::reference_relief::frame()
                        .local(glam::DVec2::new(f64::from(p.x), f64::from(p.z)) * f64::from(cfg.voxel_meters()));
                    engine_worldgen::planetary::reference_relief::normal(local.x, local.y)
                } else { normal };
                let midpoint = vertex(p.as_dvec3(), &column, water_head, correction, normal);
                if let Some(p) = progress { p.advance(1); }
                (midpoint, bed_top(&column, correction))
            },
        );
    }
    if precise_scarps && relief.is_some() {
        let frame = engine_worldgen::planetary::reference_relief::frame();
        let mut errors = Vec::new();
        let mut squared = 0.0_f64;
        for triangle in &triangles {
            let positions = triangle.map(|i| Vec3::from_array(vertices[i as usize].pos));
            let centre = (positions[0] + positions[1] + positions[2]) / 3.0 * cfg.voxel_meters();
            let local = frame.local(glam::DVec2::new(f64::from(centre.x), f64::from(centre.z)));
            if local.min_element() < 4.0 || local.max_element() > 3996.0 { continue; }
            let source = engine_worldgen::planetary::reference_relief::height(local.x, local.y);
            let error = (f64::from(centre.y) - source).abs();
            errors.push(error);
            squared += error * error;
        }
        if !errors.is_empty() {
            errors.sort_unstable_by(f64::total_cmp);
            println!("[reference-mesh] triangle_centres={} rmse_m={:.3} p95_m={:.3} maximum_m={:.3}",
                errors.len(), (squared / errors.len() as f64).sqrt(),
                errors[(errors.len() * 95 / 100).min(errors.len() - 1)], errors[errors.len() - 1]);
        }
    }
    if let Some(p) = progress { p.begin("Surface Mesh", 0); }
    let mut split_cells = SplitCells::new();
    if adaptive_relief {
        let grid_end = base as usize + samples.len();
        let origin = Vec3::from_array(vertices[base as usize].pos);
        // Keep a sparse lookup for tree footing where the mesh left its grid.
        for &tri in &triangles {
            if tri.iter().all(|&id| (id as usize) < grid_end) { continue; }
            let [a, b, c] = tri.map(|i| Vec3::from_array(vertices[i as usize].pos));
            let p = ((a - origin) + (b - origin) + (c - origin)) / (3.0 * step);
            split_cells.entry(p.z as usize * side + p.x as usize).or_default().push(tri);
        }
    }
    let mut shore = shoreline::Edges::new();
    let bed = |id: u32| beds[(id - base) as usize];
    for tri in triangles {
        shore.triangle(tri, vertices, indices, bed);
    }
    skirt(vertices, indices, base, side, SKIRT_M / cfg.voxel_meters(), &shore);
    if let Some(p) = progress { p.finish_region(); }
    split_cells
}

/// Pure CPU build: samples every face's column grid and meshes it. Safe on any thread.
/// `anchor` adds a fine patch there; the coarse face grid leaves a hole under it.
/// Keep canonical tree anchors visible while their voxel chunks stream in.
fn append_patch_trees(
    mesh: &mut crate::mesh::ChunkMesh,
    cfg: &WorldSampleConfig,
    face: Face,
    center: [i32; 2],
    side: usize,
    split_cells: &SplitCells,
    progress: Option<&progress::Progress>,
    cancel: &AtomicBool,
) -> bool {
    use super::horizon::{proxy_box, vegetation::compact_crown};
    use crate::world::BlockId;

    if cfg.world_type != crate::world::WorldType::Planet {
        return true;
    }
    let scale = cfg.voxel_meters();
    let center = atlas::join(face, DVec3::new(f64::from(center[0]), 0.0, f64::from(center[1])));
    let cell = (cfg.blocks_for_span(12.0) as f32 * scale) as f64;
    let reach = (1536.0 / cell).ceil() as i32;
    let cx = (center.x / cell).floor() as i32;
    let cz = (center.z / cell).floor() as i32;
    let ground = BlockId::Log.surface();
    let started = std::time::Instant::now();
    // Use one stable placement cell in each two-by-two group. This bounds
    // sampling and leaves density shading to represent the rest of the stand.
    let rows: Vec<_> = ((cz - reach).div_euclid(2)..=(cz + reach).div_euclid(2)).collect();
    let workers = parallel::workers(parallel::cap_from_env("VOXEL_HORIZON_WORKERS", 8));
    let row_sites = ((cx + reach).div_euclid(2) - (cx - reach).div_euclid(2) + 1) as u64;
    if let Some(p) = progress { p.begin("Vegetation", rows.len() as u64 * row_sites); }
    let placements = parallel::map_blocks(&rows, 2, workers, cancel, |&z| {
        let row: Vec<_> = ((cx - reach).div_euclid(2)..=(cx + reach).div_euclid(2)).filter_map(|x| {
            if cancel.load(Ordering::Relaxed) { return None; }
            let hash = (x as u32).wrapping_mul(0x9e3779b9) ^ (z as u32).wrapping_mul(0x85ebca6b);
            crate::world::tree_visual_lod(cfg, 2 * x + (hash & 1) as i32,
                2 * z + ((hash >> 3) & 1) as i32, 8.0)
        }).collect();
        if let Some(p) = progress { p.advance(row_sites); }
        Some(row)
    });
    if cancel.load(Ordering::Relaxed) { return false; }
    let placements = placements.expect("planet tree placement worker failed");
    if let Some(p) = progress { p.begin("Vegetation Mesh", 0); }
    let mut trees = 0;
    for row in placements {
        if cancel.load(Ordering::Relaxed) { return false; }
        for mut tree in row {
            let (tree_face, local) = atlas::split(tree.base.as_dvec3());
            if tree_face != face || local.x.abs() + f64::from(tree.radius) >= atlas::HALF_M
                || local.z.abs() + f64::from(tree.radius) >= atlas::HALF_M
            {
                continue;
            }
            let gx = (f64::from(tree.base.x) - center.x + PATCH_M * 0.5) * side as f64 / PATCH_M;
            let gz = (f64::from(tree.base.z) - center.z + PATCH_M * 0.5) * side as f64 / PATCH_M;
            if !(0.0..side as f64).contains(&gx) || !(0.0..side as f64).contains(&gz) {
                continue;
            }
            let i = gz as usize * (side + 1) + gx as usize;
            let [a, b, c, d] = [i, i + 1, i + side + 1, i + side + 2].map(|i| mesh.vertices[i].pos[1]);
            let (u, v) = (gx.fract() as f32, gz.fract() as f32);
            let regular = if u + v <= 1.0 {
                a + u * (b - a) + v * (c - a)
            } else {
                d + (1.0 - u) * (c - d) + (1.0 - v) * (b - d)
            };
            let point = Vec2::new(tree.base.x, tree.base.z) / scale;
            let cell = gz as usize * side + gx as usize;
            let height = split_cells.get(&cell).and_then(|triangles| {
                triangles.iter().find_map(|tri| {
                    let [a, b, c] = tri.map(|id| Vec3::from_array(mesh.vertices[id as usize].pos));
                    let ab = Vec2::new(b.x - a.x, b.z - a.z);
                    let ac = Vec2::new(c.x - a.x, c.z - a.z);
                    let p = point - Vec2::new(a.x, a.z);
                    let area = ab.perp_dot(ac);
                    if area.abs() <= f32::EPSILON { return None; }
                    let u = p.perp_dot(ac) / area;
                    let v = ab.perp_dot(p) / area;
                    (u >= 0.0 && v >= 0.0 && u + v <= 1.0)
                        .then_some(a.y + u * (b.y - a.y) + v * (c.y - a.y))
                })
            }).unwrap_or(regular) * scale;
            if (height - tree.base.y).abs() > tree.height.min(10.0) {
                continue;
            }
            tree.base.y = height;
            let at = (tree.base / scale).floor().as_ivec3();
            let trunk = tree.height * 0.65;
            let width = (tree.radius * 0.2).clamp(0.35, 2.0);
            proxy_box(mesh, tree.base + Vec3::Y * (trunk * 0.5),
                Vec3::new(width, trunk, width), scale, BlockId::Log.color(at), ground.shader_key());
            compact_crown(mesh, &tree, scale, tree.foliage.color(at), 1.0);
            trees += 1;
        }
    }
    println!("[planet-cover-trees] face={face:?} trees={trees} workers={workers} build_ms={:.1}",
        started.elapsed().as_secs_f64() * 1000.0);
    true
}

pub fn build_cpu(cfg: WorldSampleConfig, anchor: Option<Anchor>) -> Cpu {
    build(cfg, anchor, true, Shells::default(), None, None)
}

/// First frame coverage uses the solved parent and the ordinary shoreline
/// mesher. The full cover adds fine erosion, vegetation and adaptive banks.
pub fn build_startup(cfg: WorldSampleConfig, anchor: Option<Anchor>) -> Cpu {
    build_startup_with_progress(cfg, anchor, None)
}

fn append_shell(
    cfg: &WorldSampleConfig,
    grid: &Grid,
    shell: &crate::mesh::ChunkMesh,
    hole: Option<[f64; 4]>,
    vertices: &mut Vec<Vertex3>,
    indices: &mut Vec<u32>,
) {
    let base = vertices.len() as u32;
    vertices.extend_from_slice(&shell.vertices);
    let Some([x0, z0, x1, z1]) = hole else {
        indices.extend(shell.indices.iter().map(|&id| base + id));
        return;
    };
    let cell = grid.span_m / grid.side as f64;
    let face_x = atlas::join(grid.face, DVec3::ZERO).x;
    let scale = f64::from(cfg.voxel_meters());
    for tri in shell.indices.chunks_exact(3) {
        // Shore splits stay inside their parent grid cell. Cut whole cells
        // so the overlap and the face skirts survive a patch move.
        let centre = tri.iter().map(|&id| {
            let [x, _, z] = shell.vertices[id as usize].pos;
            glam::DVec2::new(f64::from(x) * scale - face_x, f64::from(z) * scale)
        }).sum::<glam::DVec2>() / 3.0;
        let cx = grid.min_m[0] + (((centre.x - grid.min_m[0]) / cell).floor() + 0.5) * cell;
        let cz = grid.min_m[1] + (((centre.y - grid.min_m[1]) / cell).floor() + 0.5) * cell;
        if cx <= x0 || cx >= x1 || cz <= z0 || cz >= z1 {
            indices.extend(tri.iter().map(|&id| base + id));
        }
    }
}

pub fn build_startup_with_progress(cfg: WorldSampleConfig, anchor: Option<Anchor>, progress: Option<&progress::Progress>) -> Cpu {
    build(cfg, anchor, false, Shells::default(), progress, None)
}

fn append_patch(mesh: &crate::mesh::ChunkMesh, vertices: &mut Vec<Vertex3>, indices: &mut Vec<u32>) {
    let base = vertices.len() as u32;
    vertices.extend_from_slice(&mesh.vertices);
    indices.extend(mesh.indices.iter().map(|&id| base + id));
}

fn raw_cover(cfg: WorldSampleConfig, anchor: Option<Anchor>, refined: bool, mesh: crate::mesh::ChunkMesh, shells: Shells) -> Cpu {
    let backstop = backstop_indices(&cfg, anchor, &mesh);
    Cpu { cfg, refined, anchor, vertices: mesh.vertices, indices: mesh.indices, backstop, shells, source: None, collar: None, variants: None }
}

fn build(cfg: WorldSampleConfig, anchor: Option<Anchor>, mut refined: bool, mut shells: Shells,
    progress: Option<&progress::Progress>, streaming: Option<Streaming<'_>>) -> Cpu {
    let mut vertices = Vec::with_capacity(6 * (SIDE + 1).pow(2));
    let mut indices = Vec::with_capacity(36 * SIDE * SIDE);
    let meander = engine_worldgen::planetary::meander_coverage(cfg);
    let relief = if !refined { None } else if engine_worldgen::planetary::reference_relief::active(&cfg) {
        Some(engine_worldgen::planetary::reference_relief::frame())
    } else { matches!(cfg.generator_version,
        engine_worldgen::planetary::FOLDED_SCARP_GENERATOR | engine_worldgen::planetary::BEDROCK_SCARP_GENERATOR)
        .then(|| engine_worldgen::planetary::review::catchment::playable_frame(&cfg))
        .flatten() };
    let anchor = anchor.filter(|_| cfg.is_planet());
    let patch = anchor.map(|(face, [x, z])| {
        let h = PATCH_M * 0.5;
        (face, [x as f64 - h, z as f64 - h, x as f64 + h, z as f64 + h])
    });
    for face in Face::ALL {
        let here = patch.filter(|(f, _)| *f == face).map(|(_, rect)| rect);
        let side = if refined { side(&cfg, face) } else if here.is_some() { SIDE } else { 64 };
        // The coarse hole is one coarse cell smaller than the patch so their skirts overlap.
        let cell = 2.0 * atlas::HALF_M / side as f64;
        let hole = here.map(|[a, b, c, d]| [a + cell, b + cell, c - cell, d - cell]);
        let grid = Grid { face, min_m: [-atlas::HALF_M; 2], span_m: 2.0 * atlas::HALF_M, side, refined };
        let build_shell = || {
            let mut shell = crate::mesh::ChunkMesh::empty();
            mesh_grid(&cfg, &grid, &mut shell.vertices, &mut shell.indices, &meander, None, progress);
            shell
        };
        if refined {
            if shells.faces[face as usize].is_some() {
                if let Some(p) = progress { p.finish_region(); }
            }
            let shell = shells.faces[face as usize].get_or_insert_with(|| Arc::new(build_shell()));
            append_shell(&cfg, &grid, shell, hole, &mut vertices, &mut indices);
        } else {
            append_shell(&cfg, &grid, &build_shell(), hole, &mut vertices, &mut indices);
        }
    }
    if let Some((face, [x0, z0, _, _])) = patch && let Some(key @ (_, centre)) = anchor {
        let side = if refined { patch_side(&cfg, face) } else { 96 };
        let relief = relief.filter(|frame| {
            let centre = frame.chart(2000.0, 2000.0);
            let (relief_face, point) = atlas::split(DVec3::new(centre.x, 0.0, centre.y));
            relief_face == face && point.x >= x0 - 2800.0 && point.x <= x0 + PATCH_M + 2800.0
                && point.z >= z0 - 2800.0 && point.z <= z0 + PATCH_M + 2800.0
        });
        let cached = shells.patches.iter().position(|(saved, _)| *saved == key)
            .and_then(|index| shells.patches.remove(index));
        let mesh = if let Some((_, mesh)) = cached {
            if let Some(p) = progress { p.begin("Cached Terrain", 0); p.finish_region(); }
            eprintln!("[planet-cover-patch] anchor={key:?} cache_hit=true");
            mesh
        } else {
            let mut patch_mesh = crate::mesh::ChunkMesh::empty();
            let grid = Grid { face, min_m: [x0, z0], span_m: PATCH_M, side, refined };
            let split_cells = mesh_grid(&cfg, &grid, &mut patch_mesh.vertices, &mut patch_mesh.indices, &meander, relief, progress);
            if refined {
                let never_cancel = AtomicBool::new(false);
                let cancel = streaming.as_ref().map_or(&never_cancel, |job| job.cancel);
                if let Some(job) = streaming.as_ref().filter(|_| !cancel.load(Ordering::Relaxed)) {
                    let mut terrain = crate::mesh::ChunkMesh { vertices: vertices.clone(), indices: indices.clone() };
                    append_patch(&patch_mesh, &mut terrain.vertices, &mut terrain.indices);
                    (job.terrain_ready)(raw_cover(cfg, anchor, false, terrain, shells.clone()));
                }
                refined = append_patch_trees(&mut patch_mesh, &cfg, face, centre, side, &split_cells, progress, cancel);
            }
            Arc::new(patch_mesh)
        };
        append_patch(&mesh, &mut vertices, &mut indices);
        if refined {
            shells.patches.push_front((key, mesh));
            shells.patches.truncate(RECENT_PATCHES);
        }
    }
    raw_cover(cfg, anchor, refined, crate::mesh::ChunkMesh { vertices, indices }, shells)
}

/// Depth of the curtain hung from each face border.
const SKIRT_M: f32 = 400.0;

/// Hangs a curtain below a face's border. Neighbouring faces sample different
/// chart columns, so their border heights differ and the sky showed through.
fn skirt(vertices: &mut Vec<Vertex3>, indices: &mut Vec<u32>, base: u32, side: usize, drop: f32, shore: &shoreline::Edges) {
    let at = |x: usize, z: usize| base + (z * (side + 1) + x) as u32;
    let mut ring = Vec::with_capacity(4 * side + 1);
    ring.extend((0..side).map(|x| at(x, 0)));
    ring.extend((0..side).map(|z| at(side, z)));
    ring.extend((1..=side).rev().map(|x| at(x, side)));
    ring.extend((1..=side).rev().map(|z| at(0, z)));
    ring.push(ring[0]);
    let first = vertices.len() as u32;
    let lower = |vertices: &mut Vec<Vertex3>, i: u32| {
        let mut v = vertices[i as usize];
        v.pos[1] -= drop;
        let id = vertices.len() as u32;
        vertices.push(v);
        id
    };
    for &i in &ring[..ring.len() - 1] {
        lower(vertices, i);
    }
    let count = (ring.len() - 1) as u32;
    // Both windings: the curtain is seen from either side of the seam.
    let mut span = |a, b, sa, sb| {
        indices.extend_from_slice(&[a, sa, b, b, sa, sb, a, b, sa, b, sb, sa]);
    };
    for k in 0..count {
        let (a, b) = (ring[k as usize], ring[k as usize + 1]);
        let (sa, sb) = (first + k, first + (k + 1) % count);
        if let Some([ac, bc]) = shore.get(a, b) {
            span(a, ac, sa, lower(vertices, ac));
            span(bc, b, lower(vertices, bc), sb);
        } else {
            span(a, b, sa, sb);
        }
    }
}

/// Block whose material the cover draws, and its colour shaded toward crowns
/// by the cell's canopy share. Closed stands take the leaf material.
fn cover_material(ground: crate::world::BlockId, canopy: f32, at: glam::IVec3) -> (crate::world::BlockId, [f32; 3]) {
    use crate::world::BlockId;
    let base = ground.color(at);
    let canopy = canopy.clamp(0.0, 1.0);
    if canopy <= 0.0 {
        return (ground, base);
    }
    let crown = BlockId::Leaves.color(at);
    let t = canopy * 0.75;
    let color = [0, 1, 2].map(|i| base[i] + (crown[i] - base[i]) * t);
    (if canopy > 0.5 { BlockId::Leaves } else { ground }, color)
}

/// Places outline vertices exactly where their chunks are drawn.
pub fn outlines(vertices: &[crate::mesh::LineVertex], frame: &super::planet_frame::Eye, scale: f32) -> Vec<crate::mesh::LineVertex> {
    let eye = frame.chart_m();
    vertices.iter().map(|v| {
        let point = DVec3::from_array(v.pos.map(f64::from)) * scale as f64;
        let projected = eye + frame.relative(point);
        crate::mesh::LineVertex { pos: (projected / scale as f64).as_vec3().to_array(), color: v.color }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::WorldType;

    fn cfg(scale: VoxelScalePreset) -> WorldSampleConfig {
        // The shell cover without the solved face; tests that want it raise the version.
        WorldSampleConfig { planet: None, generator_version: engine_worldgen::basin::VERSION - 1, seed: 7, world_type: WorldType::Planet, voxel_scale: scale }
    }

    #[test]
    fn cover_has_a_full_grid_per_face_and_unit_normals() {
        let cpu = build_cpu(cfg(VoxelScalePreset::OneMeter), None);
        // Shoreline splits add vertices and triangles to the six base grids.
        assert!(cpu.vertices.len() >= 6 * ((SIDE + 1) * (SIDE + 1) + 4 * SIDE));
        assert!(cpu.indices.len() >= 6 * (6 * SIDE * SIDE + 12 * 4 * SIDE));
        assert!(cpu.indices.iter().all(|&i| (i as usize) < cpu.vertices.len()));
        for v in &cpu.vertices {
            let n = glam::Vec3::from_array(v.normal);
            assert!((n.length() - 1.0).abs() < 1e-3, "normal {n}");
        }
    }

    #[test]
    fn cover_vertices_carry_the_ground_and_water_materials() {
        // The shader shades cover by material key; key 0 drew every cell as one dark family.
        let cpu = build_cpu(cfg(VoxelScalePreset::OneMeter), None);
        let water = crate::world::BlockId::Water.surface().shader_key();
        let sea = cpu.vertices.iter().filter(|v| v.surface[2] == water).count();
        let land = cpu.vertices.iter().filter(|v| v.surface[2] == 0.0 && v.surface[3] > 0.0).count();
        assert!(sea > 0 && land > 0, "sea {sea} land {land}");
        assert!(cpu.vertices.iter().all(|v| v.surface[2] == water || v.surface[3] > 0.0));
    }

    #[test]
    fn every_face_border_hangs_a_skirt_below_it() {
        let cpu = build_cpu(cfg(VoxelScalePreset::OneMeter), None);
        let grid = (SIDE + 1) * (SIDE + 1);
        let per_face = grid + 4 * SIDE;
        for face in 0..6 {
            let skirt = &cpu.vertices[face * per_face + grid..(face + 1) * per_face];
            let corner = cpu.vertices[face * per_face];
            assert_eq!(skirt[0].pos[0], corner.pos[0]);
            assert!((corner.pos[1] - skirt[0].pos[1] - SKIRT_M).abs() < 1e-3);
        }
    }

    #[test]
    fn patch_follows_the_eye_on_every_face_and_stays_inside_it() {
        let solved = WorldSampleConfig { generator_version: engine_worldgen::basin::VERSION, ..cfg(VoxelScalePreset::OneMeter) };
        let first = Face::ALL[0];
        assert_eq!(anchor_for(&solved, glam::Vec3::new(700.0, 40.0, -900.0)), Some((first, [0, 0])));
        assert_eq!(anchor_for(&solved, glam::Vec3::new(1100.0, 40.0, 3100.0)), Some((first, [2048, 4096])));
        // Near the face edge the patch is pulled inward rather than crossing it.
        let (_, [x, _]) = anchor_for(&solved, glam::Vec3::new(15_900.0, 0.0, 0.0)).unwrap();
        assert!(f64::from(x) + PATCH_M * 0.5 <= atlas::HALF_M);
        // The gap between face 0 and face 1 holds no patch.
        assert_eq!(anchor_for(&solved, glam::Vec3::new(40_000.0, 0.0, 0.0)), None);
        // Face 5 at chart x = 329,440 m gets a patch in face-local metres.
        let far = anchor_for(&cfg(VoxelScalePreset::OneMeter), glam::Vec3::new(329_440.0, 30.0, 700.0)).unwrap();
        assert_eq!(far, (Face::ALL[5], [2048 * ((329_440.0f64 - 5.0 * atlas::PITCH_M) / 2048.0).round() as i32, 0]));
        let cpu = build_cpu(solved, Some((first, [0, 0])));
        assert!(cpu.indices.iter().all(|&i| (i as usize) < cpu.vertices.len()));
        let plain = build_cpu(solved, None);
        assert!(cpu.vertices.len() > plain.vertices.len() + PATCH_SIDE * PATCH_SIDE);
        let shell = cfg(VoxelScalePreset::OneMeter);
        let with = build_cpu(shell, Some(far));
        let without = build_cpu(shell, None);
        assert!(with.indices.iter().all(|&i| (i as usize) < with.vertices.len()));
        assert!(with.vertices.len() > without.vertices.len() + SHELL_PATCH_SIDE * SHELL_PATCH_SIDE);
    }
}
