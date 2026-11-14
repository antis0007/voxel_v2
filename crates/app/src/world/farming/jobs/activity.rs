//! Individual commitments advance by elapsed simulation time, not service count.
//! Geometry certificates are shared by execution and presentation. Edits interrupt
//! subscribers before mutation; inventories are changed only by the existing executor.
use super::*;
use engine_core::fast_hash::FastSet;
use std::collections::{BTreeMap, BTreeSet};
mod legacy;
pub(super) use legacy::migrate as migrate_legacy_commitment;

const MAX_CERTIFICATE_CELLS: usize = 4096;
const MAX_CHUNKS: usize = 64;
const CERTIFICATES: usize = 512;
const PROBES_PER_TICK: usize = 4096;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Stamp {
    chunk: [i32; 3],
    version: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum Kind {
    Work { task: Task, base: u8, required: u8 },
    Walk { from: [f32; 3], to: [f32; 3] },
    Rest { at: [f32; 3] },
    Wait { at: [f32; 3] },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Commitment {
    #[serde(default)]
    configuration: u64,
    start: u64,
    pub(super) end: u64,
    kind: Kind,
    stamps: Vec<Stamp>,
}

impl Commitment {
    pub(super) fn valid(&self) -> bool {
        self.end >= self.start
            && self.end - self.start <= 120 * 60
            && !self.stamps.is_empty()
            && self.stamps.len() <= MAX_CHUNKS
            && self
                .stamps
                .iter()
                .all(|s| s.chunk.iter().all(|n| n.unsigned_abs() < 1 << 28))
            && match self.kind {
                Kind::Work {
                    task,
                    base,
                    required,
                } => task.valid() && base <= required && required <= 60,
                Kind::Walk { from, to } => {
                    Vec3::from(from).is_finite()
                        && Vec3::from(to).is_finite()
                        && Vec3::from(from).distance_squared(to.into()) <= 24.0 * 24.0
                }
                Kind::Rest { at } | Kind::Wait { at } => Vec3::from(at).is_finite(),
            }
    }

    fn current(&self, w: &World) -> bool {
        self.configuration == w.farming.activities.fingerprint
            && w.farming.activities.configuration == Some(w.sample_config())
            && self
                .stamps
                .iter()
                .all(|s| w.regions.source_version(s.chunk.into()) == s.version)
    }

    pub(super) fn position(&self, now: u64) -> Option<Vec3> {
        match self.kind {
            Kind::Walk { from, to } => {
                let t =
                    now.saturating_sub(self.start) as f64 / (self.end - self.start).max(1) as f64;
                Some(level_walk::point(from.into(), to.into(), t.min(1.0) as f32))
            }
            Kind::Rest { at } | Kind::Wait { at } => Some(at.into()),
            Kind::Work { .. } => None,
        }
    }

    pub(super) fn progress(&self, now: u64) -> Option<u8> {
        match self.kind {
            Kind::Work { base, required, .. } => Some(
                (u64::from(base) + now.saturating_sub(self.start) / PULSE).min(u64::from(required))
                    as u8,
            ),
            _ => None,
        }
    }

    pub(super) fn waiting(&self) -> bool {
        matches!(self.kind, Kind::Wait { .. })
    }

    pub(super) fn wake_at(&self, j: &State, id: u32) -> u64 {
        let Some(person) = crew::view(j, id) else {
            return self.end;
        };
        // Missions own their carried provisions and rest deadlines.
        if person.mission {
            return self.end;
        }
        let meal = j
            .production
            .as_ref()
            .and_then(|p| p.next_meal.get(&id))
            .copied();
        let routine = (person.routine.phase == routine::Phase::Work).then_some(person.routine.due);
        meal.into_iter()
            .chain(routine)
            .filter(|&at| at > self.start)
            .fold(self.end, u64::min)
    }
}

/// Derived subscriptions only. Saved commitments rebuild these after loading.
#[derive(Clone, Debug, Default)]
pub(crate) struct Runtime {
    configuration: Option<engine_voxel::WorldSampleConfig>,
    fingerprint: u64,
    pub(super) navigation: navigation::Pending,
    indexed: bool,
    chunks: BTreeMap<[i32; 3], BTreeSet<u32>>,
    people: BTreeMap<u32, Vec<[i32; 3]>>,
    invalidated: BTreeMap<u32, u64>,
    certificates: BTreeMap<[u32; 7], Vec<Stamp>>,
    probe_clock: u64,
    probes: usize,
    certificate_hits: u64,
    pub(crate) completed: u64,
    pub(crate) interrupted: u64,
}

impl Runtime {
    pub(super) fn invalidated_at(&self, id: u32) -> Option<u64> {
        self.invalidated.get(&id).copied()
    }

    fn register(&mut self, id: u32, commitment: Option<&Commitment>) {
        if let Some(old) = self.people.remove(&id) {
            for chunk in old {
                if let Some(ids) = self.chunks.get_mut(&chunk) {
                    ids.remove(&id);
                    if ids.is_empty() {
                        self.chunks.remove(&chunk);
                    }
                }
            }
        }
        if let Some(c) = commitment {
            let chunks: Vec<_> = c.stamps.iter().map(|s| s.chunk).collect();
            for &chunk in &chunks {
                self.chunks.entry(chunk).or_default().insert(id);
            }
            self.people.insert(id, chunks);
        }
    }
}

pub(super) fn rebuild(w: &mut World) {
    communities::restore_index(w);
    let configuration = w.sample_config();
    if w.farming.activities.configuration != Some(configuration) {
        let text = format!("{configuration:?}");
        let fingerprint = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |n, b| {
            (n ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        });
        w.farming.activities.configuration = Some(configuration);
        w.farming.activities.fingerprint = fingerprint;
        w.farming.activities.navigation = navigation::Pending::default();
        w.farming.activities.certificates.clear();
    }
    if w.farming.activities.indexed {
        return;
    }
    let mut runtime = Runtime {
        indexed: true,
        ..Runtime::default()
    };
    runtime.configuration = w.farming.activities.configuration;
    runtime.fingerprint = w.farming.activities.fingerprint;
    for jobs in std::iter::once(&w.farming.jobs).chain(communities::all_jobs(w)) {
        for id in crew::ids(jobs) {
            runtime.register(id, crew::view(jobs, id).and_then(|p| p.activity));
        }
    }
    w.farming.activities = runtime;
}

pub(super) fn register(w: &mut World, j: &State) {
    if let Some(id) = j.worker {
        w.farming.activities.register(id, j.activity.as_ref());
    }
}

pub(super) fn forget(w: &mut World, id: u32) {
    w.farming.activities.register(id, None);
    w.farming.activities.invalidated.remove(&id);
}

pub(super) fn report(w: &World) -> serde_json::Value {
    let a = &w.farming.activities;
    serde_json::json!({"commitments":a.people.len(),"watched_chunks":a.chunks.len(),
        "completed":a.completed,"interruptions":a.interrupted,
        "navigation_nodes":a.navigation.expanded,"shared_route_hits":a.navigation.hits,
        "corridor_cache_hits":a.certificate_hits,"corridor_probes_this_tick":a.probes})
}

pub(crate) fn prepare_corridor(w: &mut World, from: Vec3, to: Vec3) -> bool {
    let v = w.voxel_scale.meters();
    let lo = crate::world::chunk_coord_from_world_block(
        ((from.min(to) - Vec3::new(1.0, 2.0, 1.0)) / v)
            .floor()
            .as_ivec3(),
    );
    let hi = crate::world::chunk_coord_from_world_block(
        ((from.max(to) + Vec3::new(1.0, 3.0, 1.0)) / v)
            .floor()
            .as_ivec3(),
    );
    let size = (hi - lo + IVec3::ONE).as_i64vec3();
    if size.x * size.y * size.z > 256 {
        return false;
    }
    let mut cells = Vec::new();
    for x in lo.x..=hi.x {
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                cells.push(IVec3::new(x, y, z) * crate::world::CHUNK_SIZE);
            }
        }
    }
    residency::prepare_cells(w, cells)
}

