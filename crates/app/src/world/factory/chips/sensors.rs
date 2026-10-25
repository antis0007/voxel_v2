//! What a controller reads on its machine: tanks or crates on the machine's
//! networks, or its own heat, and which tasks fit which machines.
use super::super::{Kind, Network, Node, extraction::WATER_CAPACITY};
use crate::fast_hash::FastMap;
use engine_chip::firmware::{Sensor, Task};
use glam::IVec3;

/// Whether a firmware task has sensors and an output on this machine.
pub fn fits(task: Task, node: &Node) -> bool {
    match task {
        Task::FillBand => node.kind == Kind::Pump,
        Task::StockBand => node.kind.processor(),
        Task::HeatGuard => super::super::pile::is_pile(node),
    }
}

/// Channel sources for a task, in position order: tanks on the machine's
/// fluid networks, crates on its item networks, or the machine itself.
pub(super) fn sensors(fluids: &[Network], items: &[Network], nodes: &FastMap<IVec3, Node>, p: IVec3, sensor: Sensor) -> Vec<IVec3> {
    let (networks, kind) = match sensor {
        Sensor::TankLevel => (fluids, Kind::Tank),
        Sensor::CrateLevel => (items, Kind::Storage),
        Sensor::CoreHeat => return vec![p],
    };
    let mut found: Vec<IVec3> = networks
        .iter()
        .filter(|n| n.ports.contains(&p))
        .flat_map(|n| n.ports.iter().copied())
        .filter(|q| *q != p && nodes.get(q).is_some_and(|n| n.kind == kind))
        .collect();
    found.sort_by_key(|q| (q.x, q.y, q.z));
    found.dedup();
    found
}

/// A channel reading in permille: tank water, the fullest crate
/// compartment, or pile heat.
pub(super) fn level(sensor: Sensor, n: &Node) -> u16 {
    let (v, max) = match sensor {
        Sensor::TankLevel => (n.water, WATER_CAPACITY),
        Sensor::CrateLevel => (n.input.iter().copied().max().unwrap_or(0), super::super::CAPACITY),
        Sensor::CoreHeat => (n.progress, super::super::pile::MAX_HEAT),
    };
    (u64::from(v.min(max)) * 1000 / u64::from(max.max(1))) as u16
}
