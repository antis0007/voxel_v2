//! Asynchronous backdrop with authoritative edited voxel surfaces.
#[path = "horizon_edits.rs"]
mod edits;
#[path = "horizon_batches.rs"]
mod batches;
#[path = "horizon_pages.rs"]
mod pages;
#[path = "horizon_upload.rs"]
mod upload;
#[path = "horizon_vegetation.rs"]
mod vegetation;
#[path = "horizon_water.rs"]
mod water;
use crate::mesh::{ChunkMesh, Vertex3};
use crate::world::{WorldSampleConfig, distant_surface};
use glam::{IVec3, Vec3};
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Instant;

pub const FAR_PLANE: f32 = 24576.0;
const LEVELS: usize = 7;
const SIDE: usize = 97;
const HALF: usize = (SIDE - 1) / 2;
type Samples = HashMap<(i32, i32), (f32, crate::world::BlockId, u8)>;
pub struct GpuMesh {
    pub pages: Vec<GpuPage>,
    pub cull: bool,
}
pub struct GpuPage {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub count: u32,
    pub batches: Vec<batches::DrawBatch>,
}

#[derive(Default)]
pub struct Horizon {
    config: Option<WorldSampleConfig>,
    source_root: Option<Arc<std::path::PathBuf>>,
    revision: Option<u64>,
    edit_fingerprint: Option<u64>,
    fingerprint_snapshot: Option<crate::world::WorldSnapshot>,
    checked_eye: Option<IVec3>,
    checked_radius: f32,
    last_check: Option<Instant>,
    last_build: Option<Instant>,
    center: Option<(i32, i32)>,
    rendered_center: Option<(i32, i32)>,
    pending: Option<mpsc::Receiver<(Vec<pages::Page>, Samples)>>,
    upload: Option<upload::Upload>,
    samples: Samples,
    cancel: Arc<AtomicBool>,
    pub mesh: Option<GpuMesh>,
}

