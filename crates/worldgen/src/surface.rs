//! Generator 18 surface wayfinding and roadside worldbuilding.
//!
//! The surface used to be a series of isolated landmarks with no relationship
//! to one another. This layer writes a deterministic, walkable trail network
//! from the spawn clearing to the starter workshop and the nearest ruins, then
//! furnishes the route with cairns, camps, shrines, overlooks and one carved
//! trailside nook. Everything is a pure function of world config and metre
//! coordinates, so collision, rendering and saves agree.

use super::{BlockId as B, WorldSampleConfig, WorldType, hash2, sample_column};
use glam::{IVec3, Vec3};
use std::cell::RefCell;
use std::rc::Rc;

mod routes;
mod roads;
mod planet_roads;
pub(super) const SPAWN_ROAD_VERSION: u32 = crate::ruins::VERSION;
/// Trail beds fray into packed earth and dry drainage swales keep their turf.
pub const MEADOW_GROUND_VERSION: u32 = 133;
/// Blocked links survey a graded path around the slope, and the road web keeps
/// only links no third node sits between.
pub const TRAIL_SURVEY_VERSION: u32 = 186;

/// Native review camera for the first surveyed crossing, in world metres.
pub fn bridge_review_view(cfg: &WorldSampleConfig) -> Option<(Vec3, Vec3)> {
    let trail = if planet_roads::active(cfg) {
        planet_roads::first_bridge(cfg)
    } else {
        plan(cfg).trails.iter().find(|t| !t.bridges.is_empty()).cloned()
    }?;
    let span = trail.bridges.first()?;
    let t = (span.start + span.end) * 0.5;
    let (x,z) = trail.at(t);
    let y = routes::height_at(&trail,t);
    let (tx,tz) = trail.tangent(t);
    let target = Vec3::new(x,y,z);
    Some((target + Vec3::new(-tz*24.0-tx*12.0,12.0,tx*24.0-tz*12.0), target))
}

/// Surface worldbuilding rides the current v18 baseline. It is sampled only in
/// newly generated or unedited chunks; resident edits always win, as they do
/// for every other generator layer.
pub(super) const VERSION: u32 = 18;
const REACH: f32 = 280.0;
const MAX_TRAILS: usize = 4;

#[derive(Clone)]
struct Trail {
    ax: f32,
    az: f32,
    cx: f32,
    cz: f32,
    bx: f32,
    bz: f32,
    half_width: f32,
    seed: u32,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    /// Walkable surface heights, sampled along the curve (generator 37+).
    grade: Vec<f32>,
    bridges: Vec<routes::Span>,
    /// Surveyed centre line; empty for the quadratic curve through a, c, b.
    path: Vec<(f32, f32)>,
    /// Arc length at each `path` point, metres.
    arc: Vec<f32>,
}

impl Trail {
    fn new(ax: f32, az: f32, bx: f32, bz: f32, seed: u32, half_width: f32) -> Self {
        let dx = bx - ax;
        let dz = bz - az;
        let len = dx.hypot(dz).max(0.001);
        let nx = -dz / len;
        let nz = dx / len;
        let bend = (hash2(0, 0, seed) * 2.0 - 1.0) * (len * 0.16).clamp(3.0, 18.0);
        let drift = (hash2(1, 9, seed) - 0.5) * 5.0;
        let cx = (ax + bx) * 0.5 + nx * (bend + drift);
        let cz = (az + bz) * 0.5 + nz * (bend + drift);
        let min_x = ax.min(bx).min(cx) - half_width - 2.0;
        let max_x = ax.max(bx).max(cx) + half_width + 2.0;
        let min_z = az.min(bz).min(cz) - half_width - 2.0;
        let max_z = az.max(bz).max(cz) + half_width + 2.0;
        Self {
            ax,
            az,
            cx,
            cz,
            bx,
            bz,
            half_width,
            seed,
            min_x,
            max_x,
            min_z,
            max_z,
            grade: Vec::new(),
            bridges: Vec::new(),
            path: Vec::new(),
            arc: Vec::new(),
        }
    }

