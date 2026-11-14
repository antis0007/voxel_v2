//! Split the cover's wet/dry triangles before the shader selects a flat material.
use crate::{mesh::Vertex3, world::BlockId};
use glam::Vec3;
use crate::render::horizon::parallel;
use std::{collections::{HashMap, HashSet}, sync::atomic::AtomicBool};

/// Resolve features below the face grid spacing before clipping the shore.
/// Sampling must be independent of invocation order.
/// Use a shared midpoint for each edge, including on the neighbouring land
/// or sea triangle. The original perimeter is retained for the face skirts.
pub(super) fn refine(
    vertices: &mut Vec<Vertex3>,
    beds: &mut Vec<f32>,
    triangles: &mut Vec<[u32; 3]>,
    base: u32,
    side: usize,
    scale: f32,
    sea: f32,
    scout_coast: bool,
    inland: impl Fn(Vec3) -> bool,
    near_patch: impl Fn(Vec3) -> bool,
    scarps: Option<&dyn Fn(Vec3) -> bool>,
    dry_relief: Option<&dyn Fn(Vec3) -> bool>,
    precise_scarps: bool,
    workers: usize,
    sample: impl Fn(Vec3, Vec3, f32, bool) -> (Vertex3, f32) + Sync,
) {
    const EDGE_LIMIT: usize = 2048;
    const RELIEF_PROBE_LIMIT: usize = 3072;
    const SCARP_PROBE_LIMIT: usize = 16_384;
    const MIN_EDGE_M: f32 = 22.0;
    const SCOUT_BAND_M: f32 = 8.0;
    let water = BlockId::Water.surface().shader_key();
    let initial_vertices = vertices.len();
    let edge = |a: u32, b: u32| (a.min(b), a.max(b));
    let perimeter = |(a, b): (u32, u32)| {
        let a = (a - base) as usize;
        let b = (b - base) as usize;
        let stride = side + 1;
        a < stride * stride && b < stride * stride
            && ((a / stride == b / stride && (a / stride == 0 || a / stride == side))
                || (a % stride == b % stride && (a % stride == 0 || a % stride == side)))
    };
    let mut unchanged = HashSet::new();
    let mut relief_vertices = HashSet::new();
    let mut relief_normals = HashSet::new();
    // The patch can move by half its 2048 m snap before a rebuild. Rank
    // nearby relief against that entire possible eye region, not just the
    // steepest distant edges. Keep the existing bounded probe budget.
    let centre = (Vec3::from_array(vertices[base as usize].pos)
        + Vec3::from_array(vertices[base as usize + (side + 1).pow(2) - 1].pos)) * 0.5;
    let scarp_budget = if precise_scarps { 262_144 } else { SCARP_PROBE_LIMIT };
    let scarp_edge = if precise_scarps { 4.0 } else { 9.0 };
    for _ in 0..if precise_scarps { 6 } else { 3 } {
        let mut candidates = HashMap::new();
        for &[a, b, c] in triangles.iter() {
            let wet = [a, b, c].map(|id| vertices[id as usize].surface[2] == water);
            let mixed = wet[0] != wet[1] || wet[1] != wet[2];
            let river = [a, b, c].iter().any(|&id| inland(Vec3::from_array(vertices[id as usize].pos)));
            let coast = mixed || [a, b, c].iter().any(|&id| {
                (beds[(id - base) as usize] - sea).abs() * scale <= SCOUT_BAND_M
            });
            let close_coast = coast && [a, b, c].iter().any(|&id| near_patch(Vec3::from_array(vertices[id as usize].pos)));
            let shore = river || close_coast || (scout_coast && coast);
            let dry = dry_relief.is_some() && !wet.into_iter().any(|wet| wet);
            if !shore && !dry && scarps.is_none() {
                continue;
            }
            let detail = river || close_coast;
            let forced = if precise_scarps && !wet.iter().any(|&wet| wet) {
                let [va, vb, vc] = [a, b, c].map(|id| vertices[id as usize]);
                let centre = (Vec3::from_array(va.pos) + Vec3::from_array(vb.pos) + Vec3::from_array(vc.pos)) / 3.0;
                scarps.is_some_and(|contains| {
                    if !contains(centre) { return false; }
                    let (_, source) = sample(centre, Vec3::Y, 4.0, false);
                    let interpolated = (beds[(a-base) as usize] + beds[(b-base) as usize] + beds[(c-base) as usize]) / 3.0;
                    (source - interpolated).abs() * scale > 0.35
                })
            } else { false };
            for key in [edge(a, b), edge(b, c), edge(c, a)] {
                if perimeter(key) || (!forced && !mixed && unchanged.contains(&key)) {
                    continue;
                }
                let [va, vb] = [key.0, key.1].map(|id| &vertices[id as usize]);
                let [a, b] = [va.pos, vb.pos];
                let metres = (a[0] - b[0]).hypot(a[2] - b[2]) * scale;
                let midpoint = Vec3::from_array(a).lerp(Vec3::from_array(b), 0.5);
                let close = near_patch(midpoint);
                let scarp = metres > scarp_edge && scarps.is_some_and(|contains| {
                    contains(midpoint)
                });
                let minimum_relief_edge = if close { 8.0 } else { 16.0 };
                let priority = if dry && metres > minimum_relief_edge
                    && dry_relief.is_some_and(|contains| {
                    contains(midpoint)
                }) {
                    let turn = (Vec3::from_array(va.normal) - Vec3::from_array(vb.normal)).length();
                    let grade = (a[1] - b[1]).abs() * scale / metres;
                    let offset = (midpoint - centre).abs() * scale;
                    let distance = (offset.x.max(offset.z) - 1024.0).max(0.0);
                    let threshold = if close { 0.025 } else { 0.075 };
                    (turn + grade.min(1.0) * 0.35 - threshold).max(0.0) * metres
                        / (1.0 + distance / 512.0)
                } else { 0.0 };
                let shore_edge = shore && metres > if detail { 6.0 } else { MIN_EDGE_M };
                if priority == 0.0 && !scarp && !shore_edge {
                    continue;
                }
                let (_, changed, inland, shoreline, _, split_interior, dry_priority) = candidates.entry(key)
                    .or_insert((metres, false, false, false, scarp, false, 0.0_f32));
                *changed |= mixed;
                *inland |= detail;
                *shoreline |= shore_edge || (scarp && shore);
                *split_interior |= forced;
                *dry_priority = dry_priority.max(priority);
            }
        }
        let mut work: Vec<_> = candidates.into_iter().collect();
        work.sort_unstable_by(|(ak, (am, ax, ar, shore_a, _, force_a, ap)), (bk, (bm, bx, br, shore_b, _, force_b, bp))| {
            shore_b.cmp(shore_a).then(force_b.cmp(force_a)).then(br.cmp(ar)).then(bx.cmp(ax))
                .then_with(|| if *shore_a { std::cmp::Ordering::Equal } else { bp.total_cmp(ap) })
                .then(bm.total_cmp(am)).then(ak.cmp(bk))
        });
        let mut shoreline_probes = 0;
        let mut scarp_probes = 0;
        let mut dry_probes = 0;
        let mut selected = Vec::new();
        for ((a, b), (metres, mixed, _, shore, scarp, forced, priority)) in work {
            let shore = shore && shoreline_probes < EDGE_LIMIT;
            let scarp = scarp && scarp_probes < scarp_budget;
            let dry = priority > 0.0 && !shore && !scarp && dry_probes < RELIEF_PROBE_LIMIT;
            if !shore && !scarp && !dry { continue; }
            shoreline_probes += usize::from(shore);
            scarp_probes += usize::from(scarp);
            dry_probes += usize::from(dry);
            let [va, vb] = [a, b].map(|id| vertices[id as usize]);
            let p = Vec3::from_array(va.pos).lerp(Vec3::from_array(vb.pos), 0.5);
            let normal = (Vec3::from_array(va.normal) + Vec3::from_array(vb.normal))
                .try_normalize().unwrap_or(Vec3::Y);
            selected.push(((a, b), metres, mixed, shore, scarp, forced, dry, p, normal));
        }
        // Group the ordinary probes by erosion tile while preserving split IDs.
        let mut staged = Vec::new();
        if scarps.is_none() && !precise_scarps {
            let mut order: Vec<_> = (0..selected.len()).collect();
            order.sort_unstable_by_key(|&i| (probe_locality(selected[i].7, scale), i));
            staged.resize(selected.len(), None);
            let sampled = parallel::map_blocks(&order, 64, workers, &AtomicBool::new(false), |&i| {
                let (_, metres, _, _, _, _, dry, p, normal) = selected[i];
                Some((i, sample(p, normal, metres * 0.5, dry)))
            }).expect("planet cover probe worker failed");
            for (i, result) in sampled { staged[i] = Some(result); }
        }
        let mut splits = HashMap::new();
        for (i, ((a, b), metres, mixed, shore, scarp, forced, dry, p, normal)) in selected.into_iter().enumerate() {
            let va = vertices[a as usize];
            let (vertex, bed) = staged.get(i).copied().flatten().unwrap_or_else(|| sample(p, normal, metres * 0.5, dry));
            let is_water = vertex.surface[2] == water;
            let shore_split = shore && (mixed || is_water != (va.surface[2] == water));
            let midpoint_error = (bed - 0.5 * (beds[(a - base) as usize] + beds[(b - base) as usize])).abs() * scale;
            let tolerance = if precise_scarps { (metres * 0.025).max(0.25) } else { (metres * 0.07).max(0.65) };
            let scarp_split = scarp && !is_water && !mixed
                && va.surface[2] != water && (forced || midpoint_error > tolerance);
            let relief_tolerance = if near_patch(p) { (metres * 0.025).max(0.35) }
                else { (metres * 0.045).max(0.8) };
            let relief_split = dry && !is_water && !mixed && midpoint_error > relief_tolerance;
            if !shore_split && !scarp_split && !relief_split {
                unchanged.insert(edge(a, b));
                continue;
            }
            splits.insert(edge(a, b), vertices.len() as u32);
            if relief_split {
                relief_vertices.insert(vertices.len() as u32);
                relief_normals.insert(vertices.len() as u32);
                for id in [a, b] {
                    let local = (id - base) as usize;
                    let stride = side + 1;
                    // Preserve the original patch boundary shared with skirts
                    // and neighbouring shells, including its shading normals.
                    let boundary = local < stride * stride &&
                        (local / stride == 0 || local / stride == side
                            || local % stride == 0 || local % stride == side);
                    if !boundary { relief_normals.insert(id); }
                }
            }
            vertices.push(vertex);
            beds.push(bed);
        }
        if splits.is_empty() {
            break;
        }
        let previous = std::mem::take(triangles);
        triangles.reserve(previous.len() + splits.len() * 2);
        for mut corners in previous {
            let mut mids = std::array::from_fn(|i| {
                splits.get(&edge(corners[i], corners[(i + 1) % 3])).copied()
            });
            if let [Some(ab), Some(bc), Some(ca)] = mids {
                let [a, b, c] = corners;
                triangles.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
            } else if let Some(i) = (0..3).find(|&i| mids[i].is_some() && mids[(i + 2) % 3].is_none()) {
                corners.rotate_left(i);
                mids.rotate_left(i);
                let [a, b, c] = corners;
                let m = mids[0].unwrap();
                triangles.push([a, m, c]);
                if let Some(n) = mids[1] {
                    triangles.extend([[m, b, n], [m, n, c]]);
                } else {
                    triangles.push([m, b, c]);
                }
            } else {
                triangles.push(corners);
            }
        }
    }
    if !relief_vertices.is_empty() {
        // A moved midpoint changes its original endpoints' shading too.
        // Accumulate all adjacent faces so refinement does not introduce a
        // lighting seam between the old coarse grid and its new vertices.
        let mut normals = vec![Vec3::ZERO; vertices.len() - base as usize];
        for &tri in triangles.iter() {
            if !tri.iter().any(|id| relief_normals.contains(id)) { continue; }
            let [a, b, c] = tri.map(|id| Vec3::from_array(vertices[id as usize].pos));
            let normal = (b - a).cross(c - a);
            for id in tri {
                if relief_normals.contains(&id) { normals[id as usize - base as usize] += normal; }
            }
        }
        for &id in &relief_normals {
            if let Some(normal) = normals[id as usize - base as usize].try_normalize() {
                vertices[id as usize].normal = normal.to_array();
            }
        }
    }
    let relief_splits = relief_vertices.len();
    let refined = vertices.len() - initial_vertices;
    if refined > 0 {
        println!("[planet-coast] grid={side} shoreline_samples={} dry_relief_samples={relief_splits} scarps={} inland={}", refined - relief_splits, scarps.is_some(), !scout_coast || vertices[base as usize..initial_vertices].iter().any(|v| inland(Vec3::from_array(v.pos))));
    }
}

