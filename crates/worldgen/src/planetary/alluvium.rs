//! Continuous channel and delta geometry on the same sphere as the receiver graph.
use super::{SEA, hydrology::Flow, morphodynamics, tectonics::smooth, topology::Mesh};
use morphodynamics::{meander, river};
use engine_core::cube_sphere::atlas::{self, RADIUS_M};
use glam::{DVec2, DVec3, IVec3};
use std::collections::HashMap;
mod channel_refine;
pub(super) use channel_refine::channel_anchors;

pub(super) const SOURCE_AREA: f64 = 320_000.0;
const BIN_M: f64 = 384.0;
const CURVE_STEPS: usize = 6;

pub(super) struct Network {
    reaches: Vec<Reach>,
    deltas: Vec<Delta>,
    coastal_tiles: Vec<morphodynamics::Tile>,
    riverbeds: Vec<river::Tile>,
    meander: Option<meander::Tile>,
    stage_hash: Option<u64>,
    bins: HashMap<IVec3, Vec<Feature>>,
}

#[derive(Clone, Copy)]
enum Feature {
    Reach(usize),
    Delta(usize),
    Coastal(usize),
    Riverbed(usize),
    Meander,
}

#[derive(Clone, Copy)]
struct Section {
    width: f64,
    depth: f64,
}

impl Section {
    // Area is a runoff proxy in km². These coefficients fit the compressed
    // world scale; width grows faster than depth, especially in lowlands.
    fn from_runoff(area: f64, slope: f64) -> Self {
        let q = area / 1.0e6;
        let confined = smooth(0.004, 0.07, slope);
        Self {
            width: ((0.9 + 3.1 * q.powf(0.49)) * (1.0 - 0.42 * confined)).clamp(1.3, 88.0),
            depth: ((0.55 + 0.85 * q.powf(0.34)) * (1.0 + 0.15 * confined)).min(15.0),
        }
    }
}

struct Reach {
    curve: Box<[DVec3]>,
    curvature: Box<[f64]>,
    start: Section,
    end: Section,
    levels: [f64; 2],
    backwater: [f64; 2],
    valley: f64,
    confinement: f64,
    gorge: f64,
    placer: [f64; 2],
    feed: [f64; 2],
    broad_plain: bool,
    migration: Option<Migration>,
    connected: bool,
}

struct Migration {
    valley_axis: [DVec3; 5],
    banks: Box<[[f64; 2]]>,
}

pub(super) struct Floodplain {
    pub ceiling: f64,
    pub weight: f64,
    pub wall: f64,
    pub fill: f64,
}

#[derive(Default)]
pub(super) struct Sample {
    pub calm: f64,
    pub floodplain: Option<Floodplain>,
    pub bed: Option<(f64, f64, f64)>,
    /// Physical water head survives filtering of a subpixel channel bed.
    pub channel_head: Option<f64>,
    pub bar_fill: f64,
    pub overbank: Option<f64>,
    pub riparian: f64,
    pub delta: f64,
    pub deposit: f64,
    pub morph: Option<morphodynamics::Surface>,
    pub placer: Option<(f64, f64)>,
    pub riverbed: Option<river::Surface>,
    /// Natural levee crest above the floodplain, metres (read by generator 296).
    pub levee: f64,
    /// Backswamp weight and the reach's plain stage there (read by generator 296).
    pub swamp: Option<(f64, f64)>,
    /// Crevasse-splay sand lobe height on the plain, metres (generator 296).
    pub splay: f64,
    /// Mid-channel or mouth bar weight; the bed rises to just above the stage (generator 296).
    pub bar: f64,
    /// Fresh sand cover weight on splays and emergent bars (generator 296).
    pub sand: f64,
}

struct Mouth {
    frame: Delta,
    reach: usize,
    runoff: f64,
    load: f64,
    section: Section,
}

fn on_sphere(p: DVec3) -> DVec3 {
    p.normalize() * RADIUS_M
}

fn bin(p: DVec3) -> IVec3 {
    (p / BIN_M).floor().as_ivec3()
}

fn coastal_forcing(mesh: &Mesh, height: &[f64], origin: DVec3, forward: DVec3, across: DVec3) -> (f64, f64) {
    let exposure: f64 = [-0.5, 0.0, 0.5]
        .into_iter()
        .map(|bearing| {
            let offshore = forward + across * bearing;
            [500.0, 1100.0, 2200.0]
                .into_iter()
                .map(|distance| {
                    let h = mesh.sample(height, (origin + offshore * distance).normalize());
                    1.0 - smooth(SEA - 14.0, SEA, h)
                })
                .product::<f64>()
        })
        .sum::<f64>()
        / 3.0;
    let normal = origin.normalize();
    let wind = DVec3::Y.cross(normal).normalize_or_zero()
        * if normal.y.abs() > 0.42 && normal.y.abs() < 0.82 {
            -1.0
        } else {
            1.0
        };
    (exposure * wind.dot(across), exposure)
}

/// Propagate the water profile from outlets toward divides. The receiver's
/// level, lake spillways, and sea level bound the incision.
pub(super) fn channel_levels(nodes: &[DVec3], height: &[f64], flow: &Flow, generator: u32) -> Vec<f64> {
    let refined = matches!(generator, super::WATERSHED_EROSION_GENERATOR | super::VOLCANIC_SECTOR_GENERATOR);
    let canyons = generator == super::LAYERED_CANYON_GENERATOR;
    let mut levels = vec![SEA; nodes.len()];
    let lake = |i: usize| flow.spill[i] > height[i] + 0.75;
    for &i in &flow.order {
        if height[i] <= SEA { continue; }
        let r = flow.receiver[i];
        let mut lower = levels[r];
        let cut = if i == r || lake(i) || lake(r) {
            0.0
        } else {
            let area = flow.discharge[i];
            let legacy = (0.8 * (area / 1.0e6).sqrt()).min(8.0);
            if refined || canyons {
                let distance = nodes[i].distance(nodes[r]).max(1.0);
                let fall = (flow.spill[i] - flow.spill[r]).max(0.0);
                let grade = fall / distance;
                let power = smooth(0.004, 0.085, grade) * smooth(SOURCE_AREA, 5.5e6, area);
                let inland = smooth(SEA + 8.0, SEA + 92.0, height[r]);
                lower += fall.min(distance * 0.0015);
                let incision = if canyons { 23.0 } else { 34.0 };
                legacy + incision * power * inland
            } else {
                legacy
            }
        };
        levels[i] = (flow.spill[i] - cut).max(lower).max(SEA);
    }
    levels
}

impl Network {
    pub fn counts(&self) -> (usize, usize) {
        (
            self.reaches.len(),
            self.deltas.len() + self.coastal_tiles.len(),
        )
    }

