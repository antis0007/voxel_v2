//! Resonant crystal gates: a closed crystal ring around a rectangular opening
//! in a vertical plane. Attuning two matching gates links both faces of each.
use super::{Frame, Network, Portal, Realm};
use crate::world::{BlockId, World, raycast_block};
use glam::{DQuat, DVec2, DVec3, IVec3, Vec3};
use serde::{Deserialize, Serialize};

pub const FRAME_BLOCK: BlockId = BlockId::Crystal;
/// Opening cells per side; bounds the flood fill and the ring check.
const MAX_CELLS: i32 = 64;
const REACH_M: f32 = 5.0;

/// The open cells inside a ring, inclusive. `axis` is the plane normal: 0 = X, 2 = Z.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aperture {
    pub min: IVec3,
    pub max: IVec3,
    pub axis: usize,
}

impl Aperture {
    pub(crate) fn across(self) -> usize {
        2 - self.axis
    }
    fn layer(self, depth: i32) -> Self {
        let mut s = self;
        s.min[self.axis] = depth;
        s.max[self.axis] = depth;
        s
    }
    fn cells(self) -> impl Iterator<Item = IVec3> {
        let (u, a) = (self.across(), self.axis);
        (self.min[a]..=self.max[a]).flat_map(move |d| {
            (self.min.y..=self.max.y).flat_map(move |y| {
                (self.min[u]..=self.max[u]).map(move |w| {
                    let mut p = IVec3::ZERO;
                    p[u] = w;
                    p[a] = d;
                    p.y = y;
                    p
                })
            })
        })
    }
    /// Ring cells sharing an edge with the opening, every layer; corners are decorative.
    pub(crate) fn ring(self) -> Vec<IVec3> {
        let (u, a) = (self.across(), self.axis);
        let mut out = Vec::new();
        for d in self.min[a]..=self.max[a] {
            let mut at = |w: i32, y: i32| {
                let mut p = self.min;
                p[u] = w;
                p[a] = d;
                p.y = y;
                out.push(p);
            };
            for w in self.min[u]..=self.max[u] {
                at(w, self.min.y - 1);
                at(w, self.max.y + 1);
            }
            for y in self.min.y..=self.max.y {
                at(self.min[u] - 1, y);
                at(self.max[u] + 1, y);
            }
        }
        out
    }
    /// Grow through neighbouring layers with the same opening and ring.
    fn thicken(mut self, world: &World) -> Self {
        let a = self.axis;
        for _ in 0..MAX_CELLS {
            if !self.layer(self.min[a] - 1).intact(world) { break; }
            self.min[a] -= 1;
        }
        for _ in 0..MAX_CELLS {
            if !self.layer(self.max[a] + 1).intact(world) { break; }
            self.max[a] += 1;
        }
        self
    }
    pub fn intact(self, world: &World) -> bool {
        self.intact_with(world, FRAME_BLOCK)
    }
    pub(crate) fn intact_with(self, world: &World, material: BlockId) -> bool {
        self.ring().into_iter().all(|p| world.get(p) == material)
            && self.cells().all(|p| !world.solid(p))
    }
    pub fn contains_ring(self, cell: IVec3) -> bool {
        self.ring().contains(&cell)
    }
    /// Front (+Z local) faces `positive` along the normal axis.
    pub fn frame(self, voxel_m: f64, positive: bool) -> Frame {
        let u = self.across();
        let lo = self.min.as_dvec3();
        let hi = self.max.as_dvec3() + DVec3::ONE;
        let center = (lo + hi) * 0.5 * voxel_m;
        let angle = match (self.axis, positive) {
            (2, true) => 0.0,
            (2, false) => std::f64::consts::PI,
            (_, true) => std::f64::consts::FRAC_PI_2,
            (_, false) => -std::f64::consts::FRAC_PI_2,
        };
        Frame {
            center,
            rotation: DQuat::from_rotation_y(angle),
            half_size: DVec2::new(hi[u] - lo[u], hi.y - lo.y) * 0.5 * voxel_m,
        }
    }
}

/// Opening bounded by the ring that contains `crystal`, preferring the plane
/// that faces `look`. Open cells must form an exact rectangle.
pub fn find(world: &World, crystal: IVec3, look: Vec3) -> Option<Aperture> {
    find_with(world, crystal, look, FRAME_BLOCK)
}