// Locality hints match the regional and local erosion caches. They never select samples.
fn probe_locality(point: Vec3, scale: f32) -> [i32; 4] {
    let (_, local) = engine_core::cube_sphere::atlas::split((point * scale).as_dvec3());
    [(local.x / 2304.0).floor() as i32, (local.z / 2304.0).floor() as i32,
        (local.x / 448.0).floor() as i32, (local.z / 448.0).floor() as i32]
}
pub(super) struct Edges {
    water_key: f32,
    crossings: HashMap<(u32, u32), [u32; 2]>,
}

impl Edges {
    pub fn new() -> Self {
        Self {
            water_key: BlockId::Water.surface().shader_key(),
            crossings: HashMap::new(),
        }
    }

    pub fn get(&self, a: u32, b: u32) -> Option<[u32; 2]> {
        if a < b {
            self.crossings.get(&(a, b)).copied()
        } else {
            self.crossings.get(&(b, a)).map(|&[b, a]| [a, b])
        }
    }

    pub fn triangle(
        &mut self,
        mut corners: [u32; 3],
        vertices: &mut Vec<Vertex3>,
        indices: &mut Vec<u32>,
        bed: impl Fn(u32) -> f32,
    ) {
        let wet = corners.map(|id| vertices[id as usize].surface[2] == self.water_key);
        if wet[0] == wet[1] && wet[1] == wet[2] {
            indices.extend_from_slice(&corners);
            return;
        }

        // Rotate to the lone land or water vertex so both polygons keep winding.
        let lone = if wet[0] == wet[1] {
            2
        } else if wet[0] == wet[2] {
            1
        } else {
            0
        };
        corners.rotate_left(lone);
        let [a, b, c] = corners;
        let [ab_a, ab_b] = self.crossing(a, b, wet[lone], vertices, &bed);
        let [ac_a, ac_c] = self.crossing(a, c, wet[lone], vertices, &bed);
        indices.extend_from_slice(&[a, ab_a, ac_a, ab_b, b, c, ab_b, c, ac_c]);
    }