pub(super) fn wake_disrupted(w: &mut World) {
    let ids: Vec<_> = w
        .adventure
        .enemies
        .iter()
        .filter_map(|a| {
            let id = vehicle::villager_id(a)?;
            (w.farming.activities.people.contains_key(&id)
                && (!vehicle::ResidentBody::capture(a).can_schedule() || a.health <= 0.0))
                .then_some(id)
        })
        .collect();
    for id in ids {
        communities::interrupt_activity(w, id);
        w.farming.activities.register(id, None);
    }
}

/// Called before a voxel mutation, with no search or world mutation of its own.
pub(crate) fn before_edit(w: &mut World, at: IVec3) {
    if !w.sample_config().civilization_enabled() {
        return;
    }
    rebuild(w);
    let chunk = crate::world::chunk_coord_from_world_block(at).to_array();
    let Some(ids) = w.farming.activities.chunks.get(&chunk).cloned() else {
        return;
    };
    for id in ids {
        let at = w
            .farming
            .activities
            .invalidated_at(id)
            .unwrap_or(w.farming.clock);
        if communities::interrupt_activity_at(w, id, at) {
            w.farming.activities.invalidated.remove(&id);
        } else {
            w.farming.activities.invalidated.entry(id).or_insert(at);
        }
        w.farming.activities.register(id, None);
        w.farming.activities.interrupted = w.farming.activities.interrupted.saturating_add(1);
    }
}

