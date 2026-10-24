//! Bounded render-only source cover for edits outside ordinary view selection.
//! Requests never imply ownership; only a complete published GPU column does.
use super::{ChunkCache, ChunkKey};
use crate::world::WorldSnapshot;
use glam::IVec3;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) const COLUMN_LIMIT: usize = super::super::coverage::MAX_SPARSE_COLUMNS;
pub(super) const BYTE_LIMIT: usize = 1024 * 1024 * 1024;
const PENDING_COLUMNS: usize = 8;
const ENQUEUE_LIMIT: usize = 128;

struct Column {
    revision: u64,
    since: Instant,
    low_y: i32,
    high_y: i32,
    planned: Option<((i32, i32, i32), i32, i32)>,
    members: Vec<IVec3>,
    scheduled: bool,
    admitted: bool,
}

#[cfg(test)]
mod priority_tests {
    use super::*;
    use crate::render::streaming::ChunkStreamer;

    #[test]
    #[ignore = "native Vulkan buffers for exact proxy LOD activation; run serially"]
    fn gpu_invariant_lod_activation_publishes_ready_edit_column() {
        let adapter = engine_gpu::test_adapter();
        let (_device,_)=pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let q=IVec3::new(50,0,0);
        let mut cache=ChunkCache::new(IVec3::ZERO);
        cache.visible.clear();
        cache.edit_proxies.columns.insert((q.x,q.z),Column {
            revision:1,since:Instant::now(),low_y:0,high_y:0,planned:None,
            members:vec![q],scheduled:true,admitted:false,
        });
        cache.edit_proxies.rebuild_coords();
        cache.rebuild_visible_sets();
        cache.install_mesh(q,super::super::ChunkGpuMesh::test_mesh(0, 2, true, 32));
        assert!(!cache.published_coords.contains(&q),"exact proxy ownership cannot activate at coarse LOD");
        assert!(!cache.edit_proxies.columns[&(q.x,q.z)].admitted);
        assert!(cache.activate_lod(super::super::ChunkKey {coord:q,lod:0}));
        assert!(cache.published_coords.contains(&q),"invariant metadata LOD change still refreshes readiness");
        assert!(cache.edit_proxies.columns[&(q.x,q.z)].admitted);
        assert!(cache.resident_coverage(glam::Vec3::ZERO,4,1.0).contains_column(q.x,q.z));
    }

