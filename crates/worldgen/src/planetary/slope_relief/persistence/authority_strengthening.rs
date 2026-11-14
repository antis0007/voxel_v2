//! Full actual Stage fields and fine-first publication, in a dedicated process.
use super::*;
use super::super::{exposure, Key, Stage, Tiles, HOT};
use crate::planetary::{geo, World, WorldSampleConfig, WorldType};
use engine_core::{VoxelScalePreset, cube_sphere::{Face, Projection, atlas}};
use glam::DVec3;

fn clear_hot() { HOT.with(|hot| hot.borrow_mut().clear()); }

fn ground_bits(g: geo::Ground) -> Vec<u32> {
    let geo::Ground { top_m, slope, temperature, moisture, sediment, placer, river_gravel,
        river_bedrock, maat_c, permafrost, water_m, riparian, delta, delta_mud,
        suspended, intertidal, scree_m, deposit_m, deposit_bed_slope, deposit_bed_top, outlook_clearing, .. } = g;
    let mut bits = [top_m, slope, temperature, moisture, sediment, placer, maat_c,
        riparian, delta, delta_mud, suspended, intertidal, scree_m, deposit_m, deposit_bed_slope, deposit_bed_top, outlook_clearing]
        .map(f32::to_bits).to_vec();
    bits.extend([u32::from(river_gravel), u32::from(river_bedrock), u32::from(permafrost),
        u32::from(water_m.is_some()), water_m.unwrap_or(0.0).to_bits()]);
    bits
}

fn same_stage(a: &Stage, b: &Stage) {
    let Stage { grid: ag, rock_delta: ar, soil: asoil, base: ab } = a;
    let Stage { grid: bg, rock_delta: br, soil: bsoil, base: bb } = b;
    assert!(codec::same_grid(ag, bg));
    let bits = |v: &[f32]| v.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(ar), bits(br));
    assert_eq!(bits(asoil), bits(bsoil));
    assert_eq!(ab.as_ref().map(|v| bits(v)), bb.as_ref().map(|v| bits(v)));
}

fn exposed(world: &World, cfg: &WorldSampleConfig, dir: DVec3) -> bool {
    let volcanic = world.volcano_shape(dir);
    let ground = world.ground_base(cfg, dir, 8.0, volcanic);
    !volcanic.active_caldera && world.edifice_lift(dir) <= 12.0
        && exposure(&ground, true, true, false, true) > 0.1
}

fn probes(world: &World, cfg: &WorldSampleConfig) -> [DVec3; 2] {
    let interior = world.mesh.directions.iter().copied().find(|&dir| {
        let (_, s, t) = Projection::Tangent.from_sphere(dir);
        s.abs().max(t.abs()) < 0.85 && exposed(world, cfg, dir)
    }).expect("fixture needs exposed interior");
    let seam = Face::ALL.into_iter().flat_map(|face| (0..64).map(move |i| {
        Projection::Tangent.to_sphere(face, 1.0, -0.98 + 1.96 * i as f64 / 63.0)
    })).find(|&dir| exposed(world, cfg, dir)).expect("fixture needs exposed seam");
    [interior, seam]
}

fn reset(world: &mut World, authority: bool) {
    clear_hot();
    world.slope_tiles = Tiles::default();
    if authority {
        let construction = world.plates.config();
        world.slope_tiles.attach_cache(&construction, world.plates.shape_shelf, world.checksum);
        assert!(world.slope_tiles.persistence.is_some());
    }
}

fn full_chain(world: &World, cfg: &WorldSampleConfig, tile: Key) {
    let mut pure: Vec<Stage> = Vec::new();
    let mut cold: Vec<Stage> = Vec::new();
    for (spacing, passes) in [(32.0, 8), (16.0, 7), (8.0, 8), (4.0, 5)] {
        let before = statistics();
        pure.push(Stage::solve(world, cfg, tile, spacing, passes, pure.last(), false));
        assert_eq!(statistics(), before, "pure solve touched cache at {spacing}m");
        cold.push(solve_cached(world, cfg, tile, spacing, passes, cold.last(), false));
        same_stage(pure.last().unwrap(), cold.last().unwrap());
        let after = statistics();
        assert_eq!(after.attempts, before.attempts + 1);
        assert_eq!(after.misses, before.misses + 1, "not cold at {spacing}m");
        assert_eq!(after.stored, before.stored + 1);
        assert_eq!(after.hits, before.hits);
    }
    let mut warm: Vec<Stage> = Vec::new();
    for (level, (spacing, passes)) in [(32.0, 8), (16.0, 7), (8.0, 8), (4.0, 5)].into_iter().enumerate() {
        let before = statistics();
        warm.push(solve_cached(world, cfg, tile, spacing, passes, warm.last(), false));
        same_stage(&pure[level], warm.last().unwrap());
        let after = statistics();
        assert_eq!(after.attempts, before.attempts + 1);
        assert_eq!(after.hits, before.hits + 1, "not a hit at {spacing}m");
        assert_eq!(after.misses, before.misses);
        assert_eq!(after.stored, before.stored);
    }
}

