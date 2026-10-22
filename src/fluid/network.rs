//! Fluid network: containers joined by pipes, in the style of Mekanism and
//! Thermal Expansion.
//!
//! Two kinds of connection exist:
//!
//! * **Passive links** (pipes, adjacent tanks). Every container in a connected
//!   component shares its fluid. Each tick the component's contents spread out
//!   in proportion to capacity, and each node moves at most its `rate` units,
//!   so a thin pipe carries less than a big tank can hold.
//! * **Pumps**: directed transfers from one node to another at a fixed rate,
//!   such as a pump drawing from a reservoir into a pipe.
//!
//! A component can hold only one fluid kind. If two different kinds meet, the
//! component is blocked and nothing moves until one kind has been removed.
//! Every move is an exact `Container` transfer, so total units are conserved
//! each tick.

use super::containers::{Container, FluidKind, transfer};
use std::collections::VecDeque;

pub type NodeId = usize;

#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub container: Container,
    /// Most units this node can gain or lose in one tick while equalising.
    /// Pipes use a small rate; tanks use `u32::MAX`.
    pub rate: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pump {
    pub from: NodeId,
    pub to: NodeId,
    pub rate: u32,
}

#[derive(Default)]
pub struct FluidGraph {
    nodes: Vec<Node>,
    links: Vec<Vec<NodeId>>,
    pumps: Vec<Pump>,
}

impl FluidGraph {
    pub fn add_node(&mut self, container: Container, rate: u32) -> NodeId {
        self.nodes.push(Node { container, rate });
        self.links.push(Vec::new());
        self.nodes.len() - 1
    }

    /// Join two nodes with a passive link. Links are symmetric.
    pub fn connect(&mut self, a: NodeId, b: NodeId) {
        if a != b && !self.links[a].contains(&b) {
            self.links[a].push(b);
            self.links[b].push(a);
        }
    }

