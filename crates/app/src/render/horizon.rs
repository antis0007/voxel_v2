//! Asynchronous backdrop with stitched rings and authoritative edited surfaces.
#[cfg(test)]
mod collar_contract;
mod quality;
pub(crate) mod collar;
pub(crate) mod collar_set;
mod batches;
pub mod clip;
mod edits;
mod pages;
mod reference;
mod upload;
pub(super) mod vegetation;
mod structures;
mod refinement;
mod water;
mod fine;
mod columns;
pub(super) mod parallel;
pub(crate) mod detail;
use crate::mesh::{ChunkMesh, Vertex3};
use crate::world::{WorldSampleConfig, distant_surface};
use glam::{IVec3, Vec3};
use super::coverage::ResidentCoverage;
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const FAR_PLANE: f32 = 24576.0;
/// Smallest distant range the settings offer. Below the ordinary view radius
/// there is nothing left for the horizon to cover.
pub const MIN_HORIZON_RANGE_M: u32 = 2048;
static HORIZON_RANGE_M: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(4096);

/// Distance in metres out to which the distant terrain mesh is built. Every
/// sample and triangle beyond it is skipped, so cost falls with its square.
pub fn horizon_range_m() -> u32 {
    HORIZON_RANGE_M.load(Ordering::Relaxed).clamp(MIN_HORIZON_RANGE_M, FAR_PLANE as u32)
}
pub fn set_horizon_range_m(metres: u32) {
    HORIZON_RANGE_M.store(metres.clamp(MIN_HORIZON_RANGE_M, FAR_PLANE as u32), Ordering::Relaxed);
}
/// Reach of the built domain: the range plus slack for a retained mesh whose
/// centre lags the camera. The slack shrinks with the range, so short ranges
/// are not padded by the full 5.8 km that the 24 km view needs.
fn domain_reach(range_m: u32) -> f32 {
    let range = range_m.clamp(MIN_HORIZON_RANGE_M, FAR_PLANE as u32) as f32;
    let slack = MAX_CENTER_DISTANCE * std::f32::consts::SQRT_2 * (range / FAR_PLANE);
    range + slack.clamp(1024.0, MAX_CENTER_DISTANCE * std::f32::consts::SQRT_2)
}
// The outer guard ring covers the far-plane disk even while the retained mesh
// is up to MAX_CENTER_DISTANCE away from the camera. Its unused knots are not
// evaluated. Previously the grid stopped at the far plane around its OLD center.
const LEVELS: usize = 8;
/// Keep the complete grids contiguous: water and vegetation index this lattice.
const SIDE: usize = 193;
const MAX_REFINEMENT_SAMPLES: usize = 16_384;
/// Midpoint budget for the cheap far rings (6-24 km), separate from the exact one.
/// Each costs one ~4 us sample, one vertex and two triangles (~80 bytes).
const MAX_FAR_REFINEMENT_SAMPLES: usize = 49_152;
const HALF: usize = (SIDE - 1) / 2;
const CENTER_GRID: i32 = 512;
// Reserve enough real source-domain geometry for an older-centre result while
// delayed build/upload work catches up. The 2km guard caused three loading
// breaks on the measured 180m/s pressure route. All clipping/sampling margins
// derive from this same bound; terrain is never translated to the new centre.
const MAX_CENTER_DISTANCE: f32 = 4096.0;
const SOURCE_REACH: f32 = FAR_PLANE + MAX_CENTER_DISTANCE * std::f32::consts::SQRT_2 + 512.0;
const CHECK_INTERVAL: Duration = Duration::from_millis(500);
const REBUILD_INTERVAL: Duration = Duration::from_secs(2);
const RETRY_INTERVAL: Duration = Duration::from_secs(2);
/// Upper bound on the wait after repeated failures of the same target.
const MAX_RETRY_INTERVAL: Duration = Duration::from_secs(30);

/// Wait before retrying a target that has failed `streak` times in a row:
/// 2 s, 4 s, 8 s, 16 s, then 30 s. A deterministic failure, such as a base over
/// the publication budget, would otherwise rebuild every 2 s for the whole session.
fn retry_wait(streak: u32) -> Duration {
    let doublings = streak.saturating_sub(1).min(4);
    (RETRY_INTERVAL * 2u32.pow(doublings)).min(MAX_RETRY_INTERVAL)
}
// Whole-mesh publication reserves one old GPU cover plus one replacement. This
// bound includes edits and foliage, separately from the sample cache and the
// resident chunk stream. No second completed build can queue behind an upload.
// Raised from 160 MiB for the fine rings (about 56 MB more terrain at the audit
// vista), so vegetation keeps every tree beside them.
// Procedural geometry retains its original representation/admission. Enlarged
// publication capacity belongs only to exact replacements and packing overhead.
const MAX_MESH_BYTES: usize = budget::BASE_LIMIT;

fn mesh_bytes(mesh: &ChunkMesh) -> usize {
    mesh.vertices.len() * std::mem::size_of::<Vertex3>() + mesh.indices.len() * 4
}

// World geometry stays at the authoritative elevation. Lowering the entire
// landscape also lowered dry refinement knots below the sea and flooded their mask.
const HORIZON_SINK_VOXELS: f32 = 0.0;

/// Sample spacing in metres. All appenders must use the same ring lattice.
fn ring_step(level: usize) -> f32 {
    (4u32 << level) as f32
}

/// Exact summaries through 64 m (~6 km). Beyond that, one fixed 8 m cheap point
/// evaluator preserves macro detail independent of the requested ring spacing.
/// Generator 38 filters the separate residual layer at 64 m for this policy;
/// metre-scale relief remains in the surface shader, not aliased mesh peaks.
/// Policy 1 is deliberately distinct from exact policy 0: neither is an area
/// average, and a point from one policy cannot satisfy the other's query.
fn sample_detail(level: usize) -> u8 {
    if canonical_source() {
        u8::from(level > 4)
    } else {
        level.saturating_sub(3) as u8
    }
}

fn canonical_source() -> bool {
    static CANONICAL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CANONICAL.get_or_init(|| std::env::var("VOXEL_HORIZON_SOURCE").as_deref() != Ok("preview"))
}

fn sample_in_domain(dx: f32, dz: f32, step: f32, reach: f32) -> bool {
    dx.hypot(dz) <= reach + step * 3.0
}

fn cell_in_domain(x:usize,z:usize,step:f32,reach:f32)->bool {
    let closest_x=((x as f32-HALF as f32+0.5).abs()*step-step*0.5).max(0.0);
    let closest_z=((z as f32-HALF as f32+0.5).abs()*step-step*0.5).max(0.0);
    let limit=reach+step;
    closest_x*closest_x+closest_z*closest_z<=limit*limit
}

type Samples = HashMap<(i32, i32), (f32, crate::world::BlockId, u8)>;

#[derive(Clone)]
struct BaseCover {
    startup: bool,
    clip: Option<clip::ChartClip>,
    quality:Arc<Vec<quality::Candidate>>,
    refinement_eye: Vec3,
    projection_pixels: f32,
    // Extent the mesh was built to: a base built for another range is not reusable.
    range_m: u32,
    center: (i32, i32),
    mesh: Arc<ChunkMesh>,
    samples: Arc<Samples>,
    trees: Arc<Vec<vegetation::TreeSource>>,
    tree_samples: Arc<vegetation::TreeSamples>,
    // Per-pixel distant-terrain detail for this centre. Built with the base,
    // so edit rebuilds reuse it.
    detail: Option<Arc<detail::DetailMap>>,
    // Midpoints this base meshed; the next base keeps them at the lower retire bar.
    refined: Arc<HashSet<(i32, i32)>>,
}

mod budget;

type BaseKey = (bool, (i32,i32), u32, Option<clip::ChartClip>, u32, u32);
impl BaseCover {
    fn key(&self) -> BaseKey {
        (self.startup,self.center,self.range_m,self.clip,self.projection_pixels.to_bits(),self.refinement_eye.y.to_bits())
    }
}
fn matching_base_capacity(key:BaseKey,candidates:impl Iterator<Item=(BaseKey,usize)>)->usize {
    candidates.filter(|(candidate,_)|*candidate==key).map(|(_,bytes)|bytes).max().unwrap_or(0)
}
fn mesh_capacity_bytes(mesh: &ChunkMesh) -> usize {
    mesh.vertices.capacity().saturating_mul(std::mem::size_of::<Vertex3>())
        .saturating_add(mesh.indices.capacity().saturating_mul(std::mem::size_of::<u32>()))
}

enum BuildEvent {
    BudgetExhausted,
    PublicationPressure(budget::Denied),
    Base(BaseCover),
    Complete(Vec<pages::Page>, Arc<Samples>, edits::Omitted, usize),
}
pub struct GpuMesh {
    pub collared:bool,
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
    startup: bool,
    collar:Option<Arc<collar_set::Set>>,
    refinement_eye_y: f32,
    projection_pixels: f32,
    range_m: u32,
    center: (i32, i32),
    request_generation: u64,
    source_revision: u64,
    publication_limit: usize,
    fingerprint: u64,
    snapshot: crate::world::WorldSnapshot,
    eye: Vec3,
    hidden_radius: f32,
    omitted: Arc<edits::Omitted>,
    coverage: Arc<ResidentCoverage>,
    /// Globe face rect the fine rings stay inside.
    clip: Option<clip::ChartClip>,
    validation: std::cell::RefCell<[Option<ValidationMemo>;2]>,
}

#[derive(Clone)]
struct ValidationMemo {
    snapshot: crate::world::WorldSnapshot,
    eye: [u32;3],
    hidden: u32,
    coverage: Arc<ResidentCoverage>,
    valid: bool,
}

fn same_collar(a:Option<&Arc<collar_set::Set>>,b:Option<&Arc<collar_set::Set>>)->bool {
    collar_set::same(a,b)
}

impl BuildTarget {
    fn base_key(&self) -> BaseKey {
        (self.startup,self.center,self.range_m,self.clip,self.projection_pixels.to_bits(),self.refinement_eye_y.to_bits())
    }
    fn baked_column(&self, (x, z): (i32, i32)) -> bool {
        let scale = self.snapshot.cfg.voxel_meters();
        let center = Vec3::new(self.center.0 as f32, 0.0, self.center.1 as f32);
        let lo = Vec3::new(x as f32 * 32.0, 0.0, z as f32 * 32.0);
        let hi = lo + Vec3::new(32.0, 0.0, 32.0);
        // Use the guaranteed inner footprint, not the speculative guard.
        let far = ((lo.x * scale - center.x).abs().max((hi.x * scale - center.x).abs()))
            .hypot((lo.z * scale - center.z).abs().max((hi.z * scale - center.z).abs()));
        far <= self.range_m as f32
            && !self.omitted.iter().any(|&(a, b)| a.x < hi.x && b.x > lo.x && a.z < hi.z && b.z > lo.z)
    }

    #[cfg(test)]
    fn valid_for(&self, snapshot: &crate::world::WorldSnapshot) -> bool {
        self.valid_for_coverage(snapshot, self.eye, self.hidden_radius)
    }
    #[cfg(test)]
    fn valid_for_coverage(
        &self,
        snapshot: &crate::world::WorldSnapshot,
        eye: Vec3,
        hidden: f32,
    ) -> bool {
        self.valid_for_resident(snapshot, eye, hidden, &ResidentCoverage::default())
    }
    fn valid_for_resident(&self, snapshot: &crate::world::WorldSnapshot, eye: Vec3, hidden: f32, coverage: &ResidentCoverage) -> bool {
        let eye_bits=eye.to_array().map(f32::to_bits);
        if let Some(cached)=&self.validation.borrow()[0]
            && cached.eye==eye_bits && cached.hidden==hidden.to_bits()
                && cached.coverage.as_ref()==coverage
                && cached.snapshot.cfg==snapshot.cfg
                && Arc::ptr_eq(&cached.snapshot.chunks,&snapshot.chunks)
                && Arc::ptr_eq(&cached.snapshot.water_levels,&snapshot.water_levels)
                && Arc::ptr_eq(&cached.snapshot.regions.root,&snapshot.regions.root)
                && Arc::ptr_eq(&cached.snapshot.regions.records,&snapshot.regions.records) { return cached.valid; }
        let valid=self.snapshot.cfg == snapshot.cfg
            && Arc::ptr_eq(&self.snapshot.regions.root, &snapshot.regions.root)
            && !edits::changes_visible_resident(&self.snapshot, snapshot, eye, hidden, coverage)
            && edits::scope_covers_resident(&self.omitted, snapshot, eye, hidden, coverage);
        // Holding these immutable Arcs also prevents allocator-address reuse
        // from turning an old memo key into a false match.
        self.validation.borrow_mut()[0]=Some(ValidationMemo {snapshot:snapshot.clone(),eye:eye_bits,hidden:hidden.to_bits(),coverage:Arc::new(*coverage),valid});
        valid
    }
    fn matches(&self, other: &Self) -> bool {
        same_collar(self.collar.as_ref(), other.collar.as_ref()) && self.matches_without_collar(other)
    }

