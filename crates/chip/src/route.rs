//! Bounded deterministic grid router. Layer 0 is the contact layer: module
//! bodies block it except at their own ports. Upper layers route over the
//! cells. Costs weigh length, vias and congestion next to other nets' wires.
//! Each multi-pin net grows as a tree, nearest pin first. Ties break on site
//! index, so the same design and order always produce the same wires.
use crate::design::{Design, Site};
use crate::registry::{Registry, Signal};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

const STEP: u32 = 10;
const VIA: u32 = 40;
const CONGESTION: u32 = 4;
/// Search effort per pin connection, in expanded sites per grid site.
const EFFORT: usize = 16;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RouteStats {
    pub routed: Vec<u16>,
    pub failed: Vec<u16>,
    pub expanded: usize,
}

struct Grid {
    w: usize,
    h: usize,
    layers: usize,
    /// 0 = free, u16::MAX = module body, otherwise the owning net id.
    owner: Vec<u16>,
}

const BODY: u16 = u16::MAX;

impl Grid {
    fn index(&self, s: Site) -> usize {
        (usize::from(s.layer) * self.h + usize::from(s.y)) * self.w + usize::from(s.x)
    }
    fn site(&self, i: usize) -> Site {
        let x = i % self.w;
        let y = (i / self.w) % self.h;
        let l = i / (self.w * self.h);
        Site::new(x as u8, y as u8, l as u8)
    }
    fn neighbours(&self, s: Site, out: &mut Vec<Site>) {
        out.clear();
        if s.x > 0 {
            out.push(Site::new(s.x - 1, s.y, s.layer));
        }
        if usize::from(s.x) + 1 < self.w {
            out.push(Site::new(s.x + 1, s.y, s.layer));
        }
        if s.y > 0 {
            out.push(Site::new(s.x, s.y - 1, s.layer));
        }
        if usize::from(s.y) + 1 < self.h {
            out.push(Site::new(s.x, s.y + 1, s.layer));
        }
        if s.layer > 0 {
            out.push(Site::new(s.x, s.y, s.layer - 1));
        }
        if usize::from(s.layer) + 1 < self.layers {
            out.push(Site::new(s.x, s.y, s.layer + 1));
        }
    }
    fn congestion(&self, s: Site, net: u16) -> u32 {
        let mut n = Vec::with_capacity(6);
        self.neighbours(s, &mut n);
        n.iter()
            .filter(|q| q.layer == s.layer)
            .filter(|q| {
                let o = self.owner[self.index(**q)];
                o != 0 && o != BODY && o != net
            })
            .count() as u32
            * CONGESTION
    }
}

fn build_grid(design: &Design, registry: &Registry, except: &[u16]) -> Grid {
    let (w, h, layers) = (usize::from(design.width), usize::from(design.height), usize::from(design.layers));
    let mut grid = Grid { w, h, layers, owner: vec![0; w * h * layers] };
    for part in &design.parts {
        if let Some((x, y, pw, ph)) = design.rect(registry, part) {
            for yy in y..y + ph {
                for xx in x..x + pw {
                    let i = grid.index(Site::new(xx, yy, 0));
                    grid.owner[i] = BODY;
                }
            }
        }
    }
    for net in design.nets.iter().filter(|n| !except.contains(&n.id)) {
        for s in net.paths.iter().flatten() {
            if usize::from(s.x) < w && usize::from(s.y) < h && usize::from(s.layer) < layers {
                let i = grid.index(*s);
                grid.owner[i] = net.id;
            }
        }
    }
    grid
}

fn priority(design: &Design, registry: &Registry, net: u16) -> (u8, Reverse<usize>, u16) {
    let Some(n) = design.nets.iter().find(|n| n.id == net) else {
        return (9, Reverse(0), net);
    };
    let signal = n.pins.first().and_then(|pin| {
        let part = design.part(pin.part)?;
        Some(registry.module(&part.module)?.port(&pin.port)?.signal)
    });
    let class = match signal {
        Some(Signal::Power) => 0,
        Some(Signal::Clock) => 1,
        Some(Signal::Reset) => 2,
        _ => 3,
    };
    (class, Reverse(n.pins.len()), net)
}