/// An executor may temporarily own its town outside World while publishing an
/// edit. Retain the interruption tick until that owner is back in its table.
pub(crate) fn flush(w: &mut World) {
    let pending = std::mem::take(&mut w.farming.activities.invalidated);
    for (id, at) in pending {
        communities::interrupt_activity_at(w, id, at);
    }
}

pub(super) fn interrupt(j: &mut State, now: u64) {
    let Some(c) = j.activity.take() else {
        return;
    };
    if let Some(position) = c.position(now) {
        j.worker_pos = Some(position.to_array());
        j.motion_at = now;
        j.path.clear();
    }
    if let Some(work) = c.progress(now) {
        j.work = work;
        if let Some(batch) = &mut j.batch {
            batch.retain_progress(work);
        }
    }
    if matches!(c.kind, Kind::Rest { .. }) {
        j.routine.dwell = j
            .routine
            .dwell
            .saturating_add(now.min(c.end).saturating_sub(c.start));
        j.routine.last = now;
    }
    j.retry_at = now;
}

fn stamps(w: &World, cells: impl IntoIterator<Item = IVec3>) -> Option<Vec<Stamp>> {
    let mut chunks = BTreeSet::new();
    for cell in cells {
        let chunk = crate::world::chunk_coord_from_world_block(cell);
        if !w.chunks.contains_key(&chunk) {
            return None;
        }
        chunks.insert(chunk.to_array());
        if chunks.len() > MAX_CHUNKS {
            return None;
        }
    }
    Some(
        chunks
            .into_iter()
            .map(|chunk| Stamp {
                chunk,
                version: w.regions.source_version(chunk.into()),
            })
            .collect(),
    )
}

fn stationary_stamps(
    w: &World,
    j: &State,
    feet: Vec3,
    dependency: Option<IVec3>,
) -> Option<Vec<Stamp>> {
    let body = j.body.filter(|b| b.can_schedule())?;
    let v = w.voxel_scale.meters();
    let foot = (feet / v).floor().as_ivec3();
    // A quiet body may have just started falling. Only supported, ordinary
    // ground permits skipping its physical steps.
    if feet.y - foot.y as f32 * v > (v * 0.25).min(0.05) {
        return None;
    }
    let floor = w.resident_block(foot - IVec3::Y)?;
    if !floor.is_solid()
        || engine_content::shaped::is_shaped(floor)
        || engine_content::fittings::state(floor).is_some()
    {
        return None;
    }
    let dimensions = vehicle::resident_dimensions(body.kind());
    let radius = (dimensions.x.max(dimensions.z) * 0.5).max(0.4);
    let mut lo = ((feet - Vec3::new(radius, v, radius)) / v)
        .floor()
        .as_ivec3();
    let mut hi = ((feet + Vec3::new(radius, dimensions.y.max(1.8), radius)) / v)
        .floor()
        .as_ivec3();
    if let Some(at) = dependency {
        lo = lo.min(at);
        hi = hi.max(at);
    }
    let lo = crate::world::chunk_coord_from_world_block(lo);
    let hi = crate::world::chunk_coord_from_world_block(hi);
    let size = (hi - lo + IVec3::ONE).as_i64vec3();
    if size.x * size.y * size.z > MAX_CHUNKS as i64 {
        return None;
    }
    let mut cells = Vec::new();
    for x in lo.x..=hi.x {
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                cells.push(IVec3::new(x, y, z) * crate::world::CHUNK_SIZE);
            }
        }
    }
    stamps(w, cells)
}

/// Accepted inputs already belong to this worker. Completion changes no stock here.
pub(super) fn work(w: &mut World, j: &mut State, required: u8, at: IVec3) -> bool {
    let Some(task) = j.task else {
        return false;
    };
    let now = w.farming.clock;
    if let Some(c) = &j.activity {
        if matches!(c.kind, Kind::Work { task: old, .. } if old == task) && c.current(w) {
            j.work = c.progress(now).unwrap_or(j.work);
            if j.work >= required {
                j.activity = None;
                w.farming.activities.completed = w.farming.activities.completed.saturating_add(1);
                return true;
            }
            return false;
        }
        interrupt(j, now);
    }
    if j.work >= required {
        return true;
    }
    if j.body.is_none_or(|b| !b.can_schedule()) {
        j.work = j.work.saturating_add(1).min(required);
        return j.work >= required;
    }
    let Some(feet) = j.worker_pos.map(Vec3::from) else {
        return false;
    };
    let Some(stamps) = stationary_stamps(w, j, feet, Some(at)) else {
        j.work = j.work.saturating_add(1).min(required);
        return j.work >= required;
    };
    j.activity = Some(Commitment {
        configuration: w.farming.activities.fingerprint,
        start: now,
        end: now.saturating_add(u64::from(required - j.work) * PULSE),
        kind: Kind::Work {
            task,
            base: j.work,
            required,
        },
        stamps,
    });
    false
}