    pub fn morphology_checksum(&self, mut hash: u64) -> u64 {
        if let Some(stage) = self.stage_hash {
            hash = (hash ^ stage).wrapping_mul(0x100000001b3);
        }
        for reach in self.reaches.iter().filter(|reach| !reach.curvature.is_empty()) {
            for point in &reach.curve {
                for axis in point.to_array() {
                    hash = (hash ^ axis.to_bits()).wrapping_mul(0x100000001b3);
                }
            }
        }
        for tile in &self.coastal_tiles {
            hash = tile.checksum(hash);
        }
        hash
    }

    pub fn coast_origin(&self) -> Option<DVec3> {
        self.coastal_tiles.first().map(morphodynamics::Tile::origin)
    }

    pub fn bake_meander(&self, scrolls: bool, terrain: impl Fn(DVec3) -> super::geo::Ground) -> Option<meander::Tile> {
        let key = |point: DVec3| point.round().as_ivec3();
        let starts: HashMap<_, _> = self.reaches.iter().enumerate().filter(|(_, r)| r.broad_plain)
            .map(|(i, r)| (key(r.curve[0]), i)).collect();
        let mut candidates = Vec::new();
        for (first, reach) in self.reaches.iter().enumerate().filter(|(_, r)| r.broad_plain) {
            let head = reach.levels[0];
            let mut route = vec![reach.curve[0]];
            let mut width = reach.start.width;
            let mut index = first;
            let mut length = 0.0;
            for _ in 0..5 {
                let r = &self.reaches[index];
                route.extend(r.curve.iter().skip(1).copied());
                length += r.curve.windows(2).map(|s| s[0].distance(s[1])).sum::<f64>();
                width = width.min(r.start.width).min(r.end.width);
                let tail = r.levels[1];
                let chord = route[0].distance(*route.last().unwrap());
                let sinuosity = length / chord.max(1.0);
                if length > 4_300.0 { break; }
                if length >= 850.0 && chord > 220.0 && sinuosity > 1.19
                    && width >= 6.5 && head - tail >= 0.6 && (head - tail) / length < 0.014
                {
                    let score = sinuosity * (width.min(42.0) / 16.0).sqrt();
                    candidates.push((score, meander::Reach { route: route.clone(), width, head, tail }));
                }
                let Some(&next) = starts.get(&key(*route.last().unwrap())) else { break };
                index = next;
            }
        }
        candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
        candidates.into_iter().take(40).find_map(|(_, reach)| meander::Tile::bake(reach, scrolls, &terrain))
    }

    pub fn install_meander(&mut self, tile: meander::Tile, hash: u64) -> u64 {
        let origin = tile.origin();
        let bounds = DVec3::splat(tile.radius());
        self.index(Feature::Meander, origin - bounds, origin + bounds);
        let hash = tile.checksum(hash);
        self.meander = Some(tile);
        hash
    }

    pub fn meander_overlaps(&self, point: DVec3, radius: f64) -> bool {
        self.meander.as_ref().is_some_and(|tile| tile.overlaps(point, radius))
    }

    /// Upstream crossings into a local simulation window. Use the oriented
    /// regional drainage, so a river leaving the window adds no second source.
    pub fn incoming_reaches(&self, centre: DVec2, radius: f64) -> Vec<(DVec2, DVec2, f64, f64)> {
        let mut result = Vec::new();
        let body = atlas::to_body(DVec3::new(centre.x, 0.0, centre.y));
        for reach in self.reaches.iter().filter(|r| r.feed[0] > 0.0 && r.levels[1] > SEA + 1.0) {
            for pair in reach.curve.windows(2) {
                let search = pair[0].distance(pair[1]) + radius * 1.8;
                if pair[0].distance_squared(body) > search * search { continue; }
                let [a, b] = [pair[0], pair[1]].map(|p| {
                    let p = atlas::from_body(p);
                    DVec2::new(p.x, p.z)
                });
                let edge = b - a;
                let span = edge.length();
                if !(0.01..=1200.0).contains(&span) || a.distance_squared(centre) <= radius * radius {
                    continue;
                }
                let downstream = edge / span;
                let closest = (centre - a).dot(downstream);
                if closest <= 0.0 { continue; }
                let distance2 = (a + closest * downstream).distance_squared(centre);
                let chord2 = radius * radius - distance2;
                if chord2 <= 9.0 { continue; }
                let entering = closest - chord2.sqrt();
                if !(0.0..span).contains(&entering) { continue; }
                let p = a + (entering + 2.0).min(span) * downstream;
                result.push((p, downstream, reach.feed[0], reach.start.width));
                break;
            }
        }
        result
    }

    pub fn bake_riverbeds(
        &self,
        workers: usize,
        terrain: impl Fn(DVec3) -> super::geo::Ground + Sync,
        rock_strength: Option<impl Fn(DVec3, f64) -> f64 + Sync>,
    ) -> Vec<river::Tile> {
        let mut candidates = Vec::new();
        for reach in &self.reaches {
            let a = reach.curve[0];
            let b = reach.curve[reach.curve.len() - 1];
            let run = a.distance(b);
            let fall = reach.levels[0] - reach.levels[1];
            let width = reach.start.width.min(reach.end.width);
            if reach.levels[1] < SEA + 3.0 || fall < 0.35 || fall / run > 0.020
                || run < 300.0 || width < 6.5 || reach.feed[0] == 0.0
            {
                continue;
            }
            // Leave both confluences on the regional network. The solve can
            // migrate within the intervening valley and reconnect at its ends.
            let length = (run * 0.86).min(550.0);
            let centre = reach.curve[reach.curve.len() / 2];
            let forward = (b - a).reject_from_normalized(centre.normalize()).normalize();
            let origin = on_sphere(centre - forward * (length * 0.5));
            let tailwater = reach.levels[1] + reach.backwater[1] + fall * (0.5 - length / (2.0 * run));
            let score = width * (1.0 + smooth(0.001, 0.008, fall / run));
            candidates.push((score, river::Reach { origin, downstream: forward, half_width: width,
                length, tailwater, runoff: reach.feed[0], supply: reach.feed[1] }));
        }
        candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut tiles: Vec<river::Tile> = Vec::new();
        let mut candidates = candidates.into_iter().take(24).peekable();
        while tiles.len() < 6 && candidates.peek().is_some() {
            let mut batch: Vec<river::Reach> = Vec::with_capacity(workers);
            while batch.len() < workers.min(6 - tiles.len()) {
                let Some((_, reach)) = candidates.peek() else { break };
                if tiles.iter().any(|tile| tile.origin.distance(reach.origin) < 1200.0) {
                    candidates.next();
                    continue;
                }
                // A lower-ranked reach can only reuse this location if the
                // earlier bake fails. Resolve that dependency in order.
                if batch.iter().any(|other| other.origin.distance(reach.origin) < 1200.0) {
                    break;
                }
                let Some((_, reach)) = candidates.next() else { break };
                batch.push(reach);
            }
            let solved = super::map_ordered(batch.len(), workers, |i| {
                river::Tile::bake(&batch[i], &terrain, rock_strength.as_ref())
            });
            tiles.extend(solved.into_iter().flatten());
        }
        tiles
    }

