//! Optional exact columns. Persistent edits use their existing higher-priority lane.
//! This module plans immutable source cover; it never mutates/wakes the world.
use super::{ChunkCache, ChunkKey};
use crate::world::{ChunkData, WorldSampleConfig, WorldSnapshot, CHUNK_SIZE};
use glam::IVec3;
use std::{collections::{HashMap, HashSet}, path::PathBuf, sync::{Arc, Weak}, time::{Duration, Instant}};
#[path = "quality_plan.rs"]
mod planning;
#[cfg(test)]
#[path = "quality_plan_integration_tests.rs"]
mod plan_tests;

pub(super) const COLUMN_LIMIT: usize = 16;
pub(super) const BYTE_LIMIT: usize = 128 * 1024 * 1024;
const RELEASE_DELAY: Duration = Duration::from_secs(2);
const ENQUEUE_LIMIT: usize = 64;

struct Dependency {
    coord: IVec3,
    chunk: Option<Weak<ChunkData>>,
    record: Option<(u64,u64,u32,u64,u64)>,
}
impl Dependency {
    fn capture(snapshot: &WorldSnapshot, coord: IVec3) -> Self {
        Self {coord, chunk:snapshot.chunks.get(&coord).map(Arc::downgrade),
            record:snapshot.regions.records.get(&coord).map(|r|(r.generation,r.offset,r.len,r.checksum,r.version))}
    }
    fn matches(&self, snapshot: &WorldSnapshot) -> bool {
        let chunk = match (&self.chunk,snapshot.chunks.get(&self.coord)) {
            (Some(old),Some(new))=>Weak::ptr_eq(old,&Arc::downgrade(new)), (None,None)=>true, _=>false,
        };
        // Disk writeback does not change an unchanged resident mesh source.
        chunk && (self.chunk.is_some() || self.record == snapshot.regions.records.get(&self.coord)
            .map(|r|(r.generation,r.offset,r.len,r.checksum,r.version)))
    }
}
struct Column {
    since: Instant,
    unwanted_since: Option<Instant>,
    source_changed: Instant,
    members: Vec<IVec3>,
    dependencies: Vec<Dependency>,
    admitted: bool,
    planned: Option<(u64, super::visibility::ColumnRange)>,
}
#[derive(Default)]
pub(super) struct QualityProxies {
    cfg: Option<WorldSampleConfig>,
    root: Option<Arc<PathBuf>>,
    cache_epoch: u64,
    columns: HashMap<(i32,i32),Column>,
    pub(super) coords: HashSet<IVec3>,
    previous_water: Option<Arc<crate::world::WaterSnapshot>>,
    /// `ChunkCache::quality_sync_inputs` after the last visible-set rebuild.
    sync_key: Option<QualitySyncInputs>,
    plans: planning::Inputs,
}

/// What the membership sync's visible-set rebuild reads that can change
/// without a rebuild of its own: quality membership and admission, the mesh
/// set (installs, replacements and removals bump the revision), and the dirty,
/// pending and water-dirty state that gates admission.
type QualitySyncInputs = (u64, u64, usize, usize, usize);

impl QualityProxies {
    pub(super) fn columns(&self) -> impl Iterator<Item=(i32,i32)> + '_ {self.columns.keys().copied()}
    pub(super) fn desired_columns(&self) -> impl Iterator<Item=((i32,i32),&[IVec3])> + '_ {
        self.columns.iter().filter(|(_,c)|!c.members.is_empty()).map(|(&q,c)|(q,c.members.as_slice()))
    }
    pub(super) fn admitted_columns(&self) -> HashSet<(i32,i32)> {
        self.columns.iter().filter(|(_,c)|c.admitted).map(|(&q,_)|q).collect()
    }
    pub(super) fn pending_since(&self, q: &IVec3) -> Option<Instant> {
        self.columns.get(&(q.x,q.z)).filter(|c|!c.admitted && c.members.contains(q)).map(|c|c.since)
    }
    pub(super) fn mark_admitted(&mut self, q: (i32,i32)) {
        if let Some(column)=self.columns.get_mut(&q) {
            if !column.admitted {
                println!("[quality-column] published={q:?} members={} age_ms={}",column.members.len(),column.since.elapsed().as_millis());
            }
            column.admitted=true;
        }
    }
    pub(super) fn contains_column(&self, q: &(i32,i32)) -> bool {self.columns.contains_key(q)}
    fn rebuild_coords(&mut self) {self.coords=self.columns.values().flat_map(|c|c.members.iter().copied()).collect();}
}

