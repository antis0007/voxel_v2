use super::*;
#[path = "ground_stencil_reference.rs"] mod reference;

fn seams() -> Vec<DVec3> {
    let mut result = vec![DVec3::X, -DVec3::X, DVec3::Y, -DVec3::Y, DVec3::Z, -DVec3::Z];
    for a in [-1.0, 1.0] { for b in [-1.0, 1.0] {
        for c in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            for epsilon in [-1e-12, 0.0, 1e-12] {
                result.push(DVec3::new(a, b + epsilon, c).normalize());
                result.push(DVec3::new(c, a, b + epsilon).normalize());
                result.push(DVec3::new(b + epsilon, c, a).normalize());
            }
        }
    }}
    result
}

#[test]
fn ground_stencil_reuse_matches_original_mesh_sampling_at_seams() {
    for side in [4, 17, 84] { for irregular in [false, true] {
        let mesh = if irregular { topology::Mesh::irregular(side) } else { topology::Mesh::with_side(side) };
        let fields: Vec<Vec<f64>> = (0..4).map(|k| (0..mesh.directions.len()).map(|i| {
            match k {
                0 => (i % 131) as f64 * 0.03125 - 23.0,
                1 => if i % 2 == 0 { -0.0 } else { 0.0 },
                2 => ((i * 37 % 1009) as f64 - 504.0) * 1e12,
                _ => (i % 71) as f64 * 1e-200,
            }
        }).collect()).collect();
        let gradients: Vec<_> = mesh.directions.iter().enumerate().map(|(i, d)| *d * ((i % 13) as f64 - 6.0)).collect();
        for dir in mesh.directions.iter().copied().chain(seams()) {
            let (local, weights) = mesh.stencil(dir);
            let sample = |field: &[f64]| (0..4).map(|i| field[local[i]] * weights[i]).sum::<f64>();
            for field in &fields {
                assert_eq!(sample(field).to_bits(), mesh.sample(field, dir).to_bits(), "side={side} irregular={irregular}");
            }
            // Original gradient path independently recomputes its stencil.
            let (ids, old_weights) = mesh.stencil(dir);
            let old: DVec3 = (0..4).map(|i| gradients[ids[i]] * old_weights[i]).sum();
            let new: DVec3 = (0..4).map(|i| gradients[local[i]] * weights[i]).sum();
            assert_eq!(new.to_array().map(f64::to_bits), old.to_array().map(f64::to_bits));
        }
    }}
}

fn ground_bits(g: geo::Ground) -> Vec<u32> {
    // Exhaustive destructuring also guards against a newly added unchecked field.
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

fn config() -> WorldSampleConfig {
    if let Some(path) = std::env::var_os("AXIOM_GROUND_CACHE") {
        assert!(configure_geology_cache(path.into()));
    }
    let version = std::env::var("AXIOM_GROUND_VERSION").unwrap_or_else(|_| "287".into()).parse().unwrap();
    assert!([275, 277, 278, 279, 283, 286, 287].contains(&version));
    WorldSampleConfig { generator_version: version,
        seed: std::env::var("AXIOM_GROUND_SEED").unwrap_or_else(|_| "74".into()).parse().unwrap(),
        world_type: WorldType::Planet, voxel_scale: engine_core::VoxelScalePreset::OneMeter, planet: None }
}

fn world_probes(world: &World) -> Vec<DVec3> {
    let mut directions = seams();
    directions.extend(world.mesh.directions.iter().step_by((world.mesh.directions.len()/256).max(1)).copied());
    directions
}

#[test]
#[ignore = "full actual-world frozen comparison; isolated process per version/seed"]
fn ground_stencil_reuse_matches_frozen_ground_on_actual_world() {
    let cfg = config(); let world = get(&cfg); let directions = world_probes(&world);
    for dir in directions { for footprint in [0.25, 1.0, 8.0, 32.0, 128.0] {
        let volcanic = world.volcano_shape(dir);
        let expected = world.ground_base_reference(&cfg, dir, footprint, volcanic);
        let actual = world.ground_base(&cfg, dir, footprint, volcanic);
        assert_eq!(ground_bits(actual), ground_bits(expected), "version={} seed={} dir={dir:?} footprint={footprint}", cfg.generator_version,cfg.seed);
    }}
    eprintln!("[ground-stencil-exact] version={} seed={} checksum={:016x}", cfg.generator_version,cfg.seed,world.checksum);
}