    pub fn install_riverbeds(&mut self, tiles: Vec<river::Tile>, mut hash: u64) -> u64 {
        for tile in tiles {
            let bounds = DVec3::splat(tile.radius());
            self.index(Feature::Riverbed(self.riverbeds.len()), tile.origin - bounds, tile.origin + bounds);
            hash = tile.checksum(hash);
            self.riverbeds.push(tile);
        }
        hash
    }

    pub fn survey_outlooks(&self, terrain: impl Fn(DVec3) -> super::geo::Ground) -> Vec<Option<super::outlook::Outlook>> {
        self.coastal_tiles.iter().map(|tile| tile.survey_outlook(&terrain)).collect()
    }

    pub fn install_outlooks(&mut self, outlooks: Vec<Option<super::outlook::Outlook>>, mut hash: u64) -> u64 {
        for (tile, outlook) in self.coastal_tiles.iter_mut().zip(outlooks) {
            hash = tile.set_outlook(outlook, hash);
        }
        hash
    }

    pub fn outlook_points(&self) -> Option<[DVec3; 3]> {
        self.coastal_tiles.iter().find_map(morphodynamics::Tile::outlook_points)
    }

    pub fn build(
        mesh: &Mesh,
        nodes: &[DVec3],
        height: &[f64],
        flow: &Flow,
        sediment: &[f64],
        gravel: &[f64],
        erodibility: &[f64],
        gold: &[f64],
        generator: u32,
        seed: u32,
    ) -> Self {
        let morphodynamic = generator >= engine_voxel::MORPHODYNAMIC_PLANET_GENERATOR;
        let canyons = generator == super::LAYERED_CANYON_GENERATOR;
        let branching = super::branching_watersheds(generator);
        let connected = branching || canyons || generator == super::CONNECTED_RELIEF_GENERATOR;
        let mut network = Self {
            reaches: Vec::new(),
            deltas: Vec::new(),
            coastal_tiles: Vec::new(),
            riverbeds: Vec::new(),
            meander: None,
            stage_hash: None,
            bins: HashMap::new(),
        };
        let mut mouths = Vec::new();
        let mut main = vec![None; nodes.len()];
        for &i in &flow.order {
            let r = flow.receiver[i];
            if i != r
                && height[i] > SEA
                && flow.discharge[i] >= SOURCE_AREA
                && main[r].is_none_or(|j| flow.discharge[j] < flow.discharge[i])
            {
                main[r] = Some(i);
            }
        }
        // Both links of the main stem use the same projected tangent at each
        // shared node. An entering tributary may keep a distinct approach.
        let tangent = |i: usize| {
            let p = nodes[i];
            let incoming = main[i].map_or(DVec3::ZERO, |j| (p - nodes[j]).normalize_or_zero());
            let outgoing = (nodes[flow.receiver[i]] - p).normalize_or_zero();
            let radial = p.normalize();
            let heading = incoming + outgoing;
            (heading - radial * heading.dot(radial)).normalize_or_zero()
        };
        let lake = |i: usize| flow.spill[i] > height[i] + 0.75;
        let levels = channel_levels(nodes, height, flow, generator);
        let slope =
            |i: usize, r: usize| (levels[i] - levels[r]) / nodes[i].distance(nodes[r]).max(1.0);
        let stages = morphodynamic.then(|| {
            let mut stage = levels.clone();
            let mut max_rise: f64 = 0.0;
            let mut raised = 0;
            for &i in &flow.order {
                let r = flow.receiver[i];
                let area = flow.discharge[i];
                if r == i || height[i] <= SEA || lake(i) || area < SOURCE_AREA {
                    continue;
                }
                let tail = stage[r];
                let base = levels[i];
                stage[i] = base.max(tail);
                if area >= 6.0e6 {
                    let section = Section::from_runoff(area, slope(i, r));
                    stage[i] =
                        standard_step(base, tail, section, nodes[i].distance(nodes[r]), area);
                }
                let rise = stage[i] - base;
                raised += usize::from(rise > 0.05);
                max_rise = max_rise.max(rise);
            }
            network.stage_hash = Some(stage.iter().fold(0xcbf29ce484222325u64, |h, v| {
                (h ^ v.to_bits()).wrapping_mul(0x100000001b3)
            }));
            eprintln!("[worldgen] river_backwater reaches={raised} max_rise_m={max_rise:.3}");
            stage
        });
        for &i in &flow.order {
            let r = flow.receiver[i];
            if i == r
                || height[i] <= SEA
                || flow.discharge[i] < SOURCE_AREA
                || (lake(i) && lake(r) && flow.spill[i] == flow.spill[r])
            {
                continue;
            }
            let [a, b] = [nodes[i], nodes[r]];
            let link = b - a;
            let rr = flow.receiver[r];
            let link_direction = link.normalize_or_zero();
            let start_direction = if branching {
                tangent(i).try_normalize().unwrap_or(link_direction)
            } else {
                main[i].map_or(link, |j| b - nodes[j]).normalize_or_zero()
            };
            let end_direction = if branching && main[r] == Some(i) {
                tangent(r).try_normalize().unwrap_or(link_direction)
            } else if branching {
                let guide = tangent(r);
                (link_direction + guide * 0.32 * guide.dot(link_direction).max(0.0)).normalize_or_zero()
            } else if r == rr || main[r] != Some(i) {
                link.normalize()
            } else {
                (nodes[rr] - a).normalize()
            };
            let grade = slope(i, r);
            let start = Section::from_runoff(flow.discharge[i], grade);
            let history = super::uses_landscape_history(generator);
            let coastal_approach = height[r] <= SEA && main[r] == Some(i)
                && generator >= engine_voxel::COASTAL_APPROACH_PLANET_GENERATOR;
            let end = if height[r] <= SEA {
                // The backwater at an open mouth is wider than the inland reach.
                let expansion = if coastal_approach && history {
                    1.0 + 0.32 * (1.0 - smooth(0.004, 0.06, grade))
                } else if coastal_approach { 1.85 } else { 1.3 };
                Section {
                    width: (start.width * expansion).min(96.0),
                    depth: start.depth,
                }
            } else if main[r] == Some(i) && rr != r {
                Section::from_runoff(flow.discharge[r], slope(r, rr))
            } else {
                start
            };
            let confinement = smooth(0.004, 0.06, grade);
            let length = link.length();
            let bend_side = a.normalize().cross(link.normalize());
            let bend_sign = if i.wrapping_mul(0x9e3779b1).count_ones() % 2 == 0 {
                1.0
            } else {
                -1.0
            };
            let bend = if branching { DVec3::ZERO } else { bend_side
                * bend_sign
                * (start.width * 1.7).min(length * 0.19)
                * (1.0 - confinement) };
            let ctrl_a = a + start_direction * (length / 3.0) + bend;
            let outlet_bend = if coastal_approach && !branching {
                // Bend back across the approach into the wider tidal reach.
                -bend_side * bend_sign * (end.width * 2.2).min(length * 0.16)
            } else {
                bend
            };
            let ctrl_b = b - end_direction * (length / 3.0) + outlet_bend;
            let mut valley =
                (start.width.max(end.width) * (1.6 - 0.55 * confinement) + 24.0).min(160.0);
            if coastal_approach && !history {
                // Spread the cut into a low coastal apron with a gradual edge.
                valley = (valley * 1.6).min(224.0);
            }
            let mut reach = Reach::new(
                a,
                ctrl_a,
                ctrl_b,
                b,
                start,
                end,
                [levels[i], levels[r]],
                valley,
                confinement,
            );
            reach.connected = connected;
            if generator >= engine_voxel::RIVER_MORPHOLOGY_GENERATOR && (!coastal_approach || history) {
                reach.fit_valley(mesh, height, a, b, start_direction, end_direction, erodibility, seed, branching);
            }
            let refined = connected || generator == super::REFINED_DRAINAGE_GENERATOR;
            if !refined && generator >= engine_voxel::FLOODPLAIN_MEANDER_GENERATOR
                && !coastal_approach && height[r] > SEA + 3.0 && !lake(i) && !lake(r)
                && grade < 0.015 && start.width >= 6.5
            {
                reach.broad_plain = true;
                reach.valley = reach.valley.max((94.0 + 4.0 * start.width.max(end.width)).min(330.0));
            }
            if refined && !coastal_approach && !lake(i) && !lake(r) {
                channel_refine::evolve(&mut reach, mesh, height, erodibility, flow.discharge[i], grade, seed, branching);
            }
            if generator >= super::ERODED_RELIEF_GENERATOR && !coastal_approach
                && !reach.broad_plain && !lake(i) && !lake(r) && height[r] > SEA + 12.0
            {
                reach.gorge = smooth(0.017, 0.085, grade)
                    * smooth(SEA + 12.0, SEA + 50.0, height[r]);
                if canyons {
                    reach.valley = reach.valley * (1.0 - 0.24 * reach.gorge) + 10.0 * reach.gorge;
                } else {
                    reach.valley += reach.gorge * 32.0;
                }
            }
            if let Some(stages) = &stages {
                reach.backwater = [stages[i] - levels[i], stages[r] - levels[r]];
            }
            if !gold.is_empty() && height[r] > SEA {
                reach.placer = [gold[i], gold[r]];
            }
            if let Some(&load) = gravel.get(i)
                && height[r] > SEA && !lake(i) && !lake(r)
            {
                reach.feed = [flow.discharge[i], load];
            }
            network.add_reach(reach);
            if height[r] <= SEA && main[r] == Some(i) {
                let reach = network.reaches.len() - 1;
                if let Some(frame) =
                    Delta::candidate(mesh, height, a, b, flow.discharge[i], sediment[i])
                {
                    let mouth = Mouth {
                        frame,
                        reach,
                        runoff: flow.discharge[i],
                        load: sediment[i],
                        section: start,
                    };
                    if morphodynamic {
                        mouths.push(mouth);
                    } else {
                        network.add_delta(mouth);
                    }
                }
            }
        }
        if morphodynamic {
            network.bake_coast(mesh, height, mouths, generator, seed);
        }
        if connected || generator == super::REFINED_DRAINAGE_GENERATOR {
            let mut lengths = Vec::new();
            let mut plains = 0;
            for reach in network.reaches.iter().filter(|r| r.migration.is_some()) {
                let chord = reach.curve[0].distance(reach.curve[reach.curve.len() - 1]);
                let length: f64 = reach.curve.windows(2).map(|p| p[0].distance(p[1])).sum();
                lengths.push(length / chord.max(1.0));
                plains += usize::from(reach.broad_plain);
            }
            lengths.sort_unstable_by(f64::total_cmp);
            let p90 = lengths.get(lengths.len().saturating_sub(1) * 9 / 10).copied().unwrap_or(1.0);
            eprintln!("[worldgen] nested_channels reaches={} swept_floodplains={plains} sinuosity_p90={p90:.3}", lengths.len());
        }
        network
    }