    fn crossing(
        &mut self,
        a: u32,
        b: u32,
        a_wet: bool,
        vertices: &mut Vec<Vertex3>,
        bed: &impl Fn(u32) -> f32,
    ) -> [u32; 2] {
        if let Some(pair) = self.get(a, b) {
            return pair;
        }
        let (wet_id, dry_id) = if a_wet { (a, b) } else { (b, a) };
        let mut wet = vertices[wet_id as usize];
        let mut dry = vertices[dry_id as usize];
        let head = wet.pos[1];
        let depth = (head - bed(wet_id)).max(0.05);
        let clearance = (dry.pos[1] - head).max(0.05);
        let mut p =
            Vec3::from_array(wet.pos).lerp(Vec3::from_array(dry.pos), depth / (depth + clearance));
        p.y = head;

        // Coincident positions, separate materials. Every face owns a single
        // water classification and the water depth goes to zero at the edge.
        wet.pos = p.to_array();
        wet.surface[3] = 0.0;
        dry.pos = wet.pos;
        let wet_edge = vertices.len() as u32;
        vertices.extend([wet, dry]);
        let pair = if a_wet {
            [wet_edge, wet_edge + 1]
        } else {
            [wet_edge + 1, wet_edge]
        };
        if a < b {
            self.crossings.insert((a, b), pair);
        } else {
            self.crossings.insert((b, a), [pair[1], pair[0]]);
        }
        pair
    }
}

#[cfg(test)]
#[path = "shoreline_tests.rs"]
mod tests;
