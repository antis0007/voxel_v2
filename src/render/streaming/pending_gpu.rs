//! Keyed, bounded GPU validation before active or inactive publication.
use super::*;

#[derive(Default)]
pub(super) struct PendingGpuLedger(Mutex<HashMap<IVec3, usize>>);
impl PendingGpuLedger {
    pub(super) fn bytes(&self) -> usize {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).values().copied().sum()
    }
    pub(super) fn bytes_matching(&self, coords:&HashSet<IVec3>) -> usize {
        self.0.lock().unwrap_or_else(|e|e.into_inner()).iter()
            .filter(|(q,_)|coords.contains(*q)).map(|(_,bytes)|*bytes).sum()
    }
    pub(super) fn contains(&self, coord: &IVec3) -> bool {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).contains_key(coord)
    }
    pub(super) fn projected_growth(&self, cache:&ChunkCache) -> usize {
        self.0.lock().unwrap_or_else(|e|e.into_inner()).iter().map(|(q,bytes)|
            bytes.saturating_sub(cache.gpu_meshes.get(q).map_or(0,|m|m.bytes))).sum()
    }
    pub(super) fn supplemental_bytes(&self, cache: &ChunkCache) -> usize {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).iter()
            .filter(|(q,_)| cache.horizon_pin_coords.contains(*q) || cache.edit_proxies.contains(q))
            .map(|(_,bytes)| *bytes).sum()
    }
    pub(super) fn reserve(self: &Arc<Self>, coord: IVec3, bytes: usize) -> Option<PendingGpuLease> {
        let mut state=self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.contains_key(&coord) || state.len() >= 16 {return None;}
        state.insert(coord,bytes);
        Some(PendingGpuLease{ledger:self.clone(),coord})
    }
}
pub(super) struct PendingGpuLease {ledger:Arc<PendingGpuLedger>,coord:IVec3}
impl Drop for PendingGpuLease {
    fn drop(&mut self) {self.ledger.0.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.coord);}
}

pub(super) struct PendingGpuUpload {
    // Drop new handles before releasing their byte reservation. CPU retry data
    // retains its original MeshLease until commit, retry or explicit discard.
    gpu:Option<ChunkGpuMesh>,
    check:Option<gpu_upload::Check>,
    cpu:Option<CpuChunkMesh>,
    failure:Option<String>,
    token:u64,
    streamer_epoch:u64,
    cache_epoch:u64,
    reservation:Option<PendingGpuLease>,
}

impl ChunkStreamer {
    pub(super) fn cancel_pending_sources(&mut self, changed:&HashSet<IVec3>) {
        let mut owned=Vec::new();
        self.pending_uploads.retain(|pending| {
            let cpu=pending.cpu.as_ref().expect("pending CPU source");
            if changed.contains(&cpu.key.coord) {
                owned.push((cpu.key.coord,pending.token,cpu.key));false
            } else {true}
        });
        for (coord,token,key) in owned {self.release_pending_request(coord,token,key);}
    }
    pub(super) fn cpu_world_current(&self,cache:&ChunkCache,cpu:&CpuChunkMesh)->bool {
        cpu.world_epoch==self.upload_epoch && cpu.cache_epoch==cache.gpu_world_epoch
    }