    fn bake_coast(&mut self, mesh: &Mesh, height: &[f64], mut mouths: Vec<Mouth>, generator: u32, seed: u32) {
        const MAX_TILES: usize = 6;
        const FINE_TILES: usize = 2;
        let tidal = generator >= engine_voxel::COASTAL_DELTA_GENERATOR;
        let distributary = generator == super::DISTRIBUTARY_GENERATOR;
        let feedback = generator == super::DELTA_FEEDBACK_GENERATOR;
        let evolving = generator == super::EVOLVING_DELTA_GENERATOR;
        let shore_history = super::uses_landscape_history(generator);
        let mobile = tidal || generator == super::MOBILE_DELTA_GENERATOR;
        let started = std::time::Instant::now();
        mouths.sort_by(|a, b| b.load.total_cmp(&a.load).then(a.reach.cmp(&b.reach)));
        let mut centres: Vec<DVec3> = Vec::new();
        let mut budget = morphodynamics::Budget::default();
        let mut attempts = 0;
        for mouth in mouths {
            if attempts >= MAX_TILES {
                break;
            }
            let Delta {
                origin,
                forward,
                across,
                ..
            } = mouth.frame;
            let radius = morphodynamics::RADIUS;
            if centres
                .iter()
                .any(|&p| p.distance_squared(origin) < (2.0 * radius).powi(2))
            {
                continue;
            }
            let reach = &self.reaches[mouth.reach];
            let terrain = |p: DVec3| {
                let mut top = mesh.sample(height, p.normalize());
                // The coarse drainage mesh omits the uneven substrate across
                // shallow shelves. Resolve it before water chooses its paths.
                if tidal && (top - SEA).abs() < 7.0 {
                    let shore = 1.0 - smooth(2.0, 7.0, (top - SEA).abs());
                    let swales = super::noise(seed, p / 47.0, 1523) - 0.5;
                    let ridges = super::noise(seed, p / 18.0, 1529) - 0.5;
                    top += shore * if shore_history { 1.35 * swales + 0.45 * ridges }
                        else { 3.2 * swales + 0.9 * ridges };
                }
                let mut head = SEA;
                let mut sample = Sample {
                    calm: 1.0,
                    ..Sample::default()
                };
                reach.sample(0, p, top, 1.0, &mut sample);
                if let Some(plain) = sample.floodplain {
                    top -= (top - plain.ceiling).max(0.0) * plain.weight;
                }
                if let Some((bed, stage, weight)) = sample.bed {
                    if mobile {
                        top -= (top - bed).max(0.0) * weight;
                        if top < stage { head = head.max(stage); }
                    } else {
                        top = top.min(bed);
                    }
                }
                (top, head)
            };
            let (drift, exposure) = coastal_forcing(mesh, height, origin, forward, across);
            let grid = match (tidal, mobile, attempts < FINE_TILES) {
                (true, _, fine) if shore_history => morphodynamics::Grid::shore_history(fine, exposure),
                (true, _, true) if distributary => morphodynamics::Grid::DISTRIBUTARY,
                (true, _, false) if distributary => morphodynamics::Grid::DISTRIBUTARY_COARSE,
                (true, _, true) if feedback => morphodynamics::Grid::FEEDBACK,
                (true, _, false) if feedback => morphodynamics::Grid::FEEDBACK_COARSE,
                (true, _, true) if evolving => morphodynamics::Grid::EVOLVING,
                (true, _, false) if evolving => morphodynamics::Grid::EVOLVING_COARSE,
                (true, _, true) => morphodynamics::Grid::TIDAL,
                (true, _, false) => morphodynamics::Grid::TIDAL_COARSE,
                (_, true, true) => morphodynamics::Grid::MOBILE,
                (_, true, false) => morphodynamics::Grid::MOBILE_COARSE,
                _ => morphodynamics::Grid::LEGACY,
            };
            attempts += 1;
            let mut tile = morphodynamics::Tile::bake(
                origin,
                forward,
                across,
                mouth.runoff,
                mouth.load,
                mouth.section.width,
                drift,
                grid,
                terrain,
            );
            if !tile.outlet_connected {
                continue;
            }
            if !mobile && generator >= engine_voxel::COASTAL_RELIEF_PLANET_GENERATOR {
                let inlet = (generator >= engine_voxel::ESTUARY_PLANET_GENERATOR).then(|| {
                    let point = origin + forward * super::estuary::INLET;
                    let mut best = (f64::INFINITY, point, 0.0);
                    for (i, pair) in reach.curve.windows(2).enumerate() {
                        let edge = pair[1] - pair[0];
                        let t =
                            ((point - pair[0]).dot(edge) / edge.length_squared()).clamp(0.0, 1.0);
                        let at = pair[0] + edge * t;
                        let distance = at.distance_squared(point);
                        if distance < best.0 {
                            best = (distance, at, (i as f64 + t) / (reach.curve.len() - 1) as f64);
                        }
                    }
                    let [up, down] = reach.levels;
                    glam::DVec2::new((best.1 - origin).dot(across), up + (down - up) * best.2)
                });
                tile.connect_outlet(
                    mouth.section.width, inlet, drift,
                    generator == super::DELTA_ISLANDS_GENERATOR,
                );
            }
            budget.water_error = budget.water_error.max(tile.budget.water_error.abs());
            budget.sediment_error = budget.sediment_error.max(tile.budget.sediment_error.abs());
            budget.bed_change += tile.budget.bed_change;
            budget.new_land_m2 += tile.budget.new_land_m2;
            let bounds = DVec3::splat(radius);
            self.index(
                Feature::Coastal(self.coastal_tiles.len()),
                origin - bounds,
                origin + bounds,
            );
            self.coastal_tiles.push(tile);
            centres.push(origin);
        }
        eprintln!(
            "[worldgen] coastal_morphodynamics tiles={} bake_ms={:.1} bed_change_m3={:.1} new_land_m2={:.1} max_water_residual_m3={:.6} max_sediment_residual_m3={:.6}",
            centres.len(),
            started.elapsed().as_secs_f64() * 1000.0,
            budget.bed_change,
            budget.new_land_m2,
            budget.water_error,
            budget.sediment_error
        );
    }

