use super::*;
use engine_sim::atmosphere as gas;

fn open(world: &World, p: IVec3) -> bool {
    let b = world.get(p);
    // Thin fittings are not pressure-rated seals. Airlock panels are full voxels.
    !b.is_solid() && !b.is_liquid()
}

fn geometry(world: &World, h: &Habitat, i: usize) -> (f64, [f64; 3]) {
    let n = world.voxel_scale.meters().recip().round() as i32;
    let base = (h.origin + h.grid.local(i)) * n;
    let mut volume = 0;
    let mut faces = [0; 3];
    for y in 0..n { for z in 0..n { for x in 0..n {
        let p = base + IVec3::new(x, y, z);
        if !open(world, p) { continue; }
        volume += 1;
        for (axis, boundary) in [x == n - 1, y == n - 1, z == n - 1].into_iter().enumerate() {
            if boundary && open(world, p + [IVec3::X, IVec3::Y, IVec3::Z][axis]) {
                faces[axis] += 1;
            }
        }
    } } }
    let mut faces = faces.map(|count| count as f64 / (n * n) as f64);
    // The negative-X port terminates in the moving cabin's bulk gas volume.
    // It must not also connect to the vacuum behind that frame boundary.
    if h.grid.local(i).x == 2 { faces[0] = 0.0; }
    (volume as f64 / (n * n * n) as f64, faces)
}

pub(super) fn refresh(world: &World, h: &mut Habitat) {
    let cells = h.grid.cells();
    if h.volumes.len() != cells {
        h.volumes = vec![0.0; cells];
        h.faces = vec![[0.0; 3]; cells];
        h.dirty.extend(0..cells);
    }
    let mut plan = None;
    // A wing grid is several drums large; first fill takes the same wall time.
    for _ in 0..128 * cells.div_ceil(geometry::LEGACY_CELLS) {
        let Some(i) = h.dirty.pop_first() else { break };
        let (v, faces) = geometry(world, h, i);
        h.volumes[i] = v;
        h.faces[i] = faces;
        if h.gas[i][0] < 0.0 {
            let cell = h.grid.local(i);
            let p = cell.as_vec3() + Vec3::splat(0.5);
            let wing = h.wing.is_some_and(|seed| geometry::wing_pressurised(
                plan.get_or_insert_with(|| engine_worldgen::station::plan(seed)), cell));
            let inside = wing || (p.x > 5.0 && p.x < 10.0 && p.y > 0.0 && p.y < 3.0 && p.z.abs() < 2.0)
                || ((11.0..11.0 + config().length_m as f32).contains(&p.x)
                    && glam::Vec2::new(p.y - config().radius_m as f32, p.z).length() < config().radius_m as f32 + 1.0);
            let quantity = if inside { config().pressure_pa * v } else { 0.0 };
            h.gas[i] = [quantity * 0.21, quantity * 0.79];
        }
    }
}

pub(super) fn step(h: &mut Habitat, dt: f64, docked: bool) {
    if !h.dirty.is_empty() { return; }
    for i in 0..h.gas.len() {
        let local = h.grid.local(i);
        if local.x == 3 && (0..3).contains(&local.y) && (-2..2).contains(&local.z) {
            if docked {
                gas::exchange(&mut h.gas[i], h.volumes[i], &mut h.cabin,
                    cabin_volume(), h.volumes[i], dt);
            } else {
                let amount = (h.gas[i][0] + h.gas[i][1]) * (dt * 20.0).min(1.0);
                gas::transfer(&mut h.gas[i], &mut h.escaped, amount);
            }
        }
        for (axis, offset) in [IVec3::X, IVec3::Y, IVec3::Z].into_iter().enumerate() {
            let area = h.faces[i][axis];
            if area == 0.0 { continue; }
            if let Some(j) = h.index(local + offset) {
                let (left, right) = h.gas.split_at_mut(j);
                gas::exchange(&mut left[i], h.volumes[i], &mut right[0], h.volumes[j], area, dt);
            }
        }
        // The bounded exterior is an explicit vacuum sink, with an exhaust ledger.
        let edge = [0, 1, 2].into_iter().any(|axis| {
            local[axis] == h.grid.min[axis] || local[axis] == (h.grid.min + h.grid.size)[axis] - 1
        });
        if edge { let amount = h.gas[i][0] + h.gas[i][1]; gas::transfer(&mut h.gas[i], &mut h.escaped, amount); }
    }
}

pub(super) fn chamber(h: &Habitat) -> Vec<usize> {
    (0..h.grid.cells()).filter(|&i| {
        let p = h.grid.local(i);
        (5..10).contains(&p.x) && (0..3).contains(&p.y) && (-2..2).contains(&p.z)
    }).collect()
}

pub(super) fn chamber_pressure(h: &Habitat) -> f64 {
    let mut q = [0.0; 2];
    let mut v = 0.0;
    for i in chamber(h) {
        q[0] += h.gas[i][0].max(0.0); q[1] += h.gas[i][1].max(0.0);
        v += h.volumes.get(i).copied().unwrap_or(0.0);
    }
    gas::pressure(q, v)
}
