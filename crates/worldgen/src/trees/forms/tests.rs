use super::TreeForm;
use crate::{
    BlockId, CURRENT_GENERATOR_VERSION, IVec3, TreeProfile, WorldSampleConfig, WorldType,
    tree_block, tree_profile_evaluated, trees,
};
use engine_core::VoxelScalePreset;

fn cfg(version: u32) -> WorldSampleConfig {
    WorldSampleConfig {
        planet: None,
        generator_version: version,
        seed: crate::test_world::seed(),
        world_type: WorldType::Hills,
        voxel_scale: VoxelScalePreset::OneMeter,
    }
}

/// A coarse sweep across several climate provinces (about 19 km square).
fn sweep(cfg: &WorldSampleConfig) -> Vec<TreeProfile> {
    let mut trees = Vec::new();
    for cx in (-800..800).step_by(23) {
        for cz in (-800..800).step_by(23) {
            if let Some(tree) = tree_profile_evaluated(cfg, cx, cz, Some(8.0)) {
                trees.push(tree);
            }
        }
    }
    trees
}

#[test]
fn every_v33_form_grows_and_older_versions_stay_classic() {
    let current = sweep(&cfg(CURRENT_GENERATOR_VERSION));
    for form in [TreeForm::Classic, TreeForm::Umbrella, TreeForm::Weeping, TreeForm::HighCrown] {
        let count = current.iter().filter(|t| t.form == form).count();
        assert!(count > 0, "{form:?} never grew in {} trees", current.len());
    }
    assert!(sweep(&cfg(32)).iter().all(|t| t.form == TreeForm::Classic));
}

/// Diagnostic: biome shares over a 19 km square, and how many distinct
/// biomes each 2.4 km province is dominated by.
/// `cargo test -p engine_worldgen biome_census -- --ignored --nocapture`
#[test]
#[ignore]
fn biome_census() {
    use std::collections::BTreeMap;
    for version in [32, CURRENT_GENERATOR_VERSION] {
        let cfg = cfg(version);
        let mut shares = BTreeMap::new();
        let mut dominant = BTreeMap::new();
        for px in -4..4 {
            for pz in -4..4 {
                let mut local = BTreeMap::new();
                for i in 0..6 {
                    for j in 0..6 {
                        let x = px * 2400 + i * 400 + 200;
                        let z = pz * 2400 + j * 400 + 200;
                        let biome = format!("{:?}", crate::landscape::sample(&cfg, x, z).biome);
                        *local.entry(biome.clone()).or_insert(0) += 1;
                        *shares.entry(biome).or_insert(0) += 1;
                    }
                }
                let top = local.into_iter().max_by_key(|(_, n)| *n).map(|(b, _)| b).unwrap_or_default();
                *dominant.entry(top).or_insert(0) += 1;
            }
        }
        println!("v{version} shares {shares:?}");
        println!("v{version} province dominants {dominant:?}");
    }
}

/// The LOD proxy is sized from `crown_reach`/`crown_top`; a leaf outside them
/// pops when the tree changes LOD.
#[test]
fn new_forms_stay_inside_their_crown_bounds() {
    let cfg = cfg(CURRENT_GENERATOR_VERSION);
    let scale = cfg.voxel_meters();
    let mut checked = [0usize; 5];
    for tree in sweep(&cfg) {
        let slot = tree.form as usize;
        if tree.form == TreeForm::Classic || checked[slot] >= 6 {
            continue;
        }
        checked[slot] += 1;
        let reach = trees::crown_reach(&cfg, &tree);
        let top = trees::crown_top(&cfg, &tree);
        let span = (reach / scale).ceil() as i32 + 2;
        let tall = ((top + tree.trunk_h as f32 * scale) / scale).ceil() as i32 + 4;
        for dy in 1..tall {
            for dx in -span..=span {
                for dz in -span..=span {
                    let p = IVec3::new(tree.anchor_x + dx, tree.ground_y + dy, tree.anchor_z + dz);
                    let block = tree_block(&cfg, tree, p);
                    if block != BlockId::Leaves && block != BlockId::PineLeaves {
                        continue;
                    }
                    let horizontal = ((dx * dx + dz * dz) as f32).sqrt() * scale;
                    let above = dy as f32 * scale;
                    assert!(horizontal <= reach + scale, "{:?} leaf at {horizontal} m > reach {reach}", tree.form);
                    assert!(above <= top + scale, "{:?} leaf at {above} m > top {top}", tree.form);
                }
            }
        }
    }
    assert!(checked[1] > 0 && checked[2] > 0 && checked[3] > 0, "forms sampled: {checked:?}");
}
