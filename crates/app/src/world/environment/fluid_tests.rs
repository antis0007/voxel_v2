use super::*;
use crate::world::{World, WorldType};
use engine_core::VoxelScalePreset;

#[test]
fn environment_fractional_water_survives_transport_and_phase_round_trip() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut w = World::with_seed(WorldType::Superflat, scale, 92);
        let a = IVec3::new(8, 100, 8);
        let b = a + IVec3::X;
        w.set(a, BlockId::Air);
        w.set(b, BlockId::Air);
        let amount = engine_core::environment::cell_volume(scale) / 3 + 17;
        assert!(w.set_water_microlitres(a, amount));
        let units = w.water_volume(a);
        assert!(w.apply_water_pair(a, units - 1, b, 1, 1));
        assert_eq!(w.water_microlitres(a) + w.water_microlitres(b), amount);
        let before = w.water_microlitres(a);
        w.set(a, BlockId::Ice);
        assert_eq!(w.get(a), BlockId::Water, "fractional ice cannot become a full inventory item");
        w.set(a, BlockId::Steam);
        w.set(a, BlockId::Water);
        assert_eq!(w.water_microlitres(a), before);
        let units = w.water_volume(a);
        let target = w.water_volume(b);
        assert!(w.apply_water_pair(a, 0, b, target + units, units));
        assert_eq!(w.water_microlitres(a), 0);
        assert_eq!(w.water_microlitres(b), amount);
    }
}

#[test]
fn environment_fractional_rejection_preserves_source_and_encoding() {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 93);
    let p = IVec3::new(8, 100, 8);
    w.set(p, BlockId::Air);
    assert!(w.set_water_microlitres(p, 12345));
    let encoding = w.environment.water_deficit.clone();
    w.set_water_volume(p, 0);
    assert_eq!(w.water_microlitres(p), 12345);
    assert_eq!(w.environment.water_deficit, encoding);
    assert!(!w.set_water_microlitres(p, 1_000_000_001));
    assert_eq!(w.water_microlitres(p), 12345);
    assert!(w.set_water_microlitres(p, 0));
    assert!(!w.environment.water_deficit.contains_key(&key(p)));
}

#[test]
fn environment_fraction_only_changes_reach_render_snapshots_and_mesh_height() {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 94);
    let p = IVec3::new(8, 100, 8);
    w.set(p, BlockId::Air);
    assert!(w.set_water_microlitres(p, 990_000_000));
    let old = w.snapshot();
    assert!(w.set_water_microlitres(p, 980_000_000));
    let new = w.snapshot();
    assert_eq!(w.water_volume(p), 8);
    assert_eq!(old.water_levels.deficit(p), 10_000_000);
    assert_eq!(new.water_levels.deficit(p), 20_000_000);
    assert!(
        new.water_levels
            .changed_chunks(&old.water_levels)
            .any(|(c, _, _)| c == crate::world::chunk_coord_from_world_block(p))
    );
    let mut source = crate::mesh::GeneratedChunk::new(IVec3::ZERO, Vec::new());
    source.water_deficit.insert(p, new.water_levels.deficit(p));
    assert!((source.water_fill(p) - w.water_fill(p)).abs() < 1e-6);
}

#[test]
fn environment_fraction_limit_does_not_block_integral_water() {
    let mut state = State::default();
    for x in 0..MAX_FRACTIONS as i32 {
        state.water_deficit.insert((x, 0, 0), 1);
    }
    let fresh = IVec3::new(-1, 0, 0);
    assert!(state.can_encode(&[(fresh, state.quantum() * 8)]));
    assert!(state.can_encode(&[(fresh, 0)]));
    assert!(!state.can_encode(&[(fresh, 1)]));
    assert!(state.can_encode(&[(IVec3::ZERO, 1)]));
    assert_eq!(state.water_deficit.len(), MAX_FRACTIONS);
}
