//! Recipe machines on a fluid network, in the style of Mekanism processing
//! machines.
//!
//! A machine reads its inputs from network nodes, runs for a fixed number of
//! ticks, and then pushes its products into output nodes. Inputs are taken when
//! a run starts, so a half-finished run never holds ingredients that are still
//! in a tank. If the outputs have no room when a run finishes, the machine is
//! `Blocked` and keeps its finished batch until there is space. Nothing is ever
//! destroyed or duplicated along the way.

use super::containers::FluidKind;
use super::network::{FluidGraph, NodeId};

/// One fluid stream into or out of a machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Port {
    pub node: NodeId,
    pub kind: FluidKind,
    pub units: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    pub ticks: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Running {
        left: u32,
    },
    /// A finished batch is waiting for room in its output nodes.
    Blocked,
}

pub struct Machine {
    pub recipe: Recipe,
    pub state: State,
}

impl Machine {
    pub fn new(recipe: Recipe) -> Self {
        Self {
            recipe,
            state: State::Idle,
        }
    }

    /// Advance one tick and return the new state.
    pub fn tick(&mut self, graph: &mut FluidGraph) -> State {
        self.state = match self.state {
            State::Idle => {
                if self.inputs_ready(graph) {
                    self.consume_inputs(graph);
                    State::Running {
                        left: self.recipe.ticks.max(1),
                    }
                } else {
                    State::Idle
                }
            }
            State::Running { left } if left > 1 => State::Running { left: left - 1 },
            State::Running { .. } | State::Blocked => {
                if self.produce(graph) {
                    State::Idle
                } else {
                    State::Blocked
                }
            }
        };
        self.state
    }

    fn inputs_ready(&self, graph: &FluidGraph) -> bool {
        self.recipe.inputs.iter().all(|p| {
            let c = graph.container(p.node);
            c.kind() == Some(p.kind) && c.amount() >= p.units
        })
    }

    fn consume_inputs(&self, graph: &mut FluidGraph) {
        for p in &self.recipe.inputs {
            let removed = graph.extract(p.node, p.units);
            debug_assert_eq!(removed, p.units);
        }
    }

    /// Push every product, or nothing. A partial batch would split a recipe
    /// across ticks in a way the player cannot see, so it is all or none.
    fn produce(&self, graph: &mut FluidGraph) -> bool {
        let fits = self.recipe.outputs.iter().all(|p| {
            let c = graph.container(p.node);
            c.accepts(p.kind) && c.room() >= p.units
        });
        if fits {
            for p in &self.recipe.outputs {
                let inserted = graph.insert(p.node, p.kind, p.units);
                debug_assert_eq!(inserted, p.units);
            }
        }
        fits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::containers::{Container, UNITS_PER_M3};

    /// 2 H2O -> 2 H2 + O2, run in a tank-fed electrolyser.
    fn electrolyser(water_units: u32) -> (FluidGraph, Machine, NodeId, NodeId, NodeId) {
        let mut g = FluidGraph::default();
        let water = g.add_node(Container::with_volume_m3(1), u32::MAX);
        let hydrogen = g.add_node(Container::new(UNITS_PER_M3), u32::MAX);
        let oxygen = g.add_node(Container::new(UNITS_PER_M3), u32::MAX);
        g.insert(water, FluidKind::Water, water_units);
        let recipe = Recipe {
            inputs: vec![Port {
                node: water,
                kind: FluidKind::Water,
                units: 2,
            }],
            outputs: vec![
                Port {
                    node: hydrogen,
                    kind: FluidKind::Hydrogen,
                    units: 2,
                },
                Port {
                    node: oxygen,
                    kind: FluidKind::Oxygen,
                    units: 1,
                },
            ],
            ticks: 3,
        };
        (g, Machine::new(recipe), water, hydrogen, oxygen)
    }

    #[test]
    fn runs_for_its_time_then_produces_its_outputs() {
        let (mut g, mut m, water, h, o) = electrolyser(10);
        assert_eq!(m.tick(&mut g), State::Running { left: 3 });
        assert_eq!(g.container(water).amount(), 8, "inputs leave at run start");
        assert_eq!(g.container(h).amount(), 0, "products wait for the end");
        assert_eq!(m.tick(&mut g), State::Running { left: 2 });
        assert_eq!(m.tick(&mut g), State::Running { left: 1 });
        assert_eq!(m.tick(&mut g), State::Idle);
        assert_eq!(g.container(h).amount(), 2);
        assert_eq!(g.container(o).amount(), 1);
    }

    /// Hydrogen and oxygen atoms held by a node's fluid. Units are molecules,
    /// so 2 H2O -> 2 H2 + O2 changes the unit count but not the atom count.
    fn h_and_o_atoms(g: &FluidGraph, nodes: &[NodeId]) -> (u32, u32) {
        nodes.iter().fold((0, 0), |(h, o), &n| {
            let c = g.container(n);
            let units = c.amount();
            match c.kind() {
                Some(FluidKind::Water) => (h + 2 * units, o + units),
                Some(FluidKind::Hydrogen) => (h + 2 * units, o),
                Some(FluidKind::Oxygen) => (h, o + 2 * units),
                _ => (h, o),
            }
        })
    }

    #[test]
    fn atoms_are_conserved_across_a_run() {
        let (mut g, mut m, water, h, o) = electrolyser(10);
        let nodes = [water, h, o];
        let before = h_and_o_atoms(&g, &nodes);
        // Compare at the end of a full cycle: while a run is in progress its
        // inputs sit inside the machine, not in any node.
        for _ in 0..4 {
            m.tick(&mut g);
        }
        assert_eq!(m.state, State::Idle);
        assert_eq!(h_and_o_atoms(&g, &nodes), before);
    }

    #[test]
    fn idles_without_enough_input_and_consumes_nothing() {
        let (mut g, mut m, water, ..) = electrolyser(1);
        for _ in 0..5 {
            assert_eq!(m.tick(&mut g), State::Idle);
        }
        assert_eq!(g.container(water).amount(), 1);
    }

    #[test]
    fn idles_when_the_input_is_the_wrong_fluid() {
        let (mut g, mut m, water, ..) = electrolyser(10);
        // Swap the tank's contents for another fluid by draining and refilling.
        g.extract(water, 10);
        g.insert(water, FluidKind::Creosote, 10);
        assert_eq!(m.tick(&mut g), State::Idle);
        assert_eq!(g.container(water).amount(), 10);
    }

    #[test]
    fn a_full_output_blocks_then_resumes_when_space_frees() {
        let (mut g, mut m, _water, h, _o) = electrolyser(10);
        // Fill the hydrogen tank so the finished batch has nowhere to go.
        g.insert(h, FluidKind::Hydrogen, UNITS_PER_M3);
        for _ in 0..4 {
            m.tick(&mut g);
        }
        assert_eq!(m.state, State::Blocked);
        g.extract(h, UNITS_PER_M3);
        m.tick(&mut g);
        assert_eq!(m.state, State::Idle);
        assert_eq!(g.container(h).amount(), 2);
    }

    #[test]
    fn a_batch_is_all_or_nothing_across_outputs() {
        let (mut g, mut m, _water, h, o) = electrolyser(10);
        // Oxygen full, hydrogen empty: no hydrogen may be produced either.
        g.insert(o, FluidKind::Oxygen, UNITS_PER_M3);
        for _ in 0..4 {
            m.tick(&mut g);
        }
        assert_eq!(m.state, State::Blocked);
        assert_eq!(g.container(h).amount(), 0);
    }
}
