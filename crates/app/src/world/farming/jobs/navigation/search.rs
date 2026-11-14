//! Bounded, resumable searches. Waiting on terrain retains the frontier.
use super::super::*;
use std::collections::{BTreeMap, BTreeSet};

const SEARCHES: usize = 32;
const NODES_PER_TICK: usize = 512;
const NODES_PER_WAKE: usize = 128;

#[derive(Clone, Debug)]
pub(in super::super) enum Route {
    Ready(VecDeque<Vec3>),
    Pending,
    Blocked,
}

#[derive(Clone, Debug, Default)]
pub(in super::super) struct Pending {
    searches: BTreeMap<u32, Search>,
    requests: BTreeMap<u32, Request>,
    queued: VecDeque<u32>,
    active: VecDeque<u32>,
    cache: VecDeque<Cached>,
    ready: BTreeSet<u32>,
    cleanup: u32,
    pub expanded: u64,
    pub hits: u64,
}

#[derive(Clone, Debug)]
struct Request {
    feet: Vec3,
    task: Task,
    limit: usize,
    range: f32,
    result: Option<Route>,
    queued: bool,
}

#[derive(Clone, Debug)]
struct Cached {
    feet: Vec3,
    task: Task,
    path: VecDeque<Vec3>,
    versions: BTreeMap<[i32; 3], Option<u64>>,
}

#[derive(Clone, Debug)]
struct Search {
    task: Task,
    start: IVec3,
    feet: Vec3,
    limit: usize,
    range: f32,
    queue: BinaryHeap<Reverse<(u32, Reverse<u32>, [i32; 3])>>,
    previous: HashMap<IVec3, (IVec3, u32)>,
    versions: BTreeMap<[i32; 3], Option<u64>>,
}

impl Search {
    fn priority(&self, p: IVec3, cost: u32, scale: f32) -> u32 {
        let reach = if matches!(self.task, Task::Build(_)) {
            2.25
        } else {
            1.65
        };
        cost.saturating_add(
            (((metres(p, scale) + Vec3::Y * 0.7).distance(self.task.point(scale)) - reach).max(0.0)
                / scale
                * 900.0)
                .floor() as u32,
        )
    }

    fn new(w: &World, feet: Vec3, task: Task, limit: usize, range: f32) -> Self {
        let v = w.voxel_scale.meters();
        let start = ((feet + Vec3::Y * 0.15) / v).floor().as_ivec3();
        let mut s = Self {
            task,
            start,
            feet,
            limit: limit.min(4096),
            range,
            queue: BinaryHeap::new(),
            previous: HashMap::from([(start, (start, 0))]),
            versions: BTreeMap::new(),
        };
        s.queue.push(Reverse((
            s.priority(start, 0, v),
            Reverse(0),
            start.to_array(),
        )));
        s
    }

