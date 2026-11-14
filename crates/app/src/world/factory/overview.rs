//! Factorio-style factory health: every assembly sorted by what it is doing,
//! and which problem is holding the factory back. The question this answers is
//! "what limits my output?", which is the first thing a Factorio player checks.
//!
//! Classification reads only the status text the machines already publish, so
//! it cannot drift from what the look-at panel shows.
use super::{Factory, Kind, Node};

/// Read-only diagnosis of the inspected or nearest electric furnace. Uses the
/// same directed pipe route and receiver admission as live item transport.
pub fn workshop_iron(world: &crate::world::World, player: glam::Vec3) -> &'static str {
    use super::{DIRS, routes, step};
    const IRON: usize = 8;
    let f = &world.factory;
    let furnace = f
        .focus
        .filter(|p| {
            super::near(world, *p, player)
                && f.nodes.get(p).is_some_and(|n| n.kind == Kind::Furnace)
        })
        .and_then(|p| f.nodes.get_key_value(&p))
        .or_else(|| {
            f.nodes
                .iter()
                .filter(|(_, n)| n.kind == Kind::Furnace)
                .min_by(|(a, _), (b, _)| {
                    a.as_vec3()
                        .distance_squared(player)
                        .total_cmp(&b.as_vec3().distance_squared(player))
                        .then_with(|| (a.x, a.y, a.z).cmp(&(b.x, b.y, b.z)))
                })
        });
    let Some((&pos, n)) = furnace else {
        return "Place an Electric Furnace";
    };
    if f.packets.iter().any(|p| {
        p.src == pos
            && p.item == IRON
            && f.nodes.get(&p.dst).is_some_and(|n| n.kind == Kind::Storage)
    }) {
        return "Iron in Transit";
    }
    if !n.enabled {
        return "Furnace Paused";
    }
    if n.output[IRON] == 0 {
        if matches!(n.status, "Held by controller" | "Exhaust blocked") {
            return n.status;
        }
        if n.process().is_some_and(|p| p.output.0 != IRON) {
            return "Furnace Processing Another Material";
        }
        if n.mode == 1 && n.input[2] < 64 {
            return "Furnace: Add Iron Ore";
        }
        if n.mode != 1 && n.input[11] < 64 {
            return "Furnace: Add Iron Concentrate or Switch to Raw Ore";
        }
        if n.status == "Needs cable / power" {
            let Some(net) = f.power_net.get(&pos) else {
                return "Furnace: Connect Power Cable";
            };
            let mut generators = f
                .nodes
                .iter()
                .filter(|(p, n)| n.kind.generator() && f.power_net.get(p) == Some(net))
                .peekable();
            if generators.peek().is_none() && f.grid(pos).is_some_and(|grid| grid.batteries == 0) {
                return "Cable: Connect a Power Source";
            }
            if generators.peek().is_some()
                && generators.all(|(_, n)| n.enabled && n.status == "Needs fuel")
            {
                return "Connected Generator: Add Fuel";
            }
            return "Furnace: Insufficient Power";
        }
    }
    let size = step(world);
    let mut attached = false;
    let mut output = false;
    let mut storage = false;
    let mut path = false;
    let mut accepts = false;
    let inbound = routes::Inbound::new(&f.packets);
    let mut paths = routes::Routes::default();
    for (index, net) in f
        .items
        .iter()
        .enumerate()
        .filter(|(_, net)| net.ports.contains(&pos))
    {
        attached = true;
        if !DIRS.iter().any(|d| {
            let pipe = pos + *d * size;
            net.pipes.contains(&pipe)
                && !net.closed.contains(&(pipe, pos))
                && !net.no_extract.contains(&(pipe, pos))
        }) {
            continue;
        }
        output = true;
        for &dst in &net.ports {
            let receiver = &f.nodes[&dst];
            if receiver.kind != Kind::Storage {
                continue;
            }
            storage = true;
            if paths.path(index, net, size, pos, dst, None).is_none() {
                continue;
            }
            path = true;
            if !receiver.enabled || !f.takes_unless(dst, IRON, || inbound.other_than(dst, IRON)) {
                continue;
            }
            accepts = true;
            if f.open_for(dst, IRON, &inbound) {
                return if n.output[IRON] > 0 {
                    "Iron Ready for Transfer"
                } else {
                    "Iron Line Ready"
                };
            }
        }
    }
    if !attached {
        if DIRS.iter().any(|d| {
            f.items
                .iter()
                .any(|net| net.pipes.contains(&(pos + *d * size)))
        }) {
            "Furnace: Open Pipe Output Face"
        } else {
            "Furnace: Attach Item Pipe"
        }
    } else if !output {
        "Furnace: Open Pipe Output Face"
    } else if !storage {
        "Connect Storage to Item Pipe"
    } else if !path {
        "Check Pipe Connections and Storage Input Face"
    } else if !accepts {
        "Storage: Enable and Accept Iron"
    } else {
        "Storage: Free Space for Iron"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Running = 0,
    Starved = 1,
    Blocked = 2,
    Unpowered = 3,
    Idle = 4,
    Paused = 5,
    Unbuilt = 6,
    Unconfigured = 7,
}