    #[test]
    fn oldest_incomplete_column_finishes_before_continuous_repairs() {
        let mut cache = ChunkCache::new(IVec3::ZERO);
        let now = Instant::now();
        let old = IVec3::new(80, 0, 0);
        let tied = IVec3::new(0, 0, 80);
        let newer = IVec3::new(70, 0, 0);
        for (q, since) in [(old, now - Duration::from_secs(2)), (tied, now - Duration::from_secs(2)), (newer, now)] {
            cache.edit_proxies.columns.insert((q.x, q.z), Column {
                revision: 1, since, low_y: 0, high_y: 1, planned: None,
                members: vec![q, q + IVec3::Y], scheduled: true, admitted: false,
            });
        }
        cache.edit_proxies.rebuild_coords();
        let repair = IVec3::ZERO;
        let missing = IVec3::X;
        cache.visible_coords.extend([repair, missing]);
        cache.published_columns.insert((0, 0), vec![repair]);
        cache.dirty_meshes.insert(repair);
        let key = |coord| ChunkKey { coord, lod: 0 };
        let mut order = [key(repair), key(newer), key(old + IVec3::Y), key(missing), key(old), key(tied), key(tied + IVec3::Y)];
        order.sort_by_key(|&k| ChunkStreamer::ready_chunk_priority(&cache, k));
        assert_eq!(order[0].coord, missing, "missing ordinary cover keeps emergency priority");
        let tied_members: HashSet<_> = order[1..3].iter().map(|k| k.coord).collect();
        assert_eq!(tied_members, HashSet::from([tied, tied + IVec3::Y]), "equal birth times still finish one whole column");
        let old_members: HashSet<_> = order[3..5].iter().map(|k| k.coord).collect();
        assert_eq!(old_members, HashSet::from([old, old + IVec3::Y]));
        assert_eq!(order[5].coord, newer);
        assert_eq!(order[6].coord, repair, "continuous repair cannot starve bundle completion");
        cache.edit_proxies.mark_admitted((old.x, old.z));
        assert!(cache.edit_proxies.pending_since(&old).is_none());
        assert!(ChunkStreamer::ready_chunk_priority(&cache, key(newer))
            < ChunkStreamer::ready_chunk_priority(&cache, key(old)));
    }
    #[test]
    fn pending_sources_share_cover_and_whole_column_priority() {
        let mut cache = ChunkCache::new(IVec3::ZERO);
        let birth = Instant::now() - Duration::from_secs(2);
        let old = IVec3::new(80, 0, 0);
        let tied = IVec3::new(0, 0, 80);
        let newer = IVec3::new(70, 0, 0);
        for (q, since) in [(old, birth), (tied, birth), (newer, birth + Duration::from_secs(1))] {
            cache.edit_proxies.columns.insert((q.x, q.z), Column {
                revision: 1, since, low_y: 0, high_y: 1, planned: None,
                members: vec![q, q + IVec3::Y], scheduled: true, admitted: false,
            });
        }
        cache.edit_proxies.rebuild_coords();
        let repair = IVec3::ZERO;
        let extra_y = repair + IVec3::Y;
        let missing = IVec3::X;
        cache.visible_coords.extend([repair, extra_y, missing]);
        cache.published_columns.insert((0, 0), vec![repair]);
        cache.dirty_meshes.insert(repair);
        let key = |coord| ChunkKey { coord, lod: 0 };
        let mut heap: std::collections::BinaryHeap<_> = [extra_y, repair, newer, old + IVec3::Y, missing, old, tied + IVec3::Y, tied]
            .into_iter().map(|q| ChunkStreamer::source_pending_priority(&cache, key(q), true)).collect();
        let mut order = Vec::new();
        while let Some(std::cmp::Reverse((_, _, (_, _, _, _, x, y, z, _)))) = heap.pop() {
            order.push(IVec3::new(x, y, z));
        }
        assert_eq!(order[0], missing, "uncovered ordinary column remains first");
        assert_eq!(HashSet::from([order[1], order[2]]), HashSet::from([tied, tied + IVec3::Y]));
        assert_eq!(HashSet::from([order[3], order[4]]), HashSet::from([old, old + IVec3::Y]));
        assert_eq!(order[5], newer);
        assert_eq!(order[6], repair, "near continuous repair cannot outrank incomplete sources");
        assert_eq!(order[7], extra_y, "missing Y over published old cover is repair, not emergency cover");
        cache.edit_proxies.mark_admitted((tied.x, tied.z));
        assert!(ChunkStreamer::source_pending_priority(&cache, key(old), true)
            > ChunkStreamer::source_pending_priority(&cache, key(tied), true), "Reverse heap exits admitted proxy from completion lane");
    }
}

#[derive(Default)]
pub(super) struct EditProxies {
    columns: HashMap<(i32, i32), Column>,
    pub(super) coords: HashSet<IVec3>,
    quality_coords: HashSet<IVec3>,
    quality_columns: HashSet<(i32,i32)>,
    quality_admitted: HashSet<(i32,i32)>,
    latest_source_revision: u64,
    last_report: Option<Instant>,
    previous_water: Option<Arc<crate::world::WaterSnapshot>>,
    previous_hot_coords: HashSet<IVec3>,
    pub(super) water_dirty: HashSet<IVec3>,
    pub(super) source_only_completed: u64,
    pub(super) normal_promotions: u64,
    pub(super) mesh_publications: u64,
    pub(super) latest_mesh_revision: u64,
}

