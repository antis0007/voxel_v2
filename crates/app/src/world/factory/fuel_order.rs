//! A bounded fermentation request. The existing Cultivator target word is
//! zero for continuous operation, otherwise remaining delivery units plus one.
use super::{ETHANOL, Factory, Kind, Node, World, near, transit};
use glam::{IVec3, Vec3};

pub const MODE: u32 = 2;
pub const BATCH: u32 = 4;
pub(super) const MAX_TARGET: u32 = BATCH * 64 + 1;
pub const REQUEST: u32 = MAX_TARGET;

pub(super) fn valid_request(target: u32) -> bool {
    target == REQUEST
}

pub(super) fn is_batch(node: &Node) -> bool {
    node.kind == Kind::Cultivator && (1..=MAX_TARGET).contains(&node.target)
}

pub(super) fn after_delivery(target: u32, amount: u32) -> u32 {
    target.saturating_sub(amount).max(1)
}

pub(super) fn in_flight(packets: &[transit::Packet], source: IVec3) -> u32 {
    packets
        .iter()
        .filter(|p| p.src == source && p.item == ETHANOL)
        .map(|p| p.amount)
        .sum()
}

pub(super) fn saved_target(factory: &Factory, pos: IVec3, node: &Node) -> u32 {
    if !is_batch(node) || node.target <= 1 {
        return node.target;
    }
    let arriving = factory
        .packets
        .iter()
        .filter(|p| {
            p.src == pos
                && p.item == ETHANOL
                && factory
                    .nodes
                    .get(&p.dst)
                    .is_some_and(|dst| dst.kind == Kind::Storage)
        })
        .map(|p| p.amount)
        .sum();
    after_delivery(node.target, arriving)
}

#[derive(Clone)]
pub struct View {
    pub pos: IVec3,
    pub target: u32,
    pub stock: [u32; 2],
    pub batches_needed: u32,
    pub status: &'static str,
    pub ready: bool,
}

pub(super) fn view(world: &World, player: Vec3) -> Option<View> {
    let factory = &world.factory;
    let (&pos, node) = factory
        .nodes
        .iter()
        .filter(|(p, n)| n.kind == Kind::Cultivator && near(world, **p, player)
            && super::stock_order::read(n.kind, n.mode, n.target).is_none())
        .min_by(|(a, _), (b, _)| {
            a.as_vec3()
                .distance_squared(player)
                .total_cmp(&b.as_vec3().distance_squared(player))
                .then_with(|| (a.x, a.y, a.z).cmp(&(b.x, b.y, b.z)))
        })?;
    let transit = in_flight(&factory.packets, pos);
    let status = if node.target == 1 {
        "Delivered"
    } else if !node.enabled {
        "Paused"
    } else if node.target > 1 && transit > 0 {
        "In Transit"
    } else if node.target > 1 && node.output[ETHANOL] > 0 {
        "Check Pipe / Storage"
    } else {
        node.status
    };
    Some(View {
        pos,
        target: node.target,
        status,
        stock: [node.input[13], node.input[4]],
        batches_needed: if node.target > 1 {
            (node.target - 1)
                .saturating_sub(node.output[ETHANOL].saturating_add(transit))
                .div_ceil(64)
        } else {
            BATCH
        },
        ready: node.target <= 1
            && node.recipe.is_none()
            && node.progress == 0
            && node.output[ETHANOL] == 0
            && transit == 0,
    })
}