pub(crate) fn find_with(world: &World, crystal: IVec3, look: Vec3, material: BlockId) -> Option<Aperture> {
    let axes = if look.x.abs() > look.z.abs() { [0, 2] } else { [2, 0] };
    for axis in axes {
        let u = 2 - axis;
        let mut step = IVec3::ZERO;
        step[u] = 1;
        for dir in [step, -step, IVec3::Y, -IVec3::Y] {
            // Rings are several cells thick at fine scales: walk through them.
            let mut inner = crystal;
            let mut seed = crystal + dir;
            for _ in 0..MAX_CELLS {
                if world.get(seed) != material {
                    break;
                }
                inner = seed;
                seed += dir;
            }
            if let Some(a) = flood(world, seed, axis).filter(|a| a.contains_ring(inner) && a.intact_with(world, material)) {
                return Some(if material == FRAME_BLOCK { a.thicken(world) } else { a });
            }
        }
    }
    None
}

fn flood(world: &World, seed: IVec3, axis: usize) -> Option<Aperture> {
    if world.solid(seed) {
        return None;
    }
    let u = 2 - axis;
    let mut dirs = [IVec3::Y, -IVec3::Y, IVec3::ZERO, IVec3::ZERO];
    dirs[2][u] = 1;
    dirs[3][u] = -1;
    let (mut min, mut max) = (seed, seed);
    let mut seen = std::collections::HashSet::from([seed]);
    let mut queue = vec![seed];
    while let Some(p) = queue.pop() {
        for d in dirs {
            let q = p + d;
            if world.solid(q) || !seen.insert(q) {
                continue;
            }
            min = min.min(q);
            max = max.max(q);
            if max[u] - min[u] >= MAX_CELLS || max.y - min.y >= MAX_CELLS {
                return None;
            }
            queue.push(q);
        }
    }
    let area = (max[u] - min[u] + 1) * (max.y - min.y + 1);
    (seen.len() as i32 == area).then_some(Aperture { min, max, axis })
}

/// Two gates and the side each faced when attuned. Front faces link to front faces.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Link {
    pub a: Aperture,
    pub a_positive: bool,
    pub b: Aperture,
    pub b_positive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Attuned,
    Linked,
    Closed,
    Cancelled,
    /// The two openings differ in size; portals keep body scale.
    Mismatched,
}

#[derive(Default, Clone, Debug)]
pub struct Gates {
    pub links: Vec<Link>,
    pub hell: Vec<crate::world::hell_gate::built::Threshold>,
    pub(crate) circles: crate::world::adventure::fieldcraft::CircleRoute,
    pending: Option<(Aperture, bool)>,
    network: Network,
    /// Bumped whenever the network changes; renderers key caches on it.
    pub revision: u64,
    check: usize,
}

pub fn realm() -> Realm {
    Realm::Planet("local".into())
}

impl Gates {
    pub fn network(&self) -> &Network {
        &self.network
    }
    pub fn pending(&self) -> Option<Aperture> {
        self.pending.map(|(a, _)| a)
    }
    pub fn link_of(&self, a: Aperture) -> Option<usize> {
        self.links.iter().position(|l| l.a == a || l.b == a)
    }
    /// Rebuild portals from links. A link the network rejects is dropped.
    pub fn rebuild(&mut self, voxel_m: f32) {
        let m = f64::from(voxel_m);
        let mut network = Network::default();
        let mut id = 1;
        self.links.retain(|l| {
            let mut next = network.clone();
            let ok = [(l.a_positive, l.b_positive), (!l.a_positive, !l.b_positive)].into_iter().all(|(pa, pb)| {
                let portal = Portal {
                    id,
                    source: realm(),
                    destination: realm(),
                    entrance: l.a.frame(m, pa),
                    exit: l.b.frame(m, pb),
                };
                id += 2;
                next.link(portal, id - 1).is_ok()
            });
            if ok {
                network = next;
            }
            ok
        });
        self.network = network;
        // Unique across worlds, so a renderer cache never outlives a world switch.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        self.revision = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn attune(&mut self, a: Aperture, positive: bool, voxel_m: f32) -> Outcome {
        if let Some(i) = self.link_of(a) {
            self.links.remove(i);
            self.rebuild(voxel_m);
            return Outcome::Closed;
        }
        match self.pending.take() {
            Some((p, _)) if p == a => Outcome::Cancelled,
            Some((p, p_positive)) => {
                let size = |x: Aperture| (x.max - x.min).abs();
                if size(p)[p.across()] != size(a)[a.across()] || size(p).y != size(a).y {
                    self.pending = Some((a, positive));
                    return Outcome::Mismatched;
                }
                self.links.push(Link { a: p, a_positive: p_positive, b: a, b_positive: positive });
                self.rebuild(voxel_m);
                Outcome::Linked
            }
            None => {
                self.pending = Some((a, positive));
                Outcome::Attuned
            }
        }
    }
    /// One link per call; broken rings close their portals.
    pub fn audit(&mut self, world: &World) -> bool {
        if let Some((p, _)) = self.pending
            && !p.intact(world)
        {
            self.pending = None;
        }
        if self.links.is_empty() {
            return false;
        }
        self.check = (self.check + 1) % self.links.len();
        let l = self.links[self.check];
        if l.a.intact(world) && l.b.intact(world) {
            return false;
        }
        self.links.remove(self.check);
        self.rebuild(world.voxel_scale.meters());
        true
    }
}