/// Shortest path from `from` to any site already in `tree` (owned by `net`).
fn search(grid: &Grid, net: u16, from: Site, tree: &[bool], budget: usize, expanded: &mut usize) -> Option<Vec<Site>> {
    let n = grid.owner.len();
    let mut dist = vec![u32::MAX; n];
    let mut prev = vec![usize::MAX; n];
    let mut heap = BinaryHeap::new();
    let start = grid.index(from);
    dist[start] = 0;
    heap.push(Reverse((0u32, start)));
    let mut around = Vec::with_capacity(6);
    let mut spent = 0;
    while let Some(Reverse((d, i))) = heap.pop() {
        if d > dist[i] {
            continue;
        }
        if tree[i] {
            let mut path = vec![grid.site(i)];
            let mut c = i;
            while prev[c] != usize::MAX {
                c = prev[c];
                path.push(grid.site(c));
            }
            path.reverse();
            *expanded += spent;
            return Some(path);
        }
        spent += 1;
        if spent > budget {
            break;
        }
        let s = grid.site(i);
        grid.neighbours(s, &mut around);
        for &q in &around {
            let j = grid.index(q);
            let o = grid.owner[j];
            if !(o == 0 || o == net || tree[j]) {
                continue;
            }
            let step = if q.layer != s.layer { VIA } else { STEP };
            let nd = d + step + grid.congestion(q, net);
            if nd < dist[j] {
                dist[j] = nd;
                prev[j] = i;
                heap.push(Reverse((nd, j)));
            }
        }
    }
    *expanded += spent;
    None
}

/// Route the listed nets (manual ones are kept), with every other net's wire
/// as an obstacle. Nets that cannot be completed are left unrouted.
pub fn route_nets(design: &mut Design, registry: &Registry, nets: &[u16]) -> RouteStats {
    let mut order: Vec<u16> = nets
        .iter()
        .copied()
        .filter(|id| design.nets.iter().any(|n| n.id == *id && !n.manual))
        .collect();
    order.sort_by_key(|id| priority(design, registry, *id));
    order.dedup();
    for &id in &order {
        design.clear_wire(id);
    }
    let mut stats = RouteStats::default();
    let mut grid = build_grid(design, registry, &order);
    let budget = grid.owner.len() * EFFORT;
    for id in order {
        let Some(net) = design.nets.iter().find(|n| n.id == id) else { continue };
        let mut sites: Vec<Site> = net.pins.iter().filter_map(|p| design.pin_site(registry, p)).collect();
        if sites.len() < 2 || sites.len() != net.pins.len() {
            stats.failed.push(id);
            continue;
        }
        for s in &sites {
            let i = grid.index(*s);
            if grid.owner[i] == BODY {
                grid.owner[i] = id;
            }
        }
        let mut tree = vec![false; grid.owner.len()];
        tree[grid.index(sites[0])] = true;
        let mut paths = Vec::new();
        let mut ok = true;
        let mut pending: Vec<Site> = sites.split_off(1);
        while !pending.is_empty() {
            // Nearest remaining pin to the tree's root, ties by site.
            let root = sites[0];
            let k = (0..pending.len())
                .min_by_key(|&k| {
                    let s = pending[k];
                    (s.x.abs_diff(root.x) as u32 + s.y.abs_diff(root.y) as u32, s)
                })
                .unwrap_or(0);
            let from = pending.remove(k);
            if tree[grid.index(from)] {
                continue;
            }
            match search(&grid, id, from, &tree, budget, &mut stats.expanded) {
                Some(path) => {
                    for s in &path {
                        let i = grid.index(*s);
                        tree[i] = true;
                        grid.owner[i] = id;
                    }
                    paths.push(path);
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            if let Some(n) = design.nets.iter_mut().find(|n| n.id == id) {
                n.paths = paths;
            }
            stats.routed.push(id);
        } else {
            // Release the partial tree: later nets see only committed wires.
            grid = build_grid(design, registry, &[]);
            stats.failed.push(id);
        }
    }
    stats
}

/// Route every net that has no wire yet: what an edit left dirty.
pub fn route_dirty(design: &mut Design, registry: &Registry) -> RouteStats {
    let dirty: Vec<u16> = design.nets.iter().filter(|n| n.paths.is_empty()).map(|n| n.id).collect();
    route_nets(design, registry, &dirty)
}

/// Rip up every automatic wire and route the whole design again.
pub fn route_all(design: &mut Design, registry: &Registry) -> RouteStats {
    let all: Vec<u16> = design.nets.iter().map(|n| n.id).collect();
    route_nets(design, registry, &all)
}

/// One-click repair: drop broken manual wires too, then route everything.
pub fn repair(design: &mut Design, registry: &Registry, broken: &[u16]) -> RouteStats {
    for &id in broken {
        design.clear_wire(id);
    }
    route_all(design, registry)
}