    pub fn add_pump(&mut self, from: NodeId, to: NodeId, rate: u32) {
        self.pumps.push(Pump { from, to, rate });
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn container(&self, id: NodeId) -> &Container {
        &self.nodes[id].container
    }

    /// Put fluid into a node directly, for filling tanks from world sources.
    pub fn insert(&mut self, id: NodeId, kind: FluidKind, units: u32) -> u32 {
        self.nodes[id].container.insert(kind, units)
    }

    pub fn extract(&mut self, id: NodeId, units: u32) -> u32 {
        self.nodes[id].container.extract(units)
    }

    /// Total units held across every node. Used to check conservation.
    pub fn total_units(&self) -> u64 {
        self.nodes.iter().map(|n| n.container.amount() as u64).sum()
    }

    /// Advance the network by one tick: pumps first, then equalisation.
    pub fn tick(&mut self) {
        for i in 0..self.pumps.len() {
            let pump = self.pumps[i];
            self.run_pump(pump);
        }
        self.equalise();
    }

    fn run_pump(&mut self, pump: Pump) {
        let mut from = self.nodes[pump.from].container;
        let mut to = self.nodes[pump.to].container;
        transfer(&mut from, &mut to, pump.rate);
        self.nodes[pump.from].container = from;
        self.nodes[pump.to].container = to;
    }

    fn equalise(&mut self) {
        let mut seen = vec![false; self.nodes.len()];
        for start in 0..self.nodes.len() {
            if seen[start] {
                continue;
            }
            let component = self.component(start, &mut seen);
            self.equalise_component(&component);
        }
    }

    /// Breadth-first search over passive links.
    fn component(&self, start: NodeId, seen: &mut [bool]) -> Vec<NodeId> {
        let mut queue = VecDeque::from([start]);
        let mut out = Vec::new();
        seen[start] = true;
        while let Some(id) = queue.pop_front() {
            out.push(id);
            for &next in &self.links[id] {
                if !seen[next] {
                    seen[next] = true;
                    queue.push_back(next);
                }
            }
        }
        out.sort_unstable();
        out
    }

    fn equalise_component(&mut self, ids: &[NodeId]) {
        let Some(kind) = self.component_kind(ids) else {
            return;
        };
        let total: u64 = ids
            .iter()
            .map(|&i| self.nodes[i].container.amount() as u64)
            .sum();
        let capacity: u64 = ids
            .iter()
            .map(|&i| self.nodes[i].container.capacity() as u64)
            .sum();
        if capacity == 0 {
            return;
        }
        let targets = proportional_targets(
            total,
            &ids.iter()
                .map(|&i| self.nodes[i].container.capacity() as u64)
                .collect::<Vec<_>>(),
        );
        let mut deltas: Vec<i64> = ids
            .iter()
            .zip(&targets)
            .map(|(&i, &t)| {
                let delta = t as i64 - self.nodes[i].container.amount() as i64;
                let rate = self.nodes[i].rate as i64;
                delta.clamp(-rate, rate)
            })
            .collect();
        balance_to_zero(&mut deltas);

        // Withdraw from losers first so every inserted unit has a source.
        for (&id, &d) in ids.iter().zip(&deltas) {
            if d < 0 {
                self.nodes[id].container.extract((-d) as u32);
            }
        }
        for (&id, &d) in ids.iter().zip(&deltas) {
            if d > 0 {
                let inserted = self.nodes[id].container.insert(kind, d as u32);
                debug_assert_eq!(inserted as i64, d, "equalise must conserve units");
            }
        }
    }

    /// The single kind held by a component, or `None` if it is empty or mixed.
    fn component_kind(&self, ids: &[NodeId]) -> Option<FluidKind> {
        let mut kind = None;
        for &i in ids {
            if let Some(k) = self.nodes[i].container.kind() {
                match kind {
                    None => kind = Some(k),
                    Some(existing) if existing != k => return None,
                    Some(_) => {}
                }
            }
        }
        kind
    }
}

/// Split `total` across nodes in proportion to `capacities`. Each share is
/// floored, and the leftover units go to the first nodes that still have room,
/// so the targets always sum to exactly `total`.
fn proportional_targets(total: u64, capacities: &[u64]) -> Vec<u64> {
    let cap_sum: u64 = capacities.iter().sum();
    let mut targets: Vec<u64> = capacities.iter().map(|&c| total * c / cap_sum).collect();
    // Hand out rounding leftovers first to nodes with room, then to anyone, so
    // every unit is placed even when `total` exceeds the summed capacity.
    let mut leftover = total - targets.iter().sum::<u64>();
    for require_room in [true, false] {
        for (t, &c) in targets.iter_mut().zip(capacities) {
            if leftover == 0 {
                break;
            }
            if !require_room || *t < c {
                *t += 1;
                leftover -= 1;
            }
        }
    }
    targets
}

/// Adjust `deltas` so they sum to zero without exceeding their own magnitudes.
/// The larger side is scaled down to the smaller side's total, and the rounding
/// remainder is handed out one unit at a time to entries that still have room.
fn balance_to_zero(deltas: &mut [i64]) {
    let positive: i64 = deltas.iter().filter(|d| **d > 0).sum();
    let negative: i64 = -deltas.iter().filter(|d| **d < 0).sum::<i64>();
    if positive == negative {
        return;
    }
    let big_sign: i64 = if positive > negative { 1 } else { -1 };
    let (big, small) = (positive.max(negative), positive.min(negative));
    let mut scaled: Vec<i64> = deltas
        .iter()
        .map(|d| {
            if d.signum() == big_sign {
                d.abs() * small / big
            } else {
                0
            }
        })
        .collect();
    let mut remainder = small - scaled.iter().sum::<i64>();
    for (s, d) in scaled.iter_mut().zip(deltas.iter()) {
        if remainder == 0 {
            break;
        }
        if d.signum() == big_sign && *s < d.abs() {
            *s += 1;
            remainder -= 1;
        }
    }
    for (d, s) in deltas.iter_mut().zip(scaled) {
        if d.signum() == big_sign {
            *d = big_sign * s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::containers::UNITS_PER_M3;

    fn tank(m3: u32) -> Container {
        Container::with_volume_m3(m3)
    }

    fn pipe() -> Container {
        Container::new(UNITS_PER_M3 / 8)
    }

    #[test]
    fn proportional_targets_sum_exactly_to_total() {
        for total in [0u64, 1, 7, 18, 100, 1234] {
            let t = proportional_targets(total, &[3, 5, 9, 1]);
            assert_eq!(t.iter().sum::<u64>(), total);
        }
    }

    #[test]
    fn balance_to_zero_always_sums_to_zero() {
        let mut d = vec![5, -3, -4, 2, 0];
        balance_to_zero(&mut d);
        assert_eq!(d.iter().sum::<i64>(), 0);
        let mut e = vec![-10, 1, 1];
        balance_to_zero(&mut e);
        assert_eq!(e.iter().sum::<i64>(), 0);
        assert!(e[0] >= -10 && e[0] <= 0);
    }

    #[test]
    fn a_pipe_equalises_two_tanks_and_conserves_units() {
        let mut g = FluidGraph::default();
        let a = g.add_node(tank(1), u32::MAX);
        let b = g.add_node(tank(1), u32::MAX);
        g.connect(a, b);
        g.insert(a, FluidKind::Water, 1000);
        let before = g.total_units();
        for _ in 0..50 {
            g.tick();
            assert_eq!(g.total_units(), before);
        }
        let diff = g.container(a).amount() as i64 - g.container(b).amount() as i64;
        assert!(diff.abs() <= 1, "tanks should match, differ by {diff}");
    }

    #[test]
    fn equalisation_splits_by_capacity_not_by_count() {
        // A 1 m³ tank next to a 3 m³ tank: the big one holds three quarters.
        let mut g = FluidGraph::default();
        let small = g.add_node(tank(1), u32::MAX);
        let big = g.add_node(tank(3), u32::MAX);
        g.connect(small, big);
        g.insert(small, FluidKind::Water, 1024);
        for _ in 0..50 {
            g.tick();
        }
        let s = g.container(small).amount() as f32;
        let b = g.container(big).amount() as f32;
        assert!(
            (b / (s + b) - 0.75).abs() < 0.01,
            "big share {}",
            b / (s + b)
        );
    }

    #[test]
    fn a_thin_pipe_limits_flow_per_tick() {
        let mut g = FluidGraph::default();
        let reservoir = g.add_node(tank(16), u32::MAX);
        let pipe_node = g.add_node(pipe(), 4);
        g.connect(reservoir, pipe_node);
        g.insert(reservoir, FluidKind::Water, 4000);
        g.tick();
        // The pipe can gain at most its rate in one tick.
        assert!(g.container(pipe_node).amount() <= 4);
    }

    #[test]
    fn mixed_fluids_in_one_component_do_not_move() {
        let mut g = FluidGraph::default();
        let a = g.add_node(tank(1), u32::MAX);
        let b = g.add_node(tank(1), u32::MAX);
        g.connect(a, b);
        g.insert(a, FluidKind::Water, 100);
        g.insert(b, FluidKind::Creosote, 100);
        g.tick();
        assert_eq!(g.container(a).amount(), 100);
        assert_eq!(g.container(b).amount(), 100);
    }

    #[test]
    fn separate_components_do_not_exchange_fluid() {
        let mut g = FluidGraph::default();
        let a = g.add_node(tank(1), u32::MAX);
        let b = g.add_node(tank(1), u32::MAX);
        g.insert(a, FluidKind::Water, 500);
        for _ in 0..10 {
            g.tick();
        }
        assert_eq!(g.container(b).amount(), 0);
        assert_eq!(g.container(a).amount(), 500);
    }

    #[test]
    fn pump_moves_fluid_at_its_rate_and_never_overfills() {
        let mut g = FluidGraph::default();
        let source = g.add_node(tank(16), u32::MAX);
        let sink = g.add_node(Container::new(30), u32::MAX);
        g.insert(source, FluidKind::Water, 1000);
        g.add_pump(source, sink, 10);
        g.tick();
        assert_eq!(g.container(sink).amount(), 10);
        for _ in 0..10 {
            g.tick();
        }
        assert!(g.container(sink).amount() <= 30);
        assert_eq!(g.total_units(), 1000);
    }

    #[test]
    fn pump_refuses_a_kind_the_sink_does_not_accept() {
        let mut g = FluidGraph::default();
        let source = g.add_node(tank(1), u32::MAX);
        let sink = g.add_node(tank(1), u32::MAX);
        g.insert(source, FluidKind::Water, 100);
        g.insert(sink, FluidKind::Creosote, 10);
        g.add_pump(source, sink, 50);
        g.tick();
        assert_eq!(g.container(source).amount(), 100);
        assert_eq!(g.container(sink).amount(), 10);
    }
}