    /// A trail along a surveyed centre line of at least two points.
    fn polyline(path: Vec<(f32, f32)>, seed: u32, half_width: f32) -> Self {
        let (a, b) = (path[0], path[path.len() - 1]);
        let mut trail = Self::new(a.0, a.1, b.0, b.1, seed, half_width);
        let mut arc = Vec::with_capacity(path.len());
        let mut total = 0.0;
        for (i, p) in path.iter().enumerate() {
            if i > 0 {
                total += (p.0 - path[i - 1].0).hypot(p.1 - path[i - 1].1);
            }
            arc.push(total);
            trail.min_x = trail.min_x.min(p.0 - half_width - 2.0);
            trail.max_x = trail.max_x.max(p.0 + half_width + 2.0);
            trail.min_z = trail.min_z.min(p.1 - half_width - 2.0);
            trail.max_z = trail.max_z.max(p.1 + half_width + 2.0);
        }
        trail.path = path;
        trail.arc = arc;
        trail
    }

    /// Segment index and fraction along it at parameter `t` of a polyline.
    fn segment(&self, t: f32) -> (usize, f32) {
        let total = self.arc[self.arc.len() - 1];
        let s = t.clamp(0.0, 1.0) * total;
        let i = self.arc.partition_point(|&a| a <= s).clamp(1, self.arc.len() - 1) - 1;
        let span = (self.arc[i + 1] - self.arc[i]).max(1e-6);
        (i, ((s - self.arc[i]) / span).clamp(0.0, 1.0))
    }

