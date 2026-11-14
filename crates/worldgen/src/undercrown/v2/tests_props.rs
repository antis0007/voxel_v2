//! Generator 292 prop models: every prop has voxels a player can see and
//! aim at, at either voxel scale, and each kind has its own shape.
use super::tests_grid::cfg_scaled;
use super::*;
use crate::undercrown::plan::{Plan, Prop, PropKind};
use engine_core::VoxelScalePreset;

const SEEDS: [u32; 5] = [1, 7, 9001, 31337, 424242];

fn model_blocks(c: &WorldSampleConfig, plan: &Plan, prop: &Prop) -> Vec<B> {
    let (Some(layout), Some((lo, hi))) = (plan.v2.as_deref(), plan.model_box(prop)) else {
        return Vec::new();
    };
    let v = c.voxel_meters();
    let (a, b) = ((lo / v).floor().as_ivec3(), (hi / v).ceil().as_ivec3());
    let mut blocks = Vec::new();
    for y in a.y..b.y {
        for z in a.z..b.z {
            for x in a.x..b.x {
                let m = (IVec3::new(x, y, z).as_vec3() + Vec3::splat(0.5)) * v;
                blocks.extend(layout.sample(c, m, Mech::default()));
            }
        }
    }
    blocks
}

fn at_gate(seed: u32, scale: VoxelScalePreset) -> (WorldSampleConfig, Plan) {
    let c = WorldSampleConfig { generator_version: props::VERSION, ..cfg_scaled(seed, scale) };
    let plan = Plan::for_config(&c);
    (c, plan)
}

#[test]
fn every_prop_has_solid_model_voxels() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
        for seed in SEEDS {
            let (c, plan) = at_gate(seed, scale);
            assert!(plan.v2.is_some(), "seed {seed}: no layout");
            for prop in &plan.props {
                let blocks = model_blocks(&c, &plan, prop);
                assert!(blocks.iter().any(|b| b.is_solid()), "seed {seed} {scale:?}: {} ({:?}) has no voxels: {blocks:?}", prop.label, prop.kind);
            }
        }
    }
}

/// Plinths show a lit crystal cap. (The observatory lens keeps the flooded
/// gallery's authored mount, which is sampled ahead of every feature.)
#[test]
fn plinths_carry_a_crystal_cap() {
    let (c, plan) = at_gate(424242, VoxelScalePreset::OneMeter);
    for prop in plan.props.iter().filter(|p| p.kind == PropKind::Plinth) {
        assert!(model_blocks(&c, &plan, prop).contains(&B::Crystal), "{} has no crystal cap", prop.label);
    }
}

#[test]
fn older_generators_keep_the_pedestal_box() {
    let c = WorldSampleConfig { generator_version: props::VERSION - 1, ..cfg_scaled(424242, VoxelScalePreset::OneMeter) };
    let plan = Plan::for_config(&c);
    for prop in &plan.props {
        assert_eq!(plan.model_box(prop), Some(prop.model_box()));
    }
}