    pub(super) fn retire_upload_request(&mut self, key:ChunkKey) {
        self.requested.remove(&key);
        // Preserve the existing one-request-per-coordinate invariant and O(1)
        // cleanup. Pending completion calls this only after token ownership.
        self.requested_coords.remove(&key.coord);
    }
    fn release_pending_request(&mut self, coord:IVec3, token:u64, key:ChunkKey) -> bool {
        if self.pending_gpu_coords.get(&coord)!=Some(&token) {return false;}
        self.pending_gpu_coords.remove(&coord);
        self.retire_upload_request(key);
        true
    }
    pub(super) fn begin_pending_upload(&mut self, cpu:CpuChunkMesh, attempt:Result<ChunkGpuMesh,String>,
        check:gpu_upload::Check, reservation:PendingGpuLease, cache_epoch:u64) {
        let token=self.next_upload_token;
        self.next_upload_token=self.next_upload_token.wrapping_add(1);
        // Callers must not allocate a second candidate for the same coordinate.
        assert!(!self.pending_gpu_coords.contains_key(&cpu.key.coord));
        self.pending_gpu_coords.insert(cpu.key.coord,token);
        self.requested.insert(cpu.key);self.requested_coords.insert(cpu.key.coord);
        let (gpu,failure)=match attempt {Ok(gpu)=>(Some(gpu),None),Err(error)=>{
            self.upload_error=Some(error.clone());self.upload_retry_after=Instant::now()+Duration::from_secs(2);
            (None,Some(error))
        }};
        self.pending_uploads.push(PendingGpuUpload{gpu,check:Some(check),cpu:Some(cpu),failure,
            token,streamer_epoch:self.upload_epoch,cache_epoch,reservation:Some(reservation)});
    }

