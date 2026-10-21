//! Asynchronous backdrop with stitched rings and authoritative edited surfaces.
#[path = "horizon_batches.rs"]
mod batches;
#[path = "horizon_edits.rs"]
mod edits;
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
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const FAR_PLANE: f32 = 24576.0;
const LEVELS: usize = 7;
/// Keep the complete grids contiguous: water and vegetation index this lattice.
const SIDE: usize = 193;
const HALF: usize = (SIDE - 1) / 2;
const CENTER_GRID: i32 = 512;
const MAX_CENTER_DISTANCE: f32 = 2048.0;
const CHECK_INTERVAL: Duration = Duration::from_millis(500);
const REBUILD_INTERVAL: Duration = Duration::from_secs(2);
const RETRY_INTERVAL: Duration = Duration::from_secs(2);

/// Sample spacing in metres. All appenders must use the same ring lattice.
fn ring_step(level: usize) -> f32 {
    (4u32 << level) as f32
}

/// Exact sampling through 32 m, then a distinct preview footprint at each LOD.
/// The tag remains a u8 so the existing page/upload cache contract is unchanged.
fn sample_detail(level: usize) -> u8 {
    level.saturating_sub(3) as u8
}

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

/// Immutable metadata for one captured world. Every retained fingerprint owns
/// its snapshot: allocator reuse must not make Arc-based edit identities equal.
#[derive(Clone)]
struct BuildTarget {
    center: (i32, i32),
    fingerprint: u64,
    snapshot: crate::world::WorldSnapshot,
    eye: Vec3,
    hidden_radius: f32,
}

impl BuildTarget {
    fn matches(&self, other: &Self) -> bool {
        self.center == other.center
            && self.fingerprint == other.fingerprint
            && self.hidden_radius == other.hidden_radius
    }

    fn too_far_from(&self, eye: Vec3) -> bool {
        (eye.x - self.center.0 as f32)
            .abs()
            .max((eye.z - self.center.1 as f32).abs())
            > MAX_CENTER_DISTANCE
    }
}

#[derive(Default)]
pub struct Horizon {
    config: Option<WorldSampleConfig>,
    source_root: Option<Arc<std::path::PathBuf>>,
    // Observation state may advance while a worker is busy. It is never used
    // as evidence that a replacement has reached the GPU.
    checked_revision: Option<u64>,
    checked_eye: Option<IVec3>,
    checked_radius: f32,
    last_check: Option<Instant>,
    last_build: Option<Instant>,
    desired: Option<BuildTarget>,
    active: Option<BuildTarget>,
    published: Option<BuildTarget>,
    failed: Option<(BuildTarget, Instant)>,
    pending: Option<mpsc::Receiver<(Vec<pages::Page>, Samples)>>,
    upload: Option<upload::Upload>,
    // A canceled worker is retained until it exits. Repeated teleports cannot
    // create an unbounded set of detached builders, and update never joins live work.
    worker: Option<JoinHandle<()>>,
    samples: Samples,
    cancel: Arc<AtomicBool>,
    pub mesh: Option<GpuMesh>,
    revealed: f32,
    reveal_tick: Option<Instant>,
}

impl Drop for Horizon {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        // Dropping the handle detaches; never wait for a helper on the render thread.
    }
}

impl Horizon {
    pub fn visibility(&self) -> f32 {
        if self.mesh.is_some() {
            self.revealed
        } else {
            0.0
        }
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
        let now = Instant::now();
        let dt = self
            .reveal_tick
            .map(|t| now.saturating_duration_since(t).as_secs_f32())
            .unwrap_or(0.0)
            .min(0.05);
        self.reveal_tick = Some(now);
        self.update_mesh(device, queue, world, revision, eye, hidden_radius, now);
        // Decide visibility after publication/invalidation, including early returns.
        self.revealed = if self.mesh.is_some() {
            (self.revealed + dt / 1.8).min(1.0)
        } else {
            0.0
        };
    }

    fn clear_mesh(&mut self) {
        self.mesh = None;
        self.published = None;
        self.revealed = 0.0;
    }

    fn cancel_work(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.pending = None;
        self.upload = None;
        self.active = None;
        // Keep worker until is_finished(); its next receiver can never be confused
        // with this one, and at most one CPU build is active per Horizon instance.
    }

