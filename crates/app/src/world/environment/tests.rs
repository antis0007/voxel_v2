use super::*;
use crate::{
    fluid::VoxelScalePreset,
    world::{World, WorldType},
};

#[test]
fn environment_stacked_surfaces_have_distinct_owners_and_schedules() {
    let mut state = State::default();
    for y in [-8, 0, 12] {
        let p = IVec3::new(-1, y, -33);
        state.patches.insert(
            key(p),
            Patch {
                material: BlockId::Dirt,
                species: 0,
                revision: 0,
                last_tick: 0,
                extent: 1,
                soil: Soil { water: y.unsigned_abs() as u64, ..Default::default() },
            },
        );
        assert!(state.wake(p));
    }
    assert_eq!(state.patches.len(), 3);
    assert_eq!(state.due.len(), 3);
    state.patches.get_mut(&(-1, 0, -33)).unwrap().soil.water = 500;
    assert_eq!(state.patches[&(-1, 12, -33)].soil.water, 12);
}

#[test]
fn environment_reload_retains_fractional_credit_and_due_work() {
    let mut state = State { fixed_steps: 600, tick: 3, target_tick: 5, ..Default::default() };
    state.patches.insert(
        (-1, 2, -3),
        Patch {
            material: BlockId::Clay,
            species: 2,
            revision: 7,
            last_tick: 3,
            extent: 1,
            soil: Soil {
                water: 2_000,
                infiltration_credit: 51,
                flux_carry: 7,
                nutrient: 8,
                biomass: 9,
                litter: 11,
                ..Default::default()
            },
        },
    );
    state.sediment.insert((-1, 3, -3), 17.into());
    state.wake(IVec3::new(-1, 2, -3));
    let mut bytes = Vec::new();
    state.encode(&mut bytes);
    let mut words = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let loaded = State::decode(
        &mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")),
        VoxelScalePreset::QuarterMeter,
    )
    .unwrap();
    assert_eq!(loaded.patches[&(-1, 2, -3)].soil, state.patches[&(-1, 2, -3)].soil);
    assert_eq!(loaded.due, state.due);
    assert_eq!(loaded.sediment, state.sediment);
    let mut again = Vec::new();
    loaded.encode(&mut again);
    assert_eq!(bytes, again);
}

/// A budgeted service pass can leave events at or behind `tick`; a save or a
/// multiplayer image taken then must still load.
#[test]
fn environment_reload_keeps_overdue_events() {
    let mut state = State { fixed_steps: 600, tick: 5, target_tick: 5, ..Default::default() };
    assert!(state.schedule((0, 1, 0), 3) && state.schedule((4, 1, 0), 5) && state.schedule((8, 1, 0), 9));
    let mut bytes = Vec::new();
    state.encode(&mut bytes);
    let mut words = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let loaded = State::decode(
        &mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")),
        VoxelScalePreset::QuarterMeter,
    )
    .unwrap();
    assert_eq!(loaded.due, state.due);

    // The same event twice is still corrupt. Events close the record: a count,
    // then a 12-byte key and an 8-byte due tick each.
    const EVENT: usize = 20;
    let mut doubled = bytes.clone();
    let last = doubled[doubled.len() - EVENT..].to_vec();
    let count_at = doubled.len() - 3 * EVENT - 4;
    doubled.extend_from_slice(&last);
    doubled[count_at..count_at + 4].copy_from_slice(&4u32.to_le_bytes());
    let mut words = doubled.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    assert!(State::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")), VoxelScalePreset::QuarterMeter).is_err());
}

#[test]
fn environment_blocked_water_pair_preserves_both_inventories() {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 92);
    let a = IVec3::new(0, 32, 0);
    let b = a + IVec3::X;
    w.set(a, BlockId::Water);
    w.set(b, BlockId::Stone);
    w.environment.sediment.insert(key(a), 1200.into());
    assert!(!w.apply_water_pair(a, 7, b, 1, 1));
    assert_eq!(w.water_volume(a), 8);
    assert_eq!(w.environment.sediment_at(a), 1200);
    w.set(b, BlockId::Air);
    assert!(w.apply_water_pair(a, 4, b, 4, 4));
    assert_eq!(w.water_volume(a) + w.water_volume(b), 8);
    assert_eq!(w.environment.sediment_at(a), 600);
    assert_eq!(w.environment.sediment_at(b), 600);
}

#[test]
fn environment_capacity_rejects_transfer_without_consuming_source() {
    let mut state = State::default();
    for x in 0..MAX_SEDIMENT as i32 {
        state.sediment.insert((x, 0, 0), 1.into());
    }
    let a = IVec3::ZERO;
    let b = IVec3::new(-1, 0, 0);
    assert!(!state.can_transport(a, b, &FluidImpact::default()));
    assert_eq!(state.sediment_at(a), 1);
    assert_eq!(state.sediment_at(b), 0);
}

#[test]
fn environment_legacy_world_does_not_allocate_new_state() {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 92);
    w.generator_version = engine_core::environment::ENVIRONMENT_GENERATOR - 1;
    w.set(IVec3::new(0, 32, 0), BlockId::Water);
    assert!(w.environment.due.is_empty());
    assert!(w.environment.patches.is_empty());
}

#[test]
fn environment_refinement_preserves_all_stocks_and_negative_coordinates() {
    let mut state = State { cells_per_metre: 4, ..Default::default() };
    let original = Soil {
        water: 99_999,
        nutrient: 10_240,
        biomass: 123,
        litter: 456,
        infiltration_credit: 1_001,
        flux_carry: 9,
        plant_nutrient: 24,
        litter_nutrient: 11,
        erosion_work: 101,
        growth_credit: 109_001,
    };
    state.patches.insert(
        (-4, 0, -4),
        Patch { material: BlockId::Dirt, soil: original, species: 0, revision: 0, last_tick: 0, extent: 4 },
    );
    let p = IVec3::new(-1, 1, -2);
    assert_eq!(state.owner(p), (-4, 0, -4));
    assert!(state.can_refine(&[p]));
    state.refine(p);
    assert_eq!(state.owner(p), key(p));
    assert_eq!(state.patches.len(), 64);
    assert_eq!(state.patches.values().map(|p| p.soil.water).sum::<u64>(), original.water);
    assert_eq!(state.patches.values().map(|p| p.soil.nutrient).sum::<u64>(), original.nutrient);
    assert_eq!(
        state.patches.values().map(|p| p.soil.infiltration_credit).sum::<u64>(),
        original.infiltration_credit
    );
    assert_eq!(
        state.patches.values().map(|p| p.soil.biomass + p.soil.litter).sum::<u64>(),
        original.biomass + original.litter
    );
    assert_eq!(state.due.len(), 64);
}

#[test]
fn environment_mixture_admission_counts_grain_volume_and_retained_moisture() {
    let mut state = State::default();
    let a = IVec3::ZERO;
    let b = IVec3::X;
    state.sediment.insert(key(b), 1_600_000_000.into());
    assert_eq!(state.free_water_capacity(b), 3);
    assert!(state.transfer_fits(a, b, 8, 0, 3));
    assert!(!state.transfer_fits(a, b, 8, 0, 4));
}
