use super::*;
use crate::{
    fluid::VoxelScalePreset,
    player::Player,
    world::{
        World, WorldType, factory,
        inventory::{Item, SurvivalInventory},
    },
};
use glam::{IVec3, Vec3};

#[test]
fn high_rate_battery_pickup_preserves_world_and_readable_inventory() {
    for scale in [
        VoxelScalePreset::OneMeter,
        VoxelScalePreset::HalfMeter,
        VoxelScalePreset::QuarterMeter,
    ] {
        for limit in [128, 129, 255, 256] {
            let mut world = World::new(WorldType::Superflat, scale);
            let size = factory::step(&world);
            let pos = IVec3::new(4, 40, 4) * size;
            let node = factory::fixture_place(&mut world, pos, Kind::Battery).unwrap();
            node.upgrade = power::MITHRIL_TIER;
            node.energy = 123_456;
            node.mode = 1;
            factory::action(
                &mut world,
                pos.as_vec3(),
                factory::Action::SetTarget(pos, limit),
            );
            assert_eq!(world.factory.nodes[&pos].target, limit);
            let mut before_inventory = Vec::new();
            world.inventory.encode(&mut before_inventory);
            let (mut blocks, mut chunks) = (Default::default(), Default::default());
            assert!(factory::dismantle(
                &mut world,
                pos,
                &mut blocks,
                &mut chunks
            ));
            let mut inventory_bytes = Vec::new();
            world.inventory.encode(&mut inventory_bytes);
            let mut words = inventory_bytes
                .chunks_exact(4)
                .map(|word| u32::from_le_bytes(word.try_into().unwrap()));
            let restored = SurvivalInventory::decode_at_scale(
                &mut || {
                    words
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("truncated inventory"))
                },
                world.voxel_scale.meters(),
            );
            assert!(
                restored.is_ok(),
                "packing {limit} kW made inventory unreadable: {:?}",
                restored.err()
            );
            assert!(words.next().is_none());
            let node = world
                .factory
                .nodes
                .get(&pos)
                .expect("unrepresentable settings leave the battery installed");
            assert_eq!(
                (node.upgrade, node.energy, node.mode, node.target),
                (3, 123_456, 1, limit)
            );
            assert_eq!(inventory_bytes, before_inventory);
            assert!(
                factory::cells(pos, size)
                    .iter()
                    .all(|&p| world.get(p) == Kind::Battery.block())
            );
            assert!(blocks.is_empty() && chunks.is_empty());

            // Unlimited is representable and keeps the same 256 kW capability.
            // Clearing the restriction must immediately allow the normal pickup.
            factory::action(
                &mut world,
                pos.as_vec3(),
                factory::Action::SetTarget(pos, 0),
            );
            assert!(factory::dismantle(
                &mut world,
                pos,
                &mut blocks,
                &mut chunks
            ));
            assert!(!world.factory.nodes.contains_key(&pos));
            let kit = PowerKit {
                kind: Kind::Battery,
                energy: 123_456,
                upgrade: 3,
                mode: 1,
                limit: 0,
            };
            let item = Item::PowerKit(kit);
            assert_eq!(world.inventory.count_item(item), 1);
            let (tag, payload) = item.encode_u32();
            assert_eq!(Item::decode_u32(tag, payload).unwrap(), item);
            let mut player = Player::new();
            player.pos = pos.as_vec3() + Vec3::splat(12.0 * size as f32);
            assert!(factory::place(
                &mut world,
                pos,
                Kind::Battery.block(),
                &player,
                Some(item),
                &mut blocks,
                &mut chunks
            ));
            let node = &world.factory.nodes[&pos];
            assert_eq!(
                (node.upgrade, node.energy, node.mode, node.target),
                (3, 123_456, 1, 0)
            );
            assert_eq!(world.inventory.count_item(item), 0);
        }
    }
}

#[test]
fn every_representable_battery_profile_keeps_exact_charge_and_settings() {
    for upgrade in 0..=power::MITHRIL_TIER {
        for mode in 0..Kind::Battery.modes().len() as u32 {
            for limit in 0..=power::battery_rate(upgrade).min(127) {
                for energy in [0, power::battery_capacity(upgrade)] {
                    let mut node = Node::new(Kind::Battery);
                    node.upgrade = upgrade;
                    node.mode = mode;
                    node.target = limit;
                    node.energy = energy;
                    let kit = PowerKit::from_node(&node).unwrap().unwrap();
                    let (header, payload) = kit.encode();
                    assert_eq!(PowerKit::decode(header, payload).unwrap(), kit);
                    let restored = kit.node();
                    assert_eq!(
                        (
                            restored.upgrade,
                            restored.mode,
                            restored.target,
                            restored.energy
                        ),
                        (upgrade, mode, limit, energy)
                    );
                }
            }
        }
    }
}

#[test]
fn portable_header_layout_and_invalid_state_rejection_stay_strict() {
    let kit = PowerKit {
        kind: Kind::Battery,
        energy: 54_321,
        upgrade: 2,
        mode: 1,
        limit: 23,
    };
    // Existing portable header bytes must not change to accommodate bad pickup.
    assert_eq!(kit.encode(), (3_067_911, 54_321));
    assert_eq!(PowerKit::decode(3_067_911, 54_321).unwrap(), kit);
    assert!(PowerKit::decode(3_067_911 | (1 << 24), 54_321).is_err());
    assert!(
        PowerKit::from_node(&Node::new(Kind::Generator))
            .unwrap()
            .is_none()
    );
    for (upgrade, mode, energy) in [
        (256, 0, 0),
        (0, 256, 0),
        (0, 0, power::battery_capacity(0) + 1),
    ] {
        let mut node = Node::new(Kind::Battery);
        node.upgrade = upgrade;
        node.mode = mode;
        node.energy = energy;
        assert!(PowerKit::from_node(&node).is_err());
    }
}
