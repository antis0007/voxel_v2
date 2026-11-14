use super::*;
use crate::{
    fluid::VoxelScalePreset,
    world::{
        WorldType,
        inventory::{Item, ItemStack, SurvivalInventory},
    },
};

fn world(scale: VoxelScalePreset) -> World {
    let mut world = World::new(WorldType::Superflat, scale);
    world.inventory = SurvivalInventory::empty();
    world.inventory.voxel_m = scale.meters();
    world.sync_from_inventory();
    world
}

fn line(world: &mut World, offset: IVec3, amount: u32) -> IVec3 {
    let size = step(world);
    let at = |x| (offset + IVec3::new(x, 40, 0)) * size;
    fixture_place(world, at(0), Kind::Furnace).unwrap().output[6] = amount;
    for x in 1..=3 {
        fixture_place(world, at(x), Kind::Pipe).unwrap();
    }
    fixture_place(world, at(4), Kind::Storage).unwrap();
    at(4)
}

fn inventory_units(inventory: &SurvivalInventory) -> u32 {
    inventory.count_item(Item::Resource(6)) * 64 + inventory.resource_remainders[6]
}

fn factory_units(factory: &Factory) -> u32 {
    factory
        .nodes
        .values()
        .map(|n| n.input[6] + n.output[6])
        .sum::<u32>()
        + factory
            .packets
            .iter()
            .filter(|p| p.item == 6)
            .map(|p| p.amount)
            .sum::<u32>()
}

fn inventory_bytes(inventory: &SurvivalInventory) -> Vec<u8> {
    let mut bytes = Vec::new();
    inventory.encode(&mut bytes);
    bytes
}

fn saved_units(world: &World) -> u32 {
    // The production save settles in-flight items into factory buffers and
    // writes player inventory separately. There is no later simulation tick.
    let mut bytes = Vec::new();
    world.factory.encode(&mut bytes);
    let mut words = bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()));
    let factory = Factory::decode(
        &mut || words.next().ok_or_else(|| anyhow::anyhow!("short factory")),
        step(world),
        FACTORY_FORMAT,
    )
    .unwrap();
    assert!(words.next().is_none());
    let bytes = inventory_bytes(&world.inventory);
    let mut words = bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()));
    let inventory = SurvivalInventory::decode_at_scale(
        &mut || {
            words
                .next()
                .ok_or_else(|| anyhow::anyhow!("short inventory"))
        },
        world.voxel_scale.meters(),
    )
    .unwrap();
    assert!(words.next().is_none());
    factory_units(&factory) + inventory_units(&inventory)
}

#[test]
fn receiver_pickup_keeps_inflight_stock_in_an_immediate_save() {
    for scale in [
        VoxelScalePreset::OneMeter,
        VoxelScalePreset::HalfMeter,
        VoxelScalePreset::QuarterMeter,
    ] {
        for amount in [17, 64] {
            let mut world = world(scale);
            let receiver = line(&mut world, IVec3::ZERO, amount);
            world.factory.nodes.get_mut(&receiver).unwrap().input[6] = 11;
            world.inventory.resource_remainders[6] = 53;
            let size = step(&world);
            world.factory.tick(0.25, size);
            assert_eq!(world.factory.packets.len(), 1);
            assert_eq!(world.factory.reserved[&(receiver, 6)], amount);
            let before = inventory_units(&world.inventory) + factory_units(&world.factory);
            assert_eq!(saved_units(&world), before);
            assert!(dismantle(
                &mut world,
                receiver,
                &mut Default::default(),
                &mut Default::default()
            ));
            assert!(!world.factory.nodes.contains_key(&receiver));
            assert_eq!(
                saved_units(&world),
                before,
                "receiver pickup lost an in-flight shipment before the next tick"
            );
            assert!(world.factory.packets.is_empty());
            assert!(
                !world
                    .factory
                    .reserved
                    .keys()
                    .any(|(dst, _)| *dst == receiver)
            );
            for _ in 0..8 {
                world.factory.tick(0.25, size);
            }
            assert!(
                world.factory.take_refunds().is_empty(),
                "shipment refunded twice"
            );
            assert_eq!(
                inventory_units(&world.inventory) + factory_units(&world.factory),
                before
            );
        }
    }
}