pub(super) fn rest(w: &World, j: &mut State, feet: Vec3, remaining: u64) {
    if remaining == 0 || j.activity.is_some() {
        return;
    }
    let Some(stamps) = stationary_stamps(w, j, feet, None) else {
        return;
    };
    j.activity = Some(Commitment {
        configuration: w.farming.activities.fingerprint,
        start: w.farming.clock,
        end: w.farming.clock.saturating_add(remaining),
        kind: Kind::Rest {
            at: feet.to_array(),
        },
        stamps,
    });
}

pub(super) fn wait(w: &World, j: &mut State, dependency: IVec3, until: u64) {
    if j.body.is_none_or(|b| !b.can_schedule()) {
        return;
    }
    let Some(at) = j.worker_pos else {
        return;
    };
    let Some(stamps) = stationary_stamps(w, j, at.into(), Some(dependency)) else {
        return;
    };
    j.activity = Some(Commitment {
        configuration: w.farming.activities.fingerprint,
        start: w.farming.clock,
        end: until.max(w.farming.clock + 1),
        kind: Kind::Wait { at },
        stamps,
    });
    j.body = j.body.map(vehicle::ResidentBody::parked);
}

/// Certify only quiet, level corridors. Stairs, water and status effects keep
/// their physical executor; no unchecked shortcut is granted by being offscreen.
pub(super) fn travel(w: &mut World, j: &mut State) {
    if j.activity.is_some() || j.body.is_none_or(|b| !b.can_schedule()) {
        return;
    }
    let Some(from) = j.worker_pos.map(Vec3::from) else {
        return;
    };
    let Some(first) = j.path.front().copied() else {
        return;
    };
    let v = w.voxel_scale.meters();
    let clearance = (v * 0.25).min(0.05);
    if (first.y - from.y).abs() > clearance || from.y - (from.y / v).floor() * v > clearance {
        return;
    }
    let mut to = Vec3::new(first.x, from.y, first.z);
    let direction = (to - from).normalize_or_zero();
    for &point in j.path.iter().skip(1) {
        let flat = Vec3::new(point.x, from.y, point.z);
        let delta = flat - from;
        if delta.length() > 12.0
            || (point.y - from.y).abs() > clearance
            || delta.normalize_or_zero().dot(direction) < 0.9999
        {
            break;
        }
        to = flat;
    }
    let distance = from.distance(to);
    if !(0.4..=12.0).contains(&distance) {
        return;
    }
    let key = [
        from.x.to_bits(),
        from.y.to_bits(),
        from.z.to_bits(),
        to.x.to_bits(),
        to.y.to_bits(),
        to.z.to_bits(),
        j.body.unwrap().kind(),
    ];
    if let Some(cached) = w.farming.activities.certificates.get(&key)
        && cached
            .iter()
            .all(|s| w.regions.source_version(s.chunk.into()) == s.version)
    {
        let stamps = cached.clone();
        w.farming.activities.certificate_hits =
            w.farming.activities.certificate_hits.saturating_add(1);
        start_walk(w, j, from, to, stamps);
        return;
    }
    if w.farming.activities.probe_clock != w.farming.clock {
        w.farming.activities.probe_clock = w.farming.clock;
        w.farming.activities.probes = 0;
    }
    // A bounded swept volume, including support, never invokes world generation.
    let steps = (distance / (v * 0.5)).ceil() as usize;
    if steps > 128 {
        return;
    }
    let dimensions = vehicle::resident_dimensions(j.body.unwrap().kind());
    let radius = (dimensions.x.max(dimensions.z) * 0.5).max(0.4);
    let mut cells = FastSet::default();
    for i in 0..=steps {
        let point = level_walk::point(from, to, i as f32 / steps.max(1) as f32);
        let lo = ((point - Vec3::new(radius, 0.0, radius)) / v)
            .floor()
            .as_ivec3();
        let hi = ((point + Vec3::new(radius, dimensions.y.max(1.8) - 0.001, radius)) / v)
            .floor()
            .as_ivec3();
        for x in lo.x..=hi.x {
            for z in lo.z..=hi.z {
                for y in (lo.y - 1)..=hi.y {
                    let cell = IVec3::new(x, y, z);
                    if !cells.insert(cell.to_array()) {
                        continue;
                    }
                    if cells.len() > MAX_CERTIFICATE_CELLS
                        || w.farming.activities.probes >= PROBES_PER_TICK
                    {
                        return;
                    }
                    w.farming.activities.probes += 1;
                    let Some(block) = w.resident_block(cell) else {
                        return;
                    };
                    if y == lo.y - 1 {
                        if !block.is_solid()
                            || engine_content::shaped::is_shaped(block)
                            || engine_content::fittings::state(block).is_some()
                        {
                            return;
                        }
                    } else if block != B::Air {
                        return;
                    }
                }
            }
        }
    }
    let Some(stamps) = stamps(w, cells.into_iter().map(IVec3::from)) else {
        return;
    };
    if w.farming.activities.certificates.len() >= CERTIFICATES {
        w.farming.activities.certificates.pop_first();
    }
    w.farming
        .activities
        .certificates
        .insert(key, stamps.clone());
    start_walk(w, j, from, to, stamps);
}

