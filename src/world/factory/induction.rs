//! Mekanism-style induction matrix: a hollow cuboid of induction casing with
//! one or more induction ports in its walls, filled with induction cells
//! (capacity) and providers (transfer rate). Any box from 3 × 3 × 3 to
//! 9 × 9 × 9 metres forms; empty interior cells are allowed.
//!
//! The matrix keeps one energy pool, identified by its lowest-coordinate cell.
//! Every port shares that pool: a port in Input mode charges it
//! from its cable network, a port in Output mode discharges into its network.
//! Formation is recomputed when the factory topology changes, and the pool is
//! carried across re-formation. Before edits it is distributed into individual
//! cells; pickup, splits and merges conserve it without copying it to ports.
use super::{DIRS, Factory, Kind};
use glam::IVec3;
use std::collections::{HashSet, VecDeque};

/// One induction cell: 64 kWh.
pub const CELL_CAPACITY: u32 = 921_600;
/// One induction provider: 256 kW.
pub const PROVIDER_RATE: u32 = 256;
pub const MIN_SIDE: i32 = 3;
pub const MAX_SIDE: i32 = 9;
/// Largest possible pool: a 9³ box has a 7³ interior.
pub const MAX_CAPACITY: u32 = CELL_CAPACITY * 343;

pub const PORT_MODES: [&str; 2] = [
    "Input: charges the matrix from this port's cable network",
    "Output: discharges the matrix into this port's cable network",
];

fn part(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::InductionCasing | Kind::InductionPort | Kind::InductionCell | Kind::InductionProvider
    )
}

/// Why a component did not form, for the interface.
pub fn problem(code: u16) -> &'static str {
    match code {
        1 => "Box must be 3 to 9 metres on every side",
        2 => "Walls must be casing or ports, with no gaps",
        3 => "Only cells and providers may sit inside",
        4 => "Needs at least one induction port in a wall",
        5 => "Stored charge exceeds cell capacity; restore cells to reconnect",
        _ => "",
    }
}

impl Factory {
    /// Convert a pooled representation into owned per-cell charge before
    /// changing connectivity. Sorting makes split ownership independent of map
    /// iteration. Legacy port-held charge migrates here on first formation.
    pub(super) fn scatter_matrix_energy(&mut self) {
        let mut groups: std::collections::HashMap<IVec3, Vec<IVec3>> = Default::default();
        for (&p, n) in &self.nodes {
            if part(n.kind) {
                if let Some(pool) = n.pool {
                    groups.entry(pool).or_default().push(p);
                }
            }
        }
        for mut members in groups.into_values() {
            members.sort_by_key(|p| (p.x, p.y, p.z));
            let mut energy: u64 = members.iter().map(|p| self.nodes[p].energy as u64).sum();
            for p in &members {
                self.nodes.get_mut(p).unwrap().energy = 0;
            }
            for p in &members {
                let n = self.nodes.get_mut(p).unwrap();
                if n.kind == Kind::InductionCell {
                    let charge = energy.min(CELL_CAPACITY as u64) as u32;
                    n.energy = charge;
                    energy -= charge as u64;
                }
            }
            // A damaged/legacy matrix may have fewer cells than its charge.
            // Keep that charge on a port, offline, until capacity is restored.
            if energy > 0 {
                if let Some(p) = members.iter().find(|p| self.nodes[p].kind == Kind::InductionPort)
                    .or_else(|| members.iter().find(|p| self.nodes[p].kind == Kind::InductionCell)) {
                    self.nodes.get_mut(p).unwrap().energy += energy as u32;
                }
            }
        }
    }

    pub(super) fn form_matrices(&mut self, size: i32) {
        self.scatter_matrix_energy();
        let mut seen: HashSet<IVec3> = HashSet::new();
        let mut starts: Vec<IVec3> = self
            .nodes
            .iter()
            .filter(|(_, n)| part(n.kind))
            .map(|(p, _)| *p)
            .collect();
        starts.sort_by_key(|p| (p.x, p.y, p.z));
        for start in starts {
            if !seen.insert(start) {
                continue;
            }
            let mut component = vec![start];
            let mut queue = VecDeque::from([start]);
            while let Some(p) = queue.pop_front() {
                for d in DIRS {
                    let q = p + d * size;
                    if self.nodes.get(&q).is_some_and(|n| part(n.kind)) && seen.insert(q) {
                        component.push(q);
                        queue.push_back(q);
                        if component.len() > 1000 {
                            break;
                        }
                    }
                }
            }
            self.form_component(&component, size);
        }
    }