    fn index(&mut self, feature: Feature, low: DVec3, high: DVec3) {
        let low = bin(low);
        let high = bin(high);
        for x in low.x..=high.x {
            for y in low.y..=high.y {
                for z in low.z..=high.z {
                    self.bins
                        .entry(IVec3::new(x, y, z))
                        .or_default()
                        .push(feature);
                }
            }
        }
    }

    fn add_reach(&mut self, reach: Reach) {
        let width = reach.start.width.max(reach.end.width);
        let radius = DVec3::splat(width + reach.valley.max(40.0));
        let guide = reach.migration.iter().flat_map(|m| &m.valley_axis);
        let points = reach.curve.iter().chain(guide).copied();
        let low = points.clone().reduce(DVec3::min).unwrap() - radius;
        let high = points.reduce(DVec3::max).unwrap() + radius;
        self.index(Feature::Reach(self.reaches.len()), low, high);
        self.reaches.push(reach);
    }

    fn add_delta(&mut self, mouth: Mouth) {
        let Mouth {
            frame: delta,
            runoff,
            section: parent,
            ..
        } = mouth;
        let Delta {
            origin: b,
            forward,
            across,
            length,
            width,
            inland,
        } = delta;
        let radius = DVec3::splat(length.max(inland) + width * 0.2);
        self.index(Feature::Delta(self.deltas.len()), b - radius, b + radius);
        self.deltas.push(delta);
        // Unequal branches partition the parent runoff at a shared sea-level
        // junction. Each reaches beyond the depositional apron into open water.
        for (fraction, lateral, extent) in
            [(0.48, -0.20, 1.12), (0.31, 0.77, 0.95), (0.21, -0.91, 0.78)]
        {
            let branch = Section::from_runoff(runoff * fraction, 0.0);
            let start = Section {
                width: branch.width.min(parent.width),
                depth: branch.depth.min(parent.depth),
            };
            let end = Section {
                width: start.width * 0.75,
                depth: start.depth * 0.68,
            };
            let tip = b + forward * (length * extent) + across * (width * lateral);
            let ctrl_a = b + forward * (length * 0.27) + across * (width * lateral * 0.15);
            let ctrl_b = b + (tip - b) * 0.72 + across * (width * lateral * 0.17);
            self.add_reach(Reach::new(
                b, ctrl_a, ctrl_b, tip, start, end, [SEA; 2], 12.0, 0.0,
            ));
        }
    }