    fn same_scene(&self, other: &Self) -> bool {
        same_collar(self.collar.as_ref(), other.collar.as_ref()) && self.same_scene_without_collar(other)
    }
    fn matches_without_collar(&self, other: &Self) -> bool {
        self.publication_limit == other.publication_limit && self.same_scene_without_collar(other)
    }
    fn same_scene_without_collar(&self, other: &Self) -> bool {
        if self.startup != other.startup || self.range_m != other.range_m || self.clip != other.clip || self.center != other.center || self.projection_pixels.to_bits() != other.projection_pixels.to_bits() || self.refinement_eye_y.to_bits() != other.refinement_eye_y.to_bits() { return false; }
        // Desired targets are immutable between coalesced checks. Cache the
        // whole comparison, including the structural scan, across those frames.
        let eye_bits=other.eye.to_array().map(f32::to_bits);
        if let Some(cached)=&self.validation.borrow()[1]
            && cached.eye==eye_bits && cached.hidden==other.hidden_radius.to_bits()
                && cached.coverage==other.coverage
                && cached.snapshot.cfg==other.snapshot.cfg
                && Arc::ptr_eq(&cached.snapshot.chunks,&other.snapshot.chunks)
                && Arc::ptr_eq(&cached.snapshot.water_levels,&other.snapshot.water_levels)
                && Arc::ptr_eq(&cached.snapshot.regions.root,&other.snapshot.regions.root)
                && Arc::ptr_eq(&cached.snapshot.regions.records,&other.snapshot.regions.records) { return cached.valid; }
        let scale=self.snapshot.cfg.voxel_meters();
        let valid = !edits::structural_changed(&self.snapshot,&other.snapshot,self.center,&other.coverage)
            && !self.omitted.iter().any(|&(lo,hi)| self.coverage.contains_bounds(lo*scale,hi*scale,scale)
                && !other.coverage.contains_bounds(lo*scale,hi*scale,scale))
            && self.valid_for_resident(&other.snapshot, other.eye, other.hidden_radius, &other.coverage);
        self.validation.borrow_mut()[1]=Some(ValidationMemo {snapshot:other.snapshot.clone(),eye:eye_bits,
            hidden:other.hidden_radius.to_bits(),coverage:other.coverage.clone(),valid});
        valid
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
    current_collar:Option<Arc<collar_set::Set>>,
    collar_changed_at: Option<Instant>,
    collar_observation:Option<(u64,WorldSampleConfig,Arc<std::path::PathBuf>,Arc<ResidentCoverage>)>,
    quality_selector:quality::Selector,
    device_lost:Arc<AtomicBool>,
    refinement_eye_y: Option<f32>,
    refinement_projection_pixels: f32,
    request_generation: u64,
    config: Option<WorldSampleConfig>,
    source_root: Option<Arc<std::path::PathBuf>>,
    // Observation state may advance while a worker is busy. It is never used
    // as evidence that a replacement has reached the GPU.
    checked_revision: Option<u64>,
    checked_eye: Option<IVec3>,
    checked_radius: f32,
    checked_range: u32,
    last_check: Option<Instant>,
    last_build: Option<Instant>,
    desired: Option<BuildTarget>,
    active: Option<BuildTarget>,
    published: Option<BuildTarget>,
    failed: Option<(BuildTarget, Instant)>,
    budget_exhausted: Option<BuildTarget>,
    budget_exhausted_since: Option<Instant>,
    budget_pressure: bool,
    // Consecutive failures of one target, used by `retry_wait`.
    failure_streak: Option<(BuildTarget, u32)>,
    pending: Option<mpsc::Receiver<BuildEvent>>,
    // Keep one previous center for reversals. Geometry and sampling are
    // independent of the mutable edit snapshot.
    base: Option<BaseCover>,
    previous_base: Option<BaseCover>,
    building_base: bool,
    upload: Option<upload::Upload>,
    upload_limit: usize,
    // A canceled worker is retained until it exits. Repeated teleports cannot
    // create an unbounded set of detached builders, and update never joins live work.
    worker: Option<JoinHandle<()>>,
    samples: Arc<Samples>,
    cancel: Arc<AtomicBool>,
    pub mesh: Option<GpuMesh>,
    outgoing: Option<(GpuMesh, BuildTarget, Instant)>,
    /// Set once this world has published a mesh; later rebuilds keep the
    /// loader off while the far field is replaced.
    loaded_this_world: bool,
    rejected_stale: u64,
    exclusion_radius: f32,
    resident: Arc<ResidentCoverage>,
    desired_resident: Arc<ResidentCoverage>,
    shadow_generation: u64,
    // Set when published geometry changes centre: the detail map to upload,
    // or `None` to switch the shader's map off.
    detail_update: Option<Option<Arc<detail::DetailMap>>>,
    detail_center: Option<(i32, i32)>,
    chart_clip: Option<clip::ChartClip>,
}

impl Drop for Horizon {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        // Dropping the handle detaches; never wait for a helper on the render thread.
    }
}

impl Horizon {
    pub fn collar_observation_current(&self,revision:u64,cfg:WorldSampleConfig,
        root:&Arc<std::path::PathBuf>,coverage:&ResidentCoverage)->bool {
        self.collar_observation.as_ref().is_some_and(|(r,c,p,v)|
            *r==revision && *c==cfg && Arc::ptr_eq(p,root) && v.as_ref()==coverage)
    }
    pub fn current_collar(&self)->Option<&Arc<collar_set::Set>> {self.current_collar.as_ref()}

    pub(super) fn collar_settled(&self, now: Instant) -> bool {
        self.collar_changed_at.is_none_or(|changed| now.saturating_duration_since(changed) >= REBUILD_INTERVAL)
    }
    pub fn set_collar_observation(&mut self,input:Option<Arc<collar_set::Set>>,revision:u64,
        cfg:WorldSampleConfig,root:Arc<std::path::PathBuf>,coverage:ResidentCoverage) {
        let source_changed=self.collar_observation.as_ref().is_none_or(|(_,c,r,_)|
            *c!=cfg || !Arc::ptr_eq(r,&root));
        let changed=source_changed || !same_collar(self.current_collar.as_ref(),input.as_ref());
        let coverage=self.collar_observation.as_ref().filter(|(_,_,_,v)|v.as_ref()==&coverage)
            .map_or_else(||Arc::new(coverage),|(_,_,_,v)|Arc::clone(v));
        self.current_collar=input;
        self.collar_observation=Some((revision,cfg,root,coverage));
        if changed {
            self.checked_revision = None;
            self.last_check = None;
            self.collar_changed_at = Some(Instant::now());
        }
        self.refresh_collar_mode();
    }
    fn refresh_collar_mode(&mut self) {
        let eligible=self.published.as_ref().is_some_and(|published| {
            let Some(expected)=published.collar.as_ref() else {return false;};
            let Some(actual)=self.current_collar.as_ref() else {return false;};
            self.collar_observation.as_ref().is_some_and(|(_,cfg,root,coverage)|
                *cfg==published.snapshot.cfg && Arc::ptr_eq(root,&published.snapshot.regions.root)
                && expected.valid_for(actual,coverage))
        });
        if let Some(mesh)=self.mesh.as_mut() {
            let eligible=eligible && mesh.pages.iter().any(|p|p.batches.iter().any(|b|b.conditional()));
            if mesh.collared!=eligible {
                mesh.collared=eligible;
                self.shadow_generation=self.shadow_generation.wrapping_add(1);
                println!("[horizon-collar] draw_variant={} source_current={eligible}",
                    if eligible {"collared"} else {"uncollared"});
            }
        }
    }


    pub fn quality_columns(&mut self,cfg:WorldSampleConfig,eye:Vec3,forward:Vec3,
        focal:f32,viewport:[u32;2],ordinary_lod0_m:f32)->Vec<(i32,i32)> {
        let candidates=if self.config==Some(cfg) {self.base.as_ref().map(|b|b.quality.as_slice()).unwrap_or(&[])}else{&[]};
        self.quality_selector.choose(cfg,candidates,quality::View{eye,forward,focal,viewport,ordinary_lod0_m},Instant::now())
    }
    pub fn quality_parent_valid(&self,snapshot:&crate::world::WorldSnapshot,eye:Vec3,columns:&[(i32,i32)])->bool {
        self.published.as_ref().is_some_and(|p|p.valid_for_resident(snapshot,eye,self.exclusion_radius,&self.resident))
            && {let baked=self.baked_columns(columns);columns.iter().all(|c|baked.contains(c))}
    }

    pub fn set_device_loss_signal(&mut self,signal:&Arc<AtomicBool>) {
        if !Arc::ptr_eq(&self.device_lost,signal) {self.device_lost=Arc::clone(signal);}
        if signal.load(Ordering::Acquire) {self.cancel_work();}
    }


    pub fn set_refinement_projection(&mut self, pixels: f32) {
        if !pixels.is_finite() || pixels <= 0.0 { return; }
        let pixels = ((pixels / 32.0).round() * 32.0).clamp(32.0, 8192.0);
        let previous = self.refinement_projection_pixels;
        if previous == 0.0 || pixels < previous * 0.8 || pixels > previous * 1.25 {
            self.refinement_projection_pixels = pixels;
            self.checked_revision = None;
            self.last_check = None;
        }
    }
    pub fn shadow_generation(&self) -> u64 { self.shadow_generation }
    /// The distant-terrain detail map that became current with the last
    /// publish: `Some(Some(map))` to upload, `Some(None)` to disable it.
    pub fn take_detail_update(&mut self) -> Option<Option<Arc<detail::DetailMap>>> { self.detail_update.take() }
    pub fn published_revision(&self) -> Option<u64> { self.published.as_ref().map(|p|p.source_revision) }
    pub fn loaded_this_world(&self) -> bool { self.loaded_this_world }
    pub fn baked_columns(&self, columns: &[(i32,i32)]) -> std::collections::HashSet<(i32,i32)> {
        let Some(published) = &self.published else { return Default::default(); };
        columns.iter().copied().filter(|&column| published.baked_column(column)).collect()
    }