impl Health {
    pub const ALL: [Health; 8] = [
        Health::Running,
        Health::Starved,
        Health::Blocked,
        Health::Unpowered,
        Health::Idle,
        Health::Paused,
        Health::Unbuilt,
        Health::Unconfigured,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Starved => "starved",
            Self::Blocked => "blocked",
            Self::Unpowered => "unpowered",
            Self::Idle => "idle",
            Self::Paused => "paused",
            Self::Unbuilt => "unbuilt",
            Self::Unconfigured => "unconfigured",
        }
    }

    /// The cause as a state, for the production overview.
    pub fn fix(self) -> &'static str {
        match self {
            Self::Running => "Working",
            Self::Starved => "Missing Input or Fuel",
            Self::Blocked => "Output Full",
            Self::Unpowered => "No Power",
            Self::Idle => "Idle",
            Self::Paused => "Paused",
            Self::Unbuilt => "Structure Incomplete",
            Self::Unconfigured => "No Recipe",
        }
    }

    /// Problems rank as limits. Idle and paused are not limits.
    pub fn is_problem(self) -> bool {
        matches!(
            self,
            Self::Starved | Self::Blocked | Self::Unpowered | Self::Unbuilt | Self::Unconfigured
        )
    }
}

/// Health of one assembly, from its enabled flag and published status.
pub fn classify(n: &Node) -> Health {
    if !n.enabled && !n.ruptured {
        return Health::Paused;
    }
    match n.status {
        "Processing" | "Generating" | "Coking" | "Smelting" | "Charging" | "Discharging"
        | "Balancing" | "Heating" | "Heating · Kiln Lining" | "Shaft complete" | "Assembly in shaft" | "Filtering"
        | "Venting" => Health::Running,
        "Output blocked"
        | "Tank full"
        | "Exhaust blocked"
        | "Waste full"
        | "Leaking"
        | "Contaminated intake" => Health::Blocked,
        "Needs cable / power" | "Needs water / power" => Health::Unpowered,
        "Needs input" | "Needs coal" | "Needs coke" | "Needs fuel" | "Needs charge"
        | "Needs water" | "Intake dry" | "Needs filter media" | "Needs fuel gas" => Health::Starved,
        "Incomplete shell" | "Matrix incomplete" | "Waiting for world data" => Health::Unbuilt,
        "No recipe" => Health::Unconfigured,
        _ => Health::Idle,
    }
}

/// Counts for one assembly kind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub kind: Option<Kind>,
    pub counts: [u32; 8],
}

impl Row {
    /// The problem that affects the most assemblies of this kind.
    pub fn main_issue(&self) -> Option<(Health, u32)> {
        worst(&self.counts)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub counts: [u32; 8],
    /// Kinds present in the factory, most troubled first.
    pub rows: Vec<Row>,
}

impl Summary {
    pub fn total(&self) -> u32 {
        self.counts.iter().sum()
    }
    /// The factory-wide limit: the problem held by the most assemblies.
    pub fn bottleneck(&self) -> Option<(Health, u32)> {
        worst(&self.counts)
    }
}

fn worst(counts: &[u32; 8]) -> Option<(Health, u32)> {
    Health::ALL
        .into_iter()
        .filter(|h| h.is_problem())
        .map(|h| (h, counts[h as usize]))
        .filter(|(_, n)| *n > 0)
        .max_by_key(|(h, n)| (*n, std::cmp::Reverse(*h as u8)))
}

pub fn summarize(f: &Factory) -> Summary {
    summarize_nodes(f.nodes.values())
}

fn summarize_nodes<'a>(nodes: impl IntoIterator<Item = &'a Node>) -> Summary {
    let mut per_kind = vec![[0u32; 8]; Kind::COUNT];
    let mut summary = Summary::default();
    for n in nodes {
        if n.kind.conduit() || n.kind.structural() {
            continue;
        }
        let h = classify(n) as usize;
        summary.counts[h] += 1;
        per_kind[n.kind as usize][h] += 1;
    }
    summary.rows = Kind::ALL
        .into_iter()
        .filter(|k| per_kind[*k as usize].iter().any(|&c| c > 0))
        .map(|k| Row {
            kind: Some(k),
            counts: per_kind[k as usize],
        })
        .collect();
    // Most problems first; ties keep the stable kind order.
    summary.rows.sort_by_key(|r| {
        let problems: u32 = Health::ALL
            .into_iter()
            .filter(|h| h.is_problem())
            .map(|h| r.counts[h as usize])
            .sum();
        std::cmp::Reverse(problems)
    });
    summary
}

#[derive(Clone)]
pub struct Outpost {
    /// Keeps its value while machines are added, removed or the group splits.
    pub id: u32,
    pub anchor: glam::IVec3,
    pub distance_m: f32,
    pub machines: usize,
    pub health: Summary,
    pub stored: [u64; super::RESOURCE_COUNT],
    /// This base's production over the last minute and hour, and its power graph.
    pub minute: (super::stats::Sample, f32),
    pub hour: (super::stats::Sample, f32),
    pub power: Vec<(u64, u64)>,
}

