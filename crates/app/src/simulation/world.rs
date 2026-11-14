//! Spatial simulation scheduling. Near activity gets low latency; a fair lane
//! always services distant activity, independently of render distance.
use super::{cellular, residency::Residency};
use crate::{
    fluid::{FluidStats, VoxelScalePreset},
    mesh::GeneratedChunk,
    world::{
        self, BlockId as B, World,
        activity::{MAX_ACTIVE_CHUNKS, Mask},
        chunk_coord_from_world_block,
    },
};
use glam::IVec3;
use std::{
    collections::{HashSet, VecDeque},
    sync::mpsc,
    time::{Duration, Instant},
};
// The simulation shares the frame with streaming and meshing. Two and a half
// milliseconds is enough for visible flow while avoiding a permanent catch-up
// debt on fine voxel worlds.
const FRAME_BUDGET: Duration = Duration::from_micros(2_500);
// Water gets a larger slice only while its backlog grows. At the fixed slice a
// flooded cave or dam break advanced at the pace of the slowest frame, because
// every unprocessed cell waited for the next tick.
const WATER_BACKLOG_CELLS: usize = 2_048;
const WATER_BACKLOG_BUDGET: Duration = Duration::from_micros(6_000);

fn water_budget(pending: usize) -> Duration {
    if pending > WATER_BACKLOG_CELLS {
        WATER_BACKLOG_BUDGET
    } else {
        FRAME_BUDGET
    }
}
const FACTORY_BUDGET: Duration = Duration::from_micros(750);
const RIGID_BUDGET: Duration = Duration::from_micros(600);
// Per-tick work cap. Wall time (`deadline`) is the real frame guard; this only
// stops one tick from monopolising a frame. The old 768 cap starved wet areas:
// a dam break with ~850 active cells flowed at a third of its intended speed.
const MAX_CELLS: usize = 8192;
// Generated cave floors hold hundreds of thousands of unsettled water cells per
// view. Seeding them all at load filled the queue faster than sources could be
// regenerated, so a player's own edit waited tens of seconds behind them.
// Seeds farther than this (in chunks) wait until the player comes near. Cave
// floors lie two to three chunks under the surface, so the vertical reach is short.
const SEED_RADIUS: i32 = 4;
const SEED_DEPTH: i32 = 1;
const MAX_DORMANT: usize = 8192;
// Chunks a tool edit touched are visited first on every cellular tick for this
// long. In the near lane they waited their turn in a rotation of up to 125
// backlogged chunks, so a broken block's sand or water moved about once per
// half second and physics looked paused.
const EDIT_PRIORITY_TICKS: u64 = 64;
const MAX_EDIT_CHUNKS: usize = 32;
struct Recovery {
    rx: mpsc::Receiver<Option<IVec3>>,
    generation: u64,
}
pub struct WorldSimulation {
    pub(crate) player_tick: engine_core::time::TickId,
    pub(crate) cpu_region: Option<super::cpu_region::Region>,
    pub(crate) authority_error: Option<String>,
    enabled: bool,
    scale: VoxelScalePreset,
    sources: Residency,
    tool_source: super::tool_source::Source,
    accumulator: f32,
    stats: FluidStats,
    near: VecDeque<IVec3>,
    ready: VecDeque<IVec3>,
    ready_set: HashSet<IVec3>,
    next_near: Instant,
    near_centers: Vec<IVec3>,
    recovery: Option<Recovery>,
    next_recovery: Instant,
    machine_cursor: usize,
    smeltery_work: world::smeltery::Work,
    springs: super::springs::Springs,
    visited: HashSet<IVec3>,
    touched: HashSet<IVec3>,
    // Reused per-chunk snapshot of pending cells; avoids a fresh allocation each tick.
    batch: Vec<IVec3>,
    // Load-time seeds outside `SEED_RADIUS`, merged when the player approaches.
    // Runtime only: a reload re-seeds from the streamed chunks.
    dormant: crate::fast_hash::FastMap<IVec3, Box<Mask>>,
    seed_centers: Vec<IVec3>,
    // Edited chunk -> last cellular tick it keeps priority. Runtime only.
    edited: crate::fast_hash::FastMap<IVec3, u64>,
    edit_cells: VecDeque<IVec3>,
    pub status: Option<String>,
}
impl WorldSimulation {
    /// Check authority deadlines before *any* gameplay writer runs this step.
    pub(crate) fn prepare_step(&mut self) -> bool {
        if self.authority_error.is_some() { return false; }
        let result = self.player_tick.next().ok_or_else(|| "player tick exhausted".to_string())
            .and_then(|tick| match (&mut self.cpu_region, tick.cellular_tick()) {
                (Some(region), Some(ca_tick)) => region.ready(ca_tick),
                _ => Ok(true),
            });
        if matches!(result, Ok(false)) {
            self.status = Some("Waiting for GPU cellular result".into());
            return false;
        }
        if let Err(error) = result {
            self.status = Some(error.clone());
            self.authority_error = Some(error);
            return false;
        }
        if self.cpu_region.is_some() { self.status = None; }
        true
    }
    /// The per-frame wall-clock budget the windowed game grants a pass.
    pub fn live_budget(world: &World) -> super::Budget {
        super::Budget::wall(water_budget(world.activity.pending()))
    }
    pub fn new() -> Self {
        Self {
            player_tick: Default::default(),
            cpu_region: None,
            authority_error: None,
            enabled: true,
            scale: VoxelScalePreset::OneMeter,
            sources: Residency::default(),
            tool_source: Default::default(),
            accumulator: 0.0,
            stats: FluidStats::default(),
            near: VecDeque::new(),
            ready: VecDeque::new(),
            ready_set: HashSet::new(),
            next_near: Instant::now(),
            near_centers: Vec::new(),
            recovery: None,
            next_recovery: Instant::now(),
            status: None,
            machine_cursor: 0,
            smeltery_work: Default::default(),
            springs: Default::default(),
            visited: HashSet::with_capacity(MAX_CELLS),
            touched: HashSet::with_capacity(128),
            batch: Vec::with_capacity(256),
            dormant: Default::default(),
            seed_centers: Vec::new(),
            edited: Default::default(),
            edit_cells: VecDeque::new(),
        }
    }
    /// Gives the chunks an edit at `p` can wake priority for a few seconds.
    fn prioritize_edit(&mut self, tick: u64, p: IVec3) {
        let until = tick.saturating_add(EDIT_PRIORITY_TICKS);
        for d in std::iter::once(IVec3::ZERO).chain(world::activity::WAKE_NEIGHBORS) {
            let c = chunk_coord_from_world_block(p + d);
            if let Some(t) = self.edited.get_mut(&c) {
                *t = until;
            } else if self.edited.len() < MAX_EDIT_CHUNKS {
                self.edited.insert(c, until);
            }
        }
    }
    #[cfg(test)]
    pub fn edit_priority_chunks(&self) -> usize {
        self.edited.len()
    }
    /// Wakes a streamed chunk's candidate cells now if near `player`, else later.
    pub fn seed(&mut self, world: &mut World, player: IVec3, c: IVec3, mask: Box<Mask>) {
        let center = chunk_coord_from_world_block(player);
        if within_seed_radius(center, c) {
            world.activity.merge(c, &mask);
            return;
        }
        if let Some(held) = self.dormant.get_mut(&c) {
            for (a, b) in held.iter_mut().zip(mask.iter()) {
                *a |= b;
            }
            return;
        }
        if self.dormant.len() >= MAX_DORMANT {
            // Forget the farthest; its water stays still until an edit wakes it.
            let far = self.dormant.keys().copied().max_by_key(|k| (*k - center).abs().max_element());
            match far {
                Some(far) if (far - center).abs().max_element() > (c - center).abs().max_element() => {
                    self.dormant.remove(&far);
                }
                _ => return,
            }
        }
        self.dormant.insert(c, mask);
        self.seed_centers.clear();
    }
    fn wake_dormant(&mut self, world: &mut World, centers: &[IVec3]) {
        if self.seed_centers == centers {
            return;
        }
        self.seed_centers.clear();
        self.seed_centers.extend_from_slice(centers);
        let near: Vec<IVec3> = self.dormant.keys().copied()
            .filter(|&c| centers.iter().any(|&center| within_seed_radius(center, c))).collect();
        for c in near {
            if let Some(mask) = self.dormant.remove(&c) {
                world.activity.merge(c, &mask);
            }
        }
    }
    pub fn dormant_chunks(&self) -> usize {
        self.dormant.len()
    }
    pub fn reset_for_world(&mut self, scale: VoxelScalePreset) {
        self.player_tick = Default::default();
        self.cpu_region = None;
        self.authority_error = None;
        self.scale = scale;
        self.sources.reset();
        self.tool_source.reset();
        self.accumulator = 0.0;
        self.machine_cursor = 0;
        self.smeltery_work = Default::default();
        self.springs = Default::default();
        self.stats = FluidStats::default();
        self.near.clear();
        self.ready.clear();
        self.ready_set.clear();
        self.recovery = None;
        self.next_near = Instant::now();
        self.near_centers.clear();
        self.next_recovery = Instant::now();
        self.status = None;
        self.dormant.clear();
        self.seed_centers.clear();
        self.edited.clear();
        self.edit_cells.clear();
    }
    /// Block until every requested chunk has been generated and queued. Used
    /// by headless runs so a tick never depends on worker timing.
    pub fn settle(&mut self, world: &mut World) {
        if self.cpu_region.is_some() { return; }
        loop {
            self.sources.poll(world);
            for c in self.sources.completed() {
                self.queue_ready(world, c);
            }
            if self.sources.is_idle() {
                return;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    pub fn set_enabled(&mut self, v: bool) {
        self.enabled = self.cpu_region.is_some() || v;
    }
    /// Cold player sources have an independent bounded worker. The stroke
    /// stays queued while it runs; no generation or disk wait enters the frame.
    pub fn prefetch_tool_edit(&mut self, world: &mut World, p: IVec3) {
        // Start cold generation during the swing, rather than after break time.
        // This loads source data only; it grants no terrain edit admission.
        if let super::tool_source::Admission::Failed(error) = self.tool_source.prepare(world, p) {
            self.status = Some(error);
        }
    }

    pub fn prepare_tool_edit(&mut self, world: &mut World, p: IVec3) -> bool {
        self.prioritize_edit(world.activity.tick, p);
        match self.tool_source.prepare(world,p) {
            super::tool_source::Admission::Ready=>{
                if !world.activity.can_wake_edit(p) {
                    self.tool_source.waiting = true;
                    return false;
                }
                world.environment.make_player_room(p);
                world.environment.player_edit=true;
                let affected=[p,p+IVec3::Y,p-IVec3::Y,p+IVec3::X,p-IVec3::X,p+IVec3::Z,p-IVec3::Z];
                if crate::world::environment::State::enabled(world.generator_version)
                    && (!world.environment.can_wake(&affected) || !world.environment.can_refine(&affected)
                        || !world.environment.can_invalidate_light(p)) {
                    world.environment.player_edit=false;
                    self.tool_source.waiting=true;
                    false
                }else{
                    for d in std::iter::once(IVec3::Y).chain(std::iter::once(IVec3::ZERO)).chain(world::activity::WAKE_NEIGHBORS) {
                        let cell=p+d;
                        if self.edit_cells.len()<256 && !self.edit_cells.contains(&cell) { self.edit_cells.push_back(cell); }
                    }
                    true
                }
            },
            super::tool_source::Admission::Waiting=>false,
            super::tool_source::Admission::Failed(error)=>{self.status=Some(error);false}
        }
    }
    pub fn tool_edit_waiting(&self)->bool { self.tool_source.waiting }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn stats(&self) -> FluidStats {
        self.stats
    }
    fn queue_ready(&mut self, world: &World, c: IVec3) {
        if self.ready.len() >= MAX_ACTIVE_CHUNKS {
            self.ready.retain(|c| world.activity.contains(*c));
            self.ready_set.retain(|c| world.activity.contains(*c));
        }
        if world.activity.contains(c) && self.ready_set.insert(c) {
            self.ready.push_back(c);
        }
    }
    fn recover(&mut self, world: &mut World) -> bool {
        if !world.activity.recovery {
            return false;
        }
        if self.recovery.is_none() && world.simulation_writeback_pending() {
            // Publish pre-scan edits first. Pause background mutations briefly
            // so a busy frontier cannot keep this barrier dirty forever.
            world.request_simulation_writeback();
            return true;
        }
        if self.recovery.is_none() && Instant::now() >= self.next_recovery {
            let (tx, rx) = mpsc::sync_channel(2);
            let view = world.regions.view.clone();
            match std::thread::Builder::new()
                .name("simulation-recovery".into())
                .spawn(move || {
                    for &c in view.records.keys() {
                        if tx.send(Some(c)).is_err() {
                            return;
                        }
                    }
                    let _ = tx.send(None);
                }) {
                Ok(_) => {
                    self.recovery = Some(Recovery {
                        rx,
                        generation: world.activity.recovery_generation,
                    })
                }
                Err(e) => {
                    self.status = Some(format!("Cannot resume simulation activity: {e}"));
                    self.next_recovery = Instant::now() + Duration::from_secs(2);
                }
            }
        }
        if world.activity.chunks() >= MAX_ACTIVE_CHUNKS - 16 {
            return false;
        }
        let Some(recovery) = &self.recovery else {
            return false;
        };
        // Cold/migrated checkpoints lack cell masks. Stream a chunk rescan at
        // a time without materializing a world-sized coordinate list.
        match recovery.rx.try_recv() {
            Ok(Some(c)) => {
                world.activity.merge(c, &[u64::MAX; 512]);
            }
            Ok(None) => {
                if recovery.generation == world.activity.recovery_generation {
                    world.activity.recovery = false;
                }
                self.recovery = None;
                self.next_recovery = Instant::now() + Duration::from_secs(2);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.recovery = None;
                self.next_recovery = Instant::now() + Duration::from_secs(2);
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        false
    }
    pub fn update(
        &mut self,
        world: &mut World,
        player: &crate::player::Player,
        dt: f32,
        blocks: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>,
        budget: super::Budget,
    ) {
        let mut trace=crate::gameplay_trace::Trace::new();
        let Some(tick) = self.player_tick.next() else {
            self.authority_error = Some("player tick exhausted".into());
            return;
        };
        if let Some(region) = self.cpu_region.as_mut() {
            if let Some(ca_tick) = tick.cellular_tick() {
                if let Err(error) = region.advance(ca_tick) {
                    self.authority_error = Some(error);
                    return;
                }
                world.publish_ca_view(region.published.clone(), blocks, chunks);
            }
            region.settle_results(world);
            self.player_tick = tick;
            return; // No legacy, factory extraction, rigid, thermal or residency writer.
        }
        self.player_tick = tick;
        self.stats.applied_updates_last_step = 0;
        self.stats.simulated_chunks_last_step = 0;
        self.sources.poll(world);
        for c in self.sources.completed() {
            self.queue_ready(world, c);
        }
        trace.mark("simulation-sources");
        let anchor = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
        let mut centers = vec![anchor];
        #[cfg(feature = "multiplayer")]
        if let Some(context) = &world.multiplayer {
            let scale = world.voxel_scale.meters();
            centers.extend(context.players.iter().map(|p|
                chunk_coord_from_world_block((p.feet / scale).floor().as_ivec3())));
            centers.sort_unstable_by_key(|p| (p.x, p.y, p.z));
            centers.dedup();
        }
        self.wake_dormant(world, &centers);
        world.activity.promote();
        let waiting_for_writeback = self.recover(world);
        if !waiting_for_writeback {
            world::smeltery::tick(
                world, dt, &mut self.sources, &mut self.smeltery_work,
                budget.child(Duration::from_micros(350)).min(budget),
                blocks, chunks,
            );
            if world.rules.systems.automation() { world::factory::tick_world(
                world,
                player,
                dt,
                &mut self.sources,
                &mut self.machine_cursor,
                budget.child(FACTORY_BUDGET).min(budget),
                blocks,
                chunks,
            ); }
        }
        trace.mark("simulation-factory-recovery");
        self.status = self.sources.error.clone().or(self.status.take());
        if self.enabled && !waiting_for_writeback && dt.is_finite() && dt > 0.0 {
            self.springs.update(world, &mut self.sources, player.pos, dt, budget);
            super::environment::update(world, &mut self.sources, player.pos.floor().as_ivec3(), blocks, chunks,
                budget.child(Duration::from_micros(500)).min(budget));
            trace.mark("environment");
            let rigid_end = budget.child(RIGID_BUDGET).min(budget);
            if rigid_end.has_time() {
                super::rigid::tick(
                    &mut self.sources,
                    world,
                    dt,
                    world::player_aabb(
                        player.pos,
                        world.voxel_scale,
                        player.scale,
                        player.crouch_t,
                    ),
                    rigid_end,
                    blocks,
                    chunks,
                );
            }
            trace.mark("rigid");
            // Fine voxel worlds are not rendered at 50-60 fluid Hz. A 30 Hz
            // floor keeps flow responsive without paying a full simulation
            // catch-up cost every rendered frame.
            let interval = (0.08 * self.scale.meters()).max(1.0 / 30.0);
            self.accumulator = (self.accumulator + dt).min(interval * 8.0);
            let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
            let water_end = budget.child(water_budget(world.activity.pending())).min(budget);
            let mut catch_up = 0;
            while self.accumulator >= interval && catch_up < 4 && water_end.has_time() {
                self.accumulator -= interval;
                catch_up += 1;
                world.activity.tick = world.activity.tick.wrapping_add(1);
                // Rebuild the near list when the player crosses a chunk, or
                // every half second. Rebuilding it every 125 ms at quarter
                // scale scanned thousands of inactive chunk coordinates.
                if self.near_centers != centers || Instant::now() >= self.next_near {
                    self.near.clear();
                    let r = (48.0 / (32.0 * self.scale.meters())).ceil() as i32;
                    self.near.extend(world.activity.active_chunks().filter(|&c|
                        centers.iter().any(|&a| (c - a).abs().max_element() <= r)));
                    self.near.make_contiguous().sort_by_cached_key(|c| {
                        let d = centers.iter().map(|&a| (*c - a).as_i64vec3().length_squared())
                            .min().unwrap_or(0);
                        (d, c.y, c.z, c.x)
                    });
                    self.near_centers.clone_from(&centers);
                    self.next_near = Instant::now() + Duration::from_millis(500);
                }
                let body = world::player_aabb(
                    player.pos,
                    world.voxel_scale,
                    player.scale,
                    player.crouch_t,
                );
                self.visited.clear();
                self.touched.clear();
                let tick = world.activity.tick;
                self.edited.retain(|_, until| *until >= tick);
                let mut edited: Vec<IVec3> =
                    self.edited.keys().copied().filter(|&c| world.activity.contains(c)).collect();
                edited.sort_unstable_by_key(|c| {
                    let d = (*c - center).as_i64vec3();
                    (d.length_squared(), c.y, c.z, c.x)
                });
                let mut cells = 0;
                // Nearby reactions must not wait behind a saved chunk's tens
                // of thousands of older wake bits. Use the same conservative
                // rules and wall budget, with the foreground event reserve.
                for _ in 0..self.edit_cells.len().min(64) {
                    if !water_end.has_time() { break; }
                    let p=self.edit_cells.pop_front().unwrap();
                    let c=chunk_coord_from_world_block(p);
                    if !self.sources.request_for(world,c,c) {
                        self.edit_cells.push_back(p);
                        continue;
                    }
                    if !world.activity.take_cell(p) { continue; }
                    if !self.visited.insert(p) { world.activity.wake(p); continue; }
                    let foreground=world.environment.player_edit;
                    world.environment.player_edit=true;
                    let changed=cellular::step(&mut self.sources,world,p,body,&mut self.visited,blocks,chunks);
                    world.environment.player_edit=foreground;
                    self.stats.applied_updates_last_step+=changed;
                    cells+=1;
                    // A failed conservative admission leaves the wake durable.
                    if changed==0 { self.edit_cells.push_back(p); }
                }
                for visit in 0..128 {
                    if cells >= MAX_CELLS || !water_end.has_time() {
                        break;
                    }
                    let edit = edited.get(visit).copied();
                    let preferred = if edit.is_none() && visit % 4 != 0 {
                        self.near.pop_front()
                    } else {
                        None
                    };
                    let c = edit.or(preferred).or_else(|| {
                        if (visit / 4 + world.activity.tick as usize).is_multiple_of(2)
                            && let Some(c) = self.ready.pop_front() {
                                self.ready_set.remove(&c);
                                return Some(c);
                            }
                        world.activity.next_fair()
                    });
                    let Some(c) = c else {
                        break;
                    };
                    if preferred.is_some() && world.activity.contains(c) {
                        self.near.push_back(c);
                    }
                    if !world.activity.contains(c) {
                        continue;
                    }
                    if !self.touched.insert(c) {
                        continue;
                    }
                    // Admit the central source before consuming work bits.
                    let admitted = if edit.is_some() || preferred.is_some() {
                        self.sources.request_for(world, c, c)
                    } else {
                        self.sources.request_distant(world, c, c)
                    };
                    if !admitted {
                        continue;
                    }
                    // Snapshot the chunk's whole pending set before stepping any of
                    // it. Work created by transfers in this chunk waits for a
                    // later pass, while the existing backlog can drain in one
                    // pass instead of 256 cells per tick.
                    let mut batch = std::mem::take(&mut self.batch);
                    batch.clear();
                    // Bound both snapshot construction and the deadline-expiry
                    // requeue. Draining 8192 bits then reawakening almost all of
                    // them escaped the wall budget on large mined/wet chunks.
                    let batch_limit=(MAX_CELLS-cells).min(1024);
                    while batch.len() < batch_limit && water_end.has_time() {
                        let part = world.activity.take(c, batch_limit - batch.len());
                        if part.is_empty() {
                            break;
                        }
                        batch.extend_from_slice(&part);
                    }
                    for (i, &p) in batch.iter().enumerate() {
                        if !water_end.has_time() {
                            for &p in &batch[i..] {
                                world.activity.wake(p);
                            }
                            break;
                        }
                        cells += 1;
                        if !self.visited.insert(p) {
                            world.activity.wake(p);
                            continue;
                        }
                        self.stats.applied_updates_last_step += cellular::step(
                            &mut self.sources,
                            world,
                            p,
                            body,
                            &mut self.visited,
                            blocks,
                            chunks,
                        );
                    }
                    self.batch = batch;
                    self.queue_ready(world, c);
                }
                self.stats.simulated_chunks_last_step += self.touched.len();
            }
            world.advance_ripples(dt);
        }
        trace.mark("cellular");
        self.stats.active_chunks = world.activity.chunks();
        self.stats.pending_updates = world.activity.pending();
        self.stats.inflight_chunks = self.sources.pending();
        self.stats.source_bytes = self.sources.bytes();
        self.stats.recovering = world.activity.recovery || world.activity.has_deferred();
        self.stats.source_failed = self.sources.error.is_some();
        self.stats.writeback_pressure = self.sources.memory_pressure || waiting_for_writeback;
    }
}

fn within_seed_radius(center: IVec3, c: IVec3) -> bool {
    let d = (c - center).abs();
    d.x <= SEED_RADIUS && d.z <= SEED_RADIUS && d.y <= SEED_DEPTH
}

/// Runs on terrain workers using the complete voxel halo. Settled ocean water
/// does not wake simply because it enters view; only potential motion is seeded.
pub(crate) fn discover(chunk: &GeneratedChunk) -> Option<Box<Mask>> {
    // Stream contiguous rows and use the guaranteed one-voxel halo. Avoid
    // coordinate conversion and six bounds checks for every candidate cell.
    const S: usize = crate::mesh::PADDED_CHUNK_SIZE as usize;
    const PLANE: usize = S * S;
    let mut mask: Option<Box<Mask>> = None;
    for y in 0..32usize {
        for z in 0..32usize {
            let row = ((y + 1) * S + z + 1) * S + 1;
            for x in 0..32usize {
                let i = row + x;
                let b = chunk.blocks[i];
                if !matches!(b, B::Water | B::SulfuricAcid | B::Sand | B::Gravel | B::Ash | B::RigidCore)
                    && !super::thermal::active(b)
                {
                    continue;
                }
                let down = chunk.blocks[i - PLANE];
                let unstable = if super::thermal::active(b) || b == B::RigidCore {
                    true
                } else if b == B::SulfuricAcid {
                    down == B::Air || [i - 1, i + 1, i - S, i + S]
                        .into_iter().any(|side| chunk.blocks[side] == B::Air)
                        || [i - PLANE, i + PLANE, i - 1, i + 1, i - S, i + S]
                            .into_iter().any(|side| super::thermal::acid_reaction(chunk.blocks[side]).is_some())
                } else if b == B::Water {
                    let p = chunk.chunk * crate::world::CHUNK_SIZE
                        + IVec3::new(x as i32, y as i32, z as i32);
                    // Saved seepage/river films can differ inside the same
                    // legacy eighth. Seed the exact surface gradient too.
                    let fill = chunk.water_fill(p);
                    let unstable = down == B::Air
                        || (down == B::Water
                            && chunk.water_fill(p - IVec3::Y) < 1.0)
                        || [i - 1, i + 1, i - S, i + S]
                            .iter()
                            .zip([IVec3::NEG_X, IVec3::X, IVec3::NEG_Z, IVec3::Z])
                            .any(|(&n, d)| {
                                let target = if chunk.blocks[n] == B::Water {
                                    chunk.water_fill(p + d)
                                } else {
                                    0.0
                                };
                                matches!(chunk.blocks[n], B::Air | B::Water)
                                    && (fill > target + 0.000001
                                        || (fill > 0.0 && fill <= 0.125
                                            && target == 0.0
                                            && matches!(
                                                chunk.blocks[n - PLANE],
                                                B::Air | B::Water
                                            )))
                            });
                    unstable && !chunk.flora_config.is_some_and(|cfg| {
                        p.y > cfg.water_level()
                            && engine_worldgen::steady_water_column(&cfg, p.x, p.z)
                                .is_some_and(|(bed, head)| p.y > bed && (p.y as f32) < head)
                    })
                } else {
                    matches!(down, B::Air | B::Water)
                        || [i - 1, i + 1, i - S, i + S].iter().any(|&n| {
                            matches!(chunk.blocks[n], B::Air | B::Water)
                                && matches!(chunk.blocks[n - PLANE], B::Air | B::Water)
                        })
                };
                if unstable {
                    let bit = (y * 32 + z) * 32 + x;
                    mask.get_or_insert_with(|| Box::new([0; 512]))[bit / 64] |= 1 << (bit % 64);
                }
            }
        }
    }
    mask
}

#[cfg(test)]
#[path = "seed_tests.rs"]
mod seed_tests;
#[cfg(test)]
#[path = "edit_priority_tests.rs"]
mod edit_priority_tests;
