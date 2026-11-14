//! Late eruptions over the eroded shield. The summit, lava fields and the
//! materials covering them come from the same body-space geometry.
use super::SEA;
use super::tectonics::smooth;
use super::volcanism::{Bins, bins, cell};
use crate::BlockId;
use crate::planet::noise_at;
use crate::volcano::{Shaped, Vent};
use engine_core::cube_sphere::atlas::RADIUS_M;
use glam::{DVec2, DVec3};

mod collapse;

const FLOW_PITCH_M: f64 = 220.0;
const FLOW_STEP_M: f64 = 48.0;
const SUMMIT_REACH_M: f64 = 410.0;

struct Summit {
    centre: DVec3,
    along: DVec3,
    across: DVec3,
    elevation: f64,
    seed: u32,
    dissected: bool,
    collapse: Option<collapse::Collapse>,
}

impl Summit {
    fn reach(&self) -> f64 {
        if self.collapse.is_some() { collapse::REACH_M } else { SUMMIT_REACH_M }
    }

    fn shape(&self, body: DVec3, out: &mut Shaped) {
        let offset = body - self.centre;
        let distance = offset.length();
        if distance > self.reach() {
            return;
        }
        let p = DVec2::new(offset.dot(self.along), offset.dot(self.across));
        if let Some(collapse) = &self.collapse {
            collapse.shape(p, body, self.elevation, out);
            return;
        }
        let texture = noise_at(body / 76.0, self.seed);
        let mut wall = (p / DVec2::new(228.0, 166.0)).length() + (texture - 0.5) * 0.18;
        let fracture = if self.dissected {
            let broad = noise_at(body / 115.0, self.seed.wrapping_add(31));
            let notch = noise_at(body / 42.0, self.seed.wrapping_add(97));
            (broad - 0.5) * 0.44 + (notch - 0.5) * 0.12
        } else { 0.0 };
        wall += fracture;
        if wall > 1.44 {
            return;
        }

        // A broad collapse floor with a lower pit offset toward the rift.
        // The broken outer bench and the inner pit give two distinct scarps.
        let mut inner = ((p - DVec2::new(-23.0, 14.0)) / DVec2::new(112.0, 79.0)).length()
            + (texture - 0.5) * 0.12;
        if self.dissected {
            // An overlapping drop block follows the rift. Blend the shared
            // floor so the two collapse scarps meet without a step.
            let block = ((p - DVec2::new(52.0, -17.0)) / DVec2::new(79.0, 49.0)).length();
            let merge = (0.19 - (inner - block).abs()).max(0.0);
            inner = inner.min(block) - merge * merge / 0.76 + fracture * 0.6;
        }
        let tilt = if self.dissected { (p.x * 0.037 - p.y * 0.023).clamp(-7.0, 7.0) } else { 0.0 };
        let terrace = self.elevation - 34.0 + texture * 4.0 + tilt;
        let floor = terrace - 22.0 * (1.0 - smooth(0.70, 1.06, inner));
        let caldera = 1.0 - smooth(0.80, 1.06, wall);
        out.active_caldera |= wall < 1.14;
        let rim = smooth(0.91, 1.04, wall) * (1.0 - smooth(1.05, 1.33, wall));
        let breach = smooth(0.36, 0.92, p.normalize_or_zero().dot(DVec2::new(-0.8, 0.6)));
        out.metres +=
            (floor - out.metres) * caldera + rim * (8.0 + 13.0 * texture) * (1.0 - 0.86 * breach);
        out.heat = out.heat.max((1.0 - smooth(0.25, 1.44, wall)) * 0.9);

        let head = self.elevation - 43.0;
        if head > SEA && inner < 1.0 && out.metres < head {
            out.lake = Some((head, BlockId::Lava));
        } else if wall < 1.25 {
            let scarp = (0.79..1.10).contains(&wall) || (0.70..1.06).contains(&inner);
            let exposed = if self.dissected { scarp && texture + fracture > 0.36 || texture > 0.68 }
                else { scarp || texture > 0.53 };
            out.crust = Some(if exposed {
                BlockId::Basalt
            } else {
                BlockId::Ash
            });
        }
    }
}

struct Segment {
    start: DVec3,
    edge: DVec3,
    width: f64,
    gain: f64,
}

impl Segment {
    fn reach(&self) -> (DVec3, f64) {
        (
            (self.start + self.edge * 0.5).normalize(),
            self.edge.length() * 0.5 + self.width * 1.36,
        )
    }

    fn distance(&self, body: DVec3) -> f64 {
        let relative = body - self.start;
        let t = (relative.dot(self.edge) / self.edge.length_squared()).clamp(0.0, 1.0);
        (relative - self.edge * t).length() / self.width
    }
}

pub(super) struct Landforms {
    summits: Vec<Summit>,
    flows: Vec<Segment>,
    flow_bins: Bins,
    broad_flows: bool,
}

impl Landforms {
    pub fn reserves_summit(&self, dir: DVec3, margin: f64) -> bool {
        let body = dir * RADIUS_M;
        self.summits
            .iter()
            .any(|s| (body - s.centre).length_squared() < (s.reach() + margin).powi(2))
    }