    fn at(&self, t: f32) -> (f32, f32) {
        if !self.path.is_empty() {
            let (i, f) = self.segment(t);
            let (a, b) = (self.path[i], self.path[i + 1]);
            return (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
        }
        let u = 1.0 - t;
        (
            u * u * self.ax + 2.0 * u * t * self.cx + t * t * self.bx,
            u * u * self.az + 2.0 * u * t * self.cz + t * t * self.bz,
        )
    }

    fn derivative(&self, t: f32) -> (f32, f32) {
        if !self.path.is_empty() {
            let (i, _) = self.segment(t);
            let (a, b) = (self.path[i], self.path[i + 1]);
            let total = self.arc[self.arc.len() - 1];
            let span = (self.arc[i + 1] - self.arc[i]).max(1e-6);
            return ((b.0 - a.0) / span * total, (b.1 - a.1) / span * total);
        }
        let u = 1.0 - t;
        (
            2.0 * u * (self.cx - self.ax) + 2.0 * t * (self.bx - self.cx),
            2.0 * u * (self.cz - self.az) + 2.0 * t * (self.bz - self.cz),
        )
    }

    fn nearest(&self, x: f32, z: f32) -> (f32, f32) {
        if !self.path.is_empty() {
            let total = self.arc[self.arc.len() - 1].max(1e-6);
            let mut best = (f32::MAX, 0.0);
            for (i, w) in self.path.windows(2).enumerate() {
                let (ax, az, bx, bz) = (w[0].0, w[0].1, w[1].0, w[1].1);
                if x < ax.min(bx) - best.0 || x > ax.max(bx) + best.0 || z < az.min(bz) - best.0 || z > az.max(bz) + best.0 {
                    continue;
                }
                let (dx, dz) = (bx - ax, bz - az);
                let f = (((x - ax) * dx + (z - az) * dz) / (dx * dx + dz * dz).max(1e-6)).clamp(0.0, 1.0);
                let d = (x - ax - dx * f).hypot(z - az - dz * f);
                if d < best.0 {
                    best = (d, (self.arc[i] + (self.arc[i + 1] - self.arc[i]) * f) / total);
                }
            }
            return best;
        }
        let dx = self.bx - self.ax;
        let dz = self.bz - self.az;
        let len2 = (dx * dx + dz * dz).max(0.001);
        let mut t = (((x - self.ax) * dx + (z - self.az) * dz) / len2).clamp(0.0, 1.0);
        for _ in 0..3 {
            let (px, pz) = self.at(t);
            let (tx, tz) = self.derivative(t);
            let denom = tx * tx + tz * tz;
            if denom < 1e-5 {
                break;
            }
            t = (t + ((x - px) * tx + (z - pz) * tz) / denom).clamp(0.0, 1.0);
        }
        let (px, pz) = self.at(t);
        ((x - px).hypot(z - pz), t)
    }

    fn length(&self) -> f32 {
        if let Some(&total) = self.arc.last() {
            return total;
        }
        (self.bx - self.ax).hypot(self.bz - self.az)
    }

    fn tangent(&self, t: f32) -> (f32, f32) {
        let (x, z) = self.derivative(t);
        let len = x.hypot(z).max(0.001);
        (x / len, z / len)
    }
}

#[derive(Clone)]
struct Wayside {
    x: f32,
    z: f32,
    height: i32,
    kind: u8,
    tx: f32,
    tz: f32,
    nx: f32,
    nz: f32,
}

#[derive(Clone)]
struct Nook {
    x: f32,
    z: f32,
    height: i32,
    tx: f32,
    tz: f32,
    /// Unit direction from the nook mouth back toward the trail.
    fx: f32,
    fz: f32,
}

struct Plan {
    trails: Vec<Trail>,
    waysides: Vec<Wayside>,
    nooks: Vec<Nook>,
}

thread_local! {
    static PLAN: RefCell<Option<(WorldSampleConfig, Rc<Plan>)>> = const { RefCell::new(None) };
}

fn plan(cfg: &WorldSampleConfig) -> Rc<Plan> {
    if let Some(found) = PLAN.with(|p| {
        p.borrow()
            .as_ref()
            .filter(|(c, _)| c == cfg)
            .map(|(_, plan)| plan.clone())
    }) {
        return found;
    }
    let built = Rc::new(build(cfg));
    PLAN.with(|p| *p.borrow_mut() = Some((*cfg, built.clone())));
    built
}

fn build(cfg: &WorldSampleConfig) -> Plan {
    let mut trails = Vec::new();
    if cfg.generator_version >= 37 {
        trails = routes::network(cfg);
    } else {
        let mut targets: Vec<(f32, f32, f32)> = Vec::new();
        for (p, kind, _, variation) in super::sites::horizon_landmarks(cfg, (0, 0), REACH) {
            if p.y <= cfg.water_level() as f32 * cfg.voxel_meters() + 2.0 {
                continue;
            }
            // The workshop entrance faces the spawn clearing. A path to its centre
            // would disappear under the building, while this endpoint meets the
            // front step in the open.
            let target = if kind == 0 && p.x.abs() < 0.5 {
                Vec3::new(0.0, p.y, p.z + 9.0 + variation)
            } else {
                Vec3::new(p.x, p.y, p.z)
            };
            let d = target.x.hypot(target.z);
            if d > 20.0 {
                targets.push((target.x, target.z, d));
            }
        }
        targets.sort_by(|a, b| a.2.total_cmp(&b.2));
        let mut used: Vec<(f32, f32)> = Vec::new();
        for (i, (x, z, _)) in targets.into_iter().enumerate() {
            if trails.len() >= MAX_TRAILS {
                break;
            }
            if used.iter().any(|(ux, uz)| (x - ux).hypot(z - uz) < 40.0) {
                continue;
            }
            used.push((x, z));
            let seed = cfg
                .seed
                .wrapping_add((i as u32).wrapping_mul(0x9E37_79B1))
                .wrapping_add(x.to_bits().rotate_left(7))
                .wrapping_add(z.to_bits().rotate_left(17));
            trails.push(Trail::new(0.0, 0.0, x, z, seed, 1.55));
        }

    }

    // A fitted approach makes the district entrance discoverable from the
    // ordinary surface network. Steep/unbuildable approaches are rejected.
    if crate::fernwake::active(cfg) {
        let target = (292.5_f32, 177.0_f32);
        let start = trails.iter().flat_map(|t| [(t.ax,t.az),(t.bx,t.bz)])
            .min_by(|a,b| ((a.0-target.0).hypot(a.1-target.1)).total_cmp(&(b.0-target.0).hypot(b.1-target.1)))
            .unwrap_or((0.0,0.0));
        let approach = Trail::new(start.0,start.1,target.0,target.1,cfg.seed ^ 0x7fea_0178,1.8);
        if let Some(approach) = routes::fit(cfg,approach,None,Some(crate::geo::site_floor(cfg,target.0,target.1))) {
            trails.push(approach);
        }
    }

    // Furnish the route with real places to stop: a cairn, a camp, a shrine,
    // and an overlook bench. They sit clear of the walking surface.
    let mut waysides = Vec::new();
    for (ti, trail) in trails.iter().enumerate() {
        let len = trail.length();
        if len < 46.0 {
            continue;
        }
        let count = if len > 120.0 { 2 } else { 1 };
        for k in 0..count {
            let t = if count == 1 {
                0.52
            } else {
                0.34 + 0.36 * k as f32
            };
            if t * len < 15.0 || (1.0 - t) * len < 15.0 {
                continue;
            }
            let (px, pz) = trail.at(t);
            let (tx, tz) = trail.tangent(t);
            let (nx, nz) = (-tz, tx);
            let roll = hash2(ti as i32, k + 7, cfg.seed.wrapping_add(0x51ED));
            let offset = 4.1 + roll * 0.8;
            let x = px + nx * offset;
            let z = pz + nz * offset;
            if super::undercrown::reserved(cfg, Vec3::new(x, 0.0, z))
                || super::valley::reserves(cfg, x, z)
                || super::sites::surface_reserved(cfg, x, z)
            {
                continue;
            }
            let height =
                sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z)).height;
            if height <= cfg.water_level() + 2 {
                continue;
            }
            let (lo, hi) = local_slope(cfg, x, z, 2.5);
            if hi - lo > cfg.blocks_for_span(2.4) {
                continue;
            }
            if waysides
                .iter()
                .any(|w: &Wayside| (w.x - x).hypot(w.z - z) < 14.0)
            {
                continue;
            }
            let kind = ((ti as u32 * 2 + k as u32 + cfg.seed.rotate_left(5)) % 4) as u8;
            waysides.push(Wayside {
                x,
                z,
                height,
                kind,
                tx,
                tz,
                nx,
                nz,
            });
        }
    }

    // One genuinely enterable nook: a roofed stone recess cut into the bank,
    // with a short spur from the main path.
    let mut nooks = Vec::new();
    if let Some((ti, trail)) = trails
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.length().total_cmp(&b.1.length()))
    {
        let t = 0.56;
        let (px, pz) = trail.at(t);
        let (tx, tz) = trail.tangent(t);
        let (nx, nz) = (-tz, tx);
        let x = px + nx * 6.8;
        let z = pz + nz * 6.8;
        let height = sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z)).height;
        let (lo, hi) = local_slope(cfg, x, z, 3.0);
        if height > cfg.water_level() + 3
            && hi - lo <= cfg.blocks_for_span(3.2)
            && !super::undercrown::reserved(cfg, Vec3::new(x, 0.0, z))
            && !super::valley::reserves(cfg, x, z)
            && !super::sites::surface_reserved(cfg, x, z)
            && !waysides.iter().any(|w| (w.x - x).hypot(w.z - z) < 13.0)
        {
            nooks.push(Nook {
                x,
                z,
                height,
                tx,
                tz,
                fx: -nx,
                fz: -nz,
            });
            let spur = Trail::new(
                px + nx * 1.2,
                pz + nz * 1.2,
                x,
                z,
                cfg.seed ^ (ti as u32),
                0.9,
            );
            if cfg.generator_version < 37 {
                trails.push(spur);
            } else if let Some(spur) = routes::fit(cfg, spur, Some(routes::height_at(trail, t)), None) {
                trails.push(spur);
            } else {
                nooks.pop();
            }
        }
    }

    Plan {
        trails,
        waysides,
        nooks,
    }
}

