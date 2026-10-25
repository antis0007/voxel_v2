use super::tests::uniform;
use super::*;
use engine_core::VoxelScalePreset;

#[test]
fn environment_cost_fixed_and_growing_contact_sets() {
    for count in [1, 32, 256] {
        let mut w = uniform(VoxelScalePreset::QuarterMeter, B::Stone);
        let cells: Vec<_> = (0..count).map(|i| IVec3::new(2 + (i % 16) * 2, 8, 2 + (i / 16))).collect();
        let mut initial = 0;
        for &p in &cells {
            w.set(p, B::Dirt);
            w.set(p + IVec3::Y, B::Air);
            w.set_water_volume(p + IVec3::Y, 4);
            initial += w.water_microlitres(p + IVec3::Y);
        }
        let mut blocks = HashSet::new();
        let mut chunks = HashSet::new();
        let started = std::time::Instant::now();
        for tick in 1..=60 {
            for &p in &cells {
                soil::process(&mut w, p, tick, &mut blocks, &mut chunks);
            }
        }
        let elapsed = started.elapsed();
        let final_water: u64 = cells.iter().map(|&p| w.water_microlitres(p + IVec3::Y)).sum::<u64>()
            + w.environment.patches.values().map(|p| p.soil.water).sum::<u64>();
        assert_eq!(initial, final_water);
        assert!(w.environment.reserved_bytes() <= env::MAX_STATE_BYTES);
        println!(
            "environment-cost contacts={count} simulated_seconds=60 process_steps={} wall_ms={:.3} state_reservation_bytes={} dirty_chunks={}",
            count * 60,
            elapsed.as_secs_f64() * 1000.0,
            w.environment.reserved_bytes(),
            chunks.len()
        );
    }
}