impl Drop for Horizon {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Horizon {
    pub fn visibility(&self) -> f32 {
        if self.mesh.is_none() {
            return 0.0;
        }
        2.0
    }
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &mut crate::world::World,
        revision: u64,
        eye: Vec3,
        hidden_radius: f32,
    ) {
        let cfg=world.sample_config();
        if self.config != Some(cfg) || self.source_root.as_ref().is_none_or(|root| !Arc::ptr_eq(root, &world.regions.view.root)) {
            self.cancel.store(true, Ordering::Relaxed);
            self.pending = None;
            self.upload = None;
            self.mesh = None;
            self.samples.clear();
            self.center = None;
            self.config = Some(cfg);
            self.source_root = Some(world.regions.view.root.clone());
            self.rendered_center = None;
            self.last_build = None;
            self.edit_fingerprint = None;
            self.last_check = None;
        }
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok((pages, samples)) => {
                    let limit = device.limits().max_buffer_size;
                    if pages.iter().any(|page| {
                        (page.mesh.vertices.len() as u64).saturating_mul(std::mem::size_of::<Vertex3>() as u64) > limit
                            || (page.mesh.indices.len() as u64).saturating_mul(4) > limit
                    }) {
                        // Keep the last usable backdrop rather than triggering a
                        // fatal GPU validation error on unusually dense worlds.
                        eprintln!("[horizon] invalid upload page: device_limit={limit}");
                        self.samples = samples;
                        self.pending = None;
                        return;
                    }
                    self.upload = Some(upload::Upload::new(pages, samples, self.center));
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.center = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.upload.as_mut().is_some_and(|upload| upload.advance(device, queue)) {
            let (mesh, samples, center) = self.upload.take().unwrap().finish();
            self.mesh = Some(mesh);
            self.samples = samples;
            self.rendered_center = center;
        }
        // After a teleport, retain the normal streaming veil until a local
        // backdrop arrives instead of exposing an obsolete mesh's outer edge.
        if self
            .rendered_center
            .is_some_and(|(x, z)| (eye.x - x as f32).abs().max((eye.z - z as f32).abs()) > 2048.0)
        {
            self.mesh = None;
        }
        // Every LOD sample stays on a fixed world-space lattice during recentering.
        let center = (
            (eye.x / 512.0).round() as i32 * 512,
            (eye.z / 512.0).round() as i32 * 512,
        );
        if self.pending.is_some() || self.upload.is_some() { return; }
        let eye_cell = eye.floor().as_ivec3();
        let moved = self.checked_eye != Some(eye_cell) || self.checked_radius != hidden_radius;
        if self.center == Some(center) && !moved && self.revision == Some(revision) { return; }
        if self.center == Some(center) && self.last_check.is_some_and(|t| t.elapsed().as_secs_f32() < 0.5) { return; }
        self.last_check = Some(Instant::now());
        self.checked_eye = Some(eye_cell);
        self.checked_radius = hidden_radius;
        self.revision = Some(revision);
        // Changes wholly inside the shader's hidden near region are rendered by
        // resident chunks. Avoid rebuilding every distant ring for local fire.
        // Recheck on movement so those edits become authoritative before exposure.
        // Simulation can mutate the world every frame, but a busy worker or
        // an unchanged backdrop does not need another complete resident map.
        let snapshot = world.snapshot();
        let fingerprint = edits::visible_fingerprint(&snapshot, eye, hidden_radius);
        if self.center == Some(center) && self.edit_fingerprint == Some(fingerprint) { return; }
        if self.center == Some(center) && self.last_build.is_some_and(|t| t.elapsed().as_secs_f32() < 2.0) {
            self.revision = None;
            return;
        }
        self.last_build = Some(Instant::now());
        self.edit_fingerprint = Some(fingerprint);
        // Retain Arc identities until replacement; allocator reuse cannot hide edits.
        self.fingerprint_snapshot = Some(snapshot.clone());
        self.center = Some(center);
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.pending = Some(rx);
        // Keep individual uploads bounded even on adapters allowing very large
        // buffers. The limit is passed to the worker, never repaired by dropping
        // geometry on the render thread.
        let page_limit = device.limits().max_buffer_size.min(64 * 1024 * 1024);
        let samples = std::mem::take(&mut self.samples);
        std::thread::spawn(move || {
            let start = Instant::now();
            if let Some(mut result) = build(cfg, center, &cancel, samples) {
                if edits::apply(&mut result.0, &snapshot, center, &cancel).is_none() { return; }
                let batches = batches::partition(&mut result.0, cfg.voxel_meters(), center);
                println!(
                    "[horizon] center={center:?} triangles={} vertex_bytes={} index_bytes={} build_ms={:.0}",
                    result.0.indices.len() / 3,
                    result.0.vertices.len() * std::mem::size_of::<Vertex3>(),
                    result.0.indices.len() * std::mem::size_of::<u32>(),
                    start.elapsed().as_secs_f64() * 1000.0
                );
                println!("[horizon] spatial_batches={}", batches.len());
                let Some(pages) = pages::pack(result.0, batches, page_limit, &cancel) else { return; };
                println!("[horizon] upload_pages={} page_limit_bytes={} packed_vertex_bytes={} packed_index_bytes={} max_page_vertex_bytes={} max_page_index_bytes={}",
                    pages.len(), page_limit,
                    pages.iter().map(|p| p.mesh.vertices.len() * std::mem::size_of::<Vertex3>()).sum::<usize>(),
                    pages.iter().map(|p| p.mesh.indices.len() * 4).sum::<usize>(),
                    pages.iter().map(|p| p.mesh.vertices.len() * std::mem::size_of::<Vertex3>()).max().unwrap_or(0),
                    pages.iter().map(|p| p.mesh.indices.len() * 4).max().unwrap_or(0));
                let _ = tx.send((pages, result.1));
            }
        });
    }
}

fn build(
    cfg: WorldSampleConfig,
    center: (i32, i32),
    cancel: &AtomicBool,
    mut old: Samples,
) -> Option<(ChunkMesh, Samples)> {
    let mut mesh = ChunkMesh::empty();
    let mut samples = Samples::new();
    let scale = cfg.voxel_meters();
    for level in 0..LEVELS {
        let step = (8 << level) as f32;
        let base = mesh.vertices.len() as u32;
        for z in 0..SIDE {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            for x in 0..SIDE {
                let wx = center.0 as f32 + (x as f32 - HALF as f32) * step;
                let wz = center.1 as f32 + (z as f32 - HALF as f32) * step;
                let bx = (wx / scale).floor() as i32;
                let bz = (wz / scale).floor() as i32;
                let detail = if level <= 1 { 0 } else { 1 };
                let (height, block, _) = *samples.entry((bx, bz)).or_insert_with(|| {
                    old.remove(&(bx, bz))
                        .filter(|s| s.2 <= detail)
                        .unwrap_or_else(|| {
                            let (h, b) = if detail == 0 {
                                distant_surface(&cfg, bx, bz)
                            } else {
                                crate::world::landscape::preview_surface(&cfg, bx, bz)
                            };
                            (h, b, detail)
                        })
                });
                let material = block.surface();
                mesh.vertices.push(Vertex3 {
                    pos: [wx / scale, height, wz / scale],
                    color: block.color(IVec3::new(bx, height as i32, bz)),
                    normal: [0.0; 3],
                    // Keep procedural shading generic. The vertex shader resolves family
                    // colors before interpolation across mixed-material triangles.
                    surface: [material.roughness, material.specular, 0.0, material.shader_key()],
                    ao_strength: -1.0, // Explicit backdrop tag; shader uses its absolute value for AO.
                });
                // The cheaper outer heightfield does not match the inner ring.
                // Copy the inner ring's edge so the two surfaces share one line.
                if level > 0 {
                    let inner_lo = HALF / 2;
                    let inner_hi = HALF * 3 / 2;
                    let on_inner = (x == inner_lo || x == inner_hi || z == inner_lo || z == inner_hi)
                        && (inner_lo..=inner_hi).contains(&x)
                        && (inner_lo..=inner_hi).contains(&z);
                    if on_inner {
                        let px = 2 * x - HALF;
                        let pz = 2 * z - HALF;
                        if (0..SIDE).contains(&px) && (0..SIDE).contains(&pz) {
                            let prev = (base as usize) - SIDE * SIDE + pz * SIDE + px;
                            mesh.vertices.last_mut().unwrap().pos[1] = mesh.vertices[prev].pos[1];
                        }
                    }
                }
            }
        }
        // Do NOT snap outer-edge vertices before accumulating triangle face
        // normals; otherwise the inner ring's face normals never match the
        // outer ring's Exact positions in the HashMap below.
        for z in 0..SIDE - 1 {
            for x in 0..SIDE - 1 {
                // Geometry outside the complete 16 km fade is never visible.
                // Keep a one-cell margin for the last intersecting triangle.
                let reach=((x as f32-HALF as f32+0.5).abs().max((z as f32-HALF as f32+0.5).abs()))*step;
                if reach>17000.0+step {continue;}
                if level > 0
                    && (HALF / 2..HALF * 3 / 2).contains(&x)
                    && (HALF / 2..HALF * 3 / 2).contains(&z)
                {
                    continue;
                }
                let a = base + (z * SIDE + x) as u32;
                let b = a + 1;
                let c = a + SIDE as u32;
                let d = c + 1;
                for tri in [[a, c, b], [b, c, d]] {
                    let p = tri.map(|i| Vec3::from_array(mesh.vertices[i as usize].pos));
                    let normal = (p[1] - p[0]).cross(p[2] - p[0]);
                    for i in tri {
                        let v = &mut mesh.vertices[i as usize];
                        v.normal = (Vec3::from_array(v.normal) + normal).to_array();
                    }
                    mesh.indices.extend_from_slice(&tri);
                }
            }
        }
    }
    // Share normals where adjacent rings meet, as well as sharing their heights.
    let mut normals = std::collections::HashMap::new();
    for v in &mesh.vertices {
        let key = v.pos.map(f32::to_bits);
        *normals.entry(key).or_insert(Vec3::ZERO) += Vec3::from_array(v.normal);
    }
    for v in &mut mesh.vertices {
        v.normal = normals[&v.pos.map(f32::to_bits)]
            .try_normalize()
            .unwrap_or(Vec3::Y)
            .to_array();
    }
    // Odd fine-edge vertices match the coarse edge's linear height, normal,
    // and colour so adjacent rings share identical boundary attributes.
    for level in 0..LEVELS - 1 {
        let base = level * SIDE * SIDE;
        let edge = SIDE - 1;
        for i in (1..edge).step_by(2) {
            for (at, a, b) in [
                (i, i - 1, i + 1),
                (edge * SIDE + i, edge * SIDE + i - 1, edge * SIDE + i + 1),
                (i * SIDE, (i - 1) * SIDE, (i + 1) * SIDE),
                (
                    i * SIDE + edge,
                    (i - 1) * SIDE + edge,
                    (i + 1) * SIDE + edge,
                ),
            ] {
                let a = mesh.vertices[base + a];
                let b = mesh.vertices[base + b];
                let v = &mut mesh.vertices[base + at];
                v.pos[1] = (a.pos[1] + b.pos[1]) * 0.5;
                v.normal = (Vec3::from_array(a.normal) + Vec3::from_array(b.normal))
                    .normalize_or_zero()
                    .to_array();
                v.color =
                    ((Vec3::from_array(a.color) + Vec3::from_array(b.color)) * 0.5).to_array();
            }
        }
    }
    water::append(&mut mesh, &cfg);
    vegetation::append(&mut mesh,&cfg,center);
    Some((mesh, samples))
}

// Flat normals and whole voxel faces avoid smooth, jewel-like highlights.
fn proxy_box(mesh: &mut ChunkMesh, center: Vec3, size: Vec3, scale: f32, color: [f32; 3], kind: f32) {
    let h = size * 0.5;
    let corners = [Vec3::new(-h.x,-h.y,-h.z),Vec3::new(h.x,-h.y,-h.z),
        Vec3::new(h.x,h.y,-h.z),Vec3::new(-h.x,h.y,-h.z),
        Vec3::new(-h.x,-h.y,h.z),Vec3::new(h.x,-h.y,h.z),
        Vec3::new(h.x,h.y,h.z),Vec3::new(-h.x,h.y,h.z)];
    for (ids, normal) in [([4,5,6,7],Vec3::Z),([1,0,3,2],-Vec3::Z),
        ([7,6,2,3],Vec3::Y),([0,1,5,4],-Vec3::Y),
        ([5,1,2,6],Vec3::X),([0,4,7,3],-Vec3::X)] {
        let base = mesh.vertices.len() as u32;
        for id in ids {
            mesh.vertices.push(Vertex3 { pos: ((center + corners[id]) / scale).to_array(),
                normal: normal.to_array(), color, surface: [0.95,0.02,kind,0.0], ao_strength: -1.0 });
        }
        mesh.indices.extend_from_slice(&[base,base+1,base+2,base,base+2,base+3]);
    }
}
