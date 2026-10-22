//! Cached, bounded best-first routes through the current voxel collision world.
use super::*;

#[derive(Default)]
pub(super) struct Navigation {
    route: Vec<Vec3>,
    goal: Vec3,
    refresh: f32,
}

fn supported(world: &World, p: Vec3, kind: u32) -> bool {
    safe_floor(world, p) && (flying(kind) || factory::solid_point(world, p - Vec3::Y * 0.15))
}

pub(super) fn safe_floor(world: &World, p: Vec3) -> bool {
    let cell = ((p - Vec3::Y * 0.1) / world.voxel_scale.meters())
        .floor()
        .as_ivec3();
    world.get(cell) != super::super::BlockId::Lava
}

// Full body sweeps reject narrow passages and diagonal corner cutting.
fn edge(world: &World, a: Vec3, b: Vec3, kind: u32) -> bool {
    let steps = ((b - a).length() / 0.18).ceil().max(1.0) as usize;
    (1..=steps).all(|i| {
        let p = a.lerp(b, i as f32 / steps as f32);
        free(world, p, kind) && supported(world, p, kind)
    })
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
            self.route = route(world, pos, goal, kind);
        }
        let target = self.route.last().copied().unwrap_or(goal);
        ((target - pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero()
    }
}

fn route(world: &World, start: Vec3, goal: Vec3, kind: u32) -> Vec<Vec3> {
    if edge(world, start, goal, kind) {
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
                    if !free(world, q, kind) || !supported(world, q, kind) {
                        return false;
                    }
                    if (q.y - p.y).abs() < 0.01 {
                        return edge(world, p, q, kind);
                    }
                    let high = p.y.max(q.y) + 0.15;
                    let a = Vec3::new(p.x, high, p.z);
                    let b = Vec3::new(q.x, high, q.z);
                    (0..=8).all(|i| free(world, a.lerp(b, i as f32 / 8.0), kind))
                });
            let Some(q) = candidate else { continue };
            if (q - start).length_squared() > 18.0 * 18.0 {
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

/// Physics continues during windups and stagger, so a midair attacker cannot freeze.
pub(super) fn gravity(e: &mut Enemy, world: &World, dt: f32) {
    if flying(e.kind) {
        return;
    }
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
}