fn fine_first(world: &World, cfg: &WorldSampleConfig, dirs: &[DVec3], require_hits: bool) -> Vec<Vec<u32>> {
    let mut result = Vec::new();
    for &dir in dirs {
        let before = statistics();
        result.push(ground_bits(world.ground(cfg, dir, 1.0)));
        let fine = statistics();
        if require_hits {
            assert!(fine.hits >= before.hits + 4, "fine-first must restore a complete parent chain");
            assert_eq!(fine.misses, before.misses);
        }
        for footprint in [8.0, 24.0] {
            result.push(ground_bits(world.ground(cfg, dir, footprint)));
            assert_eq!(statistics().attempts, fine.attempts,
                "fine-first did not publish its detail parent for {footprint}m");
        }
    }
    result
}

#[test]
#[ignore = "actual full-grid authority; fresh AXIOM_STAGE_AUTHORITY_ROOT, run alone"]
fn stage_authority_full_chains_mixed_query_scale_and_fine_first() {
    let root = PathBuf::from(std::env::var_os("AXIOM_STAGE_AUTHORITY_ROOT")
        .expect("supply an explicit fresh private cache root"));
    assert!(!root.exists(), "fixture requires a fresh isolated root");
    if let Some(path) = std::env::var_os("AXIOM_STAGE_GEO_CACHE") {
        assert!(crate::planetary::configure_geology_cache(path.into()));
    }
    assert!(crate::planetary::configure_relief_cache(root));
    let construction = WorldSampleConfig { generator_version: 287,
        seed: std::env::var("AXIOM_STAGE_SEED").unwrap_or_else(|_| "74".into()).parse().unwrap(),
        world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter, planet: None };
    clear_hot();
    let slot = crate::planetary::Slot { world: OnceLock::new(),
        entry: Arc::new(geo::Entry::default()) };
    let published = crate::planetary::solve_in(&construction, &slot);
    assert!(published.slope_tiles.persistence.is_some());
    drop(slot);
    let mut world = Arc::try_unwrap(published).unwrap_or_else(|_| panic!("unexpected World owner"));
    assert_eq!(world.plates.config().voxel_scale, VoxelScalePreset::OneMeter);
    let dirs = probes(&world, &construction);
    let (face, s, t) = Projection::Tangent.from_sphere(dirs[0]);
    let tile = Key { face: face as u8,
        x: (s * atlas::HALF_M / super::super::CORE_M).floor() as i32,
        z: (t * atlas::HALF_M / super::super::CORE_M).floor() as i32 };
    // Same completed World and authority, different complete requested config.
    // Quarter must miss despite an otherwise identical already-stored chain.
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
        let mut query = construction; query.voxel_scale = scale;
        full_chain(&world, &query, tile);
    }
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
        let mut query = construction; query.voxel_scale = scale;
        reset(&mut world, false);
        let pure = fine_first(&world, &query, &dirs, false);
        reset(&mut world, true);
        let cold = fine_first(&world, &query, &dirs, false);
        assert_eq!(pure, cold);
        reset(&mut world, true);
        let before = statistics();
        let warm = fine_first(&world, &query, &dirs, true);
        let after = statistics();
        assert_eq!(pure, warm);
        assert!(after.hits > before.hits);
        assert_eq!(after.misses, before.misses);
        assert_eq!(after.stored, before.stored);
        assert_eq!(after.invalid, before.invalid);
        assert_eq!(after.read_errors, before.read_errors);
    }
    clear_hot();
    eprintln!("[stage-authority-full-grid-exact] seed={} stats={:?}", construction.seed, statistics());
}