    fn fail_active(&mut self, now: Instant) {
        if let Some(target) = self.active.take() {
            self.failed = Some((target, now));
        }
        self.pending = None;
        self.upload = None;
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn reap_worker(&mut self) {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            if self.worker.take().unwrap().join().is_err() {
                eprintln!("[horizon] worker panicked; keeping the last published mesh");
            }
        }
    }

    fn update_mesh(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &mut crate::world::World,
        revision: u64,
        eye: Vec3,
        hidden_radius: f32,
        now: Instant,
    ) {
        self.reap_worker();
        let cfg = world.sample_config();
        if self.config != Some(cfg)
            || self
                .source_root
                .as_ref()
                .is_none_or(|root| !Arc::ptr_eq(root, &world.regions.view.root))
        {
            self.cancel_work();
            self.clear_mesh();
            self.samples.clear();
            self.config = Some(cfg);
            self.source_root = Some(world.regions.view.root.clone());
            self.desired = None;
            self.failed = None;
            self.checked_revision = None;
            self.checked_eye = None;
            self.last_check = None;
            self.last_build = None;
        }
        let scale = cfg.voxel_meters();
        let center = snapped_center(eye);
        if !eye.is_finite()
            || !hidden_radius.is_finite()
            || hidden_radius < 0.0
            || !scale.is_finite()
            || scale <= 0.0
            || center.is_none()
        {
            self.cancel_work();
            self.clear_mesh();
            self.desired = None;
            return;
        }
        let center = center.unwrap();

        // Hard invalidation precedes receiving/advancing results and the busy
        // return. A result from a previous world or a far-away center never uploads.
        if self
            .published
            .as_ref()
            .is_some_and(|target| target.too_far_from(eye))
        {
            self.clear_mesh();
        }
        if self
            .active
            .as_ref()
            .is_some_and(|target| target.too_far_from(eye))
        {
            self.cancel_work();
        }

        let eye_cell = eye.floor().as_ivec3();
        let changed_center = self
            .desired
            .as_ref()
            .is_none_or(|target| target.center != center);
        let needs_check = changed_center
            || self.checked_eye != Some(eye_cell)
            || self.checked_radius != hidden_radius
            || self.checked_revision != Some(revision);
        let check_due = changed_center
            || self
                .last_check
                .is_none_or(|last| now.saturating_duration_since(last) >= CHECK_INTERVAL);
        if needs_check && check_due {
            let snapshot = world.snapshot();
            let fingerprint = edits::visible_fingerprint(&snapshot, eye, hidden_radius);
            self.desired = Some(BuildTarget {
                center,
                fingerprint,
                snapshot,
                eye,
                hidden_radius,
            });
            self.checked_revision = Some(revision);
            self.checked_eye = Some(eye_cell);
            self.checked_radius = hidden_radius;
            self.last_check = Some(now);
        }

        // Each receiver and upload has one immutable active target. A newer
        // desired snapshot is coalesced as a follow-up, allowing useful progress
        // while local simulation changes revision every frame. Visible edits are
        // eventually consistent here; immediate edit masking belongs to coverage.
        if let Some(result) = self.pending.as_ref().map(|rx| rx.try_recv()) {
            match result {
                Ok((pages, samples)) => {
                    self.pending = None;
                    let limit = device.limits().max_buffer_size;
                    if pages.iter().any(|page| {
                        (page.mesh.vertices.len() as u64)
                            .saturating_mul(std::mem::size_of::<Vertex3>() as u64)
                            > limit
                            || (page.mesh.indices.len() as u64).saturating_mul(std::mem::size_of::<
                                u32,
                            >(
                            )
                                as u64)
                                > limit
                    }) {
                        eprintln!(
                            "[horizon] invalid upload page: device_limit={limit}; retry scheduled"
                        );
                        self.samples = samples;
                        self.fail_active(now);
                    } else if let Some(target) = &self.active {
                        self.upload =
                            Some(upload::Upload::new(pages, samples, Some(target.center)));
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    eprintln!("[horizon] worker ended without pages; retry scheduled");
                    self.fail_active(now);
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self
            .upload
            .as_mut()
            .is_some_and(|upload| upload.advance(device, queue))
        {
            let (mesh, samples, center) = self.upload.take().unwrap().finish();
            if let Some(target) = self.active.take() {
                debug_assert_eq!(center, Some(target.center));
                // Publish geometry, cache and its source metadata together.
                self.mesh = Some(mesh);
                self.samples = samples;
                self.published = Some(target);
                self.failed = None;
            }
        }
        if self.pending.is_some() || self.upload.is_some() || self.worker.is_some() {
            return;
        }
        let Some(target) = self.desired.as_ref() else {
            return;
        };
        if self.mesh.is_some()
            && self
                .published
                .as_ref()
                .is_some_and(|ready| ready.matches(target))
        {
            return;
        }
        if self.failed.as_ref().is_some_and(|(failed, at)| {
            failed.matches(target) && now.saturating_duration_since(*at) < RETRY_INTERVAL
        }) {
            return;
        }
        if self
            .published
            .as_ref()
            .is_some_and(|ready| ready.center == target.center)
            && self
                .last_build
                .is_some_and(|last| now.saturating_duration_since(last) < REBUILD_INTERVAL)
        {
            return;
        }
        let target = target.clone();
        self.last_build = Some(now);
        self.failed = None;
        self.active = Some(target.clone());
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.pending = Some(rx);
        // This is an individual-buffer limit, not a bound on all mesh memory.
        let page_limit = device.limits().max_buffer_size.min(64 * 1024 * 1024);
        let samples = std::mem::take(&mut self.samples);
        match std::thread::Builder::new().name("horizon".into()).spawn(move || {
            let start = Instant::now();
            let center = target.center;
            let Some((mut mesh, samples)) = build(cfg, center, &cancel, samples) else { return; };
            if cancel.load(Ordering::Relaxed)
                || edits::apply(
                    &mut mesh, &target.snapshot, center, target.eye,
                    target.hidden_radius, &cancel,
                ).is_none()
                || cancel.load(Ordering::Relaxed)
            {
                return;
            }
            let batches = batches::partition(&mut mesh, cfg.voxel_meters(), center);
            if cancel.load(Ordering::Relaxed) { return; }
            println!(
                "[horizon] center={center:?} triangles={} vertex_bytes={} index_bytes={} build_ms={:.0} spatial_batches={}",
                mesh.indices.len() / 3,
                mesh.vertices.len() * std::mem::size_of::<Vertex3>(),
                mesh.indices.len() * std::mem::size_of::<u32>(),
                start.elapsed().as_secs_f64() * 1000.0,
                batches.len(),
            );
            let Some(pages) = pages::pack(mesh, batches, page_limit, &cancel) else { return; };
            if cancel.load(Ordering::Relaxed) { return; }
            println!(
                "[horizon] upload_pages={} page_limit_bytes={} packed_vertex_bytes={} packed_index_bytes={} max_page_vertex_bytes={} max_page_index_bytes={}",
                pages.len(), page_limit,
                pages.iter().map(|p| p.mesh.vertices.len() * std::mem::size_of::<Vertex3>()).sum::<usize>(),
                pages.iter().map(|p| p.mesh.indices.len() * std::mem::size_of::<u32>()).sum::<usize>(),
                pages.iter().map(|p| p.mesh.vertices.len() * std::mem::size_of::<Vertex3>()).max().unwrap_or(0),
                pages.iter().map(|p| p.mesh.indices.len() * std::mem::size_of::<u32>()).max().unwrap_or(0),
            );
            let _ = tx.send((pages, samples));
        }) {
            Ok(worker) => self.worker = Some(worker),
            Err(error) => {
                eprintln!("[horizon] could not start worker: {error}; retry scheduled");
                self.fail_active(now);
            }
        }
    }
}

/// Checked integer snapping prevents multiplication overflow at invalid positions.
fn snapped_center(eye: Vec3) -> Option<(i32, i32)> {
    if !eye.is_finite() {
        return None;
    }
    let x = ((eye.x / CENTER_GRID as f32).round() as i32).checked_mul(CENTER_GRID)?;
    let z = ((eye.z / CENTER_GRID as f32).round() as i32).checked_mul(CENTER_GRID)?;
    Some((x, z))
}

/// Shared with tree placement: height interpolation uses this parent diagonal.
fn diagonal_a_to_d(a: f32, b: f32, c: f32, d: f32) -> bool {
    (a - d).abs() <= (b - c).abs()
}

/// Validate the actual f32 lattice before sampling. Integer metre anchors avoid
/// integer overflow; checking the final positions catches float precision loss
/// and sample-coordinate saturation instead of emitting collapsed triangles.
fn ring_axis(center: i32, level: usize, scale: f32) -> Option<[(f32, i32); SIDE]> {
    let mut axis = [(0.0, 0); SIDE];
    let step = 4i64 << level;
    let min_spacing = ring_step(level) / scale * 0.5;
    for index in 0..SIDE {
        let metres = i64::from(center) + (index as i64 - HALF as i64) * step;
        let position = metres as f32 / scale;
        let cell = f64::from(position).floor();
        if !position.is_finite()
            || cell < f64::from(i32::MIN)
            || cell > f64::from(i32::MAX)
            || (index > 0
                && (position <= axis[index - 1].0 || position - axis[index - 1].0 < min_spacing))
        {
            return None;
        }
        axis[index] = (position, cell as i32);
    }
    Some(axis)
}

/// Return the triangles for a heightfield cell, optionally splitting one of
/// its boundary edges through an existing fine-ring midpoint. A split keeps
/// the parent diagonal and triangle winding. Its midpoint has already been
/// placed on the parent edge, so height interpolation is unchanged for sibling
/// water and vegetation code that still evaluates the original two triangles.
fn horizon_cell_triangles(
    corners: [u32; 4],
    heights: [f32; 4],
    split_edge: Option<[u32; 3]>,
) -> ([[u32; 3]; 3], usize) {
    let [a, b, c, d] = corners;
    let parent = if diagonal_a_to_d(heights[0], heights[1], heights[2], heights[3]) {
        [[a, c, d], [a, d, b]]
    } else {
        [[a, c, b], [b, c, d]]
    };
    let mut result = [[0; 3]; 3];
    let mut count = 0;
    for triangle in parent {
        let edge = split_edge.and_then(|[first, last, midpoint]| {
            (0..3).find_map(|i| {
                let u = triangle[i];
                let v = triangle[(i + 1) % 3];
                ((u == first && v == last) || (u == last && v == first)).then_some((
                    u,
                    v,
                    triangle[(i + 2) % 3],
                    midpoint,
                ))
            })
        });
        if let Some((u, v, opposite, midpoint)) = edge {
            result[count] = [u, midpoint, opposite];
            result[count + 1] = [midpoint, v, opposite];
            count += 2;
        } else {
            result[count] = triangle;
            count += 1;
        }
    }
    debug_assert_eq!(count, if split_edge.is_some() { 3 } else { 2 });
    (result, count)
}

/// The ring layout is a 2:1 grid with a square hole. Only the four cells strips
/// bordering that hole need transition triangles. The exterior corner cells
/// touch the fine ring at one vertex, so no cell has two split boundary edges.
fn horizon_inner_split(
    level: usize,
    x: usize,
    z: usize,
    corners: [u32; 4],
    owner: &[u32],
) -> Option<[u32; 3]> {
    if level == 0 {
        return None;
    }
    let lo = HALF / 2;
    let hi = HALF * 3 / 2;
    let previous = (level - 1) * SIDE * SIDE;
    let [a, b, c, d] = corners;
    if (lo..hi).contains(&x) {
        let fine_x = 2 * x - HALF + 1;
        if z + 1 == lo {
            return Some([c, d, owner[previous + fine_x]]);
        }
        if z == hi {
            return Some([a, b, owner[previous + (SIDE - 1) * SIDE + fine_x]]);
        }
    }
    if (lo..hi).contains(&z) {
        let fine_z = 2 * z - HALF + 1;
        if x + 1 == lo {
            return Some([b, d, owner[previous + fine_z * SIDE]]);
        }
        if x == hi {
            return Some([a, c, owner[previous + fine_z * SIDE + SIDE - 1]]);
        }
    }
    None
}

fn build(
    cfg: WorldSampleConfig,
    center: (i32, i32),
    cancel: &AtomicBool,
    mut old: Samples,
) -> Option<(ChunkMesh, Samples)> {
    // Sibling appenders index these SIDE*SIDE grids directly. Keep every grid,
    // including the inactive inner vertices, contiguous and in the old order.
    debug_assert_eq!(SIDE, HALF * 2 + 1);
    debug_assert_eq!(HALF % 2, 0);
    let mut mesh = ChunkMesh::empty();
    let vertex_count = LEVELS * SIDE * SIDE;
    mesh.vertices.reserve(vertex_count);
    mesh.indices.reserve(LEVELS * (SIDE - 1) * (SIDE - 1) * 6);
    let mut samples = Samples::new();
    let mut owner: Vec<u32> = Vec::with_capacity(vertex_count);
    let scale = cfg.voxel_meters();
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }

    // Cold horizon builds used to sample every unique knot on the render
    // worker. Gather the exact same requests first, reuse the previous build's
    // samples, then evaluate the misses in parallel. The mesh itself is still
    // assembled serially in the original owner order.
    let mut requests: HashMap<(i32, i32), (u8, f32)> = HashMap::new();
    for level in 0..LEVELS {
        let step = ring_step(level);
        let x_axis = ring_axis(center.0, level, scale)?;
        let z_axis = ring_axis(center.1, level, scale)?;
        let lo = HALF / 2;
        let hi = HALF * 3 / 2;
        for z in 0..SIDE {
            for x in 0..SIDE {
                if level > 0 && (lo..=hi).contains(&x) && (lo..=hi).contains(&z) {
                    continue;
                }
                let (_, bx) = x_axis[x];
                let (_, bz) = z_axis[z];
                let detail = sample_detail(level);
                requests
                    .entry((bx, bz))
                    .and_modify(|entry| {
                        // Finer detail always wins, matching the original
                        // level-ascending first-writer rule.
                        if detail < entry.0 {
                            *entry = (detail, step);
                        }
                    })
                    .or_insert((detail, step));
            }
        }
    }
    let mut work = Vec::new();
    for (&(bx, bz), &(detail, step)) in &requests {
        if let Some(sample) = old.remove(&(bx, bz)).filter(|sample| sample.2 == detail) {
            samples.insert((bx, bz), sample);
        } else {
            work.push(((bx, bz), detail, step));
        }
    }
    // Erosion tiles are 128 m and the worker cache holds 1024 of them. HashMap
    // order scatters neighbours across the whole ~50 km lattice, so each tile
    // was rebuilt many times. Visiting in spatial order reuses each tile.
    work.sort_unstable_by_key(|&((bx, bz), _, _)| (bz.div_euclid(128), bx.div_euclid(128), bz, bx));
    if !work.is_empty() && std::env::var_os("VOXEL_HORIZON_SERIAL").is_none() {
        let workers = std::thread::available_parallelism()
            .map(|count| count.get().min(8))
            .unwrap_or(1)
            .max(1);
        let chunk = work.len().div_ceil(workers).max(1);
        let mut output: Vec<Vec<((i32, i32), (f32, crate::world::BlockId, u8))>> =
            Vec::with_capacity(workers);
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for part in work.chunks(chunk) {
                handles.push(scope.spawn(move || {
                    if cancel.load(Ordering::Relaxed) {
                        return Vec::new();
                    }
                    part.iter()
                        .map(|&((bx, bz), detail, step)| {
                            let (height, block) = if detail == 0 {
                                distant_surface(&cfg, bx, bz)
                            } else {
                                crate::world::landscape::preview_surface_lod(&cfg, bx, bz, step)
                            };
                            ((bx, bz), (height, block, detail))
                        })
                        .collect::<Vec<_>>()
                }));
            }
            for handle in handles {
                output.push(handle.join().unwrap_or_default());
            }
        });
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        for part in output {
            for (key, sample) in part {
                samples.insert(key, sample);
            }
        }
    } else {
        for ((bx, bz), detail, step) in work {
            let (height, block) = if detail == 0 {
                distant_surface(&cfg, bx, bz)
            } else {
                crate::world::landscape::preview_surface_lod(&cfg, bx, bz, step)
            };
            samples.insert((bx, bz), (height, block, detail));
        }
    }

    // Phase 1: establish one vertex owner for every shared lattice knot. Copy
    // the complete source vertex, including its material key; copying height
    // alone leaves a visible color/roughness seam at the same geometric edge.
    for level in 0..LEVELS {
        let x_axis = ring_axis(center.0, level, scale)?;
        let z_axis = ring_axis(center.1, level, scale)?;
        let lo = HALF / 2;
        let hi = HALF * 3 / 2;
        for z in 0..SIDE {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            for x in 0..SIDE {
                if level > 0 && (lo..=hi).contains(&x) && (lo..=hi).contains(&z) {
                    let fine_x = 2 * x - HALF;
                    let fine_z = 2 * z - HALF;
                    let previous = (level - 1) * SIDE * SIDE + fine_z * SIDE + fine_x;
                    let source = owner[previous];
                    mesh.vertices.push(mesh.vertices[source as usize]);
                    owner.push(source);
                    continue;
                }
                let (vx, bx) = x_axis[x];
                let (vz, bz) = z_axis[z];
                // The prepass already resolved the finest sample for this knot.
                let Some(&(height, block, _)) = samples.get(&(bx, bz)) else {
                    return None;
                };
                if !height.is_finite() {
                    return None;
                }
                let material = block.surface();
                owner.push(mesh.vertices.len() as u32);
                mesh.vertices.push(Vertex3 {
                    pos: [vx, height, vz],
                    color: block.color(IVec3::new(bx, height as i32, bz)),
                    normal: [0.0; 3],
                    surface: [
                        material.roughness,
                        material.specular,
                        0.0,
                        material.shader_key(),
                    ],
                    ao_strength: -1.0,
                });
            }
        }
    }

    // Phase 2: place the odd fine boundary vertices on the parent edge before
    // selecting triangles or computing normals. The original builder changed
    // these heights after normal accumulation. Keep the odd vertex's material
    // identity: the coarse transition triangle now uses this same vertex.
    for level in 0..LEVELS - 1 {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let base = level * SIDE * SIDE;
        let edge = SIDE - 1;
        for i in (1..edge).step_by(2) {
            for (at, first, last) in [
                (i, i - 1, i + 1),
                (edge * SIDE + i, edge * SIDE + i - 1, edge * SIDE + i + 1),
                (i * SIDE, (i - 1) * SIDE, (i + 1) * SIDE),
                (
                    i * SIDE + edge,
                    (i - 1) * SIDE + edge,
                    (i + 1) * SIDE + edge,
                ),
            ] {
                let first = owner[base + first] as usize;
                let last = owner[base + last] as usize;
                let at = owner[base + at] as usize;
                mesh.vertices[at].pos[1] =
                    mesh.vertices[first].pos[1] * 0.5 + mesh.vertices[last].pos[1] * 0.5;
            }
        }
    }

    // Phase 3: use canonical indices on both sides of every interface and split
    // the coarse edge explicitly. Collinear T-junctions are not relied upon to
    // rasterize watertight, and no averaged float is used as a material ID.
    for level in 0..LEVELS {
        let step = ring_step(level);
        let base = level * SIDE * SIDE;
        for z in 0..SIDE - 1 {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            for x in 0..SIDE - 1 {
                if level > 0
                    && (HALF / 2..HALF * 3 / 2).contains(&x)
                    && (HALF / 2..HALF * 3 / 2).contains(&z)
                {
                    continue;
                }
                // Keep every cell whose horizontal bounds intersect the far
                // radius, with one cell of margin. The old 17 km cutoff removed
                // otherwise visible terrain before the 24,576 m far plane.
                let closest_x = ((x as f32 - HALF as f32 + 0.5).abs() * step - step * 0.5).max(0.0);
                let closest_z = ((z as f32 - HALF as f32 + 0.5).abs() * step - step * 0.5).max(0.0);
                let limit = FAR_PLANE + step;
                if closest_x * closest_x + closest_z * closest_z > limit * limit {
                    continue;
                }
                let a = owner[base + z * SIDE + x];
                let b = owner[base + z * SIDE + x + 1];
                let c = owner[base + (z + 1) * SIDE + x];
                let d = owner[base + (z + 1) * SIDE + x + 1];
                let corners = [a, b, c, d];
                let heights = corners.map(|index| mesh.vertices[index as usize].pos[1]);
                let split = horizon_inner_split(level, x, z, corners, &owner);
                let (triangles, count) = horizon_cell_triangles(corners, heights, split);
                for triangle in &triangles[..count] {
                    let positions =
                        triangle.map(|index| Vec3::from_array(mesh.vertices[index as usize].pos));
                    // Accumulate in metres so the cross product does not shrink
                    // or grow quadratically with the configured voxel scale.
                    let edge1 = (positions[1] - positions[0]) * scale;
                    let edge2 = (positions[2] - positions[0]) * scale;
                    let normal = edge1.cross(edge2);
                    if !normal.is_finite() || normal.y <= 0.0 {
                        return None;
                    }
                    for &index in triangle {
                        let vertex = &mut mesh.vertices[index as usize];
                        vertex.normal = (Vec3::from_array(vertex.normal) + normal).to_array();
                    }
                    mesh.indices.extend_from_slice(triangle);
                }
            }
        }
    }

    // Phase 4: each face contributed exactly once to its canonical owners.
    // Owners precede their aliases, so normalize once and then copy the result
    // to duplicate grid vertices needed by the water/vegetation appenders.
    for index in 0..vertex_count {
        if index % 1024 == 0 && cancel.load(Ordering::Relaxed) {
            return None;
        }
        let source = owner[index] as usize;
        mesh.vertices[index].normal = if source == index {
            Vec3::from_array(mesh.vertices[index].normal)
                .try_normalize()
                .unwrap_or(Vec3::Y)
                .to_array()
        } else {
            mesh.vertices[source].normal
        };
    }
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    water::append(&mut mesh, &cfg);
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    vegetation::append(&mut mesh, &cfg, center);
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    Some((mesh, samples))
}

#[cfg(test)]
mod profile {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::WorldType;
    use std::time::Instant;