fn dependencies(snapshot: &WorldSnapshot, column: (i32,i32), members: &[IVec3]) -> Vec<Dependency> {
    let mut coords=HashSet::new();
    for q in members {
        for y in -1..=1 {for z in -1..=1 {for x in -1..=1 {
            if let (Some(x),Some(y),Some(z))=(q.x.checked_add(x),q.y.checked_add(y),q.z.checked_add(z)) {
                coords.insert(IVec3::new(x,y,z));
            }
        }}}
    }
    // Include known vertical edits outside the previous geometric Y range.
    coords.extend(snapshot.regions.records.nearby(IVec3::new(column.0,0,column.1),1).copied());
    coords.extend(snapshot.chunks.keys().filter(|q|q.x.abs_diff(column.0)<=1 && q.z.abs_diff(column.1)<=1).copied());
    let mut coords:Vec<_>=coords.into_iter().collect();coords.sort_unstable_by_key(|q|(q.x,q.z,q.y));
    coords.into_iter().map(|q|Dependency::capture(snapshot,q)).collect()
}
fn has_edits(snapshot: &WorldSnapshot, deps: &[Dependency]) -> bool {
    deps.iter().any(|d|d.record.is_some() || snapshot.chunks.get(&d.coord)
        .is_some_and(|c|c.modified_bounds.is_some() || c.structural_bounds.is_some()))
}

impl ChunkCache {
    /// Current non-promoted records, including retained/pending cover. Root uses
    /// this exact set to prove a canonical parent before allowing retirement.
    /// Parent proof is only consumed by retirement, never by admission.
    pub fn quality_parent_validation_needed(&self) -> bool {
        let now = Instant::now();
        self.quality_proxies.columns.values().any(|column|
            column.unwanted_since.is_some_and(|since| now.duration_since(since) >= RELEASE_DELAY)
                && now.duration_since(column.source_changed) >= RELEASE_DELAY)
    }

