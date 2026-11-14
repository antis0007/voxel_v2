use super::*;
use crate::world::{World, WorldType};
use engine_core::VoxelScalePreset;

#[test]
fn outdoor_plans_deduplicate_waiting_columns_and_preserve_completed_ranges() {
    let mut plans = ColumnPlans::new();
    for c in [(-7, -2), (0, 0), (9, 5)] {
        assert_eq!(plans.range(c), None);
        assert_eq!(plans.range(c), None);
    }
    assert_eq!(plans.backlog.len(), 3);
    plans.ranges.insert((0, 0), (-2, 7, 1));
    assert_eq!(plans.range((0, 0)), Some((-2, 7, 1)));
    plans.retain(|c, _| *c == (0, 0));
    assert_eq!(plans.backlog.len(), 1);
    plans.clear();
    assert!(plans.backlog.is_empty() && plans.ranges.is_empty());
}

#[test]
fn dimension_plan_includes_the_spawn_floor() {
    for kind in [WorldType::Planet, WorldType::Nether, WorldType::Manyfold, WorldType::Hall] {
        let cfg = World::with_seed(kind, VoxelScalePreset::OneMeter, 7).sample_config();
        let spawn = crate::world::suggested_spawn(&cfg).expect("spawn");
        let cx = (spawn.x.floor() as i32).div_euclid(32);
        let cz = (spawn.z.floor() as i32).div_euclid(32);
        let (min_cy, max_cy, _) = super::super::visibility::column_y_chunk_range(cx, cz, &cfg);
        let chunk_y = (spawn.y.floor() as i32).div_euclid(32);
        assert!(
            min_cy <= chunk_y && chunk_y <= max_cy,
            "{kind:?} spawn chunk {chunk_y} outside plan {min_cy}..={max_cy}"
        );
    }
}

#[test]
fn manyfold_columns_stay_on_the_shell() {
    let cfg = World::with_seed(WorldType::Manyfold, VoxelScalePreset::OneMeter, 7).sample_config();
    let spawn = crate::world::suggested_spawn(&cfg).expect("spawn");
    let cx = (spawn.x.floor() as i32).div_euclid(32);
    let cz = (spawn.z.floor() as i32).div_euclid(32);
    let bands = engine_worldgen::manyfold::column_shell_bands(&cfg, cx, cz);
    let chunk_y = (spawn.y.floor() as i32).div_euclid(32);
    let (min_cy, max_cy, _) = super::super::visibility::column_y_chunk_range(cx, cz, &cfg);
    assert!(
        min_cy <= chunk_y && chunk_y <= max_cy,
        "spawn chunk {chunk_y} outside {min_cy}..={max_cy}"
    );
    let mut chunks = 0;
    for (lo, hi) in bands {
        if lo <= hi {
            let metres = cfg.voxel_meters();
            let a = (lo / metres).floor() as i32;
            let b = (hi / metres).floor() as i32;
            chunks += b.div_euclid(32) - a.div_euclid(32) + 1;
        }
    }
    assert!(chunks > 0 && chunks <= 6, "spawn column covers {chunks} chunks");
}