impl EditProxies {
    pub(super) fn admitted(&self, column: (i32, i32)) -> bool {
        self.columns.get(&column).is_some_and(|c| c.admitted) || self.quality_admitted.contains(&column)
    }
    pub(super) fn columns(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.columns.keys().copied().chain(self.quality_columns.iter().filter(|q|!self.columns.contains_key(*q)).copied())
    }
    pub(super) fn contains(&self, q: &IVec3) -> bool {
        self.coords.contains(q)
    }
    pub(super) fn pending_since(&self, q: &IVec3) -> Option<Instant> {
        if !self.coords.contains(q) { return None; }
        self.columns.get(&(q.x, q.z))
            .filter(|c| c.scheduled && !c.admitted).map(|c| c.since)
    }
    pub(super) fn wanted(&self, column: &(i32, i32)) -> bool {
        self.columns.contains_key(column) || self.quality_columns.contains(column)
    }
    pub(super) fn persistent_wanted(&self, column: &(i32,i32)) -> bool {self.columns.contains_key(column)}
    pub(super) fn persistent_contains(&self, q: &IVec3) -> bool {
        self.columns.get(&(q.x,q.z)).is_some_and(|c|c.scheduled && c.members.contains(q))
    }
    pub(super) fn set_quality_sources(&mut self, coords:HashSet<IVec3>, columns:HashSet<(i32,i32)>, admitted:HashSet<(i32,i32)>) {
        self.quality_coords=coords;self.quality_columns=columns;self.quality_admitted=admitted;self.rebuild_coords();
    }
    pub(super) fn set_quality_admitted(&mut self, admitted:HashSet<(i32,i32)>) {self.quality_admitted=admitted;}
    pub(super) fn promote_quality_column(&mut self, key:(i32,i32), members:&[IVec3], admitted:bool, now:Instant) {
        let low=members.iter().map(|q|q.y).min().unwrap_or(0);
        let high=members.iter().map(|q|q.y).max().unwrap_or(low);
        let c=self.columns.entry(key).or_insert_with(||Column {
            revision:self.latest_source_revision,since:now,low_y:low,high_y:high,planned:None,
            members:Vec::new(),scheduled:false,admitted:false});
        c.revision=c.revision.max(self.latest_source_revision);
        c.low_y=c.low_y.min(low);c.high_y=c.high_y.max(high);c.planned=None;
        c.members.extend_from_slice(members);c.members.sort_unstable_by_key(|q|q.y);c.members.dedup();
        c.scheduled|=!c.members.is_empty();c.admitted|=admitted;
        self.rebuild_coords();
    }
    pub(super) fn mark_admitted(&mut self, column: (i32, i32)) {
        if let Some(c) = self.columns.get_mut(&column) {
            if !c.admitted {
                println!(
                    "[edit-proxy-admit] column={column:?} members={} last_dirty_revision={} age_s={:.2}",
                    c.members.len(),
                    c.revision,
                    c.since.elapsed().as_secs_f32()
                );
            }
            c.admitted = true;
        }
    }
    pub(super) fn desired_columns(&self) -> impl Iterator<Item = ((i32, i32), &[IVec3])> {
        self.columns
            .iter()
            .filter(|(_, c)| c.scheduled && !c.members.is_empty())
            .map(|(&key, c)| (key, c.members.as_slice()))
    }
    fn rebuild_coords(&mut self) {
        self.coords = self
            .columns
            .values()
            .filter(|c| c.scheduled)
            .flat_map(|c| c.members.iter().copied())
            .collect();
        self.coords.extend(self.quality_coords.iter().copied());
    }
}