fn local_slope(cfg: &WorldSampleConfig, x: f32, z: f32, r: f32) -> (i32, i32) {
    let mut lo = i32::MAX;
    let mut hi = i32::MIN;
    for (dx, dz) in [(-r, -r), (r, -r), (-r, r), (r, r), (0.0, 0.0)] {
        let h = sample_column(
            cfg,
            cfg.blocks_for_height(x + dx),
            cfg.blocks_for_height(z + dz),
        )
        .height;
        lo = lo.min(h);
        hi = hi.max(h);
    }
    (lo, hi)
}

/// Generator hook from `sites::block`. `height` is the local natural column
/// top; `terrain_top` is one voxel above it in metres.
pub(super) fn block(cfg: &WorldSampleConfig, p: IVec3, height: i32, terrain_top: f32) -> Option<B> {
    if cfg.generator_version < VERSION || cfg.world_type == WorldType::Superflat {
        return None;
    }
    if planet_roads::active(cfg) {
        return planet_roads::block(cfg, p, terrain_top);
    }
    let v = cfg.voxel_meters();
    let m = (p.as_vec3() + Vec3::splat(0.5)) * v;
    let depth = terrain_top - m.y;
    // Trails live in the top few voxels; structures reach about four metres
    // above and nooks about two metres below. Reject the bulk of a chunk before
    // touching the cached plan.
    let clearance = if cfg.generator_version >= 62 { -18.0 } else if cfg.generator_version >= 37 { -8.0 } else { -4.5 };
    if !(clearance..=5.5).contains(&depth) {
        return None;
    }
    let plan = plan(cfg);

    for nook in &plan.nooks {
        if (m.x - nook.x).abs() <= 5.2 && (m.z - nook.z).abs() <= 5.2
            && let Some(b) = nook_block(cfg, nook, p, m) {
                return Some(b);
            }
    }
    for w in &plan.waysides {
        if (m.x - w.x).abs() <= 5.2 && (m.z - w.z).abs() <= 5.2
            && let Some(b) = wayside_block(cfg, w, p, m) {
                return Some(b);
            }
    }

    if cfg.generator_version >= roads::VERSION {
        // Where two roads overlap the nearer centreline paints, so a link
        // beside a spawn trail is not overwritten by that trail's shoulder.
        let old = routes::nearest(plan.trails.iter(), m);
        if let Some(b) = roads::block(cfg, m, terrain_top, old.map(|(d, _, _)| d)) {
            return Some(b);
        }
        return old.and_then(|o| routes::paint(cfg, o, m, terrain_top));
    }
    if cfg.generator_version >= 37 {
        return routes::block(cfg, &plan.trails, m, terrain_top);
    }

    let mut best: Option<(f32, &Trail)> = None;
    for trail in &plan.trails {
        if m.x < trail.min_x || m.x > trail.max_x || m.z < trail.min_z || m.z > trail.max_z {
            continue;
        }
        let (dist, _) = trail.nearest(m.x, m.z);
        if dist <= trail.half_width + 0.35 && best.is_none_or(|(best_dist, _)| dist < best_dist) {
            best = Some((dist, trail));
        }
    }
    let (dist, trail) = best?;
    if p.y != height || height <= cfg.water_level() + 1 {
        return None;
    }
    // Ragged edges and a few missing paving stones make this read as a worn
    // route rather than a painted ribbon.
    if dist > trail.half_width - 0.35 && hash2(p.x, p.z, trail.seed.wrapping_add(19)) > 0.62 {
        return None;
    }
    let stone = hash2(p.x, p.z, trail.seed.wrapping_add(23)) > 0.46;
    Some(if stone { B::Cobblestone } else { B::Gravel })
}

