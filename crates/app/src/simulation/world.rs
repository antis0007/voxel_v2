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
    accumulator: f32,
    stats: FluidStats,
    near: VecDeque<IVec3>,
    ready: VecDeque<IVec3>,
    ready_set: HashSet<IVec3>,
    next_near: Instant,
    near_center: Option<IVec3>,
    recovery: Option<Recovery>,
    next_recovery: Instant,
    machine_cursor: usize,
    visited: HashSet<IVec3>,
    touched: HashSet<IVec3>,
    // Reused per-chunk snapshot of pending cells; avoids a fresh allocation each tick.
    batch: Vec<IVec3>,
    pub status: Option<String>,
}
impl WorldSimulation {
    /// Check authority deadlines before *any* gameplay writer runs this step.
    pub(crate) fn prepare_step(&mut self) -> bool {
        if self.authority_error.is_some() { return false; }
        let result = self.player_tick.next().ok_or_else(|| "player tick exhausted".to_string())
            .and_then(|tick| match (&self.cpu_region, tick.cellular_tick()) {
                (Some(region), Some(ca_tick)) => region.ready(ca_tick),
                _ => Ok(()),
            });
        if let Err(error) = result {
            self.status = Some(error.clone());
            self.authority_error = Some(error);
            return false;
        }
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
            accumulator: 0.0,
            stats: FluidStats::default(),
            near: VecDeque::new(),
            ready: VecDeque::new(),
            ready_set: HashSet::new(),
            next_near: Instant::now(),
            near_center: None,
            recovery: None,
            next_recovery: Instant::now(),
            status: None,
            machine_cursor: 0,
            visited: HashSet::with_capacity(MAX_CELLS),
            touched: HashSet::with_capacity(128),
            batch: Vec::with_capacity(256),
        }
    }
    pub fn reset_for_world(&mut self, scale: VoxelScalePreset) {
        self.player_tick = Default::default();
        self.cpu_region = None;
        self.authority_error = None;
        self.scale = scale;
        self.sources.reset();
        self.accumulator = 0.0;
        self.machine_cursor = 0;
        self.stats = FluidStats::default();
        self.near.clear();
        self.ready.clear();
        self.ready_set.clear();
        self.recovery = None;
        self.next_near = Instant::now();
        self.near_center = None;
        self.next_recovery = Instant::now();
        self.status = None;
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
        self.enabled = v;
    }
    // Gameplay uses the same bounded asynchronous source loader as simulation.
    // First contact with a fine-resolution chunk must not generate it on the
    // render thread before the first voxel can be mined or placed.
    pub fn prepare_tool_edit(&mut self, world: &mut World, p: IVec3) -> bool {
        let coord = chunk_coord_from_world_block(p);
        if world.chunks.contains_key(&coord) {
            return true;
        }
        self.sources.poll(world);
        self.sources.request(world, coord) && self.sources.prepare(world, &[p])
    }
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
                world.ca_view = Some(region.published.clone());
            }
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
        world.activity.promote();
        let waiting_for_writeback = self.recover(world);
        if !waiting_for_writeback {
            world::factory::tick_world(
                world,
                player,
                dt,
                &mut self.sources,
                &mut self.machine_cursor,
                budget.child(FACTORY_BUDGET).min(budget),
                blocks,
                chunks,
            );
        }
        self.status = self.sources.error.clone().or(self.status.take());
        if self.enabled && !waiting_for_writeback && dt.is_finite() && dt > 0.0 {
            super::environment::update(world, &mut self.sources, player.pos.floor().as_ivec3(), blocks, chunks,
                budget.child(Duration::from_micros(500)).min(budget));
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
                if self.near_center != Some(center) || Instant::now() >= self.next_near {
                    self.near.clear();
                    let r = (48.0 / (32.0 * self.scale.meters())).ceil() as i32;
                    for y in -r..=r {
                        for z in -r..=r {
                            for x in -r..=r {
                                let c = center + IVec3::new(x, y, z);
                                if world.activity.contains(c) {
                                    self.near.push_back(c);
                                }
                            }
                        }
                    }
                    self.near.make_contiguous().sort_unstable_by_key(|c| {
                        let d = (*c - center).as_i64vec3();
                        (d.length_squared(), c.y, c.z, c.x)
                    });
                    self.near_center = Some(center);
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
                let mut cells = 0;
                for visit in 0..128 {
                    if cells >= MAX_CELLS || !water_end.has_time() {
                        break;
                    }
                    let preferred = if visit % 4 != 0 {
                        self.near.pop_front()
                    } else {
                        None
                    };
                    let c = preferred.or_else(|| {
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
                    if !self.sources.request_for(world, c, c) {
                        continue;
                    }
                    // Snapshot the chunk's whole pending set before stepping any of
                    // it. Cells woken by this tick's transfers wait for the next
                    // tick, so a front still advances one cell per tick, while a
                    // backlog drains in one pass instead of 256 cells per tick.
                    let mut batch = std::mem::take(&mut self.batch);
                    batch.clear();
                    while cells + batch.len() < MAX_CELLS {
                        let part = world.activity.take(c, MAX_CELLS - cells - batch.len());
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
        self.stats.active_chunks = world.activity.chunks();
        self.stats.pending_updates = world.activity.pending();
        self.stats.inflight_chunks = self.sources.pending();
        self.stats.source_bytes = self.sources.bytes();
        self.stats.recovering = world.activity.recovery || world.activity.has_deferred();
        self.stats.source_failed = self.sources.error.is_some();
        self.stats.writeback_pressure = self.sources.memory_pressure || waiting_for_writeback;
    }
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
                if !matches!(b, B::Water | B::Sand | B::Gravel | B::Ash | B::RigidCore)
                    && !super::thermal::active(b)
                {
                    continue;
                }
                let down = chunk.blocks[i - PLANE];
                let unstable = if super::thermal::active(b) || b == B::RigidCore {
                    true
                } else if b == B::Water {
                    let p = chunk.chunk * crate::world::CHUNK_SIZE
                        + IVec3::new(x as i32, y as i32, z as i32);
                    let volume = chunk.water_levels.get(&p).copied().unwrap_or(8);
                    down == B::Air
                        || (down == B::Water
                            && chunk
                                .water_levels
                                .get(&(p - IVec3::Y))
                                .copied()
                                .unwrap_or(8)
                                < 8)
                        || [i - 1, i + 1, i - S, i + S]
                            .iter()
                            .zip([IVec3::NEG_X, IVec3::X, IVec3::NEG_Z, IVec3::Z])
                            .any(|(&n, d)| {
                                let target = if chunk.blocks[n] == B::Water {
                                    chunk.water_levels.get(&(p + d)).copied().unwrap_or(8)
                                } else {
                                    0
                                };
                                matches!(chunk.blocks[n], B::Air | B::Water)
                                    && (volume.saturating_sub(target) > 1
                                        || (volume == 1
                                            && target == 0
                                            && matches!(
                                                chunk.blocks[n - PLANE],
                                                B::Air | B::Water
                                            )))
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