#[test]
fn incoming_stock_participates_in_atomic_pack_capacity_check() {
    for scale in [
        VoxelScalePreset::OneMeter,
        VoxelScalePreset::HalfMeter,
        VoxelScalePreset::QuarterMeter,
    ] {
        let mut world = world(scale);
        let receiver = line(&mut world, IVec3::ZERO, 64);
        let size = step(&world);
        world.factory.tick(0.25, size);
        let stone = Item::Block(B::Stone);
        let full = world.inventory.stack_limit(stone);
        for slot in world
            .inventory
            .hotbar
            .iter_mut()
            .chain(world.inventory.pack.iter_mut())
        {
            *slot = Some(ItemStack::new(stone, full));
        }
        world.inventory.hotbar[0] = None; // Space for the machine kit, not the incoming ingot.
        let inventory = inventory_bytes(&world.inventory);
        let packets = world.factory.packets.clone();
        let reserved = world.factory.reserved.clone();
        let (mut blocks, mut chunks) = (Default::default(), Default::default());
        assert!(dismantle(&mut world, receiver, &mut blocks, &mut chunks));
        assert!(
            world.factory.nodes.contains_key(&receiver),
            "pickup must wait for space for incoming stock too"
        );
        assert_eq!(inventory_bytes(&world.inventory), inventory);
        assert_eq!(world.factory.packets, packets);
        assert_eq!(world.factory.reserved, reserved);
        assert!(
            cells(receiver, size)
                .into_iter()
                .all(|p| world.get(p) == Kind::Storage.block())
        );
        assert!(blocks.is_empty() && chunks.is_empty());
    }
}

#[test]
fn receiver_pickup_leaves_other_shipments_and_reservations_untouched() {
    let mut world = world(VoxelScalePreset::OneMeter);
    let receiver = line(&mut world, IVec3::ZERO, 64);
    let other = line(&mut world, IVec3::new(0, 0, 5), 64);
    world.factory.tick(0.25, 1);
    assert_eq!(world.factory.packets.len(), 2);
    let remaining: Vec<_> = world
        .factory
        .packets
        .iter()
        .filter(|p| p.dst == other)
        .cloned()
        .collect();
    assert_eq!(remaining.len(), 1);
    assert!(dismantle(
        &mut world,
        receiver,
        &mut Default::default(),
        &mut Default::default()
    ));
    assert_eq!(world.factory.packets, remaining);
    assert_eq!(world.factory.reserved.len(), 1);
    assert_eq!(world.factory.reserved[&(other, 6)], 64);
    assert_eq!(saved_units(&world), 128);
    for _ in 0..8 {
        world.factory.tick(0.25, 1);
    }
    assert_eq!(world.factory.nodes[&other].input[6], 64);
    assert_eq!(inventory_units(&world.inventory), 64);
    assert!(world.factory.take_refunds().is_empty());
}

#[test]
fn fractional_incoming_stock_can_use_remainder_space_in_a_full_pack() {
    for scale in [
        VoxelScalePreset::OneMeter,
        VoxelScalePreset::HalfMeter,
        VoxelScalePreset::QuarterMeter,
    ] {
        let mut world = world(scale);
        let receiver = line(&mut world, IVec3::ZERO, 17);
        let size = step(&world);
        world.factory.tick(0.25, size);
        let stone = Item::Block(B::Stone);
        let full = world.inventory.stack_limit(stone);
        for slot in world
            .inventory
            .hotbar
            .iter_mut()
            .chain(world.inventory.pack.iter_mut())
        {
            *slot = Some(ItemStack::new(stone, full));
        }
        world.inventory.hotbar[0] = None;
        assert!(dismantle(
            &mut world,
            receiver,
            &mut Default::default(),
            &mut Default::default()
        ));
        assert!(!world.factory.nodes.contains_key(&receiver));
        assert_eq!(world.inventory.resource_remainders[6], 17);
        assert_eq!(
            world
                .inventory
                .count_item(Item::Block(Kind::Storage.block())),
            1
        );
        assert!(world.factory.packets.is_empty());
        assert_eq!(saved_units(&world), 17);
    }
}