fn start_walk(w: &World, j: &mut State, from: Vec3, to: Vec3, stamps: Vec<Stamp>) {
    let distance = from.distance(to);
    let speed = vehicle::resident_walk_speed(j.body.unwrap().kind()) * task_route_speed(j.task);
    let duration =
        (distance / speed.max(0.01) / engine_core::time::PLAYER_STEP_SECONDS).ceil() as u64;
    j.activity = Some(Commitment {
        configuration: w.farming.activities.fingerprint,
        start: w.farming.clock,
        end: w.farming.clock.saturating_add(duration),
        kind: Kind::Walk {
            from: from.to_array(),
            to: to.to_array(),
        },
        stamps,
    });
    j.body = j.body.map(vehicle::ResidentBody::parked);
}

/// Returns true while a certified commitment owns this worker. Arrival is the
/// same sampled position for a retained individual and a rendered actor.
pub(super) fn resume(w: &mut World, j: &mut State) -> bool {
    if let Some(at) = j
        .worker
        .and_then(|id| w.farming.activities.invalidated.remove(&id))
    {
        interrupt(j, at);
        return false;
    }
    let Some(c) = j.activity.as_ref() else {
        return false;
    };
    if !c.current(w) {
        // A mutation outside the normal edit adapter did not establish when the
        // certificate ceased to hold. Do not award unproven work or movement.
        let start = c.start;
        interrupt(j, start);
        j.motion_at = w.farming.clock;
        return false;
    }
    if j.body.is_some_and(|b| !b.can_schedule()) {
        interrupt(j, w.farming.clock);
        return false;
    }
    let wake = j.worker.map_or(c.end, |id| c.wake_at(j, id));
    if wake < c.end && w.farming.clock >= wake {
        interrupt(j, wake);
        return false;
    }
    if let Some(position) = c.position(w.farming.clock) {
        j.worker_pos = Some(position.to_array());
    }
    if let Some(progress) = c.progress(w.farming.clock) {
        j.work = progress;
        if let Some(batch) = &mut j.batch {
            batch.retain_progress(progress);
        }
    }
    j.motion_at = w.farming.clock;
    if w.farming.clock < c.end {
        return true;
    }
    let walking = matches!(c.kind, Kind::Walk { .. });
    let resting = matches!(c.kind, Kind::Rest { .. });
    let waiting = c.waiting();
    if walking || resting || waiting {
        j.motion_at = w.farming.clock;
        j.activity = None;
        if walking {
            let feet = Vec3::from(j.worker_pos.unwrap());
            while j
                .path
                .front()
                .is_some_and(|p| (*p - feet).length_squared() > 0.01)
            {
                j.path.pop_front();
            }
            j.path.pop_front();
            j.last_walk = None;
            j.stalled = 0;
        }
        if resting {
            j.routine.dwell = routine::REST_TICKS;
            j.routine.last = w.farming.clock;
        }
        w.farming.activities.completed = w.farming.activities.completed.saturating_add(1);
    }
    false
}

pub(crate) fn pose(w: &World, id: u32) -> Option<Vec3> {
    if !enabled(w) {
        return None;
    }
    let j = resident_state(w, id)?;
    let view = crew::view(j, id)?;
    let c = view.activity?;
    c.current(w).then(|| c.position(w.farming.clock)).flatten()
}