    pub fn sample(&self, p: DVec3, height: f64, footprint: f32) -> Sample {
        let mut sample = Sample {
            calm: 1.0,
            ..Sample::default()
        };
        let Some(features) = self.bins.get(&bin(p)) else {
            return sample;
        };
        for &feature in features {
            match feature {
                Feature::Reach(i) => {
                    self.reaches[i].sample(i as u32, p, height, f64::from(footprint), &mut sample)
                }
                Feature::Delta(i) => {
                    let delta = &self.deltas[i];
                    let (strength, deposit) = delta.sample(p);
                    sample.delta = sample.delta.max(strength);
                    sample.deposit = sample.deposit.max(deposit);
                }
                Feature::Coastal(i) => {
                    if let Some(surface) = self.coastal_tiles[i].sample(p) {
                        sample.morph = Some(surface);
                    }
                }
                Feature::Riverbed(i) => {
                    if let Some(surface) = self.riverbeds[i].sample(p, footprint)
                        && sample.riverbed.is_none_or(|other| other.blend < surface.blend)
                    {
                        sample.riverbed = Some(surface);
                    }
                }
                Feature::Meander => {
                    if let Some(surface) = self.meander.as_ref().and_then(|tile| tile.sample(p, footprint)) {
                        sample.riverbed = Some(surface);
                    }
                }
            }
        }
        sample
    }
}

/// A steady standard-step approximation. Tailwater comes from the downstream
/// reach, so a raised main stem can back up into a flatter tributary.
/// Solve for the head that provides the Manning friction loss in this reach.
fn standard_step(base: f64, tail: f64, section: Section, length: f64, runoff: f64) -> f64 {
    let bottom = base - section.depth;
    let width = 2.0 * section.width;
    let discharge = runoff * 20e-9 * 38.0;
    let mut low = base.max(tail);
    let mut high = (base + 2.0).max(low);
    let loss = |stage: f64| {
        let depth = ((stage + tail) * 0.5 - bottom).max(section.depth * 0.5);
        let area = depth * width;
        let radius = area / (width + 2.0 * depth);
        length * (0.032 * discharge / (area * radius.powf(2.0 / 3.0))).powi(2)
    };
    if low >= tail + loss(low) {
        return low;
    }
    for _ in 0..9 {
        let head = (low + high) * 0.5;
        if head < tail + loss(head) {
            low = head;
        } else {
            high = head;
        }
    }
    high
}

impl Reach {
    /// Refine the receiver link inside its valley while keeping both node
    /// positions and tangents fixed, so the tributaries still meet the trunk.
    fn fit_valley(&mut self, mesh: &Mesh, height: &[f64], a: DVec3, b: DVec3, incoming: DVec3,
        outgoing: DVec3, erodibility: &[f64], seed: u32, branching: bool) {
        let length = a.distance(b);
        let axis = |t: f64| {
            let s = 1.0 - t;
            a.lerp(b, t * t * (3.0 - 2.0 * t))
                + length * (incoming * (t * s * s) - outgoing * (t * t * s))
        };
        let half_width = self.start.width.max(self.end.width);
        let steps = (length / (2.0 * half_width).clamp(8.0, 32.0)).ceil().clamp(12.0, 32.0) as usize;
        let search = if self.connected {
            (3.8 * half_width + 32.0).min(length * 0.28)
        } else { (2.4 * half_width + 16.0).min(length * 0.19) };
        let across = a.normalize().cross((b - a).normalize());
        let floor = |p: DVec3| mesh.sample(height, p.normalize());
        let offsets: Vec<f64> = (0..=steps).map(|i| {
            let t = i as f64 / steps as f64;
            let p = axis(t);
            let low = floor(p);
            let mut weighted = 0.0;
            let mut total = 0.0;
            let samples = if self.connected { 5 } else { 3 };
            for side in -samples..=samples {
                let fraction = f64::from(side) / f64::from(samples);
                let offset = fraction * search;
                let candidate = p + across * offset;
                let rise = floor(candidate) - low;
                let material = if branching {
                    2.4 * mesh.sample(erodibility, candidate.normalize())
                        + super::noise(seed, candidate / 220.0, 284_007)
                } else if self.connected {
                    2.4 * channel_refine::material(mesh, erodibility, candidate, seed)
                } else { 0.0 };
                let cost = (rise / 3.0 + 2.0 * fraction * fraction - material).clamp(-10.0, 10.0);
                let weight = (-cost).exp();
                weighted += offset * weight;
                total += weight;
            }
            weighted / total
        }).collect();
        self.curve = (0..=steps).map(|i| {
            let t = i as f64 / steps as f64;
            let smooth_offset = (offsets[i.saturating_sub(1)] + 2.0 * offsets[i] + offsets[(i + 1).min(steps)]) * 0.25;
            on_sphere(axis(t) + across * smooth_offset * (4.0 * t * (1.0 - t)).powi(2))
        }).collect();
        self.curvature = (0..=steps).map(|i| {
            if i == 0 || i == steps {
                return 0.0;
            }
            let before = self.curve[i] - self.curve[i - 1];
            let after = self.curve[i + 1] - self.curve[i];
            let across = self.curve[i].normalize().cross((before + after).normalize_or_zero());
            let turn = (after.normalize_or_zero() - before.normalize_or_zero()).dot(across);
            2.0 * turn / (before.length() + after.length()).max(1.0)
        }).collect();
    }

    fn new(
        a: DVec3,
        ca: DVec3,
        cb: DVec3,
        b: DVec3,
        start: Section,
        end: Section,
        levels: [f64; 2],
        valley: f64,
        confinement: f64,
    ) -> Self {
        let curve = (0..=CURVE_STEPS).map(|i| {
            let t = i as f64 / CURVE_STEPS as f64;
            let s = 1.0 - t;
            on_sphere(
                a * (s * s * s) + ca * (3.0 * s * s * t) + cb * (3.0 * s * t * t) + b * (t * t * t),
            )
        }).collect();
        Self {
            curve,
            curvature: Box::default(),
            start,
            end,
            levels,
            backwater: [0.0; 2],
            valley,
            confinement,
            gorge: 0.0,
            placer: [0.0; 2],
            feed: [0.0; 2],
            broad_plain: false,
            migration: None,
            connected: false,
        }
    }