#[test]
fn cancelled_fuel_shipment_does_not_claim_a_storage_delivery() {
    let mut world = world(VoxelScalePreset::OneMeter);
    let source = IVec3::new(0, 40, 0);
    let receiver = IVec3::new(4, 40, 0);
    let node = fixture_place(&mut world, source, Kind::Cultivator).unwrap();
    node.mode = fuel_order::MODE;
    node.target = fuel_order::REQUEST;
    node.output[ETHANOL] = 64;
    for x in 1..=3 {
        fixture_place(&mut world, IVec3::new(x, 40, 0), Kind::Pipe).unwrap();
    }
    fixture_place(&mut world, receiver, Kind::Storage).unwrap();
    world.factory.tick(0.25, 1);
    assert_eq!(world.factory.packets.len(), 1);
    assert_eq!(world.factory.nodes[&source].target, fuel_order::REQUEST);
    assert!(dismantle(
        &mut world,
        receiver,
        &mut Default::default(),
        &mut Default::default()
    ));
    assert!(world.factory.packets.is_empty());
    assert_eq!(world.inventory.count_item(Item::Resource(ETHANOL as u8)), 1);
    let node = &world.factory.nodes[&source];
    assert_eq!(node.target, fuel_order::REQUEST);
    assert_eq!(
        fuel_order::saved_target(&world.factory, source, node),
        fuel_order::REQUEST
    );
    assert!(!world.factory.workshop_delivery);
    assert!(world.factory.take_refunds().is_empty());
}

#[test]
fn receiver_pickup_claims_every_packet_and_resource_exactly_once() {
    let mut world = world(VoxelScalePreset::OneMeter);
    let receiver = line(&mut world, IVec3::ZERO, 128);
    world
        .factory
        .nodes
        .get_mut(&IVec3::new(0, 40, 0))
        .unwrap()
        .output[7] = 64;
    let node = world.factory.nodes.get_mut(&receiver).unwrap();
    node.input[6] = 9;
    node.input[7] = 5;
    world.inventory.resource_remainders[6] = 55;
    world.inventory.resource_remainders[7] = 59;
    for _ in 0..3 {
        world.factory.tick(0.25, 1);
    }
    assert_eq!(world.factory.packets.len(), 3);
    assert_eq!(world.factory.reserved[&(receiver, 6)], 128);
    assert_eq!(world.factory.reserved[&(receiver, 7)], 64);
    assert!(dismantle(
        &mut world,
        receiver,
        &mut Default::default(),
        &mut Default::default()
    ));
    assert!(world.factory.packets.is_empty());
    assert!(world.factory.reserved.is_empty());
    let bytes = inventory_bytes(&world.inventory);
    let mut words = bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()));
    let restored = SurvivalInventory::decode(&mut || {
        words
            .next()
            .ok_or_else(|| anyhow::anyhow!("short inventory"))
    })
    .unwrap();
    assert!(words.next().is_none());
    for inventory in [&world.inventory, &restored] {
        assert_eq!(inventory.count_item(Item::Resource(6)), 3);
        assert_eq!(inventory.count_item(Item::Resource(7)), 2);
        assert_eq!(inventory.resource_remainders[6..=7], [0, 0]);
    }
    for _ in 0..8 {
        world.factory.tick(0.25, 1);
    }
    assert!(world.factory.take_refunds().is_empty());
    assert_eq!(inventory_bytes(&world.inventory), bytes);
}

