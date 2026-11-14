//! Bounded player-edit lane adapted from the measured `voxel_latency` build.
use super::*;

const MAX_MARKS: usize = 64;
pub(super) const QUEUE_CAPACITY: usize = 8;
const TTL: Duration = Duration::from_secs(3);
/// Longest a ready edited mesh waits for the other chunks of the same edit.
const GROUP_HOLD: Duration = Duration::from_millis(120);

pub(super) struct Mark {
    revision: u64,
    at: Instant,
    /// First note of this mark; `at` refreshes on every later edit.
    since: Instant,
    in_flight: bool,
}

impl ChunkStreamer {
    pub fn stalled_edit_debug(&self,cache:&ChunkCache,coord:IVec3)->String {
        let key=ChunkKey{coord,lod:cache.desired_lod(coord)};
        format!("coord={coord:?} mark={:?} interested={} source={} gpu_revision={:?} drawn={} dirty={:?} generating={} meshing={} ready_source={} ready_mesh={} pending_gpu={} budget_deferred={:?} gpu_bytes={} pending_bytes={} limit={} quality={} proxy={}",
            self.player_edits.get(&coord).map(|m|(m.revision,m.in_flight,m.at.elapsed().as_millis())),cache.source_interested(&coord),cache.cpu_chunks.contains_key(&coord),
            cache.gpu_meshes.get(&coord).map(|m|m.source_revision),cache.published_coords.contains(&coord),self.dirty_revisions.get(&coord),
            self.generating.contains(&key),self.meshing.contains_key(&key),self.ready_chunks.iter().any(|j|j.key.coord==coord),
            self.ready_meshes.iter().any(|j|j.key.coord==coord),self.pending_gpu_coords.contains_key(&coord),cache.budget_deferred.get(&key),
            cache.gpu_bytes,cache.pending_gpu.bytes(),cache.residency_limits.1,cache.quality_proxies.contains_column(&(coord.x,coord.z)),cache.edit_proxies.contains(&coord))
    }

    pub fn note_player_edits(&mut self, coords: impl IntoIterator<Item = IVec3>, revision: u64) {
        let now = Instant::now();
        for coord in coords {
            if self.player_edits.len() >= MAX_MARKS && !self.player_edits.contains_key(&coord)
                && let Some(old)=self.player_edits.iter().min_by_key(|(_,mark)|mark.at).map(|(&coord,_)|coord) {
                self.player_edits.remove(&old);
            }
            if let Some(mark)=self.player_edits.get_mut(&coord) {
                mark.revision=mark.revision.max(revision);mark.at=now;
            } else {
                self.player_edits.insert(coord, Mark { revision, at: now, since: now, in_flight: false });
            }
        }
    }

    /// Splits edited coordinates into those whose mesh must wait, those whose
    /// whole edit is ready to publish in one frame, and those whose pending
    /// mesh is older than the newest edit and must be dropped and remeshed.
    ///
    /// Chunks sharing a face halo publish independently, so a break on a chunk
    /// border could show the cleared cell while the neighbour still culled the
    /// faces behind it, opening a view into the caves below for a frame or more.
    /// Coordinates noted with the same revision form one edit.
    pub(super) fn edit_groups(&self, cache: &ChunkCache, ready: &HashMap<IVec3, u64>) -> EditGroups {
        edit_groups(&self.player_edits, cache, ready)
    }

    pub(super) fn needs_player_restart(&self, coord: &IVec3) -> bool {
        self.player_edits.get(coord).is_some_and(|mark| !mark.in_flight)
    }

    pub(super) fn player_edit_dispatched(&mut self, coord: IVec3, revision: u64) {
        if let Some(mark) = self.player_edits.get_mut(&coord)
            && revision >= mark.revision {
            mark.in_flight = true;
        }
    }

    pub(super) fn player_edit_published(&mut self, coord: IVec3, revision: u64) {
        if self.player_edits.get(&coord).is_some_and(|mark| revision >= mark.revision) {
            self.player_edits.remove(&coord);
        }
    }