    pub fn quality_columns(&self) -> Vec<(i32,i32)> {
        let mut columns:Vec<_>=self.quality_proxies.columns().collect();columns.sort_unstable();columns
    }
    pub(crate) fn quality_column_iter(&self) -> impl ExactSizeIterator<Item=(i32,i32)> + '_ {
        self.quality_proxies.columns.keys().copied()
    }
    pub(super) fn quality_charge_coords(&self) -> HashSet<IVec3> {
        self.quality_proxies.coords.iter().filter(|q|!self.edit_proxies.persistent_wanted(&(q.x,q.z))).copied().collect()
    }
    pub(super) fn quality_mesh_fits(&self, key:ChunkKey, bytes:usize, reserved:bool) -> bool {
        if !self.quality_proxies.coords.contains(&key.coord)
            || self.edit_proxies.persistent_wanted(&(key.coord.x, key.coord.z)) {
            return true;
        }
        let coords=self.quality_charge_coords();
        let installed:usize=coords.iter().filter_map(|q|self.gpu_meshes.get(q)).map(|m|m.bytes).sum();
        // This sub-budget describes the post-publication quality set. The
        // global reservation separately bounds old/new upload overlap.
        installed.saturating_sub(self.gpu_meshes.get(&key.coord).map_or(0,|m|m.bytes))
            .saturating_add(self.pending_gpu.bytes_matching(&coords))
            .saturating_add(if reserved {0}else{bytes})<=BYTE_LIMIT
    }
    fn sync_quality_membership(&mut self,snapshot:&WorldSnapshot,streamer:&mut super::ChunkStreamer) {
        self.quality_proxies.rebuild_coords();
        self.edit_proxies.set_quality_sources(self.quality_proxies.coords.clone(),
            self.quality_proxies.columns().collect(), self.quality_proxies.admitted_columns());
        streamer.invalidate_changed_sources(snapshot,self);
        // The rebuild walks every visible chunk. Skip it while none of its
        // inputs moved; other changes to the visible sets rebuild or flag
        // publication themselves.
        if self.publication_dirty || self.quality_proxies.sync_key != Some(self.quality_sync_inputs()) {
            self.rebuild_visible_sets();
            self.quality_proxies.sync_key = Some(self.quality_sync_inputs());
        }
    }
    fn quality_sync_inputs(&self) -> QualitySyncInputs {
        use std::hash::{Hash, Hasher};
        let mut columns:Vec<_>=self.quality_proxies.columns.iter().map(|(&q,c)|(q,c.admitted,&c.members)).collect();
        columns.sort_unstable_by_key(|c|c.0);
        let mut hasher=std::collections::hash_map::DefaultHasher::new();
        columns.hash(&mut hasher);
        (hasher.finish(), self.gpu_mesh_revision, self.dirty_meshes.len(), self.pending_gpu.len(),
            self.edit_proxies.water_dirty.len())
    }
    /// Call after current dirty/halo invalidation, using the same immutable
    /// snapshot as source-only meshing. Recommendations contain no authority.
    pub fn set_quality_columns(&mut self, snapshot:&WorldSnapshot, wanted:&[(i32,i32)], canonical_parent_valid:bool,streamer:&mut super::ChunkStreamer) -> Vec<ChunkKey> {
        let now=Instant::now();
        let same_root=self.quality_proxies.root.as_ref().is_none_or(|r|Arc::ptr_eq(r,&snapshot.regions.root));
        let same_cfg=self.quality_proxies.cfg.is_none_or(|c|c==snapshot.cfg);
        if (!same_root || !same_cfg) && self.quality_proxies.cache_epoch==self.gpu_world_epoch {
            // A world change must reset the cache/streamer before reusing GPU
            // identities; matching coordinates alone cannot establish provenance.
            return Vec::new();
        }
        if self.quality_proxies.cache_epoch!=self.gpu_world_epoch {
            self.quality_proxies=QualityProxies::default();
        }
        self.quality_proxies.cfg=Some(snapshot.cfg);
        self.quality_proxies.root=Some(Arc::clone(&snapshot.regions.root));
        self.quality_proxies.cache_epoch=self.gpu_world_epoch;
        let plan_revision = self.quality_proxies.plans.observe(snapshot);
        let mut wanted_order=Vec::new();let mut seen=HashSet::new();
        for &q in wanted {
            if wanted_order.len()==COLUMN_LIMIT {break;}
            if q.0.checked_mul(CHUNK_SIZE).and_then(|v|v.checked_add(CHUNK_SIZE)).is_none()
                || q.1.checked_mul(CHUNK_SIZE).and_then(|v|v.checked_add(CHUNK_SIZE)).is_none()
                || q.0.checked_mul(CHUNK_SIZE).and_then(|v|v.checked_sub(CHUNK_SIZE)).is_none()
                || q.1.checked_mul(CHUNK_SIZE).and_then(|v|v.checked_sub(CHUNK_SIZE)).is_none() {continue;}
            if seen.insert(q) {wanted_order.push(q);}
        }
        self.edit_proxies.quality_requested = seen.clone();
        let water_changed:HashSet<_>=self.quality_proxies.previous_water.as_ref().map_or_else(HashSet::new,|old|
            snapshot.water_levels.changed_chunks(old)
                .filter(|(q,_,_)| *crate::AB_OLD || snapshot.water_levels.surface_changed(old, *q, snapshot.cfg.voxel_scale))
                .map(|(q,_,_)|q).collect());
        self.quality_proxies.previous_water=Some(Arc::clone(&snapshot.water_levels));
        let mut retire=Vec::new();
        for (&key,c) in &mut self.quality_proxies.columns {
            if seen.contains(&key) {c.unwanted_since=None;} else {c.unwanted_since.get_or_insert(now);}
            let local_water=c.dependencies.iter().any(|d|water_changed.contains(&d.coord));
            let fresh=c.dependencies.iter().all(|d|d.matches(snapshot)) && !local_water;
            if !fresh {c.source_changed=now;}
            if canonical_parent_valid && fresh && c.unwanted_since.is_some_and(|t|now.duration_since(t)>=RELEASE_DELAY)
                && now.duration_since(c.source_changed)>=RELEASE_DELAY {retire.push(key);}
        }
        let mut retired_coords=HashSet::new();
        for key in retire {
            if let Some(c)=self.quality_proxies.columns.remove(&key) {retired_coords.extend(c.members);}
        }
        self.quality_proxies.rebuild_coords();
        for key in wanted_order {
            if self.quality_proxies.columns.len()>=COLUMN_LIMIT {break;}
            if self.edit_proxies.persistent_wanted(&key) {continue;}
            let shared:HashSet<_>=self.edit_proxies.columns().chain(self.horizon_pins.iter().copied())
                .chain(self.quality_proxies.columns()).collect();
            if !shared.contains(&key) && shared.len()>=super::edit_proxies::COLUMN_LIMIT {continue;}
            self.quality_proxies.columns.entry(key).or_insert_with(||Column {
                since:now,unwanted_since:None,source_changed:now,members:Vec::new(),dependencies:Vec::new(),admitted:false,planned:None});
        }
        let interest=self.edit_proxies.columns().chain(self.quality_proxies.columns()).collect();
        self.column_y_cache.set_supplemental_interest(interest);
        self.column_y_cache.begin(snapshot.cfg);
        let floor=snapshot.cfg.min_world_y().div_euclid(CHUNK_SIZE);
        let ceiling=(snapshot.cfg.max_world_y()-1).div_euclid(CHUNK_SIZE);
        let crown=(26.0/(CHUNK_SIZE as f32*snapshot.cfg.voxel_meters())).ceil() as i32+1;
        let columns:Vec<_>=self.quality_proxies.columns().collect();
        let mut promotions=Vec::new();
        for key in columns {
            let Some(range)=self.column_y_cache.range(key) else {continue;};
            // GPU progress still runs below. Only unchanged source planning is reused.
            if self.quality_proxies.columns[&key].planned == Some((plan_revision, range))
                && !self.edit_proxies.persistent_wanted(&key) { continue; }
            let mut low=range.0.max(floor);let mut high=range.1.min(ceiling);
            let known=dependencies(snapshot,key,&[]);
            for d in &known {
                low=low.min(d.coord.y.saturating_sub(1).max(floor));
                high=high.max(d.coord.y.saturating_add(crown).min(ceiling));
            }
            let members:Vec<_>=(low..=high).map(|y|IVec3::new(key.0,y,key.1)).collect();
            let deps=dependencies(snapshot,key,&members);
            let partial_water=deps.iter().any(|d|water_changed.contains(&d.coord))
                || members.iter().any(|q|snapshot.water_levels.in_box(*q*CHUNK_SIZE,(*q+IVec3::ONE)*CHUNK_SIZE).next().is_some());
            if self.edit_proxies.persistent_wanted(&key) || has_edits(snapshot,&deps) || partial_water {
                promotions.push((key,members));continue;
            }
            let mut proposed=self.quality_proxies.coords.clone();proposed.extend(members.iter().copied());
            proposed.retain(|q|!self.edit_proxies.persistent_wanted(&(q.x,q.z)));
            let active:usize=proposed.iter().filter_map(|q|self.gpu_meshes.get(q)).map(|m|m.bytes).sum();
            if active.saturating_add(self.pending_gpu.bytes_matching(&proposed))>BYTE_LIMIT {continue;}
            let c=self.quality_proxies.columns.get_mut(&key).unwrap();
            if c.dependencies.len()!=deps.len() || !c.dependencies.iter().all(|d|d.matches(snapshot)) {c.source_changed=now;}
            c.members=members;c.dependencies=deps;
            c.planned=Some((plan_revision,range));
            self.quality_proxies.rebuild_coords();
        }
        for (key,members) in promotions {
            let admitted=self.quality_proxies.columns.get(&key).is_some_and(|c|c.admitted);
            self.edit_proxies.promote_quality_column(key,&members,admitted,now);
            self.quality_proxies.columns.remove(&key);
        }
        // A same-LOD dirty replacement can complete without changing the mesh
        // set. It still bumps gpu_mesh_revision, so the membership sync
        // rebuilds the visible sets and republishes the column.
        self.sync_quality_membership(snapshot,streamer);
        // Once the canonical parent owns a retired column, release only its
        // exclusively quality resources. Ordinary/edit/handoff owners win.
        let mut released=false;
        for q in &retired_coords {
            if self.source_interested(q) || self.horizon_pin_coords.contains(q) {continue;}
            released|=self.gpu_meshes.remove(q).is_some();
            self.cpu_chunks.remove(q);self.dirty_meshes.remove(q);
        }
        if released {
            self.gpu_bytes=self.gpu_meshes.values().map(|m|m.bytes).sum();
            self.mark_mesh_set_changed();
        }
        if !retired_coords.is_empty() {
            let retained:HashSet<_>=self.visible_coords.iter().chain(self.published_coords.iter())
                .chain(self.edit_proxies.coords.iter()).chain(self.horizon_pin_coords.iter()).copied().collect();
            self.lod_meshes.retain(|q|!retired_coords.contains(&q) || retained.contains(&q));
        }
        let charge=self.quality_charge_coords();
        let occupied=charge.iter().filter_map(|q|self.gpu_meshes.get(q)).map(|m|m.bytes).sum::<usize>()
            .saturating_add(self.pending_gpu.bytes_matching(&charge));
        let mut keys:Vec<_>=self.quality_proxies.coords.iter().filter(|q|
            (self.gpu_meshes.get(*q).is_none_or(|m|m.lod!=0) || self.dirty_meshes.contains(*q))
            && self.budget_deferred.get(&ChunkKey{coord:**q,lod:0})
                .is_none_or(|bytes|occupied.saturating_add(*bytes)<=BYTE_LIMIT))
            .map(|&coord|ChunkKey{coord,lod:0}).collect();
        keys.sort_unstable_by_key(|key|(self.quality_proxies.pending_since(&key.coord),key.coord.x,key.coord.z,key.coord.y));
        keys.truncate(ENQUEUE_LIMIT);keys
    }
}

#[cfg(test)]
mod writeback_tests {
    use super::*;
    use crate::{fluid::VoxelScalePreset, world::{BlockId, World, WorldType}};

    #[test]
    fn quality_source_checks_voxels_before_disk_record() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let p = IVec3::new(1, 1, 1);
        world.set(p, BlockId::Air);
        let snapshot = world.snapshot();
        let mut dependency = Dependency::capture(&snapshot, IVec3::ZERO);
        assert!(dependency.chunk.is_some());
        dependency.record = Some((123, 456, 789, 101, 112));
        assert!(dependency.matches(&snapshot));
        world.set(p, BlockId::Stone);
        assert!(!dependency.matches(&world.snapshot()));
        let mut evicted = snapshot.clone();
        Arc::make_mut(&mut evicted.chunks).remove(&IVec3::ZERO);
        assert!(!dependency.matches(&evicted));
        let mut cold = Dependency::capture(&evicted, IVec3::ZERO);
        cold.record = Some((123, 456, 789, 101, 112));
        assert!(!cold.matches(&evicted));
    }
}