fn wayside_block(cfg: &WorldSampleConfig, w: &Wayside, p: IVec3, m: Vec3) -> Option<B> {
    let dx = m.x - w.x;
    let dz = m.z - w.z;
    let lx = dx * w.nx + dz * w.nz;
    let lz = dx * w.tx + dz * w.tz;
    let by = p.y - w.height;
    let ly = by as f32 * cfg.voxel_meters();
    if !(-2.0..=6.0).contains(&ly) {
        return None;
    }
    let surface = by == 0;
    match w.kind {
        0 => {
            // Cairn: a stepped stone marker visible over the hedgerow.
            if surface && lx.abs() < 1.25 && lz.abs() < 1.25 {
                return Some(B::Cobblestone);
            }
            if (0.0..=2.3).contains(&ly) && by > 0 && lx.hypot(lz) < 0.85 {
                return Some(B::WeatheredBricks);
            }
            if (2.3..=3.3).contains(&ly) && lx.hypot(lz) < 0.6 {
                return Some(B::Crystal);
            }
            if surface && (lx - 1.7).abs() < 0.5 && lz.abs() < 0.5 {
                return Some(B::Cobblestone);
            }
            if (0.0..=1.3).contains(&ly) && by > 0 && (lx - 1.7).abs() < 0.45 && lz.abs() < 0.45 {
                return Some(B::Torch);
            }
        }
        1 => {
            // Roadside camp: plank lean-to, fire, and a salvage cache.
            if surface && lx.abs() < 2.25 && lz.abs() < 2.25 {
                return Some(B::Gravel);
            }
            if (0.0..=1.2).contains(&ly) && (lx - 2.0).abs() < 0.55 && lz.abs() < 0.55 {
                return Some(B::SalvageCrate);
            }
            if (0.0..=2.2).contains(&ly) && (lx + 1.9).abs() < 0.55 && lz.abs() < 0.55 {
                return Some(B::Log);
            }
            if (1.9..=2.8).contains(&ly) && (0.0..=2.2).contains(&lx) && lz.abs() < 2.2 {
                return Some(B::Planks);
            }
            if (0.0..=1.3).contains(&ly) && (lx + 0.1).abs() < 0.45 && (lz - 1.0).abs() < 0.45 {
                return Some(B::Torch);
            }
        }
        2 => {
            // Wayshrine: paired monoliths with a crystal cap.
            if surface && lx.abs() < 2.45 && lz.abs() < 2.45 {
                return Some(B::Cobblestone);
            }
            for side in [-1.8, 1.8] {
                if (0.0..=3.2).contains(&ly) && by > 0 && (lx - side).abs() < 0.7 && lz.abs() < 0.7
                {
                    return Some(B::WeatheredBricks);
                }
                if (3.2..=4.1).contains(&ly) && (lx - side).abs() < 0.55 && lz.abs() < 0.55 {
                    return Some(B::Crystal);
                }
            }
            if (0.0..=1.3).contains(&ly) && by > 0 && lx.abs() < 0.5 && lz.abs() < 0.5 {
                return Some(B::Torch);
            }
        }
        _ => {
            // Overlook: a stone platform, bench, and a crystal brazier.
            if surface && lx.abs() < 2.4 && lz.abs() < 2.4 {
                return Some(B::Cobblestone);
            }
            if (0.0..=1.1).contains(&ly) && by > 0 && (1.4..=2.2).contains(&lx) && lz.abs() < 1.1 {
                return Some(B::Planks);
            }
            if (1.1..=1.9).contains(&ly) && (2.15..=2.5).contains(&lx) && lz.abs() < 1.2 {
                return Some(B::Log);
            }
            if (0.0..=1.0).contains(&ly) && by > 0 && (lx + 1.7).abs() < 0.55 && lz.abs() < 0.55 {
                return Some(B::WeatheredBricks);
            }
            if (1.0..=1.9).contains(&ly) && (lx + 1.7).abs() < 0.45 && lz.abs() < 0.45 {
                return Some(B::Torch);
            }
        }
    }
    None
}

