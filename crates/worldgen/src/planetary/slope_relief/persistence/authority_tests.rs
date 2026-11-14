//! Ignored integration fixture: publish through solve_in, then force disk reuse.
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

fn exposed(world: &World, cfg: &WorldSampleConfig, dir: DVec3) -> bool {
    let volcanic = world.volcano_shape(dir);
    let ground = world.ground_base(cfg, dir, 8.0, volcanic);
    !volcanic.active_caldera && world.edifice_lift(dir) <= 12.0
        && exposure(&ground, true, true, false, true) > 0.1
}

fn probes(world: &World, cfg: &WorldSampleConfig) -> [DVec3; 2] {
    let interior = world.mesh.directions.iter().copied()
        .find(|&dir| {
            let (_, s, t) = Projection::Tangent.from_sphere(dir);
            s.abs().max(t.abs()) < 0.85 && exposed(world, cfg, dir)
        }).expect("fixture needs exposed interior");
    let seam = Face::ALL.into_iter().flat_map(|face| (0..64).map(move |i| {
        Projection::Tangent.to_sphere(face, 1.0, -0.98 + 1.96 * i as f64 / 63.0)
    })).find(|&dir| exposed(world, cfg, dir)).expect("fixture needs exposed face seam");
    [interior, seam]
}

fn sample(world: &World, cfg: &WorldSampleConfig, dirs: &[DVec3]) -> Vec<Vec<u32>> {
    dirs.iter().flat_map(|&dir| [40.0, 24.0, 8.0, 5.0, 1.0].into_iter()
        .map(move |footprint| ground_bits(world.ground(cfg, dir, footprint)))).collect()
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

#[test]
#[ignore = "actual-world authority/disk hits; fresh AXIOM_STAGE_AUTHORITY_ROOT, serial process"]
fn stage_authority_publication_disk_hits_and_mutable_world_bypass() {
    let root = PathBuf::from(std::env::var_os("AXIOM_STAGE_AUTHORITY_ROOT")
        .expect("supply an explicit fresh private cache root"));
    assert!(!root.exists(), "fixture requires a fresh isolated root");
    if let Some(path) = std::env::var_os("AXIOM_STAGE_GEO_CACHE") {
        assert!(crate::planetary::configure_geology_cache(path.into()));
    }
    assert!(crate::planetary::configure_relief_cache(root));
    let cfg = WorldSampleConfig { generator_version: 287,
        seed: std::env::var("AXIOM_STAGE_SEED").unwrap_or_else(|_| "74".into()).parse().unwrap(),
        world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter, planet: None };
    clear_hot();
    // get() delegates to this exact publication function. A private Slot lets the
    // fixture regain unique ownership after dropping the publication root.
    let slot = crate::planetary::Slot { world: OnceLock::new(),
        entry: Arc::new(geo::Entry::default()) };
    let published = crate::planetary::solve_in(&cfg, &slot);
    assert!(published.slope_tiles.persistence.is_some());
    drop(slot);
    let mut world = Arc::try_unwrap(published).unwrap_or_else(|_| panic!("unexpected World owner"));
    let dirs = probes(&world, &cfg);
    let before = statistics();
    let cold = sample(&world, &cfg, &dirs);
    let after_cold = statistics();
    assert!(after_cold.misses > before.misses);
    assert!(after_cold.stored > before.stored);

    // Same immutable World, fresh in-memory cache: all canonical closures must
    // use disk values and preserve fine-parent publication/mixed footprints.
    clear_hot();
    world.slope_tiles = Tiles::default();
    let construction = world.plates.config();
    world.slope_tiles.attach_cache(&construction, world.plates.shape_shelf, world.checksum);
    let warm = sample(&world, &cfg, &dirs);
    let after_warm = statistics();
    assert_eq!(cold, warm);
    assert!(after_warm.hits > after_cold.hits);
    assert_eq!(after_warm.misses, after_cold.misses);
    assert_eq!(after_warm.stored, after_cold.stored);
    let checksum = world.checksum;
    drop(world);

    // Direct build intentionally has no authority even after global configure.
    // Its changed history must not reuse the now-existing disk entry with the
    // same cfg/checksum/key. Direct Stage::solve itself remains uninstrumented.
    clear_hot();
    let mut direct = crate::planetary::build(&cfg, &geo::Progress::default(), true);
    assert_eq!(direct.checksum, checksum);
    assert!(direct.slope_tiles.persistence.is_none());
    direct.history_top.clear();
    direct.slope_tiles = Tiles::default();
    clear_hot();
    let (face, s, t) = Projection::Tangent.from_sphere(dirs[0]);
    let tile = Key { face: face as u8,
        x: (s * atlas::HALF_M / super::super::CORE_M).floor() as i32,
        z: (t * atlas::HALF_M / super::super::CORE_M).floor() as i32 };
    let before_pure = statistics();
    let expected = Stage::solve(&direct, &cfg, tile, 32.0, 8, None, false);
    assert_eq!(statistics(), before_pure);
    let actual = solve_cached(&direct, &cfg, tile, 32.0, 8, None, false);
    same_stage(&actual, &expected);
    let after_bypass = statistics();
    assert_eq!(after_bypass.attempts, before_pure.attempts);
    assert_eq!(after_bypass.hits, before_pure.hits);
    assert_eq!(after_bypass.bypassed, before_pure.bypassed + 1);
    let mut unsupported = cfg; unsupported.generator_version = 286;
    assert!(sealed_context(&unsupported, true, checksum).is_none());
    clear_hot();
    eprintln!("[stage-authority-exact] seed={} cold={after_cold:?} warm={after_warm:?} final={after_bypass:?}",
        cfg.seed);
}
