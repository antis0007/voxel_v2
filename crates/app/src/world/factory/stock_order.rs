//! Refill a filtered, reachable crate using ordinary process batches and pipe
//! reservations. The machine's existing target word stores the stock order.
use super::{DIRS, Factory, Kind, Node, recipes};
use glam::IVec3;
use std::collections::{HashMap, HashSet};

const TAG: u32 = 1 << 31;
pub const AMOUNTS: [u32; 4] = [4, 8, 16, 32];

#[derive(Clone, Copy)]
pub struct Order {
    pub item: usize,
    pub count: u32,
}

impl Order {
    pub fn target(self) -> u32 {
        TAG | (self.item as u32) << 8 | self.count
    }
}

pub fn read(kind: Kind, mode: u32, target: u32) -> Option<Order> {
    saved_order(target).filter(|order| outputs(kind, mode).contains(&order.item))
}

pub(super) fn saved_order(target: u32) -> Option<Order> {
    if target & TAG == 0 { return None; }
    let item = ((target & !TAG) >> 8) as usize;
    let count = target & 255;
    if !AMOUNTS.contains(&count) || item >= super::RESOURCE_COUNT { return None; }
    Some(Order { item, count })
}

pub fn outputs(kind: Kind, mode: u32) -> &'static [usize] {
    match (kind, mode) {
        (Kind::Furnace, recipes::GOLD_MODE) => &[super::GOLD_INGOT],
        (Kind::Furnace, 0 | 1) => &[6, 7, 8],
        (Kind::AlloyKiln, 0) => &[14],
        (Kind::AlloyKiln, 1) => &[15],
        (Kind::Fabricator, _) => recipes::fabrication_output(mode),
        (Kind::Cultivator, super::fuel_order::MODE) => &[super::ETHANOL],
        _ => &[],
    }
}

pub(super) fn recipe(n: &Node, order: Order) -> usize {
    match n.kind {
        Kind::Furnace if n.mode == recipes::GOLD_MODE => recipes::GOLD_RECIPE,
        Kind::Furnace if n.mode == 1 => order.item - 6,
        Kind::Furnace => order.item + 3,
        _ => n.mode as usize,
    }
}

pub(super) struct Line {
    pub dest: IVec3,
    pub needed: bool,
    pub stocked: bool,
}

#[derive(Clone, Copy)]
pub struct View {
    pub stored: u32,
    pub incoming: u32,
}

pub(super) fn view(factory: &Factory, pos: IVec3) -> Option<View> {
    let source = factory.nodes.get(&pos)?;
    let order = read(source.kind, source.mode, source.target)?;
    let index = factory.stock_receivers.binary_search_by_key(&pos.to_array(), |(p, _)| p.to_array()).ok()?;
    let dest = factory.stock_receivers[index].1;
    let node = factory.nodes.get(&dest)?;
    if node.kind != Kind::Storage || node.filter != Some(order.item) || !node.enabled { return None; }
    Some(View {
        stored: node.input[order.item],
        incoming: factory.reserved.get(&(dest, order.item)).copied().unwrap_or(0),
    })
}

pub(super) fn room(order: Order, dest: &Node, reserved: u32) -> u32 {
    (order.count * 64).saturating_sub(dest.input[order.item].saturating_add(reserved))
}

/// Pick one receiver per machine, with deterministic ownership when two
/// machines on the same bus try to maintain the same crate and product.
pub(super) fn lines(factory: &mut Factory, size: i32) -> HashMap<IVec3, Line> {
    let mut sources: Vec<_> = factory.nodes.iter()
        .filter_map(|(&p, n)| read(n.kind, n.mode, n.target).map(|order| (p, order)))
        .collect();
    sources.sort_unstable_by_key(|(p, _)| p.to_array());
    let mut claimed = HashSet::new();
    let mut lines = HashMap::new();
    let mut candidates = Vec::new();
    for (source, order) in sources {
        candidates.clear();
        let mut networks = [None; 6];
        for (side, dir) in DIRS.iter().enumerate() {
            let Some(&network) = factory.item_net.get(&(source + *dir * size)) else { continue; };
            if networks[..side].contains(&Some(network)) { continue; }
            networks[side] = Some(network);
            for &dest in &factory.items[network].ports {
                let n = &factory.nodes[&dest];
                if n.kind == Kind::Storage && n.enabled && n.filter == Some(order.item)
                    && !claimed.contains(&(dest, order.item))
                {
                    candidates.push((dest, network));
                }
            }
        }
        candidates.sort_unstable_by_key(|(dest, network)| {
            let d = (*dest - source).abs();
            (d.x + d.y + d.z, dest.to_array(), *network)
        });
        let Some((dest, _)) = candidates.iter().copied().find(|&(dest, network)| {
            factory.routes.path(network, &factory.items[network], size, source, dest, None).is_some()
        }) else { continue; };
        claimed.insert((dest, order.item));
        let node = &factory.nodes[&dest];
        let reserved = factory.reserved.get(&(dest, order.item)).copied().unwrap_or(0);
        lines.insert(source, Line {
            dest,
            needed: room(order, node, reserved) > factory.nodes[&source].output[order.item],
            stocked: node.input[order.item] >= order.count * 64,
        });
    }
    lines
}
