//! Cached, bounded best-first routes through the current voxel collision world.
use super::*;

#[derive(Default)]
pub(super) struct Navigation {
    route: Vec<Vec3>,
    goal: Vec3,
    refresh: f32,
}

pub(super) fn safe_floor(world: &World, p: Vec3) -> bool {
    let cell = ((p - Vec3::Y * 0.1) / world.voxel_scale.meters())
        .floor()
        .as_ivec3();
    world.get(cell) != super::super::BlockId::Lava
}

/// Search radius around the start, in metres.
const REACH: f32 = 18.0;

/// Blocks read during one route search, kept in a dense cube around the start.
///
/// A search tests several thousand overlapping bodies. Each body covers a few
/// cells at one-metre voxels and over a hundred at quarter-metre voxels, and a
/// read outside the simulated chunks falls back to sampling the generator.
/// Reading each cell once makes the search cost follow the cells it visits,
/// with the same answers: the world does not change during a search.
struct Cells<'a> {
    world: &'a World,
    cfg: super::super::WorldSampleConfig,
    scale: f32,
    origin: IVec3,
    size: i32,
    /// 0 is unread, otherwise the block id plus one.
    blocks: std::cell::RefCell<Vec<u16>>,
}

impl<'a> Cells<'a> {
    fn new(world: &'a World, start: Vec3) -> Self {
        let scale = world.voxel_scale.meters();
        // The tallest body is under four metres; cells beyond the cube are
        // read from the world directly.
        let half = ((REACH + 4.0) / scale).ceil() as i32 + 2;
        Self {
            world,
            cfg: world.sample_config(),
            scale,
            origin: (start / scale).floor().as_ivec3() - IVec3::splat(half),
            size: half * 2,
            // Allocated on the first read: a straight-line route reads few cells.
            blocks: std::cell::RefCell::new(Vec::new()),
        }
    }

    fn get(&self, p: IVec3) -> super::super::BlockId {
        let d = p - self.origin;
        if d.cmplt(IVec3::ZERO).any() || d.cmpge(IVec3::splat(self.size)).any() {
            return self.world.get(p);
        }
        let mut blocks = self.blocks.borrow_mut();
        if blocks.is_empty() {
            *blocks = vec![0; (self.size as usize).pow(3)];
        }
        let slot = &mut blocks[((d.y * self.size + d.z) * self.size + d.x) as usize];
        if *slot == 0 {
            *slot = self.world.get(p) as u16 + 1;
        }
        super::super::BlockId::from_u32(u32::from(*slot) - 1)
    }

    /// `free` through the memo.
    fn free(&self, pos: Vec3, kind: u32) -> bool {
        let (min, max) = body_bounds(pos, kind, self.scale);
        !super::super::collision::collides_with(self.cfg, min, max, IVec3::ZERO, |p| self.get(p), Some(self.world))
    }

    /// `factory::solid_point` through the memo.
    fn solid_point(&self, point: Vec3) -> bool {
        let p = point / self.scale;
        let cell = p.floor().as_ivec3();
        if cell.y > self.cfg.min_world_y() && !self.get(cell).is_solid() {
            return false;
        }
        factory::conduit_bounds(self.world, cell)
            .is_none_or(|bounds| bounds.iter().any(|&(a, b)| p.cmpge(a).all() && p.cmple(b).all()))
    }

    /// `supported` through the memo.
    fn supported(&self, p: Vec3, kind: u32) -> bool {
        let cell = ((p - Vec3::Y * 0.1) / self.scale).floor().as_ivec3();
        self.get(cell) != super::super::BlockId::Lava && (flying(kind) || self.solid_point(p - Vec3::Y * 0.15))
    }

    // Full body sweeps reject narrow passages and diagonal corner cutting.
    fn edge(&self, a: Vec3, b: Vec3, kind: u32) -> bool {
        let steps = ((b - a).length() / 0.18).ceil().max(1.0) as usize;
        (1..=steps).all(|i| {
            let p = a.lerp(b, i as f32 / steps as f32);
            self.free(p, kind) && self.supported(p, kind)
        })
    }
}