    /// Scope validation + normal queue ordering, not a GPU-idle/completion fence.
    pub(super) fn poll_pending_uploads(&mut self, device:&wgpu::Device, lost:&AtomicBool, cache:&mut ChunkCache,
        max_publications:usize, started:Instant) -> (usize,usize,usize) {
        if !self.pending_uploads.is_empty()
            && let Err(error)=device.poll(wgpu::PollType::Poll) {
                for pending in &mut self.pending_uploads {
                    pending.check=None;pending.failure=Some(format!("GPU polling failed: {error}"));
                }
            }
        if lost.load(Ordering::Acquire) {
            for pending in &mut self.pending_uploads {
                pending.check=None;pending.failure=Some("GPU device loss reported during pending publication".into());
            }
        }
        let mut published=0;let mut vertices=0;let mut indices=0;let mut i=0;
        while i<self.pending_uploads.len() {
            let pending=&mut self.pending_uploads[i];
            if let Some(check)=pending.check.as_mut() {
                match gpu_upload::poll(check) {
                    std::task::Poll::Pending=>{i+=1;continue;}
                    std::task::Poll::Ready(error)=>{
                        pending.check=None;
                        if error.is_some() {pending.failure=error;}
                    }
                }
            }
            // A callback/future may report loss while Poll progresses.
            // Re-read the same signal before any ready-success install.
            if lost.load(Ordering::Acquire) {
                pending.failure=Some("GPU device loss reported before pending publication".into());
            }
            // Completed futures are never polled twice. Ready successes may
            // remain queued when this frame's publication work is exhausted.
            let shape=pending.cpu.as_ref().expect("pending CPU source");
            let exceeds_geometry=vertices+shape.mesh.vertices.len()>CHUNK_UPLOAD_VERTEX_BUDGET
                || indices+shape.mesh.indices.len()>CHUNK_UPLOAD_INDEX_BUDGET;
            if pending.failure.is_none() && (published>=max_publications
                || (published>0 && (started.elapsed()>=UPLOAD_TIME_BUDGET || exceeds_geometry))) {i+=1;continue;}
            let mut pending=self.pending_uploads.swap_remove(i);
            let mut cpu=pending.cpu.take().expect("pending CPU source");let key=cpu.key;
            let owns_request=self.pending_gpu_coords.get(&key.coord)==Some(&pending.token);
            if !owns_request {continue;} // Never erase a newer request's markers.
            let same_world=pending.streamer_epoch==self.upload_epoch
                && cpu.world_epoch==self.upload_epoch && cpu.cache_epoch==cache.gpu_world_epoch && pending.cache_epoch==cache.gpu_world_epoch
                && pending.reservation.as_ref().is_some_and(|r|Arc::ptr_eq(&r.ledger,&cache.pending_gpu));
            let interested=cache.source_interested(&key.coord);
            let stale=edited_mesh_is_stale(self.dirty_revisions.get(&key.coord).copied(),cpu.revision);
            if !same_world || !interested || stale {
                self.release_pending_request(key.coord,pending.token,key);
                if same_world && interested && stale {
                    cache.dirty_meshes.insert(key.coord);
                    self.push_pending_key(ChunkKey{coord:key.coord,lod:cache.desired_lod(key.coord)});
                }
                continue;
            }
            if let Some(error)=pending.failure.take() {
                eprintln!("Chunk GPU upload rejected at {:?} job={} upload={}: {error}",key,cpu.job_id,pending.token);
                self.upload_error=Some(error);self.upload_retry_after=Instant::now()+Duration::from_secs(2);
                self.release_pending_request(key.coord,pending.token,key);
                // Drop unvalidated/partial destinations and their reservation
                // before retrying the still-current immutable CPU result.
                drop(pending);
                self.requested.insert(key);self.requested_coords.insert(key.coord);
                self.ready_meshes.push_back(cpu);
                continue;
            }
            let desired_lod=cache.desired_lod(key.coord);
            if desired_lod!=key.lod && cache.gpu_meshes.get(&key.coord)
                .is_some_and(|m|m.lod==desired_lod&&!cache.dirty_meshes.contains(&key.coord)) {
                self.release_pending_request(key.coord,pending.token,key);continue;
            }
            let bytes=pending.gpu.as_ref().expect("validated destination").bytes;
            cache.trim_lod_budget();
            if !cache.pending_publication_fits(key,bytes) {
                cache.budget_deferred.insert(key,bytes);
                self.release_pending_request(key.coord,pending.token,key);
                drop(pending);
                self.requested.insert(key);self.requested_coords.insert(key.coord);
                self.ready_meshes.push_back(cpu);
                continue;
            }
            let mut gpu=pending.gpu.take().expect("validated destination");
            gpu.boundary_profile = collar::Installed::new(collar::Scope {
                coord:key.coord, lod:key.lod, world_epoch:cpu.world_epoch,
                cache_epoch:cpu.cache_epoch, job_id:cpu.job_id, revision:cpu.revision,
            }, std::mem::take(&mut cpu.boundary_profile));
            // Main-thread atomic accounting transfer: no intervening GPU
            // allocation or publication occurs before installation below.
            drop(pending.reservation.take());
            if desired_lod==key.lod || !cache.gpu_meshes.contains_key(&key.coord) {
                if cache.edit_proxies.persistent_contains(&key.coord) {
                    cache.edit_proxies.mesh_publications+=1;
                    cache.edit_proxies.latest_mesh_revision=cache.edit_proxies.latest_mesh_revision.max(cpu.revision);
                }
                cache.install_mesh_with_casters(key.coord,gpu,std::mem::take(&mut cpu.shadow_casters));
            } else {cache.lod_meshes.remember(key.coord,gpu);cache.trim_lod_budget();}
            if desired_lod!=key.lod && cache.gpu_meshes.get(&key.coord).is_none_or(|m|m.lod!=desired_lod) {
                let desired=ChunkKey{coord:key.coord,lod:desired_lod};
                if cache.activate_lod(desired){self.last_stats.lod_cache_hits+=1;}else{self.push_pending_key(desired);}
            }
            // The stale check above and this commit are in the same main-thread
            // turn. A later/newer request must not be cleared by this completion.
            if self.release_pending_request(key.coord,pending.token,key)
                && !edited_mesh_is_stale(self.dirty_revisions.get(&key.coord).copied(),cpu.revision) {
                self.dirty_revisions.remove(&key.coord);
            }
            self.upload_error=None;published+=1;vertices+=cpu.mesh.vertices.len();indices+=cpu.mesh.indices.len();
        }
        (published,vertices,indices)
    }
}

#[cfg(test)]
#[path="pending_gpu_tests.rs"]
mod tests;