#[test]
fn pipe_pickup_does_not_lose_crossing_stock_when_pack_is_full() {
    for scale in [
        VoxelScalePreset::OneMeter,
        VoxelScalePreset::HalfMeter,
        VoxelScalePreset::QuarterMeter,
    ] {
        let mut world = world(scale);
        let receiver = line(&mut world, IVec3::ZERO, 64);
        let size = step(&world);
        let pipe = receiver - IVec3::X * (2 * size);
        world.factory.tick(0.25, size);
        let stone = Item::Block(B::Stone);
        let full = world.inventory.stack_limit(stone);
        for slot in world
            .inventory
            .hotbar
            .iter_mut()
            .chain(world.inventory.pack.iter_mut())
        {
            *slot = Some(ItemStack::new(stone, full));
        }
        world.inventory.hotbar[0] = None; // The pipe fits; its crossing shipment does not.
        world.sync_from_inventory();
        let inventory = inventory_bytes(&world.inventory);
        let packets = world.factory.packets.clone();
        let reserved = world.factory.reserved.clone();
        let (mut blocks, mut chunks) = (Default::default(), Default::default());
        assert!(dismantle(&mut world, pipe, &mut blocks, &mut chunks));
        // These are the production factory/adventure refund and slot-reconciliation
        // operations. A rejected whole item cannot remain in the legacy counter.
        world.factory.tick(0.25, size);
        for (item, amount) in world.factory.take_refunds() {
            world.expedition.add(item, amount);
        }
        world.reconcile_legacy_to_inventory();
        world.sync_from_inventory();
        assert_eq!(
            inventory_units(&world.inventory) + factory_units(&world.factory),
            64,
            "packing a busy pipe dropped its shipment through full-pack refund reconciliation"
        );
        assert!(world.factory.nodes.contains_key(&pipe));
        assert_eq!(inventory_bytes(&world.inventory), inventory);
        assert_eq!(world.factory.reserved, reserved);
        assert_eq!(world.factory.packets.len(), packets.len());
        assert!(blocks.is_empty() && chunks.is_empty());
        assert!(
            cells(pipe, size)
                .into_iter()
                .all(|p| world.get(p) == Kind::Pipe.block())
        );
        assert_eq!(saved_units(&world), 64);
    }
}

#[test]
fn pipe_pickup_claims_crossing_stock_before_an_immediate_save() {
    for scale in [
        VoxelScalePreset::OneMeter,
        VoxelScalePreset::HalfMeter,
        VoxelScalePreset::QuarterMeter,
    ] {
        let mut world = world(scale);
        let receiver = line(&mut world, IVec3::ZERO, 64);
        let size = step(&world);
        let pipe = receiver - IVec3::X * (2 * size);
        world.factory.tick(0.25, size);
        let (mut blocks, mut chunks) = (Default::default(), Default::default());
        assert!(dismantle(&mut world, pipe, &mut blocks, &mut chunks));
        assert!(!world.factory.nodes.contains_key(&pipe));
        assert_eq!(
            inventory_units(&world.inventory),
            64,
            "crossing stock belongs to the pickup, not a later refund"
        );
        assert!(world.factory.packets.is_empty());
        assert!(world.factory.reserved.is_empty());
        assert_eq!(world.factory.nodes[&receiver].input[6], 0);
        assert_eq!(saved_units(&world), 64);
        for _ in 0..8 {
            world.factory.tick(0.25, size);
        }
        assert!(world.factory.take_refunds().is_empty());
        assert_eq!(
            inventory_units(&world.inventory) + factory_units(&world.factory),
            64
        );
    }
}

#[test]
fn pipe_pickup_preserves_other_routes_to_the_same_receiver() {
    let mut world = world(VoxelScalePreset::OneMeter);
    let receiver = line(&mut world, IVec3::ZERO, 64);
    fixture_place(&mut world, receiver + IVec3::Z * 4, Kind::Furnace)
        .unwrap()
        .output[6] = 64;
    for z in 1..=3 {
        fixture_place(&mut world, receiver + IVec3::Z * z, Kind::Pipe).unwrap();
    }
    world.factory.tick(0.25, 1);
    assert_eq!(world.factory.packets.len(), 2);
    assert_eq!(world.factory.reserved[&(receiver, 6)], 128);
    let pipe = receiver - IVec3::X * 2;
    let unaffected = world
        .factory
        .packets
        .iter()
        .find(|packet| !packet.path.contains(&pipe))
        .unwrap()
        .clone();
    let (mut blocks, mut chunks) = (Default::default(), Default::default());
    assert!(dismantle(&mut world, pipe, &mut blocks, &mut chunks));
    assert_eq!(world.factory.packets, vec![unaffected]);
    assert_eq!(world.factory.reserved[&(receiver, 6)], 64);
    assert_eq!(inventory_units(&world.inventory), 64);
    assert_eq!(saved_units(&world), 128);
    for _ in 0..8 {
        world.factory.tick(0.25, 1);
    }
    assert_eq!(world.factory.nodes[&receiver].input[6], 64);
    assert!(world.factory.reserved.is_empty());
    assert!(world.factory.take_refunds().is_empty());
    assert_eq!(saved_units(&world), 128);
}

