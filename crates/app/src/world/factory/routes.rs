//! Cached pipe routes. Every source keeps one breadth-first tree until the
//! item networks are rebuilt, and each route read from it equals the route
//! `transit::find_path` searches for: same start order, same neighbour order,
//! same first cell that touches the destination.
use super::{DIRS, Network, transit};
use glam::IVec3;
use std::collections::HashMap;

const NONE: u32 = u32::MAX;
const ROOT: u32 = u32::MAX - 1;

/// Per-network route trees, indexed like `Factory::items`.
#[derive(Default)]
pub(super) struct Routes {
    size: i32,
    nets: Vec<Option<NetRoutes>>,
}

struct NetRoutes {
    cells: Vec<IVec3>,
    cell_index: HashMap<IVec3, u32>,
    /// Open pipe neighbour of each cell along each of `DIRS`, or `NONE`.
    links: Vec<[u32; 6]>,
    port_count: usize,
    port_index: HashMap<IVec3, u32>,
    /// Ports each cell touches through an open face: `touch[touch_at[c]..touch_at[c + 1]]`.
    touch_at: Vec<u32>,
    touch: Vec<u32>,
    /// Cells touching each port, sorted by (x, y, z): where a search from it starts.
    starts: Vec<Vec<u32>>,
    /// Trees from each port, then from each single pipe cell (extractor pulls).
    trees: Vec<Option<Tree>>,
}

struct Tree {
    /// Cell each cell was reached from; `ROOT` for starts, `NONE` if unreachable.
    parent: Vec<u32>,
    /// First cell in search order that touches each port.
    first: Vec<u32>,
}

impl Routes {
    /// Drops every tree; call whenever the item networks are rebuilt.
    pub(super) fn clear(&mut self) {
        self.nets.clear();
    }

    /// Same result as `transit::find_path(network, from, to, size, via)`.
    pub(super) fn path(
        &mut self,
        ni: usize,
        network: &Network,
        size: i32,
        from: IVec3,
        to: IVec3,
        via: Option<IVec3>,
    ) -> Option<Vec<IVec3>> {
        if self.size != size {
            self.nets.clear();
            self.size = size;
        }
        if self.nets.len() <= ni {
            self.nets.resize_with(ni + 1, || None);
        }
        let slot = &mut self.nets[ni];
        if slot
            .as_ref()
            .is_none_or(|r| r.cells.len() != network.conduits.len() || r.port_count != network.ports.len())
        {
            *slot = Some(NetRoutes::new(network, size));
        }
        let routes = slot.as_mut()?;
        let Some(&target) = routes.port_index.get(&to) else {
            return transit::find_path(network, from, to, size, via);
        };
        let key = match via {
            Some(v) => {
                let &cell = routes.cell_index.get(&v)?;
                let joined = DIRS.iter().any(|d| v + *d * size == from)
                    && !network.closed.contains(&(v, from))
                    && !network.no_extract.contains(&(v, from));
                if !joined {
                    return None;
                }
                routes.port_count + cell as usize
            }
            None => match routes.port_index.get(&from) {
                Some(&port) => port as usize,
                None => return transit::find_path(network, from, to, size, via),
            },
        };
        if routes.trees[key].is_none() {
            let tree = if key < routes.port_count {
                routes.grow(&routes.starts[key])
            } else {
                routes.grow(&[(key - routes.port_count) as u32])
            };
            routes.trees[key] = Some(tree);
        }
        let tree = routes.trees[key].as_ref()?;
        let mut cell = tree.first[target as usize];
        if cell == NONE {
            return None;
        }
        let mut route = Vec::new();
        while cell != ROOT {
            route.push(routes.cells[cell as usize]);
            cell = tree.parent[cell as usize];
        }
        let mut path = Vec::with_capacity(route.len() + 2);
        path.push(from);
        path.extend(route.into_iter().rev());
        path.push(to);
        Some(path)
    }
}

/// Resources in flight toward each machine, kept current through one routing
/// pass so a barrel check does not scan every packet.
pub(super) struct Inbound(HashMap<IVec3, (usize, bool)>);

impl Inbound {
    pub(super) fn new(packets: &[transit::Packet]) -> Self {
        let mut inbound = Self(HashMap::new());
        for p in packets {
            inbound.add(p.dst, p.item);
        }
        inbound
    }

    pub(super) fn add(&mut self, dst: IVec3, item: usize) {
        let entry = self.0.entry(dst).or_insert((item, false));
        if entry.0 != item {
            entry.1 = true;
        }
    }

    /// True when a packet of a resource other than `item` is headed to `dst`.
    pub(super) fn other_than(&self, dst: IVec3, item: usize) -> bool {
        self.0.get(&dst).is_some_and(|&(first, mixed)| mixed || first != item)
    }
}

impl NetRoutes {
    fn new(network: &Network, size: i32) -> Self {
        let cells = network.conduits.clone();
        let cell_index: HashMap<IVec3, u32> = cells.iter().enumerate().map(|(i, c)| (*c, i as u32)).collect();
        let port_index: HashMap<IVec3, u32> =
            network.ports.iter().enumerate().map(|(i, p)| (*p, i as u32)).collect();
        let mut links = Vec::with_capacity(cells.len());
        let mut touch_at = Vec::with_capacity(cells.len() + 1);
        let mut touch = Vec::new();
        let mut starts = vec![Vec::new(); network.ports.len()];
        for (ci, &c) in cells.iter().enumerate() {
            let mut open = [NONE; 6];
            touch_at.push(touch.len() as u32);
            for (side, d) in DIRS.into_iter().enumerate() {
                let n = c + d * size;
                if network.closed.contains(&(c, n)) {
                    continue;
                }
                if let Some(&ni) = cell_index.get(&n) {
                    open[side] = ni;
                }
                if let Some(&port) = port_index.get(&n) {
                    if !network.no_insert.contains(&(c, n)) {
                        touch.push(port);
                    }
                    if !network.no_extract.contains(&(c, n)) {
                        starts[port as usize].push(ci as u32);
                    }
                }
            }
            links.push(open);
        }
        touch_at.push(touch.len() as u32);
        for list in &mut starts {
            list.sort_by_key(|&ci| {
                let p = cells[ci as usize];
                (p.x, p.y, p.z)
            });
        }
        let trees = (0..network.ports.len() + cells.len()).map(|_| None).collect();
        Self {
            cells,
            cell_index,
            links,
            port_count: network.ports.len(),
            port_index,
            touch_at,
            touch,
            starts,
            trees,
        }
    }

    /// Full breadth-first search; `find_path` stops early on the same order.
    fn grow(&self, starts: &[u32]) -> Tree {
        let mut parent = vec![NONE; self.cells.len()];
        let mut first = vec![NONE; self.port_count];
        let mut queue = Vec::with_capacity(self.cells.len());
        for &s in starts {
            parent[s as usize] = ROOT;
            queue.push(s);
        }
        let mut head = 0;
        while let Some(&c) = queue.get(head) {
            head += 1;
            let ci = c as usize;
            for &port in &self.touch[self.touch_at[ci] as usize..self.touch_at[ci + 1] as usize] {
                if first[port as usize] == NONE {
                    first[port as usize] = c;
                }
            }
            for n in self.links[ci] {
                if n != NONE && parent[n as usize] == NONE {
                    parent[n as usize] = c;
                    queue.push(n);
                }
            }
        }
        Tree { parent, first }
    }
}