impl Navigation {
    pub fn heading(
        &mut self,
        world: &World,
        pos: Vec3,
        goal: Vec3,
        kind: u32,
        dt: f32,
        slot: u32,
        budget: &mut usize,
    ) -> Vec3 {
        self.refresh -= dt;
        if (goal - self.goal).length_squared() > 6.25 {
            self.route.clear();
            self.refresh = 0.0;
        }
        while self
            .route
            .last()
            .is_some_and(|p| (*p - pos).length_squared() < 0.25)
        {
            self.route.pop();
        }
        // At most one actor searches per frame; collision still resolves every tick.
        if self.refresh <= 0.0 && *budget > 0 {
            *budget -= 1;
            self.refresh = 0.65 + (slot % 5) as f32 * 0.07;
            self.goal = goal;
            let started = std::time::Instant::now();
            self.route = route(world, pos, goal, kind);
            crate::gameplay_trace::note("nav.route", started.elapsed().as_secs_f64() * 1000.0);
        }
        let target = self.route.last().copied().unwrap_or(goal);
        ((target - pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero()
    }
}

fn route(world: &World, start: Vec3, goal: Vec3, kind: u32) -> Vec<Vec3> {
    let cells = Cells::new(world, start);
    if cells.edge(start, goal, kind) {
        return vec![goal];
    }
    struct Node {
        p: Vec3,
        parent: usize,
        cost: f32,
        closed: bool,
    }
    let mut nodes = vec![Node {
        p: start,
        parent: 0,
        cost: 0.0,
        closed: false,
    }];
    let heuristic = |p: Vec3| (p - goal).length();
    let mut best = 0;
    for _ in 0..96 {
        let Some(index) = (0..nodes.len())
            .filter(|&i| !nodes[i].closed)
            .min_by(|&a, &b| {
                (nodes[a].cost + heuristic(nodes[a].p))
                    .total_cmp(&(nodes[b].cost + heuristic(nodes[b].p)))
            })
        else {
            break;
        };
        nodes[index].closed = true;
        let p = nodes[index].p;
        if heuristic(p) < heuristic(nodes[best].p) {
            best = index;
        }
        if heuristic(p) < 1.0 {
            best = index;
            break;
        }
        for (x, z) in [
            (1., 0.),
            (-1., 0.),
            (0., 1.),
            (0., -1.),
            (1., 1.),
            (1., -1.),
            (-1., 1.),
            (-1., -1.),
        ] {
            if nodes.len() >= 384 {
                break;
            }
            let base = p + Vec3::new(x, 0.0, z) * 0.8;
            // Match the locomotor's one-metre hop and conservative step-down.
            let candidate = [0.0, 0.25, 0.5, 0.75, 1.0, -0.25, -0.5, -0.75]
                .into_iter()
                .map(|dy| base + Vec3::Y * dy)
                .find(|&q| {
                    if !cells.free(q, kind) || !cells.supported(q, kind) {
                        return false;
                    }
                    if (q.y - p.y).abs() < 0.01 {
                        return cells.edge(p, q, kind);
                    }
                    let high = p.y.max(q.y) + 0.15;
                    let a = Vec3::new(p.x, high, p.z);
                    let b = Vec3::new(q.x, high, q.z);
                    (0..=8).all(|i| cells.free(a.lerp(b, i as f32 / 8.0), kind))
                });
            let Some(q) = candidate else { continue };
            if (q - start).length_squared() > REACH * REACH {
                continue;
            }
            let cost = nodes[index].cost + (q - p).length();
            if let Some(existing) = nodes.iter().position(|n| (n.p - q).length_squared() < 0.04) {
                if cost < nodes[existing].cost {
                    nodes[existing].cost = cost;
                    nodes[existing].parent = index;
                    nodes[existing].closed = false;
                }
            } else {
                nodes.push(Node {
                    p: q,
                    parent: index,
                    cost,
                    closed: false,
                });
            }
        }
    }
    let mut result = Vec::new();
    while best != 0 {
        result.push(nodes[best].p);
        best = nodes[best].parent;
    }
    result
}

pub(super) fn up_at(world: &World, pos: Vec3) -> Vec3 {
    let g = engine_worldgen::manyfold::acceleration_for(world.generator_version, pos);
    (-g).try_normalize().unwrap_or(Vec3::Y)
}

/// Body clearance. World-up creatures keep the axis-aligned box; a tilted shell
/// uses a short column along local up so the body stands on the floor it walks.
pub(super) fn clear(world: &World, pos: Vec3, kind: u32, up: Vec3) -> bool {
    if up.dot(Vec3::Y) > 0.999 {
        return free(world, pos, kind);
    }
    let s = world.voxel_scale.meters();
    let height = 1.6;
    let steps = (height / s).ceil().max(1.0) as i32;
    let side = up.cross(if up.z.abs() < 0.9 { Vec3::Z } else { Vec3::X }).normalize_or_zero() * 0.28;
    for i in 0..=steps {
        let p = pos + up * (height * i as f32 / steps as f32);
        for sample in [p, p + side, p - side] {
            if world.solid((sample / s).floor().as_ivec3()) {
                return false;
            }
        }
    }
    true
}

pub(super) fn on_floor(world: &World, pos: Vec3, up: Vec3) -> bool {
    if up.dot(Vec3::Y) > 0.999 {
        return factory::solid_point(world, pos - Vec3::Y * 0.65);
    }
    let s = world.voxel_scale.meters();
    world.solid(((pos - up * 0.35) / s).floor().as_ivec3())
}

/// Physics continues during windups and stagger, so a midair attacker cannot freeze.
pub(super) fn gravity(e: &mut Enemy, world: &World, dt: f32) {
    if flying(e.kind) {
        return;
    }
    let up = up_at(world, e.pos);
    e.up = up;
    if up.dot(Vec3::Y) > 0.999 {
        e.velocity.y = (e.velocity.y - dt * 14.0).max(-14.0);
        let dy = e.velocity.y * dt;
        let steps = (dy.abs() / 0.18).ceil().max(1.0) as usize;
        for _ in 0..steps {
            let q = e.pos + Vec3::Y * (dy / steps as f32);
            if free(world, q, e.kind) {
                e.pos = q;
            } else {
                e.velocity.y = 0.0;
                break;
            }
        }
        return;
    }
    let g = engine_worldgen::manyfold::acceleration_for(world.generator_version, e.pos);
    e.velocity += g * dt;
    let falling = e.velocity.dot(-up);
    if falling > 14.0 {
        e.velocity -= -up * (falling - 14.0);
    }
    let delta = e.velocity * dt;
    let steps = (delta.length() / 0.18).ceil().max(1.0) as usize;
    for _ in 0..steps {
        let q = e.pos + delta / steps as f32;
        if clear(world, q, e.kind, up) {
            e.pos = q;
        } else {
            let into = e.velocity.dot(-up);
            if into > 0.0 {
                e.velocity += up * into;
            }
            break;
        }
    }
}