/// Machines closer than this (metres) belong to the same base.
pub const BASE_LINK_M: f32 = 24.0;

fn base_member(n: &Node) -> bool {
    !n.kind.conduit() && !n.kind.structural()
}

/// Machine cells that form bases, sorted. Every caller of `Bases::refresh`
/// passes this list so base ids agree between the factory tick and the UI.
pub fn base_cells(nodes: &crate::fast_hash::FastMap<glam::IVec3, Node>) -> Vec<glam::IVec3> {
    let mut cells: Vec<_> = nodes.iter().filter(|(_, n)| base_member(n)).map(|(&p, _)| p).collect();
    cells.sort_unstable_by_key(|p| (p.x, p.y, p.z));
    cells
}

/// Nearby machines form an outpost independently of what terrain is loaded.
/// Long pipe and power links do not merge distant camps in the dashboard.
pub fn outposts(world: &crate::world::World, player: glam::Vec3, bases: &mut super::bases::Bases) -> Vec<Outpost> {
    let mut nodes: Vec<_> = world.factory.nodes.iter()
        .filter(|(_, n)| base_member(n))
        .map(|(&p, n)| (p, n)).collect();
    nodes.sort_unstable_by_key(|(p, _)| (p.x, p.y, p.z));
    let scale = world.voxel_scale.meters();
    let positions: Vec<_> = nodes.iter().map(|(p, _)| *p).collect();
    let groups = bases.refresh(&positions, BASE_LINK_M * super::step(world) as f32);
    let mut result = Vec::with_capacity(groups.len());
    for (base, group) in groups {
        let mut stored = [0u64; super::RESOURCE_COUNT];
        let mut distance_sq = f32::INFINITY;
        for &i in &group {
            let (p, n) = nodes[i];
            distance_sq = distance_sq.min(p.as_vec3().distance_squared(player));
            if n.kind == Kind::Storage {
                for (amount, input) in stored.iter_mut().zip(n.input) {
                    *amount += u64::from(input);
                }
            }
        }
        let id = bases.list[base].id;
        let history = world.factory.by_base.get(id);
        result.push(Outpost {
            id,
            anchor: bases.list[base].anchor,
            distance_m: distance_sq.sqrt() * scale,
            machines: group.len(),
            health: summarize_nodes(group.into_iter().map(|i| nodes[i].1)),
            stored,
            minute: history.map(|h| h.window(false)).unwrap_or_default(),
            hour: history.map(|h| h.window(true)).unwrap_or_default(),
            power: history.map(|h| h.power_history()).unwrap_or_default(),
        });
    }
    result.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::factory::Node;

    fn node(kind: Kind, status: &'static str) -> Node {
        let mut n = Node::new(kind);
        n.status = status;
        n
    }

    #[test]
    fn statuses_map_to_the_intended_health() {
        assert_eq!(
            classify(&node(Kind::Crusher, "Processing")),
            Health::Running
        );
        assert_eq!(
            classify(&node(Kind::Crusher, "Needs input")),
            Health::Starved
        );
        assert_eq!(
            classify(&node(Kind::Crusher, "Output blocked")),
            Health::Blocked
        );
        assert_eq!(
            classify(&node(Kind::Crusher, "Needs cable / power")),
            Health::Unpowered
        );
        assert_eq!(classify(&node(Kind::Battery, "Full")), Health::Idle);
        assert_eq!(
            classify(&node(Kind::CokeOven, "Incomplete shell")),
            Health::Unbuilt
        );
        assert_eq!(
            classify(&node(Kind::Assembler, "No recipe")),
            Health::Unconfigured
        );
        let mut paused = node(Kind::Crusher, "Processing");
        paused.enabled = false;
        assert_eq!(classify(&paused), Health::Paused);
    }

    #[test]
    fn the_summary_ranks_the_most_stuck_kind_first() {
        let mut f = Factory::default();
        for x in 0..2 {
            f.nodes.insert(
                glam::IVec3::new(x, 0, 0),
                node(Kind::Crusher, "Needs input"),
            );
        }
        f.nodes
            .insert(glam::IVec3::new(0, 2, 0), node(Kind::Furnace, "Processing"));
        f.nodes.insert(
            glam::IVec3::new(0, 4, 0),
            node(Kind::Furnace, "Needs cable / power"),
        );
        let s = summarize(&f);
        assert_eq!(s.total(), 4);
        assert_eq!(s.rows[0].kind, Some(Kind::Crusher));
        assert_eq!(s.rows[0].main_issue(), Some((Health::Starved, 2)));
        assert_eq!(s.bottleneck(), Some((Health::Starved, 2)));
    }

    #[test]
    fn a_healthy_factory_has_no_bottleneck() {
        let mut f = Factory::default();
        f.nodes
            .insert(glam::IVec3::ZERO, node(Kind::Crusher, "Processing"));
        assert_eq!(summarize(&f).bottleneck(), None);
    }
}