fn nook_block(cfg: &WorldSampleConfig, n: &Nook, p: IVec3, m: Vec3) -> Option<B> {
    let v = cfg.voxel_meters();
    let dx = m.x - n.x;
    let dz = m.z - n.z;
    let r = dx.hypot(dz);
    if r > 4.8 {
        return None;
    }
    let by = p.y - n.height;
    let floor_depth = (1.0 / v).round().max(1.0) as i32;
    let wall_top = (3.2 / v).ceil().max(2.0) as i32;
    let roof_thickness = (0.9 / v).ceil().max(1.0) as i32;
    if !(-floor_depth..=wall_top + roof_thickness).contains(&by) {
        return None;
    }
    let mouth = (dx * n.fx + dz * n.fz) / r.max(0.001) > 0.72;
    let (nx, nz) = (-n.tz, n.tx);
    let lx = dx * nx + dz * nz;
    let lz = dx * n.tx + dz * n.tz;

    // Floor and threshold.
    if by >= -floor_depth && by < 0 && r < 3.75 {
        return Some(if r > 2.85 {
            B::WeatheredBricks
        } else {
            B::Cobblestone
        });
    }
    // Carve the room and its short mouth.
    if by > -floor_depth && by < wall_top && (r < 2.9 || (mouth && r < 4.7)) {
        // Bench against the inner wall, clear of the entrance.
        if by < -floor_depth + 2 && lx > 1.0 && lx < 2.1 && lz.abs() < 1.2 {
            return Some(B::Planks);
        }
        if by < -floor_depth + 2 && (lx + 1.5).abs() < 0.4 && (lz - 1.3).abs() < 0.4 {
            return Some(B::Torch);
        }
        return Some(B::Air);
    }
    // Retaining wall.
    if (2.9..4.05).contains(&r) && by > -floor_depth && by < wall_top && !mouth {
        return Some(B::WeatheredBricks);
    }
    // Low stone roof with a crystal skylight.
    if by >= wall_top && by < wall_top + roof_thickness && r < 4.25 {
        if r < 0.55 && hash2(p.x, p.z, cfg.seed.wrapping_add(991)) > 0.45 {
            return Some(B::Crystal);
        }
        return Some(B::WeatheredBricks);
    }
    None
}