    fn sample(&self, index: u32, p: DVec3, height: f64, footprint: f64, sample: &mut Sample) {
        let reach = self.start.width.max(self.end.width) + self.valley.max(40.0);
        let nearest = |path: &[DVec3]| {
            let mut result = (reach * reach, 0, 0.0);
            for (i, pair) in path.windows(2).enumerate() {
                let edge = pair[1] - pair[0];
                let u = ((p - pair[0]).dot(edge) / edge.length_squared()).clamp(0.0, 1.0);
                let d2 = p.distance_squared(pair[0] + edge * u);
                if d2 < result.0 {
                    result = (d2, i, u);
                }
            }
            result
        };
        let (distance2, i, u) = nearest(&self.curve);
        let (plain_distance, plain_t, plain_bend) = if let Some(migration) = &self.migration {
            // The same valley floor can hold several limbs of a meander.
            // Its longitudinal grade must not jump between their stages.
            let (d2, i, u) = nearest(&migration.valley_axis);
            (d2.sqrt(), (i as f64 + u) / 4.0, 0.0)
        } else {
            (distance2.sqrt(), (i as f64 + u) / (self.curve.len() - 1) as f64, 1.0)
        };
        if distance2 >= reach * reach && plain_distance >= reach {
            return;
        }
        let t = (i as f64 + u) / (self.curve.len() - 1) as f64;
        let width = self.start.width + (self.end.width - self.start.width) * t;
        // Arc position and signed offset feed the seeded splays and bars.
        let edge = self.curve[i + 1] - self.curve[i];
        let closest = self.curve[i] + edge * u;
        let lateral = (p - closest).dot(closest.normalize().cross(edge.normalize()));
        let arc = self.curve.windows(2).take(i).map(|w| w[0].distance(w[1])).sum::<f64>() + edge.length() * u;
        let length = self.curve.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>();
        // Positional seed: the reach index and its seeded start point.
        let start = self.curve[0];
        let key = index ^ (start.x as i64 as u32).rotate_left(7) ^ (start.z as i64 as u32).rotate_left(19)
            ^ (start.y as i64 as u32).rotate_left(27);
        let unconfined = (1.0 - self.confinement).clamp(0.0, 1.0) * (1.0 - self.gorge);
        let bend = if self.curvature.is_empty() {
            0.0
        } else {
            let edge = self.curve[i + 1] - self.curve[i];
            let closest = self.curve[i] + edge * u;
            let across = closest.normalize().cross(edge.normalize());
            let signed = (p - closest).dot(across);
            let inside = if self.connected { (signed / (0.20 * width)).clamp(-1.0, 1.0) }
                else { signed.signum() };
            let curvature = self.curvature[i] * (1.0 - u) + self.curvature[i + 1] * u;
            (curvature * inside * width * 6.0).clamp(-0.85, 0.85)
        };
        let distance = distance2.sqrt();
        let plain_width = self.start.width + (self.end.width - self.start.width) * plain_t;
        let bank = (plain_distance - plain_width).max(0.0);
        let level = self.levels[0] + (self.levels[1] - self.levels[0]) * t;
        let rise = self.backwater[0] + (self.backwater[1] - self.backwater[0]) * t;
        let head = level + rise;
        let flat_share = if self.broad_plain { 0.54 } else { 0.36 };
        let [swept, deposition] = self.migration.as_ref().map_or([0.0; 2], |migration| {
            let banks = &migration.banks;
            let at = plain_t * (banks.len() - 1) as f64;
            let j = (at as usize).min(banks.len() - 2);
            std::array::from_fn(|k| banks[j][k] + (at - j as f64) * (banks[j + 1][k] - banks[j][k]))
        });
        let plain_bend = plain_bend * bend;
        let plain = (self.valley * (flat_share - 0.21 * self.confinement)
            * (1.0 + 0.55 * plain_bend) * (1.0 - 0.55 * self.gorge))
            .max(swept);
        let mut weight = 1.0 - smooth(plain, self.valley, bank);
        let wall_t = ((bank - plain) / (self.valley - plain)).clamp(0.0, 1.0);
        if self.gorge > 0.0 {
            // A narrow bottom opens into a steep wall and a gentler shoulder.
            let steep_profile = 1.0 - wall_t.powi(3) * (10.0 + wall_t * (6.0 * wall_t - 15.0));
            weight += self.gorge * (steep_profile - weight);
        }
        sample.calm = sample.calm.min(1.0 - weight);
        let plain_grade = if self.broad_plain { 0.010 } else { 0.023 };
        let plain_level = self.levels[0] + (self.levels[1] - self.levels[0]) * plain_t;
        let ceiling = plain_level + 0.8 + bank * (plain_grade + self.confinement * 0.14) * (1.0 - 0.46 * plain_bend);
        let fill = deposition * smooth(0.0, width * 0.5, bank)
            * (1.0 - smooth(plain * 0.35, plain.max(0.1), bank));
        let cut = |ceiling: f64, weight: f64| (height - ceiling).max(0.0) * weight;
        let floor_moisture = if self.gorge > 0.0 {
            1.0 - self.gorge * smooth(2.0, 15.0, height - cut(ceiling, weight) - level)
        } else {
            1.0
        };
        sample.riparian = sample.riparian.max(weight * smooth(1.3, 9.0, width) * floor_moisture);
        if self.placer != [0.0; 2] && distance < width + plain * 0.6 {
            // Riffle margins and the lowest shelves retain a gravel lag.
            // The fast channel centre and the higher floodplain carry less.
            let margin = smooth(0.30, 0.74, distance / width)
                * (1.0 - smooth(0.0, plain * 0.6, bank));
            let grade = (self.placer[0] * (1.0 - t) + self.placer[1] * t) * margin * (1.0 + 0.30 * bend);
            if sample.placer.is_none_or(|(_, previous)| grade > previous) {
                sample.placer = Some((level + 1.5, grade));
            }
        }
        if distance >= width && self.gorge == 0.0 {
            // Overbank flow drops its coarsest load at the bank: a low, wide
            // natural levee that falls away to a wet backswamp.
            let reach_m = (width * 2.5).clamp(8.0, 60.0);
            let b = distance - width;
            let crest = 1.4 * (1.0 - self.confinement).clamp(0.0, 1.0) * smooth(3.0, 14.0, width);
            let levee = crest * smooth(0.0, 0.12 * reach_m, b) * (-3.0 * b / reach_m).exp()
                * (1.0 - smooth(1.2 * reach_m, 1.8 * reach_m, b)) * weight;
            // Crevasse splays: one or two seeded breaches where a flood cut the
            // levee and spread a thin, fingered sand lobe out across the plain.
            let lobe_m = (width * 7.0).clamp(40.0, 240.0).min(0.85 * (reach - width).max(0.0));
            let mut breach = 0.0_f64;
            if unconfined > 0.0 && length > 300.0 && lobe_m > 20.0 && b < lobe_m {
                for k in 0..1 + hash(key, 1) % 2 {
                    let centre = length * (0.18 + 0.64 * unit(hash(key, 10 + k)));
                    let side = if hash(key, 20 + k) & 1 == 0 { 1.0 } else { -1.0 };
                    if lateral * side <= 0.0 { continue; }
                    let x = b / lobe_m;
                    let half = width * 0.7 + lobe_m * 0.5 * x;
                    let across = (arc - centre) / half;
                    let fan = 1.0 - smooth(0.55, 1.0, across.abs());
                    if fan <= 0.0 { continue; }
                    let fingers = 0.72 + 0.28 * (across * 7.0 + unit(hash(key, 30 + k)) * 6.3).cos();
                    let profile = smooth(0.0, 0.12, x) * (1.0 - x * x);
                    let lift = crest.max(0.6) * 0.55 * fan * fingers * profile * weight;
                    sample.splay = sample.splay.max(lift);
                    sample.sand = sample.sand.max(fan * (1.0 - smooth(0.55, 0.95, x)) * weight);
                    breach = breach.max((1.0 - smooth(0.25, 0.6, across.abs())) * (1.0 - smooth(0.2, 0.45, x)));
                }
            }
            sample.levee = sample.levee.max(levee * (1.0 - breach));
            if plain > 1.8 * reach_m {
                let swamp = smooth(0.9 * reach_m, 1.6 * reach_m, b)
                    * (1.0 - smooth(plain * 0.75, plain, bank)) * weight;
                if sample.swamp.is_none_or(|(w, _)| swamp > w) && swamp > 0.0 {
                    sample.swamp = Some((swamp, plain_level));
                }
            }
        }
        if distance >= width && rise > 0.8 && weight > 0.5 && head > ceiling {
            sample.overbank = Some(sample.overbank.map_or(head, |water| water.max(head)));
        }
        if weight > 0.0
            && sample
                .floodplain
                .as_ref()
                .is_none_or(|f| cut(f.ceiling, f.weight).max(f.fill) < cut(ceiling, weight).max(fill))
        {
            let mut wall = (height - ceiling).max(0.0) * 6.0 * wall_t * (1.0 - wall_t) / (self.valley - plain);
            if self.gorge > 0.0 {
                wall *= 1.0 + self.gorge * (5.0 * wall_t * (1.0 - wall_t) - 1.0);
            }
            sample.floodplain = Some(Floodplain {
                ceiling,
                weight,
                wall,
                fill,
            });
        }
        if distance < width {
            let depth = self.start.depth + (self.end.depth - self.start.depth) * t;
            let cross = distance / width;
            let edge = if self.connected { 1.0 - smooth(0.86, 1.0, cross) } else { 1.0 };
            if edge > 0.0 {
                sample.channel_head = Some(sample.channel_head.map_or(head, |old| old.max(head)));
            }
            let resolved = edge * (1.0 - smooth((width * 2.0).max(8.0), (width * 8.0).max(32.0), footprint));
            // The same curve erodes the outer pool and builds the inner bar.
            // The bias falls to zero at the channel axis and bankfull edge.
            let bar = bend * cross * (1.0 - cross * cross);
            let bed = level - depth * (1.0 - cross.powi(4) - bar);
            if resolved > 0.0
                && sample.bed.is_none_or(|(bottom, previous_head, weight)| {
                    let current = cut(bottom, weight);
                    let candidate = cut(bed, resolved);
                    candidate > current || (candidate == current && head > previous_head)
                })
            {
                sample.bed = Some((bed, head, resolved));
                sample.bar_fill = depth * bar.max(0.0) * (1.0 - self.confinement);
                // Seeded lozenge bars in wide unconfined reaches, and a broad
                // middle-ground bar where a reach meets the sea.
                let mut lozenge = 0.0_f64;
                if unconfined > 0.0 && width >= 9.0 {
                    let spacing = width * 6.0;
                    let at = arc / spacing + unit(hash(key, 40));
                    let cell = at.floor() as i64 as u32;
                    if unit(hash(key ^ cell.wrapping_mul(0x9E37_79B9), 41)) < 0.55 {
                        let along = (at.fract() - 0.5) / 0.32;
                        let centre = (unit(hash(key ^ cell.wrapping_mul(0x85EB_CA6B), 42)) - 0.5) * 0.7;
                        let side = (lateral / width - centre) / 0.28;
                        lozenge = (1.0 - smooth(0.6, 1.0, (along * along + side * side).sqrt())) * unconfined;
                    }
                }
                let mouth = if self.levels[1] <= SEA + 0.5 {
                    smooth(0.80, 0.95, t) * (1.0 - smooth(0.25, 0.5, cross)) * smooth(10.0, 30.0, width)
                } else { 0.0 };
                sample.bar = lozenge.max(mouth) * resolved;
                sample.sand = sample.sand.max(sample.bar);
            }
        }
    }
}