    #[test]
    #[ignore]
    fn profile_cold_horizon_build() {
        let cfg = WorldSampleConfig {
            generator_version: 18,
            seed: 2_738_958_700,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let cancel = AtomicBool::new(false);
        let start = Instant::now();
        let built = build(cfg, (0, 0), &cancel, Samples::new());
        let elapsed = start.elapsed();
        assert!(built.is_some());
        let mesh = built.unwrap().0;
        println!(
            "cold horizon build: {elapsed:?}, vertices={}, indices={}",
            mesh.vertices.len(),
            mesh.indices.len()
        );
    }
}

fn proxy_box(
    mesh: &mut ChunkMesh,
    center: Vec3,
    size: Vec3,
    scale: f32,
    color: [f32; 3],
    kind: f32,
) {
    let h = size * 0.5;
    let corners = [
        Vec3::new(-h.x, -h.y, -h.z),
        Vec3::new(h.x, -h.y, -h.z),
        Vec3::new(h.x, h.y, -h.z),
        Vec3::new(-h.x, h.y, -h.z),
        Vec3::new(-h.x, -h.y, h.z),
        Vec3::new(h.x, -h.y, h.z),
        Vec3::new(h.x, h.y, h.z),
        Vec3::new(-h.x, h.y, h.z),
    ];
    for (ids, normal) in [
        ([4, 5, 6, 7], Vec3::Z),
        ([1, 0, 3, 2], -Vec3::Z),
        ([7, 6, 2, 3], Vec3::Y),
        ([0, 1, 5, 4], -Vec3::Y),
        ([5, 1, 2, 6], Vec3::X),
        ([0, 4, 7, 3], -Vec3::X),
    ] {
        let base = mesh.vertices.len() as u32;
        for id in ids {
            mesh.vertices.push(Vertex3 {
                pos: ((center + corners[id]) / scale).to_array(),
                normal: normal.to_array(),
                color,
                surface: [0.95, 0.02, kind, 0.0],
                ao_strength: -1.0,
            });
        }
        mesh.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

#[cfg(test)]
mod sample_speed {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::{World, WorldType};

    /// `cargo test --release sample_speed -- --ignored --nocapture`
    /// Cold horizon build time, the cost a player sees at startup.
    #[test]
    #[ignore]
    fn cold_horizon_build() {
        let cfg = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 7).sample_config();
        let cancel = AtomicBool::new(false);
        let start = Instant::now();
        let (mesh, _) = build(cfg, (0, 0), &cancel, Samples::new()).unwrap();
        println!(
            "cold horizon build {:.0} ms, {} vertices",
            start.elapsed().as_secs_f64() * 1e3,
            mesh.vertices.len()
        );
    }
}