    pub fn bake(
        rifts: impl Iterator<Item = (Vent, DVec3, u32)>,
        ground: impl Fn(DVec3) -> f64,
        dissected: bool,
        fault_blocks: bool,
        deep_collapse: bool,
    ) -> Self {
        let mut summits = Vec::new();
        let mut flows = Vec::new();
        for (vent, along, seed) in rifts {
            let centre = vent.dir * RADIUS_M;
            let across = vent.dir.cross(along);
            let elevation = ground(vent.dir);
            summits.push(Summit {
                centre,
                along,
                across,
                elevation,
                seed,
                dissected,
                collapse: fault_blocks.then(|| collapse::Collapse::new(seed, deep_collapse && elevation > SEA + 114.0)),
            });
            let side = if seed & 1 == 0 { 1.0 } else { -1.0 };
            let outlets = if fault_blocks {
                [(-690.0, 160.0), (333.0, -495.0 * side), (681.0, 192.0 * side)]
            } else {
                [(-176.0, 132.0), (94.0, -162.0 * side), (274.0, 146.0 * side)]
            };
            for (x, y) in outlets {
                let rift = along * x + across * y;
                let start = (centre + rift).normalize();
                route_flow(start, rift.normalize(), &ground, &mut flows, fault_blocks);
            }
        }
        let flow_bins = bins(flows.iter().map(Segment::reach), FLOW_PITCH_M);
        Self {
            summits,
            flows,
            flow_bins,
            broad_flows: fault_blocks,
        }
    }

    pub fn shape(&self, dir: DVec3, out: &mut Shaped) {
        let body = dir * RADIUS_M;
        if let Some(nearby) = self.flow_bins.get(&cell(dir, FLOW_PITCH_M)) {
            if self.broad_flows {
                let rough = noise_at(body / 123.0, 12_681);
                let clinker = noise_at(body / 31.0, 12_701);
                let deposit = nearby.iter().map(|&i| {
                    let segment = &self.flows[i];
                    let edge = segment.distance(body) + (rough - 0.5) * 0.34;
                    segment.gain * (1.0 - smooth(0.18, 1.06, edge))
                }).fold(0.0, f64::max);
                out.metres += deposit * (3.5 + 2.2 * rough + 1.4 * clinker);
                if deposit > 0.48 {
                    out.crust = Some(BlockId::Basalt);
                }
            } else {
                let distance = nearby.iter().map(|&i| self.flows[i].distance(body))
                    .fold(f64::INFINITY, f64::min);
                if distance < 1.28 {
                    let lobes = noise_at(body / 84.0, 12_681);
                    let clinker = noise_at(body / 22.0, 12_701);
                    let thickness = 1.0 - smooth(0.57, 1.12, distance + (lobes - 0.5) * 0.36);
                    out.metres += thickness * (4.5 + 5.0 * lobes + 2.4 * clinker);
                    if thickness > 0.16 {
                        out.crust = Some(if thickness < 0.38 && clinker > 0.61 {
                            BlockId::Ash
                        } else {
                            BlockId::Basalt
                        });
                    }
                }
            }
        }
        for summit in &self.summits {
            summit.shape(body, out);
        }
    }
}

fn route_flow(
    mut at: DVec3,
    mut heading: DVec3,
    ground: &impl Fn(DVec3) -> f64,
    flows: &mut Vec<Segment>,
    broad: bool,
) {
    let start = flows.len();
    let mut height = ground(at);
    for step in 0..45 {
        let tangent = (heading - at * at.dot(heading)).normalize();
        let cross = at.cross(tangent);
        let mut next = None;
        let mut best = 0.0;
        for angle in [0.0_f64, 0.35, -0.35, 0.72, -0.72, 1.18, -1.18] {
            let course = tangent * angle.cos() + cross * angle.sin();
            let candidate = (at + course * (FLOW_STEP_M / RADIUS_M)).normalize();
            let elevation = ground(candidate);
            let score = height - elevation - 0.45 - angle.abs() * 1.1;
            if score > best {
                best = score;
                next = Some((candidate, elevation, course));
            }
        }
        let Some((candidate, elevation, course)) = next else {
            break;
        };
        let spreading = smooth(0.0, 20.0, f64::from(step));
        let shore = 1.0 - smooth(SEA, SEA + 45.0, height);
        let width = if broad { 56.0 + spreading * 123.0 + 30.0 * shore }
            else { 29.0 + spreading * 39.0 + 29.0 * shore };
        flows.push(Segment {
            start: at * RADIUS_M,
            edge: (candidate - at) * RADIUS_M,
            width,
            gain: 1.0,
        });
        if elevation < SEA - 3.0 {
            break;
        }
        at = candidate;
        height = elevation;
        heading = course;
    }
    if broad {
        let length = flows.len() - start;
        for (i, segment) in flows[start..].iter_mut().enumerate() {
            let from_source = i as f64 + 0.5;
            let to_toe = length as f64 - from_source;
            segment.gain = smooth(0.0, 4.0, from_source) * smooth(0.0, 5.0, to_toe);
        }
    }
}