    pub(super) fn covers_streaming_column(&self, cfg: WorldSampleConfig, column: (i32, i32)) -> bool {
        self.mesh.is_some() && self.published.as_ref().is_some_and(|target| {
            target.snapshot.cfg == cfg && target.clip.is_none() && target.baked_column(column)
        })
    }
    pub fn resident_coverage(&self) -> &ResidentCoverage { &self.resident }
    pub fn set_resident_coverage(&mut self, coverage: ResidentCoverage, desired: ResidentCoverage) {
        if self.resident.as_ref()!=&coverage { self.resident=Arc::new(coverage); }
        if self.desired_resident.as_ref()!=&desired { self.desired_resident=Arc::new(desired); }
    }
    pub fn retention_columns(&self) -> Vec<(i32,i32)> {
        let mut result=Vec::new();
        if let Some(published)=&self.published {
            let scale=published.snapshot.cfg.voxel_meters();
            for &(lo,hi) in published.omitted.iter() {
                if published.coverage.contains_bounds(lo*scale,hi*scale,scale) {
                    let a=lo.floor().as_ivec3().div_euclid(IVec3::splat(32));
                    let b=(hi.ceil().as_ivec3()-IVec3::ONE).div_euclid(IVec3::splat(32));
                    for z in a.z..=b.z { for x in a.x..=b.x { result.push((x,z)); } }
                }
            }
        }
        // Active jobs may still be constructing their omission list. Keep the
        // already-ready edited columns captured by that immutable source view.
        if let Some(active) = &self.active
            && let [x, z, width, 1] = active.coverage.header
        {
            // Desired coverage is a dense local window. Query the saved index
            // for that window instead of walking the entire save every frame.
            let radius = width / 2;
            let center = IVec3::new(x + radius, 0, z + radius);
            let records = active.snapshot.regions.records.nearby(center, radius);
            for q in active.snapshot.chunks.keys().chain(records) {
                if active.coverage.contains_column(q.x,q.z) { result.push((q.x,q.z)); }
            }
        }
        let mut seen=std::collections::HashSet::new();
        result.retain(|p|seen.insert(*p));
        result
    }
    pub fn exclusion_radius(&self) -> f32 {
        self.exclusion_radius
    }
    pub fn visibility(&self) -> f32 {
        if self.mesh.is_some() { 1.0 } else { 0.0 }
    }
    /// Staged and total pages of the upload in flight.
    pub fn upload_pages(&self) -> Option<(usize, usize)> { self.upload.as_ref().map(upload::Upload::pages) }
    /// True while the worker builds the base cover, before page meshing.
    pub fn building_base(&self) -> bool { self.pending.is_some() && self.building_base }
    fn retained_base_capacity(&self, key: BaseKey) -> usize {
        // At most ONE exact base is reused. Shared Arcs/previous-base aliases
        // are not summed; an upgrade or mismatched recipe gets no credit.
        matching_base_capacity(key,self.base.iter().chain(self.previous_base.iter())
            .map(|base|(base.key(),mesh_capacity_bytes(&base.mesh))))
    }
    pub fn budget_exhausted(&self) -> bool {
        self.budget_exhausted.as_ref().is_some_and(|failed|
            self.desired.as_ref().is_some_and(|desired| if self.budget_pressure {
                failed.same_scene(desired)
            } else { failed.matches(desired) }))
    }
    fn budget_exhausted_for_limit(&self, world: &crate::world::World, limit: usize) -> bool {
        self.budget_exhausted() && self.config == Some(world.sample_config())
            && self.source_root.as_ref().is_some_and(|root| Arc::ptr_eq(root, &world.regions.view.root))
            && self.budget_exhausted.as_ref().is_some_and(|failed| self.budget_pressure || failed.publication_limit == limit)
    }
    pub fn budget_exhausted_for(&self, world: &crate::world::World) -> bool {
        let retained=self.desired.as_ref().map_or(0,|target|self.retained_base_capacity(target.base_key()));
        let decision=budget::choose(retained);
        if self.budget_exhausted_since.is_some_and(|at| budget::telemetry_grace(decision.denial, at.elapsed())) { return false; }
        self.budget_exhausted_for_limit(world, decision.limit)
    }
    pub fn work_stage(&self) -> &'static str {
        if self.upload.is_some() { "upload" } else if self.pending.is_some() { "build" } else { "idle" }
    }
    pub fn published_source(&self) -> Option<crate::world::WorldSampleConfig> {
        self.published.as_ref().map(|p|p.snapshot.cfg)
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &mut crate::world::World,
        revision: u64,
        eye: Vec3,
        hidden_radius: f32,
        fast_start: bool,
    ) {
        static ENV_RANGE: std::sync::Once = std::sync::Once::new();
        // `VOXEL_HORIZON_RANGE_M` seeds the range for A/B captures with one exe.
        ENV_RANGE.call_once(|| {
            if let Some(m) = std::env::var("VOXEL_HORIZON_RANGE_M").ok().and_then(|v| v.trim().parse().ok()) {
                set_horizon_range_m(m);
            }
        });
        let now = Instant::now();
        self.exclusion_radius = hidden_radius.max(0.0);
        if eye.is_finite() {
            let previous = self.refinement_eye_y.unwrap_or((eye.y/64.0).round()*64.0);
            let cfg = world.sample_config();
            let sea = cfg.water_level() as f32 * cfg.voxel_meters();
            let next = if refinement_eye_moved(previous, eye.y, sea) { (eye.y/64.0).round()*64.0 } else { previous };
            if self.refinement_eye_y != Some(next) {
                self.refinement_eye_y = Some(next);
                self.checked_revision = None;
                self.last_check = None;
            }
        }
        self.update_mesh(device, queue, world, revision, eye, hidden_radius, now, fast_start);
    }

    pub(super) fn lod_transition(&self, now: Instant) -> Option<(&GpuMesh, u32)> {
        let (mesh, _, started) = self.outgoing.as_ref()?;
        Some((mesh, super::lod_transition_phase(*started, now)?))
    }

    pub fn clear_mesh(&mut self) {
        self.outgoing = None;
        if self.mesh.is_some() { self.shadow_generation=self.shadow_generation.wrapping_add(1); }
        self.mesh = None;
        self.published = None;
    }

    fn cancel_work(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.pending = None;
        self.upload = None;
        self.active = None;
        self.building_base = false;
        // Keep worker until is_finished(); its next receiver can never be confused
        // with this one, and at most one CPU build is active per Horizon instance.
    }

    fn stop_expanded_upload(&mut self, reason: budget::Denied) {
        self.budget_exhausted = self.active.take();
        self.budget_exhausted_since = Some(Instant::now());
        self.budget_pressure = true;
        self.pending = None;
        self.upload = None; // Drops only unpublished pages; previous valid cover survives.
        self.upload_limit = 0;
        self.cancel.store(true, Ordering::Relaxed);
        eprintln!("[horizon] publication_pressure_abort reason={} partial_pages_published=false", reason.label());
    }
    fn advance_upload(&mut self,device:&wgpu::Device,queue:&wgpu::Queue,now:Instant)->bool {
        // Also runs on the final advance immediately before publishing. The
        // allowance stays immutable: pressure aborts, never shrinks a partial job.
        if self.upload_limit > budget::BASE_LIMIT
            && let Some(upload) = self.upload.as_ref()
            && let Err(reason) = budget::upload_admission(self.upload_limit, upload.unallocated_gpu_bytes()) {
            self.stop_expanded_upload(reason);
            return false;
        }
        match self.upload.as_mut().map(|upload|upload.advance(device,queue,&self.device_lost)) {
            Some(Ok(ready))=>{
                if self.device_lost.load(Ordering::Acquire) {self.fail_active(now);false} else {ready}
            }
            Some(Err(error))=>{
                eprintln!("[horizon] upload_failed center={:?} request_generation={:?}: {error}",
                    self.active.as_ref().map(|t|t.center),self.active.as_ref().map(|t|t.request_generation));
                // fail_active drops only unpublished work; the old draw bundle
                // and its immutable source/coverage metadata remain installed.
                self.fail_active(now);
                false
            }
            None=>false,
        }
    }

    fn fail_active(&mut self, now: Instant) {
        if let Some(target) = self.active.take() {
            let streak = match &self.failure_streak {
                Some((previous, count)) if previous.matches(&target) => count.saturating_add(1),
                _ => 1,
            };
            self.failure_streak = Some((target.clone(), streak));
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
            && self.worker.take().unwrap().join().is_err() {
                eprintln!("[horizon] worker panicked; keeping the last published mesh");
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
        fast_start: bool,
    ) {
        self.reap_worker();
        // Pressure rejection is sticky for the same scene, even when dropping
        // its own buffers restores apparent headroom or allowance oscillates.
        // Only a materially different scene/root request can retry it.
        if self.outgoing.as_ref().is_some_and(|(_, previous, started)| {
            super::lod_transition_phase(*started, now).is_none()
                || !previous.valid_for_resident(&world.snapshot(), eye, hidden_radius, &self.resident)
        }) {
            self.outgoing = None;
        }
        // Only a different world brings the loader back; the horizon rebuilds
        // in place during play, and the far field should not hide gameplay.
        if !self.source_root.as_ref().is_some_and(|root| Arc::ptr_eq(root, &world.regions.view.root)) {
            self.loaded_this_world = false;
        }
        let cfg = world.sample_config();
        if self.config != Some(cfg)
            || self
                .source_root
                .as_ref()
                .is_none_or(|root| !Arc::ptr_eq(root, &world.regions.view.root))
        {
            self.cancel_work();
            self.clear_mesh();
            self.samples = Arc::default();
            self.base = None;
            self.previous_base = None;
            self.detail_center = None;
            self.detail_update = Some(None);
            self.config = Some(cfg);
            self.source_root = Some(world.regions.view.root.clone());
            self.desired = None;
            self.failed = None;
            self.failure_streak = None;
            self.budget_exhausted = None;
            self.budget_exhausted_since = None;
            self.budget_pressure = false;
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
        let snapped = center.unwrap();
        // 128 m dead band around the 512 m snapping boundary. Desired state
        // still advances during work, without thrashing when the camera hovers.
        let center = self.desired.as_ref().map_or(snapped, |target| {
            if (eye.x - target.center.0 as f32)
                .abs()
                .max((eye.z - target.center.1 as f32).abs())
                < 384.0
            {
                target.center
            } else {
                snapped
            }
        });

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
        // One read per check, so the target and `checked_range` never disagree.
        let range_m = horizon_range_m();
        // A moving player can lose the previous cover during a rebuild.
        // Restore ground first, then fill in distant foliage as at startup.
        let startup = fast_start && self.mesh.is_none();
        let changed_center = self
            .desired
            .as_ref()
            .is_none_or(|target| target.center != center);
        // Do not reinterpret our own in-flight CPU pages as a new request's
        // budget change. Pressure is checked with stage-aware remaining peaks;
        // a new allowance is sampled once the current request releases ownership.
        let projection_pixels=if self.refinement_projection_pixels>0.0 {self.refinement_projection_pixels} else {512.0};
        let refinement_eye_y=self.refinement_eye_y.unwrap_or((eye.y/64.0).round()*64.0);
        let key=(startup,center,range_m,self.chart_clip,projection_pixels.to_bits(),refinement_eye_y.to_bits());
        let publication_limit=self.active.as_ref().map_or_else(
            ||budget::choose(self.retained_base_capacity(key)).limit,|active|active.publication_limit);
        let needs_check = self.desired.as_ref().is_some_and(|d| d.publication_limit != publication_limit)
            || changed_center
            || self.desired.as_ref().is_some_and(|d| d.startup != startup || d.coverage != self.desired_resident)
            || self.checked_eye != Some(eye_cell)
            || self.checked_radius != hidden_radius
            || self.checked_range != range_m
            || self.checked_revision != Some(revision);
        let check_due = changed_center
            || self
                .last_check
                .is_none_or(|last| now.saturating_duration_since(last) >= CHECK_INTERVAL);
        // Keep the published terrain as fallback while the next build repairs
        // exposed omissions. Dropping the entire cover for one missing patch
        // leaves every unstreamed column blank. Replacement matching checks
        // omission scope against the coalesced view below; upload still
        // validates against the live view before taking ownership.
        if needs_check && check_due {
            let snapshot = world.snapshot();
            self.request_generation=self.request_generation.wrapping_add(1);
            // Local changes still request a coalesced proxy update, but they
            // cannot cancel far work while resident GPU geometry owns them.
            let fingerprint = edits::visible_fingerprint(&snapshot, eye, 0.0);
            self.desired = Some(BuildTarget {
                startup,
                collar:self.collar_observation.as_ref().filter(|(_,c,r,_)|*c==cfg && Arc::ptr_eq(r,&world.regions.view.root))
                    .and(self.current_collar.clone()),
                refinement_eye_y,
                projection_pixels,
                range_m,
                center,
                request_generation:self.request_generation,
                source_revision:revision,
                publication_limit,
                fingerprint,
                snapshot,
                eye,
                hidden_radius,
                omitted: Arc::default(),
                coverage: self.desired_resident.clone(),
                clip: self.chart_clip,
                validation: Default::default(),
            });
            self.checked_revision = Some(revision);
            self.checked_eye = Some(eye_cell);
            self.checked_radius = hidden_radius;
            self.checked_range = range_m;
            self.last_check = Some(now);
        }

        // Each receiver and upload has one immutable active target. A newer
        // desired snapshot is coalesced as a follow-up, allowing useful progress
        // while local simulation changes revision every frame. Revalidate that
        // target's OWN footprint, not the desired center, before any GPU work.
        // An A-center result can still be useful after B has been requested.
        if self.upload.is_some()
            && self.active.as_ref().is_some_and(|target| {
                !target.valid_for_resident(&world.snapshot(), eye, hidden_radius, &self.resident)
            })
        {
            self.rejected_stale += 1;
            if self.rejected_stale <= 8 && std::env::var("VOXEL_HORIZON_TRACE").as_deref()==Ok("1")
                && let Some(target) = &self.active {
                    edits::trace_changes(
                        &target.snapshot,
                        &world.snapshot(),
                        target.eye,
                        target.hidden_radius,
                    );
                }
            eprintln!(
                "[horizon] reject_stale={} stage=build_or_upload",
                self.rejected_stale
            );
            self.cancel_work();
            self.last_check = None;
            self.checked_revision = None;
            self.last_build = None;
        }
        if let Some(result) = self.pending.as_ref().map(|rx| rx.try_recv()) {
            match result {
                Ok(BuildEvent::PublicationPressure(reason)) => self.stop_expanded_upload(reason),
                Ok(BuildEvent::BudgetExhausted) => {
                    self.budget_exhausted = self.active.take();
                    self.budget_exhausted_since = Some(now);
                    self.budget_pressure = false;
                    self.pending = None;
                    self.upload = None;
                    eprintln!("[horizon] terminal_budget_failure allowance_bytes={} base_unchanged=true", self.budget_exhausted.as_ref().map_or(budget::BASE_LIMIT, |target| target.publication_limit));
                }
                Ok(BuildEvent::Base(base)) => {
                    if let Some(previous) = self.base.replace(base)
                        && self.base.as_ref().is_some_and(|base| base.center != previous.center)
                    {
                        self.previous_base = Some(previous);
                    }
                    self.building_base = false;
                }
                Ok(BuildEvent::Complete(pages, samples, omitted, publication_limit)) => {
                    self.pending = None;
                    let expansion_denial = if publication_limit > budget::BASE_LIMIT {
                        budget::upload_admission(publication_limit,pages.iter().map(|page| mesh_bytes(&page.mesh)).sum()).err()
                    } else { None };
                    if let Some(target) = &mut self.active {
                        target.omitted = Arc::new(omitted);
                        *target.validation.get_mut()=Default::default();
                    }
                    if self.active.as_ref().is_some_and(|target| {
                        !target.valid_for_resident(&world.snapshot(), eye, hidden_radius, &self.resident)
                    }) {
                        if self.rejected_stale < 8 && (std::env::var("VOXEL_HORIZON_TRACE").as_deref()==Ok("1")
                            || std::env::var("VOXEL_HORIZON_REJECT_TRACE").as_deref()==Ok("1"))
                            && let Some(target) = &self.active {
                                let snapshot = world.snapshot();
                                eprintln!("[horizon-reject] eye={eye:?} strict_radius={hidden_radius} omitted={}",target.omitted.len());
                                edits::trace_rejection(&target.snapshot,&snapshot,&target.omitted,eye,hidden_radius,&self.resident);
                            }
                        self.rejected_stale += 1;
                        self.samples = samples;
                        self.fail_active(now);
                        self.checked_revision = None;
                        self.last_check = None;
                        self.last_build = None;
                        eprintln!(
                            "[horizon] reject_stale={} stage=completed_cpu",
                            self.rejected_stale
                        );
                        return;
                    }
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
                    } else if let Some(reason) = expansion_denial {
                        // Recheck at transfer from the CPU worker before any new GPU allocation.
                        self.stop_expanded_upload(reason);
                    } else if let Some(target) = &self.active {
                        self.upload_limit = publication_limit;
                        self.upload =
                            Some(upload::Upload::new(pages, samples, Some(target.center)).with_live_budget(self.loaded_this_world));
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    eprintln!("[horizon] worker ended without pages; retry scheduled");
                    self.fail_active(now);
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.outgoing.is_none() && self.advance_upload(device,queue,now) {
            let (mesh, samples, center) = self.upload.take().unwrap().finish();
            if let Some(target) = self.active.take() {
                debug_assert_eq!(center, Some(target.center));
                // Publish geometry, cache and its source metadata together.
                println!(
                    "[horizon] publish built_center={:?} desired_center={:?} fingerprint={:016x} source_revision={} rejected_stale={} gpu_bytes={} cache_samples={} request_generation={} source_cfg={:?}",
                    target.center,
                    self.desired.as_ref().map(|t| t.center),
                    target.fingerprint,
                    target.source_revision,
                    self.rejected_stale,
                    mesh.pages
                        .iter()
                        .map(|p| p.vertices.size() + p.indices.size())
                        .sum::<u64>(),
                    samples.len(),target.request_generation,target.snapshot.cfg
                );
                let previous = self.mesh.replace(mesh).zip(self.published.take());
                self.outgoing = previous.and_then(|(mut old, published)| {
                    // Omitted areas must still have resident coverage, and
                    // edited surfaces cannot reappear through an outgoing mesh.
                    if published.snapshot.cfg != target.snapshot.cfg
                        || !Arc::ptr_eq(&published.snapshot.regions.root, &target.snapshot.regions.root)
                        || !published.valid_for_resident(&target.snapshot, eye, hidden_radius, &self.resident)
                    {
                        return None;
                    }
                    old.collared = published.collar.as_ref().zip(self.current_collar.as_ref())
                        .is_some_and(|(built, current)| built.valid_for(current, &self.resident));
                    Some((old, published, Instant::now()))
                });
                self.loaded_this_world = true;
                if target.startup {
                    println!("[horizon] startup terrain published; completing distant detail");
                    self.last_check = None;
                }
                self.shadow_generation=self.shadow_generation.wrapping_add(1);
                if !target.startup && self.detail_center != Some(target.center) {
                    self.detail_center = Some(target.center);
                    self.detail_update = Some(self.base.as_ref()
                        .filter(|base| base.center == target.center)
                        .and_then(|base| base.detail.clone()));
                }
                self.samples = samples;
                self.published = Some(target);
                self.refresh_collar_mode();
                self.failed = None;
                self.failure_streak = None;
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
        // The original cover already has a runtime-masked fallback. Wait for
        // changing resident edges to settle before uploading a collar-only replacement.
        if self.mesh.is_some()
            && (target.collar.is_none() || !self.collar_settled(now))
            && self.published.as_ref().is_some_and(|ready| ready.matches_without_collar(target))
        {
            return;
        }
        if self.budget_exhausted() { return; }
        let streak = self
            .failure_streak
            .as_ref()
            .filter(|(failed, _)| failed.matches(target))
            .map_or(1, |(_, count)| *count);
        if self.failed.as_ref().is_some_and(|(failed, at)| {
            failed.matches(target) && now.saturating_duration_since(*at) < retry_wait(streak)
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
        let reusable = |base: &BaseCover| {
            base.center == target.center && base.range_m == target.range_m && base.clip == target.clip
                && base.projection_pixels.to_bits() == target.projection_pixels.to_bits()
                && base.refinement_eye.y.to_bits() == target.refinement_eye_y.to_bits()
        };
        if self.previous_base.as_ref().is_some_and(reusable)
            && !self.base.as_ref().is_some_and(reusable)
        {
            std::mem::swap(&mut self.base, &mut self.previous_base);
        }
        let tree_samples = self.base.as_ref().map(|base| base.tree_samples.clone()).unwrap_or_default();
        // The detail map depends only on the source and centre.
        let prior_detail = self.base.as_ref().filter(|base| base.center == target.center).and_then(|base| base.detail.clone());
        let held = self.base.as_ref().filter(|base| base.range_m == target.range_m).map(|base| base.refined.clone()).unwrap_or_default();
        let base = self.base.as_ref().filter(|base| reusable(base)).cloned();
        let cache_hit = base.as_ref().is_some_and(|base| base.startup == target.startup);
        self.building_base = !cache_hit;
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.pending = Some(rx);
        // This is an individual-buffer limit, not a bound on all mesh memory.
        let page_limit = device.limits().max_buffer_size.min(64 * 1024 * 1024);
        let samples = std::mem::take(&mut self.samples);
        match std::thread::Builder::new().name("horizon".into()).spawn(move || {
            // One unchanged procedural base and ONE admitted publication pass.
            // The allowance does not enlarge the base or alter its geometry.
            let start = Instant::now();
            let center = target.center;
            let delay = std::env::var("VOXEL_HORIZON_DELAY_MS").ok()
                .and_then(|v| v.parse::<u64>().ok()).unwrap_or(0).min(30000);
            for _ in 0..delay.div_ceil(10) {
                if cancel.load(Ordering::Relaxed) { return; }
                std::thread::sleep(Duration::from_millis(10));
            }
            let base = if cache_hit { base.unwrap() } else {
                let built = if let Some(ground) = base.filter(|base| base.startup && !target.startup) {
                    // The first cover is still on screen. Reuse its exact terrain and
                    // water on the worker, then add the original foliage to the copy.
                    let mut mesh = ChunkMesh { vertices: ground.mesh.vertices.clone(), indices: ground.mesh.indices.clone() };
                    let mut tree_samples = (*tree_samples).clone();
                    finish_base_detail(&mut mesh, &cfg, center, target.clip, &cancel, &mut tree_samples)
                        .map(|trees| (mesh, ground.samples, trees, tree_samples, ground.refined))
                } else {
                    build_with_focus(cfg, center, Vec3::new(center.0 as f32,target.refinement_eye_y,center.1 as f32), target.projection_pixels, &cancel, Arc::unwrap_or_clone(samples.clone()),(*tree_samples).clone(), target.range_m, target.clip, &held, target.startup)
                        .map(|(mesh, samples, trees, tree_samples, refined)| {
                            (mesh, Arc::new(samples), trees, tree_samples, Arc::new(refined))
                        })
                };
                let Some((mesh, samples, trees, tree_samples, refined)) = built else { return; };
                if mesh_bytes(&mesh) > MAX_MESH_BYTES {
                    eprintln!("[horizon] procedural base exceeds unchanged base budget");
                    let _ = tx.send(BuildEvent::BudgetExhausted);
                    return;
                }
                let quality_started = Instant::now();
                let Some(quality)=quality::summarize(cfg,center,&samples,&cancel) else {return;};
                println!("[horizon-timing] quality_ms={:.0}", quality_started.elapsed().as_secs_f64() * 1000.0);
                let detail = if target.startup { None } else if let Some(prior) = prior_detail.clone() { Some(prior) } else if detail::enabled() {
                    let detail_started = Instant::now();
                    let Some(map) = detail::build(&cfg, center, &cancel) else { return; };
                    println!("[horizon-timing] detail_ms={:.0}", detail_started.elapsed().as_secs_f64() * 1000.0);
                    Some(Arc::new(map))
                } else { None };
                let base = BaseCover { startup: target.startup, clip: target.clip, quality:Arc::new(quality),center, refinement_eye: Vec3::new(center.0 as f32,target.refinement_eye_y,center.1 as f32), projection_pixels: target.projection_pixels, range_m: target.range_m, mesh: Arc::new(mesh), samples, trees:Arc::new(trees), tree_samples:Arc::new(tree_samples), detail, refined };
                if tx.send(BuildEvent::Base(base.clone())).is_err() { return; }
                base
            };
            if cancel.load(Ordering::Relaxed) { return; }
            println!("[horizon] base_cache hit={cache_hit} center={center:?} refinement_eye={:?} focal_pixels={}", base.refinement_eye, base.projection_pixels);
            let publication_limit=target.publication_limit;
            if cancel.load(Ordering::Relaxed) { return; }
            if publication_limit > budget::BASE_LIMIT && let Err(reason) = budget::build_admission(publication_limit,mesh_capacity_bytes(&base.mesh)) {
                eprintln!("[horizon] publication_expansion_denied reason={}", reason.label());
                let _ = tx.send(BuildEvent::PublicationPressure(reason));
                return;
            }
            println!("[horizon] publication_attempt=1 limit_bytes={} base_bytes={} retained_base_capacity={} geometry_unchanged=true headroom_reserved=false", publication_limit, mesh_bytes(&base.mesh), mesh_capacity_bytes(&base.mesh));
            // Copy only on the worker: edits clip indices and append patches.
            // The main thread retains just Arcs, never copying a large mesh.
            let mut mesh = ChunkMesh { vertices: base.mesh.vertices.clone(), indices: base.mesh.indices.clone() };
            // Sampling is immutable until the next base needs to consume old entries.
            let samples = Arc::clone(&base.samples);
            if cancel.load(Ordering::Relaxed) { return; }
            let mut edit_budget_exhausted = false;
            let Some(omitted) = edits::apply_with_budget(
                    &mut mesh, &target.snapshot, center, target.eye,
                    target.hidden_radius, &target.coverage, &cancel, &base.trees,
                    publication_limit, &mut edit_budget_exhausted,
                ) else {
                    if edit_budget_exhausted && !cancel.load(Ordering::Relaxed) { let _=tx.send(BuildEvent::BudgetExhausted); }
                    return;
                };
            if cancel.load(Ordering::Relaxed) { return; }
            // Canonicalize bit-identical vertices regardless of pressure. Tree
            // replacement obligations are consumed; collars/batches follow.
            // Triangle order and full vertex attributes remain unchanged.
            let before_weld=mesh_bytes(&mesh);
            edits::weld(&mut mesh);
            println!("[horizon] final_bitwise_weld before={before_weld} after={}",mesh_bytes(&mesh));
            if cancel.load(Ordering::Relaxed) { return; }
            if mesh_bytes(&mesh) > publication_limit {
                eprintln!("[horizon] mesh budget exceeded: bytes={} limit={publication_limit}", mesh_bytes(&mesh));
                let _=tx.send(BuildEvent::BudgetExhausted);
                return;
            }
            let mut variants=None;
            if let Some(input)=target.collar.as_ref() {
                match collar_set::apply(&mut mesh,input,&cancel,publication_limit) {
                    None=>return,
                    Some(collar::Outcome::Ineligible)=>{},
                    Some(collar::Outcome::Applied(ranges))=>variants=Some(ranges),
                }
            }
            if cancel.load(Ordering::Relaxed) {return;}
            let batches = batches::partition_variants(&mut mesh,cfg.voxel_meters(),center,variants.as_ref());
            if cancel.load(Ordering::Relaxed) { return; }
            println!(
                "[horizon] center={center:?} triangles={} vertex_bytes={} index_bytes={} build_ms={:.0} spatial_batches={} startup={}",
                mesh.indices.len() / 3,
                mesh.vertices.len() * std::mem::size_of::<Vertex3>(),
                mesh.indices.len() * std::mem::size_of::<u32>(),
                start.elapsed().as_secs_f64() * 1000.0,
                batches.len(), target.startup,
            );
            let Some(pages) = pages::pack(mesh, batches, page_limit, publication_limit, &cancel) else {
                if cancel.load(Ordering::Relaxed) { return; }
                eprintln!("[horizon] page packing exceeded publication budget={publication_limit}");
                let _=tx.send(BuildEvent::BudgetExhausted);
                return;
            };
            if cancel.load(Ordering::Relaxed) { return; }
            println!(
                "[horizon] upload_pages={} page_limit_bytes={} packed_vertex_bytes={} packed_index_bytes={} max_page_vertex_bytes={} max_page_index_bytes={}",
                pages.len(), page_limit,
                pages.iter().map(|p| p.mesh.vertices.len() * std::mem::size_of::<Vertex3>()).sum::<usize>(),
                pages.iter().map(|p| p.mesh.indices.len() * std::mem::size_of::<u32>()).sum::<usize>(),
                pages.iter().map(|p| p.mesh.vertices.len() * std::mem::size_of::<Vertex3>()).max().unwrap_or(0),
                pages.iter().map(|p| p.mesh.indices.len() * std::mem::size_of::<u32>()).max().unwrap_or(0),
            );
            let packed_bytes: usize = pages.iter().map(|p| mesh_bytes(&p.mesh)).sum();
            println!("[horizon] publication_prepared attempt=1 packed_bytes={packed_bytes} limit_bytes={publication_limit}");
            let _ = tx.send(BuildEvent::Complete(pages, samples, omitted, publication_limit));
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


/// Keep all original cell edges; an interior source point cannot introduce a
/// ring or neighbour T-junction. Existing split-edge cells retain their mesh.
fn horizon_refinement_triangles(
    corners:[u32;4], heights:[f32;4], split:Option<[u32;3]>, middle:Option<u32>,
) -> ([[u32;3];4],usize) {
    if let Some(m) = middle.filter(|_| split.is_none()) {
        let [a,b,c,d] = corners;
        return ([[a,c,m],[c,d,m],[d,b,m],[b,a,m]],4);
    }
    let (original,count) = horizon_cell_triangles(corners,heights,split);
    let mut result = [[0;3];4];
    result[..count].copy_from_slice(&original[..count]);
    (result,count)
}

/// Items each prepass worker claims at a time. Small enough to balance the
/// ~7x cost spread between lattice regions, large enough to reuse erosion tiles.
const PREPASS_BLOCK: usize = 512;

/// Samples every `(position, detail, step)` in `work` into `samples`.
/// `VOXEL_HORIZON_WORKERS` caps the worker count for timing sweeps.
fn evaluate_samples(
    cfg: WorldSampleConfig,
    cancel: &AtomicBool,
    mut work: Vec<((i32, i32), u8, f32)>,
    samples: &mut Samples,
) -> Option<()> {
    // Erosion tiles are 128 m and the worker cache holds 1024 of them. HashMap
    // order scatters neighbours across the whole ~50 km lattice, so each tile
    // was rebuilt many times. Visiting in spatial order reuses each tile.
    work.sort_unstable_by_key(|&((bx, bz), _, _)| (bz.div_euclid(128), bx.div_euclid(128), bz, bx));
    let canonical = canonical_source();
    let workers = parallel::workers(parallel::cap_from_env("VOXEL_HORIZON_WORKERS", 16));
    let evaluated = parallel::map_blocks(&work, PREPASS_BLOCK, workers, cancel, |&((bx, bz), detail, step)| {
        let (height, block) = if detail == 0 {
            distant_surface(&cfg, bx, bz)
        } else if canonical {
            crate::world::landscape::preview_surface_canonical(&cfg, bx, bz)
        } else {
            crate::world::landscape::preview_surface_lod(&cfg, bx, bz, step)
        };
        Some(((bx, bz), (height, block, detail)))
    })?;
    samples.extend(evaluated);
    Some(())
}

/// Whether the eye left the band that keeps the refinement focus height.
/// Projected error scales with 1/distance and distance is at least the height
/// above sea, so a 25% band bounds the error change the way the projection
/// band does. A fixed 48 m band rebuilt the base dozens of times per descent.
fn refinement_eye_moved(previous: f32, eye_y: f32, sea: f32) -> bool {
    let band = ((previous - sea).abs() * 0.25).max(48.0);
    (eye_y - previous).abs() > band
}

fn build(cfg: WorldSampleConfig, center: (i32,i32), cancel: &AtomicBool, old: Samples,
    trees: vegetation::TreeSamples) -> Option<(ChunkMesh, Samples, Vec<vegetation::TreeSource>, vegetation::TreeSamples)> {
    let scale = cfg.voxel_meters();
    let height = distant_surface(&cfg, (center.0 as f32/scale).floor() as i32, (center.1 as f32/scale).floor() as i32).0 * scale;
    build_with_focus(cfg, center, Vec3::new(center.0 as f32, ((height+100.0)/64.0).round()*64.0, center.1 as f32), 512.0, cancel, old, trees, horizon_range_m(), None, &HashSet::new(), false)
        .map(|(mesh, samples, trees, tree_samples, _)| (mesh, samples, trees, tree_samples))
}

type BaseBuild = (ChunkMesh, Samples, Vec<vegetation::TreeSource>, vegetation::TreeSamples, HashSet<(i32, i32)>);

fn build_with_focus(
    cfg: WorldSampleConfig,
    center: (i32, i32),
    focus: Vec3,
    projection_pixels: f32,
    cancel: &AtomicBool,
    mut old: Samples,
    mut tree_samples:vegetation::TreeSamples,
    // Range of the target being built: read once so one build never mixes values
    // when the setting changes mid-build.
    range_m: u32,
    chart_clip: Option<clip::ChartClip>,
    held: &HashSet<(i32, i32)>,
    startup: bool,
) -> Option<BaseBuild> {
    let reach = domain_reach(range_m);
    // Sibling appenders index these SIDE*SIDE grids directly. Keep every grid,
    // including the inactive inner vertices, contiguous and in the old order.
    debug_assert_eq!(SIDE, HALF * 2 + 1);
    debug_assert_eq!(HALF % 2, 0);
    let mut lap = std::time::Instant::now();
    let mut mesh = ChunkMesh::empty();
    let vertex_count = LEVELS * SIDE * SIDE;
    mesh.vertices.reserve(vertex_count + MAX_REFINEMENT_SAMPLES + MAX_FAR_REFINEMENT_SAMPLES);
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
                if !sample_in_domain(
                    (x as f32 - HALF as f32) * step,
                    (z as f32 - HALF as f32) * step,
                    step,
                    reach,
                ) {
                    continue;
                }
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
    evaluate_samples(cfg, cancel, work, &mut samples)?;
    println!("[horizon-timing] prepass_ms={:.0}", lap.elapsed().as_secs_f64() * 1000.0);
    lap = std::time::Instant::now();


    let surface = cfg.water_level() as f32 + 1.0 - crate::mesh::WATER_SURFACE_INSET;
    let refinement_points = refinement::sample(cfg, center, focus, projection_pixels, reach, cancel, &mut samples, &mut old, held)?;
    let mut refined = HashSet::with_capacity(refinement_points.len());
    println!("[horizon-timing] refinement_ms={:.0}", lap.elapsed().as_secs_f64() * 1000.0);
    lap = std::time::Instant::now();
    let mut refinement_level_samples = [0usize; LEVELS];
    for &index in refinement_points.keys() { refinement_level_samples[index / (SIDE*SIDE)] += 1; }

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
                // Outside the supported disk these contiguous knots exist only
                // for sibling lattice indexing; no emitted triangle uses them.
                let (height, block, _) = samples.get(&(bx, bz)).copied().unwrap_or((
                    0.0,
                    crate::world::BlockId::Stone,
                    sample_detail(level),
                ));
                if !height.is_finite() {
                    return None;
                }
                let material = block.surface();
                owner.push(mesh.vertices.len() as u32);
                mesh.vertices.push(Vertex3 {
                    // Shared source heights remain world-aligned. Published
                    // column ownership, rather than a depth sink, selects cover.
                    pos: [vx, height - HORIZON_SINK_VOXELS, vz],
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

    println!("[horizon-timing] phase1_ms={:.0}", lap.elapsed().as_secs_f64() * 1000.0);
    lap = std::time::Instant::now();
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

    let mut refinement_refined = 0usize;
    println!("[horizon-timing] phase2_ms={:.0}", lap.elapsed().as_secs_f64() * 1000.0);
    lap = std::time::Instant::now();
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
                if !cell_in_domain(x,z,step,reach) { continue; }
                let a = owner[base + z * SIDE + x];
                let b = owner[base + z * SIDE + x + 1];
                let c = owner[base + (z + 1) * SIDE + x];
                let d = owner[base + (z + 1) * SIDE + x + 1];
                let corners = [a, b, c, d];
                let heights = corners.map(|index| mesh.vertices[index as usize].pos[1]);
                let split = horizon_inner_split(level, x, z, corners, &owner);
                let mut middle = None;
                if split.is_none()
                    && let Some(&((bx,bz),was_held)) = refinement_points.get(&(base+z*SIDE+x)) {
                        let &(height,block,_) = samples.get(&(bx,bz))?;
                        let height = height - HORIZON_SINK_VOXELS;
                        let parent = if diagonal_a_to_d(heights[0],heights[1],heights[2],heights[3]) {
                            (heights[0]+heights[3])*0.5
                        } else { (heights[1]+heights[2])*0.5 };
                        let material = block.surface();
                        let novel_material = corners.iter().all(|&i| mesh.vertices[i as usize].surface[3] != material.shader_key());
                        let distance = (Vec3::new(bx as f32,height,bz as f32)*scale).distance(focus).max(step);
                        let projected_error = (height-parent).abs()*scale*projection_pixels/distance;
                        let bar = refinement::bar(was_held);
                        if projected_error >= bar || (height < surface) != (parent < surface)
                            || (novel_material && step*projection_pixels/distance >= bar / refinement::REFINE_PIXELS) {
                            if !height.is_finite() { return None; }
                            let index = mesh.vertices.len() as u32;
                            mesh.vertices.push(Vertex3 {
                                pos:[bx as f32,height,bz as f32],
                                color:block.color(IVec3::new(bx,height as i32,bz)),normal:[0.0;3],
                                surface:[material.roughness,material.specular,0.0,material.shader_key()],ao_strength:-1.0,
                            });
                            owner.push(index);
                            middle = Some(index);
                            refined.insert((bx,bz));
                            refinement_refined += 1;
                        }
                    }
                let (triangles, count) = horizon_refinement_triangles(corners, heights, split, middle);
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

    println!("[horizon-timing] cells_ms={:.0}", lap.elapsed().as_secs_f64() * 1000.0);
    lap = std::time::Instant::now();

    // Phase 4: each face contributed exactly once to its canonical owners.
    // Owners precede their aliases, so normalize once and then copy the result
    // to duplicate grid vertices needed by the water/vegetation appenders.
    for index in 0..mesh.vertices.len() {
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
    println!("[horizon] refinement_samples={} refinement_lod_samples={refinement_level_samples:?} refinement_refined_cells={refinement_refined}",refinement_points.len());
    // Columns first: their cut removes lattice out to the outer square, which
    // would also remove a fine grid appended before it. Fine's own cut then
    // only trims column boundary slivers, and its grid is appended last.
    // Time each column/fine/reference stage separately; columns dominate.
    if fine::fits_chart(center, chart_clip) {
        let split_started = std::time::Instant::now();
        columns::apply(&mut mesh, &cfg, center, cancel, fine::HALF_M)?;
        println!("[horizon-timing] split_columns_ms={:.0}", split_started.elapsed().as_secs_f64() * 1000.0);
        let split_started = std::time::Instant::now();
        fine::apply(&mut mesh, &cfg, center, cancel, chart_clip)?;
        println!("[horizon-timing] split_fine_ms={:.0}", split_started.elapsed().as_secs_f64() * 1000.0);
    }
    let split_started = std::time::Instant::now();
    reference::apply(&mut mesh, &cfg);
    println!("[horizon-timing] split_reference_ms={:.0}", split_started.elapsed().as_secs_f64() * 1000.0);
    println!("[horizon-timing] columns_fine_reference_ms={:.0}", lap.elapsed().as_secs_f64() * 1000.0);
    lap = std::time::Instant::now();
    water::append(&mut mesh, &cfg);
    println!("[horizon-timing] water_ms={:.0}", lap.elapsed().as_secs_f64() * 1000.0);
    lap = std::time::Instant::now();
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    println!("[horizon-timing] mesh_ms={:.0}", lap.elapsed().as_secs_f64() * 1000.0);
    let trees = if startup {
        Vec::new()
    } else {
        finish_base_detail(&mut mesh, &cfg, center, chart_clip, cancel, &mut tree_samples)?
    };
    Some((mesh, samples, trees,tree_samples,refined))
}

fn finish_base_detail(
    mesh: &mut ChunkMesh,
    cfg: &WorldSampleConfig,
    center: (i32, i32),
    chart_clip: Option<clip::ChartClip>,
    cancel: &AtomicBool,
    tree_samples: &mut vegetation::TreeSamples,
) -> Option<Vec<vegetation::TreeSource>> {
    let start = Instant::now();
    let mut trees = vegetation::append(mesh, cfg, center, cancel, tree_samples);
    println!("[horizon-timing] vegetation_ms={:.0}", start.elapsed().as_secs_f64() * 1000.0);
    let start = Instant::now();
    let before = mesh_bytes(mesh);
    structures::append(mesh, cfg, center, chart_clip, cancel, &mut trees);
    println!("[horizon-timing] structures_ms={:.1} structures_bytes={}",
        start.elapsed().as_secs_f64() * 1000.0, mesh_bytes(mesh) - before);
    reference::apply_voxels(mesh, cfg, cancel)?;
    // Review-only exact patches rewrite index order. Their dedicated source
    // comparison runs have no edit fixture; never reuse pre-rewrite offsets.
    if std::env::var("VOXEL_HORIZON_EXACT_PATCH").is_ok_and(|s|!s.trim().is_empty()) { trees.clear(); }
    (!cancel.load(Ordering::Relaxed)).then_some(trees)
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
            planet: None,
            generator_version: 18,
            seed: 2_738_958_700,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let cancel = AtomicBool::new(false);
        let start = Instant::now();
        let built = build(cfg, (0, 0), &cancel, Samples::new(),Default::default());
        let elapsed = start.elapsed();
        assert!(built.is_some());
        let mesh = built.unwrap().0;
        // Compare against a `VOXEL_HORIZON_SERIAL=1` run: the mesh must not
        // depend on worker count or scheduling.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash_slice(bytemuck::cast_slice::<_, u8>(&mesh.vertices), &mut hasher);
        std::hash::Hash::hash_slice(&mesh.indices, &mut hasher);
        println!(
            "cold horizon build: {elapsed:?}, vertices={}, indices={}, checksum={:016x}",
            mesh.vertices.len(),
            mesh.indices.len(),
            std::hash::Hasher::finish(&hasher)
        );
    }
}

pub(super) fn proxy_box(
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
mod refinement_eye_band {
    use super::refinement_eye_moved;

    #[test]
    fn near_ground_keeps_the_fixed_band() {
        assert!(!refinement_eye_moved(64.0, 100.0, 12.0));
        assert!(refinement_eye_moved(64.0, 128.0, 12.0));
    }

    #[test]
    fn high_flight_rebuilds_on_relative_height() {
        // 3 km above sea: a 500 m descent stays inside the 25% band.
        assert!(!refinement_eye_moved(3012.0, 2512.0, 12.0));
        assert!(refinement_eye_moved(3012.0, 2200.0, 12.0));
        let mut eye = 3012.0;
        let mut rebuilds = 0;
        let mut focus = eye;
        while eye > 12.0 {
            eye -= 8.0;
            if refinement_eye_moved(focus, eye, 12.0) { focus = (eye / 64.0_f32).round() * 64.0; rebuilds += 1; }
        }
        assert!(rebuilds < 30, "descent rebuilt the base {rebuilds} times");
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
        let (mesh, _, _, _) = build(cfg, (0, 0), &cancel, Samples::new(),Default::default()).unwrap();
        println!(
            "cold horizon build {:.0} ms, {} vertices",
            start.elapsed().as_secs_f64() * 1e3,
            mesh.vertices.len()
        );
    }
}

#[cfg(test)]
mod continuity_contract {
    use super::*;
    use crate::{
        fluid::VoxelScalePreset,
        world::{BlockId, World, WorldType},
    };

    fn target(world: &mut World, eye: Vec3) -> BuildTarget {
        let snapshot = world.snapshot();
        BuildTarget {
            startup: false,
            collar:None,
            refinement_eye_y: (eye.y/64.0).round()*64.0,
            projection_pixels: 512.0,
            range_m: FAR_PLANE as u32,
            center: (0, 0),
            request_generation:0,
            source_revision:world.revision(),
            publication_limit: budget::BASE_LIMIT,
            fingerprint: edits::visible_fingerprint(&snapshot, eye, 128.0),
            snapshot,
            eye,
            hidden_radius: 128.0,
            omitted: Arc::default(),
            coverage: Arc::default(),
            clip: None,
            validation: Default::default(),
        }
    }

    #[test]
    fn reusable_base_credit_counts_real_capacity_once_and_rejects_other_recipes() {
        let key:BaseKey=(false,(0,0),1024,None,512.0f32.to_bits(),0.0f32.to_bits());
        let mut other=key;other.0=true;
        assert_eq!(matching_base_capacity(key,std::iter::empty()),0);
        assert_eq!(matching_base_capacity(key,[(other,999)].into_iter()),0);
        assert_eq!(matching_base_capacity(key,[(key,100),(key,100),(other,999)].into_iter()),100);
        let mut mesh=ChunkMesh {vertices:Vec::with_capacity(10),indices:Vec::with_capacity(12)};
        let allocated=mesh_capacity_bytes(&mesh);
        assert_eq!(allocated,mesh.vertices.capacity()*std::mem::size_of::<Vertex3>()+mesh.indices.capacity()*4);
        mesh.indices.push(0);assert_eq!(mesh_capacity_bytes(&mesh),allocated);
    }

    #[test]
    fn terminal_budget_failure_blocks_only_the_same_world_and_request() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let requested = target(&mut world, Vec3::new(0.0, 30.0, 0.0));
        let mut horizon = Horizon::default();
        horizon.desired = Some(requested.clone());
        horizon.budget_exhausted = Some(requested.clone());
        assert!(horizon.budget_exhausted());
        horizon.config = Some(world.sample_config());
        horizon.source_root = Some(world.regions.view.root.clone());
        assert!(horizon.budget_exhausted_for_limit(&world, budget::BASE_LIMIT));
        assert!(!horizon.budget_exhausted_for_limit(&world, budget::MAX_PUBLICATION_LIMIT));
        let replacement = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        assert!(!horizon.budget_exhausted_for_limit(&replacement, budget::BASE_LIMIT));
        let mut moved = requested.clone();
        moved.center = (512, 0);
        horizon.desired = Some(moved);
        assert!(!horizon.budget_exhausted());
        let mut other = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        horizon.desired = Some(target(&mut other, Vec3::new(0.0, 30.0, 0.0)));
        assert!(!horizon.budget_exhausted());
    }

    #[test]
    fn pressure_revocation_drops_only_unpublished_state_and_never_marks_ready() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let mut job = target(&mut world, Vec3::new(0.0, 30.0, 0.0));
        job.publication_limit = budget::MAX_PUBLICATION_LIMIT;
        let mut horizon = Horizon::default();
        horizon.active = Some(job.clone());
        horizon.desired = Some(job);
        horizon.upload_limit = budget::MAX_PUBLICATION_LIMIT;
        horizon.stop_expanded_upload(budget::Denied::GpuPressure);
        assert!(horizon.active.is_none() && horizon.upload.is_none());
        assert!(horizon.mesh.is_none() && horizon.published.is_none());
        assert!(!horizon.loaded_this_world);
        assert!(horizon.budget_pressure && horizon.budget_exhausted());
        let mut reduced = horizon.desired.clone().unwrap();
        reduced.publication_limit = budget::BASE_LIMIT;
        horizon.desired = Some(reduced);
        // Own freed allocation/telemetry lag may lower then raise allowance.
        // Neither observation permits the same failed scene to retry.
        assert!(horizon.budget_exhausted());
        let mut restored = horizon.desired.clone().unwrap();
        restored.publication_limit = budget::MAX_PUBLICATION_LIMIT;
        horizon.desired = Some(restored);
        assert!(horizon.budget_exhausted());
        assert!(horizon.mesh.is_none());
        let mut moved = horizon.desired.clone().unwrap();
        moved.center = (512,0);
        horizon.desired = Some(moved);
        assert!(!horizon.budget_exhausted());
    }

    #[test]
    fn changed_allowance_is_a_new_request_without_changing_geometry() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let requested = target(&mut world, Vec3::new(0.0, 30.0, 0.0));
        let mut enlarged = requested.clone();
        enlarged.publication_limit = budget::MAX_PUBLICATION_LIMIT;
        assert_eq!(enlarged.projection_pixels, requested.projection_pixels);
        assert_eq!(enlarged.range_m, requested.range_m);
        assert!(Arc::ptr_eq(&enlarged.snapshot.regions.root, &requested.snapshot.regions.root));
        assert!(!enlarged.matches(&requested));
        let mut horizon = Horizon::default();
        horizon.budget_exhausted = Some(requested);
        horizon.desired = Some(enlarged);
        assert!(!horizon.budget_exhausted());
    }

    #[test]
    fn omitted_edit_scope_is_revalidated_before_exposure() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let eye = Vec3::new(0.0, 30.0, 0.0);
        let mut job = target(&mut world, eye);
        job.omitted = Arc::new(vec![(Vec3::new(2.0, 40.0, 2.0), Vec3::new(4.0, 42.0, 4.0))]);
        assert!(job.valid_for_coverage(&world.snapshot(), eye, 128.0));
        assert!(!job.valid_for_coverage(&world.snapshot(), Vec3::new(300.0, 30.0, 0.0), 32.0));
    }

    #[test]
    fn local_simulation_does_not_invalidate_remote_job_but_remote_edit_does() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let job = target(&mut world, Vec3::new(0.0, 30.0, 0.0));
        world.set(IVec3::new(1, 20, 1), BlockId::Stone);
        assert!(job.valid_for(&world.snapshot()));
        world.set(IVec3::new(400, 20, 0), BlockId::Stone);
        assert!(!job.valid_for(&world.snapshot()));
    }

    #[test]
    fn partial_water_changes_are_local_content_dependencies() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let p = IVec3::new(400, 20, 0);
        world.set_water_volume(p, 2);
        let job = target(&mut world, Vec3::new(0.0, 30.0, 0.0));
        world.set_water_volume(IVec3::new(1, 20, 1), 6);
        assert!(job.valid_for(&world.snapshot()));
        world.set_water_volume(p, 6);
        assert!(!job.valid_for(&world.snapshot()));
    }

    #[test]
    fn valley_plan_probe_cannot_poison_canonical_column_cache() {
        let cfg = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 2738958700)
            .sample_config();
        let after_plan = std::thread::spawn(move || {
            distant_surface(&cfg, 0, 0);
            distant_surface(&cfg, -264, -448)
        })
        .join()
        .unwrap();
        let fresh_worker = std::thread::spawn(move || distant_surface(&cfg, -264, -448))
            .join()
            .unwrap();
        assert_eq!(
            after_plan, fresh_worker,
            "uncarved construction probes must not enter the final terrain cache"
        );
    }

    #[test]
    fn buried_cell_delta_is_not_a_whole_chunk_visibility_change() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let p = IVec3::new(40, 0, 112);
        world.set(p, BlockId::Water);
        let old = world.snapshot();
        world.set(p, BlockId::Air);
        let eye = Vec3::new(40.0, 40.0, 112.0);
        assert!(!edits::changes_visible(&old, &world.snapshot(), eye, 0.0));
        let edits: Vec<_> = (0..=16)
            .map(|y| (IVec3::new(40, y, 112), BlockId::Air))
            .collect();
        world.set_many(&edits);
        assert!(
            edits::changes_visible(&old, &world.snapshot(), eye, 0.0),
            "an opened shaft invalidates the old opaque-plane proof"
        );
    }

    #[test]
    fn first_hot_structural_edit_keeps_local_bounds_and_requires_refinement() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let old = world.snapshot();
        let p = IVec3::new(400,0,0);
        world.set(p, BlockId::Sand);
        let current = world.snapshot();
        let q = crate::world::chunk_coord_from_world_block(p);
        assert_eq!(current.chunks[&q].modified_bounds,
            Some((IVec3::new(16,0,0), IVec3::new(17,1,1))));
        assert_eq!(current.chunks[&q].structural_bounds,current.chunks[&q].modified_bounds);
        assert!(edits::changes_visible(&old, &current, Vec3::new(400.0,40.0,0.0), 0.0),
            "structural geometry remains represented before a new view exposes it");
        world.set(IVec3::new(400,40,0), BlockId::Stone);
        assert!(edits::changes_visible(&old, &world.snapshot(), Vec3::new(400.0,40.0,0.0), 0.0));
    }

    #[test]
    fn masked_structural_edit_requests_refinement_without_canceling_useful_cover() {
        let mut world=World::with_seed(WorldType::Superflat,VoxelScalePreset::OneMeter,7);
        let eye=Vec3::new(0.0,30.0,0.0);
        let old=target(&mut world,eye);
        world.set(IVec3::new(1,20,1),BlockId::Stone);
        let desired=target(&mut world,eye);
        assert!(old.valid_for(&world.snapshot()),"resident cover permits the old job to finish");
        assert!(!old.matches(&desired),"the future parent still needs the structural edit");
        let old=world.snapshot();
        world.set_water_volume(IVec3::new(400,40,0),2);
        let current=world.snapshot();
        let q=crate::world::chunk_coord_from_world_block(IVec3::new(400,40,0));
        assert!(current.chunks[&q].structural_bounds.is_none());
        assert!(!edits::structural_changed(&old,&current,(0,0),&ResidentCoverage::default()),"fluid microstate is a separate dependency class");
    }

    #[test]
    fn resident_structural_edit_waits_for_its_column_to_leave_residency() {
        let mut world=World::with_seed(WorldType::Superflat,VoxelScalePreset::OneMeter,7);
        let eye=Vec3::new(0.0,30.0,0.0);
        let published=target(&mut world,eye);
        world.set(IVec3::new(1,20,1),BlockId::Stone);
        let mut resident=ResidentCoverage { header:[-1,-1,3,1], ..Default::default() };
        for z in -1..=1 { for x in -1..=1 { assert!(resident.set_column(x,z)); } }
        let mut desired=target(&mut world,eye);
        desired.coverage=Arc::new(resident);
        assert!(published.matches(&desired),"the resident mesh draws the edit");
        let mut desired=target(&mut world,eye);
        resident.header[3]=0;
        desired.coverage=Arc::new(resident);
        assert!(!published.matches(&desired),"leaving residency exposes the edit to the cover");
    }

    #[test]
    fn full_water_depth_owner_proof_tracks_fractional_volume_changes() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let cells: Vec<_> = (24..40).flat_map(|y| (-1..=2).flat_map(move |z|
            (-1..=2).map(move |x| (IVec3::new(x,y,z), BlockId::Water)))).collect();
        world.set_many(&cells);
        let omitted = vec![(Vec3::new(0.0,20.0,0.0), Vec3::new(1.0,22.0,1.0))];
        let eye = Vec3::new(0.5,42.0,0.5);
        assert!(edits::scope_covers(&omitted, &world.snapshot(), eye, 0.0));
        for &(p, _) in &cells { world.set_water_volume(p, 4); }
        assert!(edits::scope_covers(&omitted,&world.snapshot(),eye,0.0),
            "submerged fractional cells still use full-height rendered geometry");
        for (p, _) in cells { if p.y>24 {world.set(p,BlockId::Air);} }
        assert!(!edits::scope_covers(&omitted, &world.snapshot(), eye, 0.0),
            "cached submerged proof must not survive drainage to a free fractional surface");
    }

    #[test]
    fn world_switch_rejects_same_seed_different_world_provenance() {
        let mut a = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let mut b = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let job = target(&mut a, Vec3::ZERO);
        assert!(!job.valid_for(&b.snapshot()));
        assert!(job.valid_for(&a.snapshot()));
    }

    #[test]
    fn whole_match_memo_rechecks_world_edits_and_published_owner_changes() {
        let mut world=World::with_seed(WorldType::Superflat,VoxelScalePreset::OneMeter,7);
        let eye=Vec3::new(0.5,80.0,0.5);
        let mut job=target(&mut world,eye);let mut desired=target(&mut world,eye);
        let mut owned=ResidentCoverage::default();owned.header=[0,0,1,1];owned.set_column(0,0);
        let owned=Arc::new(owned);
        job.coverage=owned.clone();job.omitted=Arc::new(vec![(Vec3::ZERO,Vec3::splat(8.0))]);desired.coverage=owned.clone();
        assert!(job.matches(&desired));assert!(job.matches(&desired));
        assert!(job.validation.borrow()[1].is_some());
        Arc::make_mut(&mut desired.coverage).words=[[0;4];131];
        assert!(!job.matches(&desired),"losing a required owner invalidates an otherwise identical memo key");
        desired.coverage=owned.clone();assert!(job.matches(&desired));
        let mut other=World::with_seed(WorldType::Superflat,VoxelScalePreset::OneMeter,7);
        let foreign=target(&mut other,eye);assert!(!job.matches(&foreign));
        assert!(job.matches(&desired),"a cached false result from a different world cannot poison this one");
        world.set(IVec3::new(400,40,0),BlockId::Stone);
        let edited=target(&mut world,eye);assert!(!job.matches(&edited),"a newer structural dependency cannot reuse the old match");
    }

    #[test]
    fn cpu_provenance_does_not_embed_large_gpu_ownership_payloads() {
        assert!(std::mem::size_of::<ValidationMemo>()<512);
        assert!(std::mem::size_of::<BuildTarget>()<2048);
        assert!(std::mem::size_of::<Horizon>()<16*1024,"Windows main-stack initialization must stay bounded");
    }

    #[test]
    fn sparse_only_owner_changes_invalidate_both_validation_memos() {
        let mut world=World::with_seed(WorldType::Superflat,VoxelScalePreset::OneMeter,7);
        let eye=Vec3::new(4096.5,80.0,0.5);
        let mut job=target(&mut world,eye);let mut desired=target(&mut world,eye);
        let mut owned=ResidentCoverage::default();owned.header=[0,0,1,1];assert!(owned.set_column(128,0));
        let owned=Arc::new(owned);
        job.coverage=owned.clone();desired.coverage=owned.clone();
        job.omitted=Arc::new(vec![(Vec3::new(4096.0,72.0,0.0),Vec3::new(4104.0,79.0,8.0))]);
        let snapshot=world.snapshot();
        assert!(job.matches(&desired));
        assert!(job.valid_for_resident(&snapshot,eye,0.0,&owned));
        let mut unowned=*owned;unowned.sparse_header=[0;4];
        assert_eq!(owned.header,unowned.header);assert_eq!(owned.words,unowned.words);
        desired.coverage=Arc::new(unowned);
        assert!(!job.matches(&desired),"sparse-only owner loss cannot reuse the whole-match memo");
        assert!(!job.valid_for_resident(&snapshot,eye,0.0,&unowned),"nor can it reuse the source/scope memo");
        desired.coverage=owned.clone();assert!(job.matches(&desired));
    }

    #[test]
    fn built_center_provenance_is_not_relabelled_for_desired_center() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let a = target(&mut world, Vec3::ZERO);
        let mut b = a.clone();
        b.center = (512, 0);
        assert!(!a.matches(&b));
        assert!(a.valid_for(&world.snapshot()));
        assert_eq!(a.center, (0, 0));
        assert!(!a.too_far_from(Vec3::new(512.0, 0.0, 0.0)));
    }

    #[test]
    fn guard_ring_encloses_retained_center_view_and_shared_edges() {
        assert!(ring_step(LEVELS - 1) * HALF as f32 > FAR_PLANE + MAX_CENTER_DISTANCE);
        for level in 0..LEVELS {
            for z in 0..SIDE - 1 {
                for x in 0..SIDE - 1 {
                    let step = ring_step(level);
                    let dx = ((x as f32 - HALF as f32 + 0.5).abs() * step - step * 0.5).max(0.0);
                    let dz = ((z as f32 - HALF as f32 + 0.5).abs() * step - step * 0.5).max(0.0);
                    if dx.hypot(dz) > FAR_PLANE + MAX_CENTER_DISTANCE * 2.0_f32.sqrt() + step {
                        continue;
                    }
                    for (a, b) in [(x, z), (x + 1, z), (x, z + 1), (x + 1, z + 1)] {
                        assert!(sample_in_domain(
                            (a as f32 - HALF as f32) * step,
                            (b as f32 - HALF as f32) * step,
                            step,
                            domain_reach(FAR_PLANE as u32)
                        ));
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "bounded full horizon A/B/A cache-order exercise; run explicitly"]
    fn retained_and_fresh_cache_are_identical_in_both_center_orders() {
        let cfg = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 2738958700)
            .sample_config();
        let cancel = AtomicBool::new(false);
        let (_, a, _, _) = build(cfg, (0, 0), &cancel, Samples::new(),Default::default()).unwrap();
        let (_, b_from_a, _, _) = build(cfg, (512, 0), &cancel, a.clone(),Default::default()).unwrap();
        let (_, b_fresh, _, _) = build(cfg, (512, 0), &cancel, Samples::new(),Default::default()).unwrap();
        let mismatches: Vec<_> = b_from_a
            .iter()
            .filter(|(key, value)| b_fresh.get(key) != Some(value))
            .take(8)
            .collect();
        assert!(
            mismatches.is_empty() && b_from_a.len() == b_fresh.len(),
            "bounded cache mismatches: {mismatches:?}"
        );
        let (_, a_from_b, _, _) = build(cfg, (0, 0), &cancel, b_from_a,Default::default()).unwrap();
        let mismatches: Vec<_> = a
            .iter()
            .filter(|(key, value)| a_from_b.get(key) != Some(value))
            .take(8)
            .collect();
        assert!(
            mismatches.is_empty() && a.len() == a_from_b.len(),
            "bounded return-cache mismatches: {mismatches:?}"
        );
        println!(
            "[horizon-cache] A/B/A fresh/retained equal samples_a={} samples_b={}",
            a.len(),
            b_fresh.len()
        );
    }

    #[test]
    #[ignore = "actual Vulkan upload/provenance stress; run explicitly, serially"]
    fn gpu_publication_rejects_edit_and_world_switch_at_both_stages() {
        let adapter = engine_gpu::test_adapter();
        println!("[horizon-gpu-contract] adapter={:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let mut other = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 8);
        let make = |world: &mut World| {
            let job = target(world, Vec3::ZERO);
            let v = Vertex3 {
                pos: [0.0; 3],
                color: [1.0; 3],
                normal: [0.0, 1.0, 0.0],
                surface: [0.9, 0.0, 0.0, 0.0],
                ao_strength: -1.0,
            };
            let page = pages::Page {
                mesh: ChunkMesh {
                    vertices: vec![v; 3],
                    indices: vec![0, 1, 2],
                },
                batches: Vec::new(),
            };
            let (tx, rx) = mpsc::sync_channel(1);
            tx.send(BuildEvent::Complete(vec![page], Arc::default(), Vec::new(), budget::BASE_LIMIT))
                .unwrap();
            let mut horizon = Horizon::default();
            horizon.config = Some(world.sample_config());
            horizon.source_root = Some(world.regions.view.root.clone());
            horizon.active = Some(job);
            horizon.pending = Some(rx);
            // Keep the scheduling path occupied; these tests supply their own
            // tiny immutable completed job rather than spending seconds on terrain.
            horizon.worker = Some(std::thread::spawn(|| {
                std::thread::sleep(Duration::from_secs(1))
            }));
            horizon
        };
        unsafe {
            std::env::set_var("VOXEL_HORIZON_UPLOAD_BYTES", "4");
        }
        for stage in ["completed_cpu", "pending_upload"] {
            for switch in [false, true] {
                let mut horizon = make(&mut world);
                if stage == "pending_upload" {
                    let revision = world.revision();
                    horizon.update(&device, &queue, &mut world, revision, Vec3::ZERO, 128.0, false);
                    assert!(horizon.upload.is_some());
                    assert!(horizon.mesh.is_none());
                }
                if switch {
                    let revision = other.revision();
                    horizon.update(&device, &queue, &mut other, revision, Vec3::ZERO, 128.0, false);
                } else {
                    world.set(
                        IVec3::new(400, 20, 0),
                        if world.get(IVec3::new(400, 20, 0)) == BlockId::Stone {
                            BlockId::Air
                        } else {
                            BlockId::Stone
                        },
                    );
                    let revision = world.revision();
                    horizon.update(&device, &queue, &mut world, revision, Vec3::ZERO, 128.0, false);
                }
                queue.submit([]);
                assert!(horizon.upload.is_none());
                assert!(horizon.mesh.is_none());
                println!("[horizon-gpu-contract] rejected stage={stage} world_switch={switch}");
            }
        }
        let mut horizon = make(&mut world);
        for _ in 0..100 {
            let revision = world.revision();
            horizon.update(
                &device,
                &queue,
                &mut world,
                revision,
                Vec3::new(512.0, 30.0, 0.0),
                128.0,
                false,
            );
            queue.submit([]);
            if horizon.mesh.is_some() {
                break;
            }
        }
        assert_eq!(horizon.published.as_ref().unwrap().center, (0, 0));
        assert_eq!(horizon.desired.as_ref().unwrap().center, (512, 0));
        assert!(horizon.rejected_stale == 0);
        unsafe {
            std::env::remove_var("VOXEL_HORIZON_UPLOAD_BYTES");
        }
        println!(
            "[horizon-gpu-contract] A result published at actual A footprint while B desired; constrained upload complete"
        );
    }
}

#[cfg(test)]
mod seam_measure {
    use crate::fluid::VoxelScalePreset;
    use crate::world::{World, WorldType, distant_surface};

    /// `cargo test --release seam_measure -- --ignored --nocapture`
    /// Height step where a finer ring meets a coarser one. The coarse edge is
    /// linearly interpolated between its nodes; the fine ring has real nodes
    /// along the same line.
    #[test]
    #[ignore]
    fn ring_boundary_steps() {
        let cfg = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 7).sample_config();
        for coarse in [8i32, 16, 32] {
            let fine = coarse / 2;
            let (mut worst, mut sum, mut n) = (0.0f32, 0.0f32, 0);
            // Boundary line x = 1000 m, z sweeps 0..1000 m.
            for zi in 0..(1000 / fine) {
                let z = zi * fine;
                let a = distant_surface(&cfg, 1000, z).0;
                let b = distant_surface(&cfg, 1000, z + fine).0;
                let _ = (a, b);
            }
            // Coarse nodes every `coarse`; sample the fine nodes between them.
            for zi in 0..(1000 / coarse) {
                let z0 = zi * coarse;
                let z1 = z0 + coarse;
                let h0 = distant_surface(&cfg, 1000, z0).0;
                let h1 = distant_surface(&cfg, 1000, z1).0;
                let mut z = z0 + fine;
                while z < z1 {
                    let t = (z - z0) as f32 / coarse as f32;
                    let lerp = h0 + (h1 - h0) * t;
                    let exact = distant_surface(&cfg, 1000, z).0;
                    let d = (lerp - exact).abs();
                    worst = worst.max(d);
                    sum += d;
                    n += 1;
                    z += 2 * fine;
                }
            }
            println!(
                "coarse {coarse} m vs fine {fine} m: mean step {:.2} m, worst {:.2} m ({n} nodes)",
                sum / n as f32,
                worst
            );
        }
    }
}


#[cfg(test)]
mod refinement_contract {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn interior_refinement_keeps_external_edges_and_exact_water_coverage() {
        let world=crate::world::World::new(crate::world::WorldType::Hills,crate::fluid::VoxelScalePreset::OneMeter);
        let cfg=world.sample_config();
        let surface=cfg.water_level() as f32+1.0-crate::mesh::WATER_SURFACE_INSET;
        for island in [true,false] {
            let corner=surface+if island {-8.0}else{8.0};
            let middle=surface+if island {8.0}else{-8.0};
            let points=[Vec3::new(-8.0,corner,-8.0),Vec3::new(8.0,corner,-8.0),
                Vec3::new(-8.0,corner,8.0),Vec3::new(8.0,corner,8.0),Vec3::new(0.0,middle,0.0)];
            let (triangles,count)=horizon_refinement_triangles([0,1,2,3],[corner;4],None,Some(4));
            let mut edges=BTreeMap::<(u32,u32),(usize,i32)>::new();
            let mut area=0.0;
            for tri in &triangles[..count] {
                let [a,b,c]=tri.map(|i|points[i as usize]);
                let normal=(b-a).cross(c-a);
                assert!(normal.y>0.0);
                area+=normal.y*0.5;
                for e in 0..3 {
                    let (a,b)=(tri[e],tri[(e+1)%3]);
                    let value=edges.entry((a.min(b),a.max(b))).or_default();
                    value.0+=1;value.1+=if a<b {1}else{-1};
                }
            }
            assert!((area-256.0).abs()<0.001);
            let boundary:Vec<_>=edges.iter().filter(|(_,v)|v.0==1).map(|(k,_)|*k).collect();
            assert_eq!(boundary,vec![(0,1),(0,2),(1,3),(2,3)],"no new neighbour/ring edge knots");
            assert!(edges.values().all(|&(count,orientation)|count==1 || (count==2&&orientation==0)));
            let mut mesh=ChunkMesh::empty();
            for point in points {
                mesh.vertices.push(Vertex3 {pos:point.to_array(),color:[0.3;3],normal:Vec3::Y.to_array(),surface:[0.8,0.04,0.0,1.0],ao_strength:-1.0});
            }
            mesh.indices=triangles[..count].iter().flatten().copied().collect();
            let land_count=mesh.indices.len();
            water::append(&mut mesh,&cfg);
            let water_area:f32=mesh.indices[land_count..].chunks_exact(3).map(|tri| {
                let [a,b,c]=[tri[0],tri[1],tri[2]].map(|i|Vec3::from_array(mesh.vertices[i as usize].pos));
                assert!((a.y-surface).abs()<0.0001&&(b.y-surface).abs()<0.0001&&(c.y-surface).abs()<0.0001);
                (b-a).cross(c-a).y.abs()*0.5
            }).sum();
            // Slope is 16 voxels over the half-width 8, so the clip at
            // `surface - COAST_BIAS_VOXELS` sits (8 - bias)/16 of the way from
            // the low knot to the high one, and area scales with its square.
            let wet_reach=(8.0-water::COAST_BIAS_VOXELS)/16.0;
            let expected=256.0*if island {1.0-(1.0-wet_reach)*(1.0-wet_reach)}else{wet_reach*wet_reach};
            assert!((water_area-expected).abs()<0.002,"canonical dry island/wet pond must survive without a duplicate full sheet");
        }
    }

    #[test]
    fn interior_refinement_preserves_existing_split_edge_triangles() {
        for edge in [[0,1,5],[0,2,5],[1,3,5],[2,3,5]] {
            let (old,n)=horizon_cell_triangles([0,1,2,3],[0.0,1.0,3.0,2.0],Some(edge));
            let (refined,m)=horizon_refinement_triangles([0,1,2,3],[0.0,1.0,3.0,2.0],Some(edge),Some(4));
            assert_eq!(&old[..n],&refined[..m]);
        }
    }
}

#[cfg(test)]
mod retry_wait_tests {
    use super::*;

    #[test]
    fn repeated_failures_back_off_then_cap() {
        let seconds = |streak| retry_wait(streak).as_secs();
        assert_eq!(seconds(1), 2);
        assert_eq!(seconds(2), 4);
        assert_eq!(seconds(3), 8);
        assert_eq!(seconds(4), 16);
        assert_eq!(seconds(5), 30);
        assert_eq!(seconds(50), 30);
    }
}
