//! Factorio-style factory health: every assembly sorted by what it is doing,
//! and which problem is holding the factory back. The question this answers is
//! "what limits my output?", which is the first thing a Factorio player checks.
//!
//! Classification reads only the status text the machines already publish, so
//! it cannot drift from what the look-at panel shows.
use super::{Factory, Kind, Node};

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

    /// What the player should do about it.
    pub fn fix(self) -> &'static str {
        match self {
            Self::Running => "Working.",
            Self::Starved => "Missing ingredients or fuel. Pipe them in, or use Load from your pack.",
            Self::Blocked => "Output full. Pull products out with a pipe, or collect them.",
            Self::Unpowered => "No power reaches it. Run cable from a generator, battery or matrix.",
            Self::Idle => "Nothing to do right now. Normal for a finished line.",
            Self::Paused => "Paused by the player. Start it again when the line is ready.",
            Self::Unbuilt => "Its shell or matrix is incomplete. The look-at panel names the missing part.",
            Self::Unconfigured => "No recipe set. Open the machine and choose one.",
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
    if !n.enabled {
        return Health::Paused;
    }
    match n.status {
        "Processing" | "Generating" | "Coking" | "Smelting" | "Charging" | "Discharging"
        | "Balancing" | "Heating" | "Shaft complete" | "Assembly in shaft" => Health::Running,
        "Output blocked" | "Tank full" => Health::Blocked,
        "Needs cable / power" => Health::Unpowered,
        "Needs input" | "Needs coal" | "Needs coke" | "Needs fuel" | "Needs charge"
        | "Needs water" | "Intake dry" => Health::Starved,
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
    let mut per_kind = vec![[0u32; 8]; Kind::COUNT];
    let mut summary = Summary::default();
    for n in f.nodes.values() {
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
        assert_eq!(classify(&node(Kind::Crusher, "Processing")), Health::Running);
        assert_eq!(classify(&node(Kind::Crusher, "Needs input")), Health::Starved);
        assert_eq!(classify(&node(Kind::Crusher, "Output blocked")), Health::Blocked);
        assert_eq!(classify(&node(Kind::Crusher, "Needs cable / power")), Health::Unpowered);
        assert_eq!(classify(&node(Kind::Battery, "Full")), Health::Idle);
        assert_eq!(classify(&node(Kind::CokeOven, "Incomplete shell")), Health::Unbuilt);
        assert_eq!(classify(&node(Kind::Assembler, "No recipe")), Health::Unconfigured);
        let mut paused = node(Kind::Crusher, "Processing");
        paused.enabled = false;
        assert_eq!(classify(&paused), Health::Paused);
    }

    #[test]
    fn the_summary_ranks_the_most_stuck_kind_first() {
        let mut f = Factory::default();
        for x in 0..2 {
            f.nodes.insert(glam::IVec3::new(x, 0, 0), node(Kind::Crusher, "Needs input"));
        }
        f.nodes.insert(glam::IVec3::new(0, 2, 0), node(Kind::Furnace, "Processing"));
        f.nodes.insert(glam::IVec3::new(0, 4, 0), node(Kind::Furnace, "Needs cable / power"));
        let s = summarize(&f);
        assert_eq!(s.total(), 4);
        assert_eq!(s.rows[0].kind, Some(Kind::Crusher));
        assert_eq!(s.rows[0].main_issue(), Some((Health::Starved, 2)));
        assert_eq!(s.bottleneck(), Some((Health::Starved, 2)));
    }

    #[test]
    fn a_healthy_factory_has_no_bottleneck() {
        let mut f = Factory::default();
        f.nodes.insert(glam::IVec3::ZERO, node(Kind::Crusher, "Processing"));
        assert_eq!(summarize(&f).bottleneck(), None);
    }
}