struct Delta {
    origin: DVec3,
    forward: DVec3,
    across: DVec3,
    inland: f64,
    length: f64,
    width: f64,
}

impl Delta {
    fn candidate(
        mesh: &Mesh,
        height: &[f64],
        a: DVec3,
        b: DVec3,
        runoff: f64,
        load: f64,
    ) -> Option<Self> {
        // A substantial sediment load and a shallow shelf give the deposit
        // room to grow. A cliff or deep ocean stays an open river mouth.
        if runoff < 35.0e6 || load < 1.0e6 {
            return None;
        }
        let normal = b.normalize();
        let forward = (normal * (a - b).dot(normal) - (a - b)).normalize();
        let across = normal.cross(forward);
        let length = (load / 28.0).sqrt().clamp(210.0, 680.0);
        let width = length * 0.56;
        let offshore =
            |s: f64, d: f64| mesh.sample(height, (b + forward * s + across * d).normalize());
        if [(-0.32, 0.25), (0.0, 0.65), (0.32, 0.25)]
            .into_iter()
            .any(|(d, s)| !(SEA - 28.0..=SEA + 1.0).contains(&offshore(length * s, width * d)))
        {
            return None;
        }
        Some(Self {
            origin: b,
            forward,
            across,
            inland: a.distance(b) * 0.6,
            length,
            width,
        })
    }

    fn sample(&self, p: DVec3) -> (f64, f64) {
        let d = p - self.origin;
        let along = d.dot(self.forward);
        let x = along
            / if along < 0.0 {
                self.inland
            } else {
                self.length
            };
        let y = d.dot(self.across) / self.width;
        let spread = 0.57 + 0.56 * smooth(-0.55, 0.48, x);
        let radius = x.hypot((y - 0.08 * x) / spread);
        let lobe = 1.0 - smooth(0.45, 1.0, radius);
        // The submerged apron tapers into the shelf; the flatter inner
        // lobe can rise just above high water between the distributaries.
        let surface = SEA - 3.5 + 5.9 * smooth(0.13, 0.93, lobe);
        (lobe, surface)
    }
}

/// Positional integer hash for seeded reach features (no global state).
fn hash(key: u32, salt: u32) -> u32 {
    let mut n = key.wrapping_mul(0x9E37_79B1) ^ salt.wrapping_mul(0x85EB_CA77);
    n ^= n >> 15;
    n = n.wrapping_mul(0x2C1B_3C6D);
    n ^= n >> 12;
    n = n.wrapping_mul(0x297A_2D39);
    n ^ (n >> 15)
}

fn unit(n: u32) -> f64 { f64::from(n) / f64::from(u32::MAX) }