#[test]
fn source_pickup_leaves_departed_shipments_in_transit() {
    let mut world = world(VoxelScalePreset::OneMeter);
    let receiver = line(&mut world, IVec3::ZERO, 64);
    let source = receiver - IVec3::X * 4;
    world.factory.tick(0.25, 1);
    let packets = world.factory.packets.clone();
    let reserved = world.factory.reserved.clone();
    let (mut blocks, mut chunks) = (Default::default(), Default::default());
    assert!(dismantle(&mut world, source, &mut blocks, &mut chunks));
    assert!(!world.factory.nodes.contains_key(&source));
    assert_eq!(world.factory.packets, packets);
    assert_eq!(world.factory.reserved, reserved);
    assert_eq!(inventory_units(&world.inventory), 0);
    assert_eq!(saved_units(&world), 64);
    for _ in 0..8 {
        world.factory.tick(0.25, 1);
    }
    assert_eq!(world.factory.nodes[&receiver].input[6], 64);
    assert!(world.factory.reserved.is_empty());
    assert!(world.factory.take_refunds().is_empty());
    assert_eq!(saved_units(&world), 64);
}

#[test]
fn rejected_pipe_pickup_keeps_exact_inventory_packets_and_reservations() {
    for scale in [
        VoxelScalePreset::OneMeter,
        VoxelScalePreset::HalfMeter,
        VoxelScalePreset::QuarterMeter,
    ] {
        for readonly in [false, true] {
            let mut world = world(scale);
            let receiver = line(&mut world, IVec3::ZERO, 64);
            let size = step(&world);
            let pipe = receiver - IVec3::X * (2 * size);
            world.factory.tick(0.25, size);
            if readonly {
                // A published simulation snapshot owns edits. The legacy World
                // writer must reject removal even though the pack has room.
                world.ca_view = Some(std::sync::Arc::new(
                    crate::simulation::cpu_region::Snapshot {
                        origin: IVec3::splat(1000),
                        tick: 1,
                        applied_sequence: 0,
                        cells: Box::new([engine_sim::Cell::filled(B::Air as u8); 512]),
                        hash: 0,
                        stats: Default::default(),
                    },
                ));
            } else {
                let stone = Item::Block(B::Stone);
                let full = world.inventory.stack_limit(stone);
                for slot in world
                    .inventory
                    .hotbar
                    .iter_mut()
                    .chain(world.inventory.pack.iter_mut())
                {
                    *slot = Some(ItemStack::new(stone, full));
                }
                world.inventory.hotbar[0] = None;
                world.sync_from_inventory();
            }
            let inventory = inventory_bytes(&world.inventory);
            let packets = world.factory.packets.clone();
            let reserved = world.factory.reserved.clone();
            let (mut blocks, mut chunks) = (Default::default(), Default::default());
            assert!(dismantle(&mut world, pipe, &mut blocks, &mut chunks));
            assert!(world.factory.nodes.contains_key(&pipe));
            assert_eq!(inventory_bytes(&world.inventory), inventory);
            assert_eq!(world.factory.packets, packets);
            assert_eq!(world.factory.reserved, reserved);
            assert!(blocks.is_empty() && chunks.is_empty());
            assert!(
                cells(pipe, size)
                    .into_iter()
                    .all(|p| world.get(p) == Kind::Pipe.block())
            );
            assert_eq!(saved_units(&world), 64);
        }
    }
}