    fn advance(&mut self, w: &World, budget: usize) -> (Route, Vec<IVec3>, usize) {
        let v = w.voxel_scale.meters();
        let mut query = super::Query::new(w, v);
        let mut spent = 0;
        while spent < budget {
            let Some(entry @ Reverse((_, Reverse(cost), cell))) = self.queue.pop() else {
                return (Route::Blocked, Vec::new(), spent);
            };
            spent += 1;
            let at = IVec3::from(cell);
            if self.previous[&at].1 != cost {
                continue;
            }
            if query.stand(at)
                && let Some(point) = super::work_point(w, at, v)
                && reachable(w, point, self.task, v)
                && [-0.1, 0.1].into_iter().all(|x| {
                    [-0.1, 0.1]
                        .into_iter()
                        .all(|z| reachable(w, point + Vec3::new(x, 0.0, z), self.task, v))
                })
            {
                let mut route = VecDeque::new();
                let mut cell = at;
                while cell != self.start {
                    route.push_front(metres(cell, v));
                    cell = self.previous[&cell].0;
                }
                if let Some(last) = route.back_mut() {
                    *last = point;
                }
                if route.is_empty() && !reachable(w, self.feet, self.task, v) {
                    route.push_back(point);
                }
                self.remember(w, &query);
                return (Route::Ready(route), Vec::new(), spent);
            }
            if self.previous.len() >= self.limit {
                return (Route::Blocked, Vec::new(), spent);
            }
            for delta in super::DIRECTIONS {
                let diagonal = delta.x != 0 && delta.z != 0;
                if diagonal
                    && (!query.stand(at + IVec3::X * delta.x)
                        || !query.stand(at + IVec3::Z * delta.z))
                {
                    continue;
                }
                let height = if diagonal {
                    0
                } else {
                    (1.0 / v).floor() as i32
                };
                let next = std::iter::once(0)
                    .chain((1..=height).flat_map(|dy| [dy, -dy]))
                    .map(|dy| at + delta + IVec3::Y * dy)
                    .find(|&p| query.stand(p));
                let Some(next) = next else {
                    continue;
                };
                if diagonal && !query.diagonal_clear(at, next) {
                    continue;
                }
                if next.y != at.y {
                    let top = at.y.max(next.y) + (1.8 / v).ceil() as i32;
                    if [at, next].into_iter().any(|p| {
                        (p.y..top).any(|y| known(w, IVec3::new(p.x, y, p.z)) != Some(B::Air))
                    }) {
                        continue;
                    }
                }
                let rate = if query.paved(at) && query.paved(next) {
                    900.0
                } else {
                    1000.0
                };
                let step =
                    cost.saturating_add(((next - at).as_vec3().length() * rate).ceil() as u32);
                if metres(next, v).distance_squared(self.feet) <= self.range * self.range
                    && self
                        .previous
                        .get(&next)
                        .is_none_or(|&(_, best)| step < best)
                {
                    self.previous.insert(next, (at, step));
                    self.queue.push(Reverse((
                        self.priority(next, step, v),
                        Reverse(step),
                        next.to_array(),
                    )));
                }
            }
            if !query.missing.is_empty() {
                self.queue.push(entry);
                self.remember(w, &query);
                return (Route::Pending, query.missing, spent);
            }
        }
        self.remember(w, &query);
        (Route::Pending, Vec::new(), spent)
    }

    fn remember(&mut self, w: &World, query: &super::Query<'_>) {
        let v = w.voxel_scale.meters();
        let target =
            crate::world::chunk_coord_from_world_block((self.task.point(v) / v).floor().as_ivec3());
        self.versions
            .entry(target.to_array())
            .or_insert_with(|| w.regions.source_version(target));
        for &cell in query.cells.keys() {
            let height = (1.8 / v).ceil() as i32;
            for at in (-1..=height)
                .step_by(crate::world::CHUNK_SIZE as usize)
                .map(|y| cell + IVec3::Y * y)
                .chain([cell + IVec3::Y * height])
            {
                let chunk = crate::world::chunk_coord_from_world_block(at);
                self.versions
                    .entry(chunk.to_array())
                    .or_insert_with(|| w.regions.source_version(chunk));
            }
        }
    }
}

pub(in super::super) fn route(
    w: &mut World,
    j: &mut State,
    feet: Vec3,
    task: Task,
    limit: usize,
    range: f32,
) -> Route {
    let Some(id) = j.worker else {
        return Route::Blocked;
    };
    let mut pool = std::mem::take(&mut w.farming.activities.navigation);
    let cached = pool
        .cache
        .iter()
        .find(|c| {
            c.task == task
                && c.feet.distance_squared(feet) < 0.01
                && c.versions
                    .iter()
                    .all(|(p, v)| w.regions.source_version((*p).into()) == *v)
        })
        .map(|c| c.path.clone());
    let result = if let Some(path) = cached {
        pool.hits = pool.hits.saturating_add(1);
        pool.requests.remove(&id);
        pool.searches.remove(&id);
        pool.ready.remove(&id);
        pool.active.retain(|&other| other != id);
        Route::Ready(path)
    } else if let Some(request) = pool.requests.get_mut(&id)
        && request.task == task
        && request.feet.distance_squared(feet) < 0.04
    {
        match request.result.take() {
            Some(result) => {
                pool.requests.remove(&id);
                pool.ready.remove(&id);
                result
            }
            None => Route::Pending,
        }
    } else {
        let queued = pool.requests.get(&id).is_some_and(|r| r.queued);
        pool.ready.remove(&id);
        pool.requests.insert(
            id,
            Request {
                feet,
                task,
                limit,
                range,
                result: None,
                queued: true,
            },
        );
        pool.searches.remove(&id);
        pool.active.retain(|&other| other != id);
        if !queued {
            pool.queued.push_back(id);
        }
        Route::Pending
    };
    j.navigation_wait = matches!(result, Route::Pending);
    w.farming.activities.navigation = pool;
    result
}

