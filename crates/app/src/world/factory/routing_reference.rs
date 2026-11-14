//! The item routing pass as it was before route caching: a fresh port scan,
//! sort and path search for every candidate. Tests run it beside the cached
//! pass and require identical factories.
use super::*;

impl Factory {
    pub(super) fn reference_route(&mut self, size: i32) {
        self.reference_extractors(size);
        self.reference_distributors(size);
        let mut moved = HashSet::new();
        let mut near: Vec<(u8, i32, i32, IVec3)> = Vec::new();
        for network in &self.items {
            let net = &network.ports;
            let mut flow = false;
            if net.is_empty() {
                continue;
            }
            let port_count = net.len() as i32;
            for k in 0..net.len() {
                let p = net[(k + self.cursor) % net.len()];
                if moved.contains(&p) || (!self.nodes[&p].enabled && self.nodes[&p].kind != Kind::Assembler) {
                    continue;
                }
                'items: for i in 0..RESOURCE_COUNT {
                    if self.nodes[&p].kind == Kind::Feeder
                        && self.nodes[&p].filter.is_some_and(|filter| filter != i)
                    {
                        continue;
                    }
                    let available = self.nodes[&p].output[i].min(64);
                    if available == 0 {
                        continue;
                    }
                    near.clear();
                    for (offset, &q) in net.iter().enumerate() {
                        if q == p {
                            continue;
                        }
                        let dest = &self.nodes[&q];
                        if !dest.enabled || dest.kind == Kind::Feeder || !self.container_takes(q, i) {
                            continue;
                        }
                        let reserved = self.reserved.get(&(q, i)).copied().unwrap_or(0);
                        if dest.input[i].saturating_add(reserved) >= CAPACITY {
                            continue;
                        }
                        let dist = (q.x - p.x).abs() + (q.y - p.y).abs() + (q.z - p.z).abs();
                        let tie = (offset as i32 + self.cursor as i32) % port_count;
                        let storage = u8::from(dest.kind == Kind::Storage);
                        near.push((storage, dist, tie, q));
                    }
                    near.sort_unstable_by_key(|row| (row.0, row.1, row.2));
                    for &(_, _, _, q) in &near {
                        let room = CAPACITY.saturating_sub(
                            self.nodes[&q].input[i] + self.reserved.get(&(q, i)).copied().unwrap_or(0),
                        );
                        let amount = available.min(room);
                        if amount == 0 {
                            continue;
                        }
                        if self.packets.len() >= transit::MAX_PACKETS {
                            break 'items;
                        }
                        let Some(path) = transit::find_path(network, p, q, size, None) else {
                            continue;
                        };
                        self.nodes.get_mut(&p).unwrap().output[i] -= amount;
                        *self.reserved.entry((q, i)).or_insert(0) += amount;
                        self.packets.push(transit::Packet { item: i, amount, src: p, dst: q, path, progress: 0.0 });
                        flow = true;
                        moved.insert(p);
                        break 'items;
                    }
                }
            }
            if flow {
                for p in &network.conduits {
                    self.nodes.get_mut(p).unwrap().activity = 1.0;
                }
            }
        }
    }

    fn reference_extractors(&mut self, size: i32) {
        let mut extractors: Vec<IVec3> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.kind == Kind::Pipe && n.mode == 1 && n.enabled)
            .map(|(p, _)| *p)
            .collect();
        extractors.sort_by_key(|p| (p.x, p.y, p.z));
        for p in extractors {
            if self.packets.len() >= transit::MAX_PACKETS {
                return;
            }
            let Some(&ni) = self.item_net.get(&p) else {
                continue;
            };
            let network = &self.items[ni];
            let filter = self.nodes[&p].filter;
            'pull: for (side, d) in DIRS.into_iter().enumerate() {
                if !self.side_open(p, side) {
                    continue;
                }
                let s = p + d * size;
                let Some(src) = self.nodes.get(&s) else {
                    continue;
                };
                if matches!(src.kind, Kind::Pipe | Kind::Cable | Kind::FluidPipe | Kind::Feeder) {
                    continue;
                }
                let from_crate = src.kind == Kind::Storage;
                for i in 0..RESOURCE_COUNT {
                    if filter.is_some_and(|f| f != i) {
                        continue;
                    }
                    let have = if from_crate { src.input[i] } else { src.output[i] };
                    let available = have.min(64);
                    if available == 0 {
                        continue;
                    }
                    let mut destinations: Vec<_> = (0..network.ports.len())
                        .map(|k| network.ports[(k + self.cursor) % network.ports.len()])
                        .filter(|q| *q != s)
                        .collect();
                    destinations.sort_by_key(|q| self.nodes[q].kind == Kind::Storage);
                    for q in destinations {
                        let dest = &self.nodes[&q];
                        if !dest.enabled || dest.kind == Kind::Feeder || !self.container_takes(q, i) {
                            continue;
                        }
                        let room = CAPACITY
                            .saturating_sub(dest.input[i] + self.reserved.get(&(q, i)).copied().unwrap_or(0));
                        let amount = available.min(room);
                        if amount == 0 {
                            continue;
                        }
                        let Some(path) = transit::find_path(network, s, q, size, Some(p)) else {
                            continue;
                        };
                        let source = self.nodes.get_mut(&s).unwrap();
                        if from_crate {
                            source.input[i] -= amount;
                        } else {
                            source.output[i] -= amount;
                        }
                        *self.reserved.entry((q, i)).or_insert(0) += amount;
                        self.packets.push(transit::Packet { item: i, amount, src: s, dst: q, path, progress: 0.0 });
                        self.nodes.get_mut(&p).unwrap().activity = 1.0;
                        break 'pull;
                    }
                }
            }
        }
    }

    fn reference_distributors(&mut self, size: i32) {
        let mut pipes: Vec<IVec3> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.kind == Kind::Pipe && n.mode == 2 && n.enabled)
            .map(|(p, _)| *p)
            .collect();
        pipes.sort_by_key(|p| (p.x, p.y, p.z));
        let mut served = HashSet::new();
        for pipe in pipes {
            if self.packets.len() >= transit::MAX_PACKETS {
                return;
            }
            let Some(&ni) = self.item_net.get(&pipe) else {
                continue;
            };
            if !served.insert(ni) {
                continue;
            }
            let filter = self.nodes[&pipe].filter;
            let storages: Vec<IVec3> = self.items[ni]
                .ports
                .iter()
                .copied()
                .filter(|q| self.nodes.get(q).is_some_and(|n| n.kind == Kind::Storage && n.enabled))
                .collect();
            if storages.len() < 2 {
                continue;
            }
            let mut choice: Option<(usize, IVec3, IVec3, u32)> = None;
            for item in 0..RESOURCE_COUNT {
                if filter.is_some_and(|f| f != item) {
                    continue;
                }
                let mut fullest: Option<(IVec3, u32)> = None;
                let mut emptiest: Option<(IVec3, u32)> = None;
                for &q in &storages {
                    let node = &self.nodes[&q];
                    let incoming = self.reserved.get(&(q, item)).copied().unwrap_or(0);
                    let present = node.input[item] + incoming;
                    if node.input[item] > 0 && fullest.is_none_or(|(_, n)| node.input[item] > n) {
                        fullest = Some((q, node.input[item]));
                    }
                    if node.accepts(item) && emptiest.is_none_or(|(_, n)| present < n) {
                        emptiest = Some((q, present));
                    }
                }
                let (Some((src, have)), Some((dst, there))) = (fullest, emptiest) else {
                    continue;
                };
                if src == dst || have <= there {
                    continue;
                }
                let gap = have - there;
                if choice.is_none_or(|(_, _, _, best)| gap > best) {
                    choice = Some((item, src, dst, gap));
                }
            }
            let Some((item, src, dst, gap)) = choice else {
                continue;
            };
            let incoming = self.reserved.get(&(dst, item)).copied().unwrap_or(0);
            let room = CAPACITY.saturating_sub(self.nodes[&dst].input[item] + incoming);
            let have = self.nodes[&src].input[item];
            let amount = gap.min(64).min(have).min(room);
            if amount == 0 {
                continue;
            }
            let Some(path) = transit::find_path(&self.items[ni], src, dst, size, None) else {
                continue;
            };
            self.nodes.get_mut(&src).unwrap().input[item] -= amount;
            *self.reserved.entry((dst, item)).or_insert(0) += amount;
            self.packets.push(transit::Packet { item, amount, src, dst, path, progress: 0.0 });
            let pipe_node = self.nodes.get_mut(&pipe).unwrap();
            pipe_node.activity = 1.0;
            pipe_node.status = "Balancing";
        }
    }
}
