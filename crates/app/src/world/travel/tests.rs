use super::*;
use crate::world::adventure::wildlife::vehicle;
use crate::world::inventory::FoodKind;
use crate::world::{BlockId, ChunkData, WorldType};
use engine_core::VoxelScalePreset;
use std::collections::HashSet;
use std::sync::Arc;

/// One resident chunk of stone below y = 9, with a water strip at x 4..=10, z 8..=12 on top.
fn world() -> World {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 31);
    let mut blocks = Box::new([BlockId::Air; 32 * 32 * 32]);
    for y in 0..9 {
        for z in 0..32 {
            for x in 0..32 {
                blocks[ChunkData::local_index(x, y, z)] = BlockId::Stone;
            }
        }
    }
    for z in 8..=12 {
        for x in 4..=10 {
            blocks[ChunkData::local_index(x, 9, z)] = BlockId::Water;
        }
    }
    w.chunks.insert(IVec3::ZERO, Arc::new(ChunkData {
        modified_bounds: None,
        structural_bounds: None,
        blocks,
        light: Box::new([0; 32 * 32 * 32]),
        torches: HashSet::new(),
    }));
    w
}

#[test]
fn travel_sidecar_round_trips() {
    let mut s = State { day: 7, next_boat: 3, ..State::default() };
    s.used.insert((2, 1), 4);
    s.away.insert(1, 7);
    s.boats.insert(2, Boat { pos: Vec3::new(1.5, -2.0, 300.25), yaw: 0.5 });
    let bytes = sidecar::encode(&s);
    let back = sidecar::decode(&bytes).expect("decodes");
    assert_eq!((back.day, back.next_boat), (7, 3));
    assert_eq!(back.used, s.used);
    assert_eq!(back.away, s.away);
    assert_eq!(back.boats, s.boats);
    let mut bad = bytes.clone();
    bad[10] ^= 1;
    assert!(sidecar::decode(&bad).is_err(), "checksum guards the body");
    assert!(sidecar::decode(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn travel_trades_spend_uses_and_restock_daily() {
    let mut w = world();
    let t = trades::page(0)[0].clone();
    let need = t.give[0];
    for _ in 0..4 {
        w.inventory.try_insert(ItemStack::new(need.item, need.count * 4));
    }
    let start = w.inventory.count_item(t.get.item);
    assert_eq!(trade(&mut w, 0, 99), Err(Refusal::Unknown));
    for _ in 0..t.uses {
        assert_eq!(trade(&mut w, 0, 0), Ok(()));
    }
    assert_eq!(trade(&mut w, 0, 0), Err(Refusal::OutOfStock));
    assert_eq!(w.inventory.count_item(t.get.item), start + t.get.count * u32::from(t.uses));
    assert_eq!(w.inventory.count_item(need.item), need.count * 16 - need.count * u32::from(t.uses));
    w.farming.clock += DAY_TICKS;
    assert_eq!(trade(&mut w, 0, 0), Ok(()), "a new day restocks");
    assert_eq!(w.travel.uses_left(0, 0, &t), t.uses - 1);
    // Another villager's stock is separate.
    assert_eq!(w.travel.uses_left(3, 0, &t), t.uses);
}

#[test]
fn travel_trade_refuses_missing_inputs() {
    let mut w = world();
    assert_eq!(trade(&mut w, 2, 0), Err(Refusal::Missing));
    w.inventory.try_insert(ItemStack::new(Item::Food(FoodKind::RawFish), 6));
    assert_eq!(trade(&mut w, 2, 0), Ok(()));
    assert_eq!(w.inventory.count_item(Item::Food(FoodKind::RawFish)), 0);
}

#[test]
fn travel_heading_follows_camera_yaw() {
    let mut input = Input::default();
    input.forward = true;
    assert!(heading(&input, 0.0).distance(Vec3::NEG_Z) < 1e-5);
    assert!(heading(&input, std::f32::consts::FRAC_PI_2).distance(Vec3::X) < 1e-5);
    input.forward = false;
    input.right = true;
    assert!(heading(&input, 0.0).distance(Vec3::X) < 1e-5);
    input.back = true;
    assert!((heading(&input, 0.0).length() - 1.0).abs() < 1e-5, "diagonals are normalised");
    assert_eq!(heading(&Input::default(), 1.0), Vec3::ZERO);
}

#[test]
fn travel_boat_floats_on_water_and_stops_at_shore() {
    let mut w = world();
    let y = vehicle::float_y(&w, Vec3::new(8.5, 9.6, 10.5)).expect("water column");
    assert!((9.8..10.0).contains(&y), "hull sits just under the surface: {y}");
    assert_eq!(vehicle::float_y(&w, Vec3::new(14.5, 9.2, 10.5)), None, "dry stone has no surface");
    let id = w.travel.add_boat(Boat { pos: Vec3::new(8.5, y, 10.5), yaw: 0.0 }).expect("room");
    vehicle::service(&mut w, Vec3::new(8.5, 10.0, 10.5));
    let ride = Ride::Boat(id);
    let mut seat = Vec3::ZERO;
    for _ in 0..240 {
        seat = vehicle::drive(&mut w, ride, Vec3::X, false, 1.0 / 120.0).expect("boat is loaded");
    }
    assert!((10.0..11.0).contains(&seat.x), "the boat crossed the water and stopped at the shore: {seat}");
    assert!((seat.y - y - 0.15).abs() < 1e-4);
}

#[test]
fn travel_boat_placement_consumes_the_item() {
    let mut w = world();
    w.inventory.hotbar[0] = Some(ItemStack::new(BOAT, 1));
    let eye = Vec3::new(7.5, 11.5, 13.5);
    let forward = (Vec3::new(7.5, 9.5, 10.5) - eye).normalize();
    assert_eq!(interact(&mut w, eye, forward, 0), Use::Taken);
    assert_eq!(w.travel.boats.len(), 1);
    assert_eq!(w.inventory.hotbar[0], None);
}