    pub(super) fn requeue_player_edit(&mut self, cache: &ChunkCache, coord: IVec3) -> bool {
        if !self.player_edits.contains_key(&coord) || self.requested_coords.contains(&coord) {
            return false;
        }
        let Some(source) = cache.cpu_chunks.get(&coord) else { return false };
        let key = ChunkKey { coord, lod: cache.desired_lod(coord) };
        let revision = self.dirty_revisions.get(&coord).copied().unwrap_or(0);
        self.requested.insert(key);
        self.requested_coords.insert(coord);
        self.ready_chunks.push_front(ChunkJob { key, revision, source: source.clone() });
        self.player_edit_dispatched(coord,revision);
        true
    }

    pub(super) fn dispatch_player_edits(&mut self, cache: &mut ChunkCache) -> usize {
        let now = Instant::now();
        // A busy edited world can need longer than the ordinary expiry to
        // obtain a source. Keep relevant edits urgent until publication.
        self.player_edits.retain(|coord,mark| cache.source_interested(coord) || now.duration_since(mark.at)<TTL);
        if self.player_edits.is_empty() {
            return 0;
        }
        let mut sent = 0;
        // Scan only the queue present at entry. A full urgent queue retains the
        // source on the ordinary lane; the render thread never blocks on send.
        for _ in 0..self.ready_chunks.len() {
            let Some(mut job) = self.ready_chunks.pop_front() else { break };
            if !self.player_edits.contains_key(&job.key.coord) {
                self.ready_chunks.push_back(job);
                continue;
            }
            if !cache.source_interested(&job.key.coord) {
                self.retire_upload_request(job.key);
                continue;
            }
            let desired = ChunkKey { coord: job.key.coord, lod: cache.desired_lod(job.key.coord) };
            if job.key != desired {
                self.requested.remove(&job.key);
                self.requested.insert(desired);
                job.key = desired;
            }
            let cancelled = Arc::new(AtomicBool::new(false));
            let job_id = self.next_mesh_job_id;
            let next = MeshJob {
                key: job.key, revision: job.revision, job_id,
                world_epoch: self.upload_epoch, cache_epoch: cache.gpu_world_epoch,
                chunk: job.source.clone(), cancelled: cancelled.clone(),
                shadow_baseline: cache.shadow_casters.baseline(job.key.coord, job.key.lod),
            };
            match self.urgent_job_tx.try_send(next) {
                Ok(()) => {
                    job.source.water_motion.seal();
                    self.next_mesh_job_id = self.next_mesh_job_id.wrapping_add(1);
                    if let Some((_, older)) = self.meshing.insert(job.key, (job_id, cancelled)) {
                        older.store(true, Ordering::Relaxed);
                    }
                    self.player_edit_dispatched(job.key.coord,job.revision);
                    sent += 1;
                }
                Err(_) => {
                    self.ready_chunks.push_front(job);
                    break;
                }
            }
        }
        sent
    }
}

/// Pending uploads of edited chunks sorted by what this frame may do with them.
#[derive(Default)]
pub(super) struct EditGroups {
    /// Wait: another chunk of the same edit is not ready yet.
    pub held: HashSet<IVec3>,
    /// Publish together, bypassing the upload budget.
    pub release: HashSet<IVec3>,
    /// Older than the chunk's newest edit: drop the mesh and remesh now.
    /// Keeping it pending counted as in flight and blocked the newer remesh
    /// for the whole hold; publishing it could open a border hole next to a
    /// neighbour whose matching mesh is still in flight.
    pub superseded: HashSet<IVec3>,
}

fn edit_groups(marks: &HashMap<IVec3, Mark>, cache: &ChunkCache, ready: &HashMap<IVec3, u64>) -> EditGroups {
    let mut groups: HashMap<u64, (bool, bool, Vec<IVec3>)> = HashMap::new();
    let mut out = EditGroups::default();
    for (&coord, mark) in marks {
        if !cache.source_interested(&coord) {
            continue;
        }
        let pending = ready.get(&coord).copied();
        if pending.is_some_and(|r| r < mark.revision) {
            out.superseded.insert(coord);
            continue;
        }
        let group = groups.entry(mark.revision).or_insert((true, false, Vec::new()));
        let shown = cache.gpu_meshes.get(&coord).is_some_and(|m| m.source_revision >= mark.revision);
        group.0 &= shown || pending.is_some();
        group.1 |= mark.since.elapsed() < GROUP_HOLD;
        group.2.push(coord);
    }
    for (complete, young, coords) in groups.into_values() {
        if complete {
            out.release.extend(coords);
        } else if young {
            out.held.extend(coords);
        }
    }
    out
}

