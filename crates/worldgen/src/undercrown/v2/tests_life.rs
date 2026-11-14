//! Generator 302 loot and spell finds: present only from the gate, solid
//! and aimable at either voxel scale, ids unique, original props untouched.
use super::tests_grid::cfg_scaled;
use super::*;
use crate::undercrown::plan::{Plan, PropKind};
use engine_core::VoxelScalePreset;
use std::collections::HashSet;

fn plan_at(version: u32, seed: u32, scale: VoxelScalePreset) -> (WorldSampleConfig, Plan) {
    let c = WorldSampleConfig { generator_version: version, ..cfg_scaled(seed, scale) };
    let plan = Plan::for_config(&c);
    (c, plan)
}

fn finds(plan: &Plan) -> usize {
    plan.props.iter().filter(|p| matches!(p.kind, PropKind::Cache | PropKind::Lectern)).count()
}

#[test]
fn finds_appear_only_from_the_gate() {
    let (_, before) = plan_at(life::VERSION - 1, 424242, VoxelScalePreset::OneMeter);
    let (_, after) = plan_at(life::VERSION, 424242, VoxelScalePreset::OneMeter);
    assert_eq!(finds(&before), 0);
    assert_eq!(finds(&after), 7);
    // The original props keep their ids and positions.
    for (a, b) in before.props.iter().zip(&after.props) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.pos, b.pos);
    }
}

#[test]
fn prop_ids_are_unique() {
    let (_, plan) = plan_at(life::VERSION, 424242, VoxelScalePreset::OneMeter);
    let mut seen = HashSet::new();
    for p in &plan.props {
        assert!(seen.insert(p.id), "duplicate id for {}", p.label);
    }
}

#[test]
fn every_find_has_solid_model_voxels() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
        for seed in [1, 9001, 424242] {
            let (c, plan) = plan_at(life::VERSION, seed, scale);
            let layout = plan.v2.as_deref().expect("layout");
            let v = c.voxel_meters();
            for prop in plan.props.iter().filter(|p| matches!(p.kind, PropKind::Cache | PropKind::Lectern)) {
                let (lo, hi) = plan.model_box(prop).expect("model box");
                let (a, b) = ((lo / v).floor().as_ivec3(), (hi / v).ceil().as_ivec3());
                let mut solid = 0;
                for y in a.y..b.y {
                    for z in a.z..b.z {
                        for x in a.x..b.x {
                            let m = (IVec3::new(x, y, z).as_vec3() + Vec3::splat(0.5)) * v;
                            solid += usize::from(layout.sample(&c, m, Mech::default()).is_some_and(|b| b.is_solid()));
                        }
                    }
                }
                assert!(solid > 0, "seed {seed} {scale:?}: {} has no voxels", prop.label);
            }
        }
    }
}