    fn form_component(&mut self, component: &[IVec3], size: i32) {
        let lo = component.iter().fold(IVec3::MAX, |a, p| a.min(*p));
        let hi = component.iter().fold(IVec3::MIN, |a, p| a.max(*p));
        let dims = (hi - lo) / size + IVec3::ONE;
        let mut ports: Vec<IVec3> = component
            .iter()
            .copied()
            .filter(|p| self.nodes[p].kind == Kind::InductionPort)
            .collect();
        ports.sort_by_key(|p| (p.x, p.y, p.z));
        let mut code = 0u16;
        let (mut cells, mut providers) = (0u32, 0u32);
        if dims.min_element() < MIN_SIDE || dims.max_element() > MAX_SIDE {
            code = 1;
        } else {
            'scan: for y in 0..dims.y {
                for z in 0..dims.z {
                    for x in 0..dims.x {
                        let c = lo + IVec3::new(x, y, z) * size;
                        let wall = x == 0
                            || y == 0
                            || z == 0
                            || x == dims.x - 1
                            || y == dims.y - 1
                            || z == dims.z - 1;
                        let kind = self.nodes.get(&c).map(|n| n.kind);
                        if wall {
                            if !matches!(kind, Some(Kind::InductionCasing | Kind::InductionPort)) {
                                code = 2;
                                break 'scan;
                            }
                        } else {
                            match kind {
                                None => {}
                                Some(Kind::InductionCell) => cells += 1,
                                Some(Kind::InductionProvider) => providers += 1,
                                Some(_) => {
                                    code = 3;
                                    break 'scan;
                                }
                            }
                        }
                    }
                }
            }
            if code == 0 && ports.is_empty() {
                code = 4;
            }
        }
        // The pool lives on whichever port held it; gather it before
        // reassigning so nothing is lost when the master changes.
        let pooled: u64 = component.iter().map(|p| self.nodes[p].energy as u64).sum();
        if pooled > MAX_CAPACITY as u64 {
            // Several charged matrices joined into an oversized component.
            // Keep the scattered owners; a single u32 pool must not truncate it.
            for p in component {
                let n = self.nodes.get_mut(p).unwrap();
                n.formed = false;
                n.pool = None;
                n.structure = (0, 1);
                n.store_cap = 0;
                n.store_rate = 0;
            }
            return;
        }
        let cap = (cells as u64 * CELL_CAPACITY as u64).min(MAX_CAPACITY as u64) as u32;
        if code == 0 && pooled > cap as u64 { code = 5; }
        let formed = code == 0;
        let master = component.iter().filter(|p| self.nodes[p].kind == Kind::InductionCell)
            .min_by_key(|p| (p.x, p.y, p.z)).copied().or_else(|| ports.first().copied());
        for p in component {
            let n = self.nodes.get_mut(p).unwrap();
            n.formed = formed;
            n.structure = if formed {
                (cells as u16, providers as u16)
            } else {
                (0, code)
            };
            n.pool = master;
            n.store_cap = 0;
            n.store_rate = 0;
            n.energy = 0;
        }
        if let Some(m) = master {
            // An unformed matrix keeps its charge on the master port until
            // it is rebuilt; a formed one clamps to its cells.
            let n = self.nodes.get_mut(&m).unwrap();
            n.energy = pooled as u32;
            if formed {
                n.store_cap = cap;
                // Capacity deficits are retained but isolated, never clamped.
                n.store_rate = if pooled <= cap as u64 { providers * PROVIDER_RATE } else { 0 };
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::world::factory::Node;

    pub(crate) fn build(f: &mut Factory, lo: IVec3, side: i32, port: IVec3) {
        for y in 0..side {
            for z in 0..side {
                for x in 0..side {
                    let c = lo + IVec3::new(x, y, z);
                    let wall = [x, y, z].iter().any(|v| *v == 0 || *v == side - 1);
                    let kind = if c == port {
                        Kind::InductionPort
                    } else if wall {
                        Kind::InductionCasing
                    } else if x == 1 && y == 1 && z == 1 {
                        Kind::InductionProvider
                    } else {
                        Kind::InductionCell
                    };
                    f.nodes.insert(c, Node::new(kind));
                }
            }
        }
    }

    #[test]
    fn orphaned_legacy_charge_without_a_port_survives_repeated_formation() {
        let mut f = Factory::default();
        let mut n = Node::new(Kind::InductionCell);
        n.energy = 2 * CELL_CAPACITY;
        f.nodes.insert(IVec3::ZERO, n);
        f.nodes.insert(IVec3::NEG_X, Node::new(Kind::InductionCasing));
        for _ in 0..5 { f.form_matrices(1); f.scatter_matrix_energy(); }
        assert_eq!(f.nodes[&IVec3::ZERO].energy, 2 * CELL_CAPACITY);
        assert!(!f.nodes[&IVec3::ZERO].formed);
        assert_eq!(f.nodes[&IVec3::NEG_X].energy, 0);
    }

    #[test]
    fn a_four_cube_forms_with_seven_cells_and_one_provider() {
        let mut f = Factory::default();
        let port = IVec3::new(0, 1, 1);
        build(&mut f, IVec3::ZERO, 4, port);
        f.form_matrices(1);
        let n = &f.nodes[&port];
        assert!(n.formed);
        assert_eq!(n.structure, (7, 1));
        assert_eq!(f.pool_of(port).unwrap().1, 7 * CELL_CAPACITY);
        assert_eq!(f.pool_of(port).unwrap().2, PROVIDER_RATE);
    }

    #[test]
    fn a_hole_in_the_wall_breaks_it_and_the_charge_survives() {
        let mut f = Factory::default();
        let port = IVec3::new(0, 1, 1);
        build(&mut f, IVec3::ZERO, 3, port);
        f.form_matrices(1);
        f.nodes.get_mut(&port).unwrap().energy = 5000;
        f.nodes.remove(&IVec3::new(2, 2, 2));
        f.form_matrices(1);
        assert!(!f.nodes[&port].formed);
        assert_eq!(f.nodes[&port].structure, (0, 2));
        assert_eq!(f.pool_of(port).unwrap().0, 5000);
    }

    #[test]
    fn removing_ports_cells_and_rejoining_never_copies_or_discards_charge() {
        let mut f = Factory::default();
        let port = IVec3::new(0, 1, 1);
        build(&mut f, IVec3::ZERO, 4, port);
        let extra_port = IVec3::new(0, 2, 1);
        f.nodes.insert(extra_port, Node::new(Kind::InductionPort));
        f.form_matrices(1);
        let identity = f.nodes[&port].pool.unwrap();
        f.nodes.get_mut(&identity).unwrap().energy = 2 * CELL_CAPACITY + 123;
        f.scatter_matrix_energy();
        assert_eq!(f.nodes.remove(&port).unwrap().energy, 0);
        f.form_matrices(1);
        assert_eq!(f.nodes[&extra_port].pool, Some(identity));
        assert_eq!(f.pool_of(extra_port).unwrap().0, 2 * CELL_CAPACITY + 123);
        f.scatter_matrix_energy();
        let removed = f.nodes.remove(&identity).unwrap();
        assert_eq!(removed.energy, CELL_CAPACITY);
        f.form_matrices(1);
        assert_eq!(f.pool_of(extra_port).unwrap().0 + removed.energy, 2 * CELL_CAPACITY + 123);
        f.nodes.insert(identity, removed);
        f.nodes.insert(port, Node::new(Kind::InductionPort));
        f.form_matrices(1);
        assert!(f.nodes[&port].formed);
        assert_eq!(f.pool_of(port).unwrap().0, 2 * CELL_CAPACITY + 123);
        for _ in 0..5 { f.form_matrices(1); }
        assert_eq!(f.pool_of(port).unwrap().0, 2 * CELL_CAPACITY + 123);
    }
}