impl ChunkCache {
    /// Call with the current immutable snapshot after all world mutations.
    /// Returned keys use the existing bounded streamer queues. Parent revision
    /// alone is deliberately insufficient to retire cover that it omitted.
    pub fn update_edit_proxies(
        &mut self,
        snapshot: &WorldSnapshot,
        dirty_chunks: &HashSet<IVec3>,
        source_revision: u64,
        _parent_source_revision: Option<u64>,
    ) -> Vec<ChunkKey> {
        // Minimum spec: no render-only cover for edits beyond the view radius.
        if crate::low_spec::active() { return Vec::new(); }
        self.edit_proxies.latest_source_revision=source_revision;
        let now = Instant::now();
        let span = 32.0 * snapshot.cfg.voxel_scale.meters();
        let collar = (10.0 / span).ceil().max(1.0) as i32;
        let crown = (26.0 / span).ceil() as i32 + 1;
        let radius = i64::from(
            super::render_distance_settings()
                .view_distance_chunks
                .clamp(1, 64),
        );
        let mut deferred = 0usize;
        let mut changed = false;
        // Stable admission: preserve existing cover and its age across ticks.
        let mut changed_chunks = dirty_chunks.clone();
        if let Some(previous) = &self.edit_proxies.previous_water {
            for (q, old, new) in snapshot.water_levels.changed_chunks(previous) {
                if old == new
                    || !self.edit_proxies.previous_hot_coords.contains(&q)
                    || !snapshot.chunks.contains_key(&q)
                    || dirty_chunks.contains(&q)
                {
                    continue;
                }
                changed_chunks.insert(q);
                // Volume-only changes need the same dependency revision checks
                // as block edits, including all mesher boundary halos.
                for y in -1..=1 {
                    for z in -1..=1 {
                        for x in -1..=1 {
                            if let (Some(x), Some(y), Some(z)) =
                                (q.x.checked_add(x), q.y.checked_add(y), q.z.checked_add(z))
                            {
                                self.edit_proxies.water_dirty.insert(IVec3::new(x, y, z));
                            }
                        }
                    }
                }
            }
        }
        self.edit_proxies.previous_water = Some(Arc::clone(&snapshot.water_levels));
        self.edit_proxies.previous_hot_coords = snapshot.chunks.keys().copied().collect();
        let mut dirty: Vec<_> = changed_chunks
            .iter()
            .filter(|q| snapshot.chunks.contains_key(*q))
            .copied()
            .collect();
        dirty.sort_unstable_by_key(|q| (q.x, q.z, q.y));
        for q in dirty {
            for dz in -collar..=collar {
                for dx in -collar..=collar {
                    let Some(x) = q.x.checked_add(dx) else {
                        continue;
                    };
                    let Some(z) = q.z.checked_add(dz) else {
                        continue;
                    };
                    let key = (x, z);
                    let offset_x = i64::from(x) - i64::from(self.center_chunk.x);
                    let offset_z = i64::from(z) - i64::from(self.center_chunk.z);
                    // Ordinary columns already have their own complete-column lane.
                    // Do not replace the entire near view with deeper proxy bundles.
                    if offset_x.abs() <= radius
                        && offset_z.abs() <= radius
                        && offset_x * offset_x + offset_z * offset_z <= radius * radius
                        && !self.edit_proxies.wanted(&key)
                    {
                        continue;
                    }
                    if !self.edit_proxies.columns.contains_key(&key) {
                        let union:HashSet<_>=self.edit_proxies.columns().chain(self.horizon_pins.iter().copied()).collect();
                        if union.len() >= COLUMN_LIMIT && !union.contains(&key) {
                            deferred += 1;
                            continue;
                        }
                        let mut low_y = q.y.saturating_sub(1);
                        let mut high_y = q.y.saturating_add(crown);
                        // Persisted roofs, underground construction and authored
                        // crowns can lie outside procedural terrain bounds. Include
                        // their vertical support before declaring the entire XZ
                        // column owned, even while those records are still cold.
                        for edited in snapshot
                            .chunks
                            .keys()
                            .copied()
                            .chain(
                                snapshot
                                    .regions
                                    .records
                                    .nearby(IVec3::new(x, 0, z), 0)
                                    .copied(),
                            )
                            .filter(|p| p.x == x && p.z == z)
                        {
                            low_y = low_y.min(edited.y.saturating_sub(1));
                            high_y = high_y.max(edited.y.saturating_add(crown));
                        }
                        self.edit_proxies.columns.insert(
                            key,
                            Column {
                                revision: source_revision,
                                since: now,
                                low_y,
                                high_y,
                                planned: None,
                                members: Vec::new(),
                                scheduled: false,
                                admitted: false,
                            },
                        );
                        changed = true;
                    }
                    let c = self.edit_proxies.columns.get_mut(&key).unwrap();
                    c.revision = c.revision.max(source_revision);
                    c.low_y = c.low_y.min(q.y.saturating_sub(1));
                    c.high_y = c.high_y.max(q.y.saturating_add(crown));
                }
            }
        }
        let interest: HashSet<_> = self.edit_proxies.columns().collect();
        self.column_y_cache.set_supplemental_interest(interest);
        self.column_y_cache.begin(snapshot.cfg);
        let floor = snapshot.cfg.min_world_y().div_euclid(32);
        let ceiling = (snapshot.cfg.max_world_y() - 1).div_euclid(32);
        for (&key, c) in &mut self.edit_proxies.columns {
            let Some(range) = self.column_y_cache.range(key) else {
                continue;
            };
            let plan = (range, c.low_y, c.high_y);
            if c.planned == Some(plan) {
                continue;
            }
            // Full exact terrain/cave/crown range, plus all known edited height.
            // No erosion or source generation occurs on this thread.
            c.members = (range.0.min(c.low_y).max(floor)..=range.1.max(c.high_y).min(ceiling))
                .map(|y| IVec3::new(key.0, y, key.1))
                .collect();
            c.planned = Some(plan);
            changed = true;
        }
        let running = self
            .edit_proxies
            .columns
            .values()
            .filter(|c| c.scheduled && !c.admitted)
            .count();
        let mut waiting: Vec<_> = self
            .edit_proxies
            .columns
            .iter()
            .filter(|(_, c)| !c.scheduled && !c.members.is_empty())
            .map(|(&key, c)| (c.since, key))
            .collect();
        waiting.sort_unstable();
        for (_, key) in waiting
            .into_iter()
            .take(PENDING_COLUMNS.saturating_sub(running))
        {
            self.edit_proxies.columns.get_mut(&key).unwrap().scheduled = true;
            changed = true;
        }
        if changed {
            self.edit_proxies.rebuild_coords();
            self.rebuild_visible_sets();
        }
        let mut keys: Vec<_> = self
            .edit_proxies
            .coords
            .iter()
            .copied()
            .filter(|q| {
                self.gpu_meshes.get(q).is_none_or(|m| m.lod != 0) || self.dirty_meshes.contains(q)
            })
            .map(|coord| ChunkKey { coord, lod: 0 })
            .collect();
        keys.sort_unstable_by_key(|key| {
            // coords also contains quality-only columns. Their pending age
            // comes from the quality scheduler, not the persistent edit map.
            let since = self.edit_proxies.columns.get(&(key.coord.x, key.coord.z))
                .map(|c| c.since)
                .or_else(|| self.quality_proxies.pending_since(&key.coord))
                .unwrap_or(now);
            (since, key.coord.x, key.coord.z, key.coord.y)
        });
        keys.truncate(ENQUEUE_LIMIT);
        if !self.edit_proxies.columns.is_empty()
            && self
                .edit_proxies
                .last_report
                .is_none_or(|at| now.duration_since(at) >= Duration::from_secs(2))
        {
            let admitted = self
                .edit_proxies
                .columns
                .values()
                .filter(|c| c.admitted)
                .count();
            let oldest = self
                .edit_proxies
                .columns
                .values()
                .filter(|c| !c.admitted)
                .map(|c| now.duration_since(c.since).as_secs_f32())
                .fold(0.0_f32, f32::max);
            println!(
                "[edit-proxies] wanted={} admitted={} members={} deferred={} oldest_pending_s={oldest:.2} bytes={} limit_bytes={BYTE_LIMIT} source_only_completed={} source_only_discovery=disabled normal_promotions={} mesh_publications={} latest_mesh_revision={}",
                self.edit_proxies.columns.len(),
                admitted,
                self.edit_proxies.coords.len(),
                deferred,
                self.supplemental_gpu_bytes(),
                self.edit_proxies.source_only_completed,
                self.edit_proxies.normal_promotions,
                self.edit_proxies.mesh_publications,
                self.edit_proxies.latest_mesh_revision
            );
            self.edit_proxies.last_report = Some(now);
        }
        keys
    }