/// One shared navigation lane advances even when the requesting people sleep.
/// A waiting person keeps a small request; only 32 requests own search scratch.
pub(in super::super) fn service(w: &mut World) {
    let mut pool = std::mem::take(&mut w.farming.activities.navigation);
    let mut wake = Vec::new();
    // Reclaim cancelled requests incrementally, including completed paths whose
    // owners died or changed jobs before collecting them.
    use std::ops::Bound::{Excluded, Unbounded};
    let cleanup: Vec<_> = pool
        .requests
        .range((Excluded(pool.cleanup), Unbounded))
        .take(8)
        .map(|(&id, r)| (id, r.task))
        .collect();
    if cleanup.is_empty() {
        pool.cleanup = 0;
    }
    for (id, task) in cleanup {
        pool.cleanup = id;
        if resident_state(w, id)
            .and_then(|j| crew::view(j, id))
            .is_none_or(|p| p.task != Some(task))
        {
            pool.requests.remove(&id);
            pool.searches.remove(&id);
            pool.ready.remove(&id);
        }
    }
    for _ in 0..64 {
        if pool.searches.len() >= SEARCHES || pool.ready.len() >= 128 {
            break;
        }
        let Some(id) = pool.queued.pop_front() else {
            break;
        };
        let Some(request) = pool.requests.get_mut(&id) else {
            continue;
        };
        request.queued = false;
        if request.result.is_some() || pool.searches.contains_key(&id) {
            continue;
        }
        if resident_state(w, id)
            .and_then(|j| crew::view(j, id))
            .is_none_or(|p| p.task != Some(request.task))
        {
            pool.requests.remove(&id);
            continue;
        }
        pool.searches.insert(
            id,
            Search::new(w, request.feet, request.task, request.limit, request.range),
        );
        pool.active.push_back(id);
    }
    let mut left = NODES_PER_TICK;
    let turns = pool.active.len().min(SEARCHES * 2);
    for _ in 0..turns {
        if left == 0 || pool.ready.len() >= 128 {
            break;
        }
        let Some(id) = pool.active.pop_front() else {
            break;
        };
        let Some(mut search) = pool.searches.remove(&id) else {
            continue;
        };
        if resident_state(w, id)
            .and_then(|j| crew::view(j, id))
            .is_none_or(|p| p.task != Some(search.task))
        {
            pool.requests.remove(&id);
            continue;
        }
        if search
            .versions
            .iter()
            .any(|(p, v)| w.regions.source_version((*p).into()) != *v)
        {
            search = Search::new(w, search.feet, search.task, search.limit, search.range);
        }
        let (result, missing, spent) = search.advance(w, left.min(NODES_PER_WAKE));
        left = left.saturating_sub(spent);
        pool.expanded = pool.expanded.saturating_add(spent as u64);
        if !missing.is_empty() {
            residency::prepare_cells(w, missing);
        }
        if matches!(result, Route::Pending) {
            pool.searches.insert(id, search);
            pool.active.push_back(id);
        } else {
            if let Route::Ready(path) = &result {
                if pool.cache.len() == 256 {
                    pool.cache.pop_front();
                }
                pool.cache.push_back(Cached {
                    feet: search.feet,
                    task: search.task,
                    path: path.clone(),
                    versions: search.versions,
                });
            }
            if let Some(request) = pool.requests.get_mut(&id) {
                request.result = Some(result);
            }
            pool.ready.insert(id);
            wake.push(id);
        }
    }
    w.farming.activities.navigation = pool;
    for id in wake {
        communities::interrupt_activity(w, id);
    }
}
