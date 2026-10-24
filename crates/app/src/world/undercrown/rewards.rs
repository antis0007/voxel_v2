//! Claim rewards for Undercrown plinths, from `assets/undercrown_rewards.ron`.
use crate::world::inventory::{DroppedStack, Item, ItemStack};
use crate::world::World;
use engine_worldgen::undercrown::ObjectId;
use glam::Vec3;
use std::sync::LazyLock;

const REWARDS_RON: &str = include_str!("../../../../../assets/undercrown_rewards.ron");

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Row {
    pub district: u8,
    pub room: u16,
    pub label: String,
    pub items: Vec<(Item, u32)>,
}

static ROWS: LazyLock<Vec<Row>> = LazyLock::new(|| {
    ron::from_str(REWARDS_RON).unwrap_or_else(|error| panic!("assets/undercrown_rewards.ron: {error}"))
});

pub fn for_prop(id: ObjectId) -> &'static [(Item, u32)] {
    ROWS.iter()
        .find(|r| r.district == id.district() && r.room == id.room())
        .map_or(&[], |r| r.items.as_slice())
}

/// Each reward goes to the pack, or drops at `at` when the pack is full.
pub fn grant(world: &mut World, id: ObjectId, at: Vec3) {
    for &(item, count) in for_prop(id) {
        let (inserted, remainder) = world.inventory.try_insert(ItemStack::new(item, count));
        if inserted > 0 {
            world.inventory.notify_pickup(item, inserted);
        }
        if remainder > 0 && world.inventory.dropped.len() < 1024 {
            world.inventory.dropped.push(DroppedStack { pos: at, stack: ItemStack::new(item, remainder) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_worldgen::undercrown::{Plan, PropKind};
    use engine_worldgen::{WorldSampleConfig, WorldType};
    use engine_core::VoxelScalePreset;

    #[test]
    fn undercrown_rewards_ron_round_trips_and_covers_every_plinth() {
        let back: Vec<Row> = ron::from_str(&ron::to_string(&*ROWS).unwrap()).unwrap();
        assert_eq!(back.len(), ROWS.len());
        let cfg = WorldSampleConfig { generator_version: 32, seed: 424242, world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter };
        let plan = Plan::for_config(&cfg);
        for prop in plan.props.iter().filter(|p| p.kind == PropKind::Plinth) {
            let row = ROWS.iter().find(|r| r.district == prop.id.district() && r.room == prop.id.room());
            let row = row.unwrap_or_else(|| panic!("no reward row for {}", prop.label));
            assert_eq!(row.label, prop.label);
            assert!(!row.items.is_empty());
        }
    }
}
