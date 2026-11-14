//! External runoff enters each refinement through the parent receiver graph.
use super::{World, Mesh, Face, Projection, atlas};
use glam::{DVec2, DVec3};
use std::collections::BTreeSet;

pub(super) fn add(world: &World, face: Face, mesh: &Mesh, fixed: &[bool], rain: &mut [f64], momentum: Option<&mut [DVec2]>) {
    let grid = mesh.grid;
    let mut inlets = Inlets::new(mesh, fixed, rain, momentum);
    let axes = face.basis();
    let chart = |dir: DVec3| {
        let normal = dir.dot(axes.n.as_dvec3());
        let factor = atlas::HALF_M / std::f64::consts::FRAC_PI_4;
        DVec2::new((dir.dot(axes.u.as_dvec3()) / normal).atan(),
            (dir.dot(axes.v.as_dvec3()) / normal).atan()) * factor
    };
    // Only coarse cells touching the boundary can supply a crossing edge.
    let mut candidates = BTreeSet::new();
    for (i, &point) in mesh.points.iter().enumerate() {
        if !grid.on_border(i) { continue; }
        let dir = Projection::Tangent.to_sphere(face, point.x / atlas::HALF_M, point.y / atlas::HALF_M);
        let (ids, _) = world.mesh.stencil(dir);
        for id in ids {
            candidates.insert(id);
            candidates.extend(world.mesh.neighbours[id].iter().map(|&(j, _)| j));
        }
    }
    for i in candidates {
        let receiver = world.flow.receiver[i];
        if receiver == i || world.height[i] <= super::SEA { continue; }
        let start = chart(world.river_nodes[i].normalize());
        let end = chart(world.river_nodes[receiver].normalize());
        let speed = if inlets.momentum.is_some() {
            let length = world.river_nodes[i].distance(world.river_nodes[receiver]).max(1.0);
            (38.0 * (world.height[i] - world.height[receiver]).max(0.0) / length).sqrt()
        } else { 0.0 };
        inlets.add(start, end, world.flow.discharge[i], speed);
    }
}

pub(super) struct Inlets<'a> {
    mesh: &'a Mesh,
    fixed: &'a [bool],
    rain: &'a mut [f64],
    momentum: Option<&'a mut [DVec2]>,
    low: DVec2,
    high: DVec2,
}

impl<'a> Inlets<'a> {
    pub(super) fn new(mesh: &'a Mesh, fixed: &'a [bool], rain: &'a mut [f64], momentum: Option<&'a mut [DVec2]>) -> Self {
        let grid = mesh.grid;
        let low = DVec2::from_array(grid.min_m);
        let high = low + DVec2::splat((grid.n - 1) as f64 * grid.cell_m);
        Self { mesh, fixed, rain, momentum, low, high }
    }

    pub(super) fn add(&mut self, start: DVec2, end: DVec2, discharge: f64, speed: f64) {
        let lo = self.low;
        let hi = self.high;
        if start.cmpge(lo).all() && start.cmple(hi).all() { return; }
        if start.min(end).cmpgt(hi).any() || start.max(end).cmplt(lo).any() { return; }
        let mesh = self.mesh;
        let grid = mesh.grid;
        let direction = end - start;
        let mut enter = 0.0_f64;
        let mut exit = 1.0_f64;
        for axis in 0..2 {
            if direction[axis].abs() < 1e-9 {
                if start[axis] < lo[axis] || start[axis] > hi[axis] { exit = -1.0; }
                continue;
            }
            let a = (lo[axis] - start[axis]) / direction[axis];
            let b = (hi[axis] - start[axis]) / direction[axis];
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
        }
        if enter >= exit { return; }
        let point = start + direction * (enter + (exit - enter).min(grid.cell_m * 2.0 / direction.length()) * 0.5);
        let (x, z, _, _) = grid.cell(point.x, point.y);
        let node = (z.saturating_sub(1)..=(z + 2).min(grid.n - 1)).flat_map(|z| {
            (x.saturating_sub(1)..=(x + 2).min(grid.n - 1)).map(move |x| z * grid.n + x)
        }).filter(|&j| !self.fixed[j]).min_by(|&a, &b| {
            mesh.points[a].distance_squared(point).total_cmp(&mesh.points[b].distance_squared(point))
                .then(a.cmp(&b))
        });
        if let Some(j) = node {
            self.rain[j] += discharge / mesh.area[j];
            if let Some(momentum) = &mut self.momentum {
                momentum[j] += direction.normalize_or_zero() * (discharge * speed);
            }
        }
    }
}