/// True when vegetation should leave space for the trail, a wayside, or a nook.
/// Called only for actual tree/bush voxels, so it does not burden empty sky.
pub(super) fn trail_clears(cfg: &WorldSampleConfig, m: Vec3) -> bool {
    if cfg.generator_version < VERSION || cfg.world_type == WorldType::Superflat {
        return false;
    }
    if planet_roads::active(cfg) {
        return crate::planet_freight::clears(cfg, m) || planet_roads::clears(cfg, m);
    }
    let plan = plan(cfg);
    if plan
        .trails
        .iter()
        .any(|t| t.nearest(m.x, m.z).0 < t.half_width + 1.6)
    {
        return true;
    }
    if roads::clears(cfg, m) {
        return true;
    }
    if plan
        .waysides
        .iter()
        .any(|w| (m.x - w.x).hypot(m.z - w.z) < 5.8)
    {
        return true;
    }
    plan.nooks
        .iter()
        .any(|n| (m.x - n.x).hypot(m.z - n.z) < 6.5)
}

/// Camera along a planned link. Planet reviews address Anchor district cells.
pub(crate) fn road_review_view(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Option<(Vec3, Vec3)> {
    let trail = if planet_roads::active(cfg) {
        planet_roads::first_link(cfg, rx, rz)
    } else {
        roads::first_link(cfg, rx, rz)
    }?;
    let t0 = (20.0 / trail.length()).min(0.4);
    let t1 = (t0 + 0.4).min(1.0);
    let (ex, ez) = trail.at(t0);
    let (tx, tz) = trail.at(t1);
    // Stand above the road cut as well as the road, so the same pose is valid
    // at generators with no road: the player is never placed in the hillside.
    let ground = |x: f32, z: f32| cfg.world_y(sample_column(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z)).height + 1);
    let eye_y = routes::height_at(&trail, t0).max(ground(ex, ez)) + 2.4;
    Some((Vec3::new(ex, eye_y, ez), Vec3::new(tx, routes::height_at(&trail, t1) + 0.5, tz)))
}

