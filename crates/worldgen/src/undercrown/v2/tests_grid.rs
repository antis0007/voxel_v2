//! A walker's flood fill over a sampled layout, at any voxel scale: a body
//! two metres tall, a one-metre climb, a four-metre drop, and swimming.
use super::*;
use crate::WorldType;
use crate::undercrown::plan::{Plan, RoomSpec};
use engine_core::VoxelScalePreset;
use std::collections::{HashSet, VecDeque};

/// The generator under test: the current one, or `UC_GEN` to run this
/// module's tests against a dormant gate.
pub(super) fn generator() -> u32 {
    std::env::var("UC_GEN").ok().and_then(|s| s.parse().ok()).unwrap_or(crate::CURRENT_GENERATOR_VERSION)
}

pub(super) fn cfg_scaled(seed: u32, voxel_scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version: generator(), seed, world_type: WorldType::Hills, voxel_scale }
}

pub(super) fn plan_scaled(seed: u32, scale: VoxelScalePreset) -> (WorldSampleConfig, Plan) {
    let c = cfg_scaled(seed, scale);
    let plan = Plan::for_config(&c);
    (c, plan)
}

#[derive(Clone)]
pub(super) struct Grid {
    pub min: IVec3,
    pub size: IVec3,
    pub v: f32,
    pub blocks: Vec<B>,
    body: i32,
    climb: i32,
    drop: i32,
}

impl Grid {
    pub fn new(c: &WorldSampleConfig, l: &Layout, mech: Mech) -> Self {
        let v = c.voxel_meters();
        let min = (l.min / v).floor().as_ivec3();
        let size = (l.max / v).ceil().as_ivec3() - min + IVec3::ONE;
        let mut blocks = Vec::with_capacity((size.x * size.y * size.z) as usize);
        for y in 0..size.y {
            for z in 0..size.z {
                for x in 0..size.x {
                    let p = min + IVec3::new(x, y, z);
                    blocks.push(l.sample(c, (p.as_vec3() + Vec3::splat(0.5)) * v, mech).unwrap_or(B::Stone));
                }
            }
        }
        let cells = |m: f32| ((m / v).round() as i32).max(1);
        Self { min, size, v, blocks, body: cells(2.0), climb: cells(1.0), drop: cells(4.0) }
    }

    fn slot(&self, p: IVec3) -> Option<usize> {
        let q = p - self.min;
        if q.cmplt(IVec3::ZERO).any() || q.cmpge(self.size).any() {
            return None;
        }
        Some(((q.y * self.size.z + q.z) * self.size.x + q.x) as usize)
    }

    pub fn get(&self, p: IVec3) -> B {
        self.slot(p).map_or(B::Stone, |i| self.blocks[i])
    }

    /// Overwrite the cells a mechanism change writes.
    pub fn apply(&mut self, c: &WorldSampleConfig, l: &Layout, mech: Mech, edits: &[(IVec3, B)]) {
        for &(p, _) in edits {
            if let Some(i) = self.slot(p) {
                self.blocks[i] = l.sample(c, (p.as_vec3() + Vec3::splat(0.5)) * self.v, mech).unwrap_or(B::Stone);
            }
        }
    }

    fn open(&self, p: IVec3) -> bool {
        let b = self.get(p);
        !b.is_solid() && b != B::Lava
    }

    pub fn stand(&self, p: IVec3) -> bool {
        (0..self.body).all(|k| self.open(p + IVec3::Y * k)) && (self.get(p - IVec3::Y).is_solid() || self.get(p) == B::Water || self.get(p - IVec3::Y) == B::Water)
    }

    pub fn reach(&self, start: IVec3) -> HashSet<IVec3> {
        self.reach_list(start).into_iter().collect()
    }

    /// Every standing cell reachable from `start`, in visiting order.
    pub fn reach_list(&self, start: IVec3) -> Vec<IVec3> {
        let mut s = start;
        while !self.stand(s) && s.y > self.min.y {
            s.y -= 1;
        }
        let mut visited = vec![false; self.blocks.len()];
        let mut seen = Mark { visited: &mut visited, grid: self, list: Vec::new() };
        seen.insert(s);
        let mut queue = VecDeque::from([s]);
        while let Some(p) = queue.pop_front() {
            for dir in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                for dy in (-self.drop..=self.climb).rev() {
                    let q = p + dir + IVec3::Y * dy;
                    if dy > 0 && !(0..self.body + dy).all(|k| self.open(p + IVec3::Y * k)) {
                        continue;
                    }
                    if dy < 0 && !(dy..0).all(|k| self.open(p + dir + IVec3::Y * (k + self.body))) {
                        continue;
                    }
                    if self.stand(q) {
                        if seen.insert(q) {
                            queue.push_back(q);
                        }
                        break;
                    }
                }
            }
            // Swimming: up out of water, or down into it.
            let (up, down) = (p + IVec3::Y, p - IVec3::Y);
            if self.get(p) == B::Water && self.stand(up) && seen.insert(up) {
                queue.push_back(up);
            }
            if self.get(down) == B::Water && self.stand(down) && seen.insert(down) {
                queue.push_back(down);
            }
        }
        seen.list
    }

    /// Voxel holding a point in metres.
    pub fn cell(&self, m: Vec3) -> IVec3 {
        (m / self.v).floor().as_ivec3()
    }

    pub fn centre(&self, p: IVec3) -> Vec3 {
        (p.as_vec3() + Vec3::splat(0.5)) * self.v
    }

    pub fn in_room<'a>(&self, seen: impl IntoIterator<Item = &'a IVec3>, r: &RoomSpec) -> bool {
        seen.into_iter().any(|&p| {
            let m = self.centre(p);
            m.x >= r.x0 && m.x < r.x1 && m.z >= r.z0 && m.z < r.z1 && m.y >= r.floor - 1.0 && m.y < r.floor + r.headroom
        })
    }

    pub fn rooms(&self, plan: &Plan, seen: &[IVec3]) -> Vec<&'static str> {
        plan.rooms.iter().filter(|r| self.in_room(seen, r)).map(|r| r.name).collect()
    }

    /// A prop is in reach from some standing spot: eyes 1.6 m up, 3.2 m away.
    pub fn touches<'a>(&self, seen: impl IntoIterator<Item = &'a IVec3>, pos: Vec3) -> bool {
        seen.into_iter().any(|&p| (self.centre(p) - Vec3::Y * (self.v * 0.5) + Vec3::Y * 1.6 - pos).length() < 3.2)
    }
}

/// The visited set of a flood fill: a bitmap over the grid and the list.
struct Mark<'a> {
    visited: &'a mut Vec<bool>,
    grid: &'a Grid,
    list: Vec<IVec3>,
}

impl Mark<'_> {
    fn insert(&mut self, p: IVec3) -> bool {
        let Some(i) = self.grid.slot(p) else {
            return false;
        };
        let fresh = !self.visited[i];
        if fresh {
            self.visited[i] = true;
            self.list.push(p);
        }
        fresh
    }
}

pub(super) fn start_cell(plan: &Plan, g: &Grid) -> IVec3 {
    g.cell(plan.anchor + Vec3::new(-4.0, 0.0, -6.0))
}