/// Crystal ring cell under the player's aim, with the gate it closes.
pub fn aimed(world: &World, player: &crate::player::Player) -> Option<(Aperture, bool)> {
    let scale = world.voxel_scale.meters();
    let eye = player.eye(world.voxel_scale);
    let look = player.forward_dir();
    let (hit, _) = raycast_block(world, eye, look, REACH_M / scale)?;
    if world.get(hit) != FRAME_BLOCK {
        return None;
    }
    let a = find(world, hit, look)?;
    let center = (a.min.as_vec3() + a.max.as_vec3() + Vec3::ONE) * 0.5;
    Some((a, (eye - center)[a.axis] >= 0.0))
}

/// Prompt for the HUD while a gate ring is aimed at.
pub fn focus(world: &World, player: &crate::player::Player) -> Option<crate::world::interaction::Prompt> {
    let (a, _) = aimed(world, player)?;
    let gates = &world.portals;
    Some(crate::world::interaction::Prompt {
        title: if gates.link_of(a).is_some() {
            "Crystal gate · linked"
        } else if gates.pending() == Some(a) {
            "Crystal gate · attuned"
        } else {
            "Crystal gate"
        },
        action: if gates.link_of(a).is_some() {
            "F  Close"
        } else if gates.pending() == Some(a) {
            "F  Release"
        } else if gates.pending().is_some() {
            "F  Link"
        } else {
            "F  Attune"
        },
    })
}

pub fn interact(world: &mut World, player: &crate::player::Player) -> Option<Outcome> {
    let (a, positive) = aimed(world, player)?;
    let scale = world.voxel_scale.meters();
    let outcome = world.portals.attune(a, positive, scale);
    match outcome {
        Outcome::Attuned => {
            world.goals.note(crate::world::goals::Event::GateAttuned);
            world.expedition.say("Gate Attuned · Interact with a matching frame");
        }
        Outcome::Linked => {
            crate::world::progression::thresholds::Milestone::LinkedCrystal.record(&mut world.progression);
            world.goals.note(crate::world::goals::Event::GateAttuned);
            world.goals.note(crate::world::goals::Event::GatesLinked);
            world.expedition.say("Gates Linked · Walk through");
        }
        Outcome::Mismatched => world.expedition.say("Gate Openings Must Match"),
        _ => {}
    }
    let (cue, gain, pitch) = match outcome {
        Outcome::Attuned => (crate::audio::Cue::Ward, 0.45, 1.3),
        Outcome::Linked => (crate::audio::Cue::Rift, 0.6, 0.8),
        Outcome::Closed | Outcome::Cancelled => (crate::audio::Cue::Echo, 0.35, 0.7),
        Outcome::Mismatched => (crate::audio::Cue::Warning, 0.4, 1.0),
    };
    let center = (a.min.as_vec3() + a.max.as_vec3() + Vec3::ONE) * 0.5 * scale;
    if world.adventure.sounds.len() < 64 {
        world.adventure.sounds.push(crate::audio::Event { cue, pos: Some(center), gain, pitch });
    }
    Some(outcome)
}