/// Diegetic name for the biome/location HUD. These labels are intentionally
/// local and small; they give the surface a human scale without inventing a
/// second quest layer.
pub(super) fn location(cfg: &WorldSampleConfig, m: Vec3) -> Option<&'static str> {
    if cfg.generator_version < VERSION || cfg.world_type == WorldType::Superflat || planet_roads::active(cfg) {
        return None;
    }
    // Authored places own their labels; a trail may run up to the door without
    // renaming the ruin.
    if super::sites::surface_reserved(cfg, m.x, m.z) {
        return None;
    }
    let plan = plan(cfg);
    if plan
        .nooks
        .iter()
        .any(|n| (m.x - n.x).hypot(m.z - n.z) < 6.0)
    {
        return Some("Bank hollow");
    }
    for w in &plan.waysides {
        if (m.x - w.x).hypot(m.z - w.z) < 6.0 {
            return Some(match w.kind {
                0 => "Cairn of the quiet road",
                1 => "Wayfarer's rest",
                2 => "Twin-stone wayshrine",
                _ => "The long overlook",
            });
        }
    }
    if plan
        .trails
        .iter()
        .any(|t| t.nearest(m.x, m.z).0 < t.half_width + 3.0)
    {
        return Some("The surveyor's trace");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::VoxelScalePreset;

    fn config() -> WorldSampleConfig {
        WorldSampleConfig {
            planet: None,
            generator_version: VERSION,
            seed: crate::test_world::seed(),
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        }
    }

    #[test]
    fn trail_plan_connects_spawn_workshop_and_nearby_ruins() {
        let cfg = config();
        let p = plan(&cfg);
        assert!(p.trails.len() >= 2, "only {} trails", p.trails.len());
        // The starter workshop is the shortest route out of the clearing.
        let workshop = p
            .trails
            .iter()
            .map(|t| (t.bx.hypot(t.bz), t))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap();
        assert!(workshop.0 < 70.0, "workshop trail is {:.0} m", workshop.0);
        assert!(
            !p.waysides.is_empty() || !p.nooks.is_empty(),
            "trail has no furnished stopping points"
        );
        for w in &p.waysides {
            assert!(
                !crate::sites::surface_reserved(&cfg, w.x, w.z),
                "wayside overlaps an authored site or homestead"
            );
        }
        for n in &p.nooks {
            assert!(
                !crate::sites::surface_reserved(&cfg, n.x, n.z),
                "nook overlaps an authored site or homestead"
            );
        }
    }

    #[test]
    fn trail_surface_is_paved_and_vegetation_is_cleared_at_every_scale() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let cfg = WorldSampleConfig {
                voxel_scale: scale,
                ..config()
            };
            let p = plan(&cfg);
            let trail = p
                .trails
                .iter()
                .find(|t| t.length() > 28.0)
                .expect("a trail long enough to sample");
            let (x, z) = trail.at(0.62);
            let height =
                sample_column(&cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z)).height;
            let top = (height + 1) as f32 * cfg.voxel_meters();
            let block = block(
                &cfg,
                IVec3::new(cfg.blocks_for_height(x), height, cfg.blocks_for_height(z)),
                height,
                top,
            );
            assert!(
                matches!(block, Some(B::Cobblestone | B::Gravel)),
                "path midpoint at {scale:?} is {block:?}"
            );
            assert!(trail_clears(&cfg, Vec3::new(x, top, z)));
            assert_eq!(
                location(&cfg, Vec3::new(x, top, z)),
                Some("The surveyor's trace"),
                "trail name missing at {scale:?}"
            );
        }
    }

    #[test]
    fn nook_carves_headroom_below_the_surface() {
        let cfg = config();
        let p = plan(&cfg);
        let Some(nook) = p.nooks.first() else {
            return;
        };
        let x = cfg.blocks_for_height(nook.x);
        let z = cfg.blocks_for_height(nook.z);
        let ground = (nook.height + 1) as f32 * cfg.voxel_meters();
        for by in 0..=1 {
            let inside = block(
                &cfg,
                IVec3::new(x, nook.height + by, z),
                nook.height,
                ground,
            );
            assert_eq!(
                inside,
                Some(B::Air),
                "nook did not carve usable headroom at +{by}"
            );
        }
        let floor = block(&cfg, IVec3::new(x, nook.height - 1, z), nook.height, ground);
        assert_eq!(floor, Some(B::Cobblestone), "nook has no floor");
    }
}

#[cfg(test)]
#[path = "surface/survey_tests.rs"]
mod survey_tests;