    /// Region-load interest only. The caller requests persisted records in each
    /// member's +/- one chunk halo without waking or discovering activity.
    pub fn edit_proxy_source_coords(&self) -> Vec<IVec3> {
        self.edit_proxies.coords.iter().copied().collect()
    }
    pub fn edit_proxy_columns(&self) -> Vec<(i32, i32)> {
        self.edit_proxies.columns().collect()
    }

    /// The caller proves these columns were baked, not omitted, by this parent.
    pub fn retire_edit_proxies(
        &mut self,
        baked_columns: &HashSet<(i32, i32)>,
        parent_source_revision: u64,
    ) {
        let before = self.edit_proxies.columns.len();
        self.edit_proxies.columns.retain(|key, c| {
            !(baked_columns.contains(key) && c.revision <= parent_source_revision)
        });
        if before != self.edit_proxies.columns.len() {
            self.edit_proxies.rebuild_coords();
            self.column_y_cache
                .set_supplemental_interest(self.edit_proxies.columns.keys().copied().collect());
            self.rebuild_visible_sets();
        }
    }

    pub(super) fn ordinary_source_interested(&self, q: IVec3) -> bool {
        let d = q.as_i64vec3() - self.center_chunk.as_i64vec3();
        let r = i64::from(
            super::render_distance_settings()
                .view_distance_chunks
                .clamp(1, 64),
        );
        self.visible_coords.contains(&q)
            && d.x.abs() <= r
            && d.z.abs() <= r
            && d.x * d.x + d.z * d.z <= r * r
    }

    pub(super) fn supplemental_gpu_bytes(&self) -> usize {
        self.gpu_meshes
            .iter()
            .filter(|(q, _)| self.horizon_pin_coords.contains(*q) || self.edit_proxies.contains(q))
            .map(|(_, mesh)| mesh.bytes)
            .sum()
    }
}