#[cfg(test)]
mod source_tests {
    use super::*;
    use crate::{fluid::VoxelScalePreset, world::{BlockId, World, WorldType}};

    #[test]
    fn player_source_copy_keeps_worker_snapshot_and_releases_its_charge() {
        let ledger = Arc::new(VoxelAllocations::default());
        let source = crate::mesh::GeneratedChunk::new(IVec3::ZERO,
            vec![BlockId::Air; (PADDED_CHUNK_SIZE as usize).pow(3)]);
        let frozen = Arc::new(ResidentChunk::new(source,&ledger));
        let bytes = ledger.0.load(Ordering::Relaxed);
        let mut patched = frozen.clone();
        let p = IVec3::splat(2);
        let mut world = World::with_seed(WorldType::Superflat,VoxelScalePreset::OneMeter,7);
        world.set(p,BlockId::Stone);
        patch_source_halo(Arc::make_mut(&mut patched),Some(&vec![p]),&world.snapshot());
        assert_eq!(frozen.get_world(p),BlockId::Air);
        assert_eq!(patched.get_world(p),BlockId::Stone);
        assert_eq!(ledger.0.load(Ordering::Relaxed),bytes*2);
        drop(patched);
        assert_eq!(ledger.0.load(Ordering::Relaxed),bytes);
        drop(frozen);
        assert_eq!(ledger.0.load(Ordering::Relaxed),0);
    }

    #[test]
    fn one_edit_publishes_together_and_holds_until_complete() {
        let (a, b) = (IVec3::new(-1, 0, 0), IVec3::ZERO);
        let mut cache = ChunkCache::new(IVec3::ZERO);
        cache.visible = vec![ChunkKey { coord: a, lod: 0 }, ChunkKey { coord: b, lod: 0 }];
        cache.rebuild_visible_sets();
        let now = Instant::now();
        let mark = |since| Mark { revision: 7, at: now, since, in_flight: true };
        let mut marks = HashMap::from([(a, mark(now)), (b, mark(now))]);
        let g = edit_groups(&marks, &cache, &HashMap::from([(a, 7)]));
        assert_eq!((g.held, g.release.len()), (HashSet::from([a, b]), 0));
        let g = edit_groups(&marks, &cache, &HashMap::from([(a, 7), (b, 8)]));
        assert_eq!((g.held.len(), g.release), (0, HashSet::from([a, b])));
        // An older edit stops waiting and falls back to per-chunk publication.
        let old = now.checked_sub(GROUP_HOLD * 2).unwrap_or(now);
        marks.values_mut().for_each(|m| m.since = old);
        let g = edit_groups(&marks, &cache, &HashMap::from([(a, 7)]));
        assert!(g.held.is_empty() && g.release.is_empty() && g.superseded.is_empty());
    }

    #[test]
    fn superseded_pending_mesh_is_dropped_not_held_or_published() {
        let (a, b) = (IVec3::new(-1, 0, 0), IVec3::ZERO);
        let mut cache = ChunkCache::new(IVec3::ZERO);
        cache.visible = vec![ChunkKey { coord: a, lod: 0 }, ChunkKey { coord: b, lod: 0 }];
        cache.rebuild_visible_sets();
        let now = Instant::now();
        let mark = |revision| Mark { revision, at: now, since: now, in_flight: true };
        // One edit at revision 7 marked both chunks; `a` still carries the
        // previous edit's mesh (revision 5) while `b` waits for its own.
        let marks = HashMap::from([(a, mark(7)), (b, mark(7))]);
        let g = edit_groups(&marks, &cache, &HashMap::from([(a, 5)]));
        assert_eq!((g.held, g.release.len(), g.superseded), (HashSet::from([b]), 0, HashSet::from([a])));
        // Mixed marks: a border edit at 5 marked both, then an interior edit in
        // `a` at 7. `b` completes its own group and publishes; `a` is remeshed.
        let marks = HashMap::from([(a, mark(7)), (b, mark(5))]);
        let g = edit_groups(&marks, &cache, &HashMap::from([(a, 5), (b, 5)]));
        assert_eq!((g.held.len(), g.release, g.superseded), (0, HashSet::from([b]), HashSet::from([a])));
    }
}
