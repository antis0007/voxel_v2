//! Generators 32-35 have shipped: the Undercrown must sample bit-identically
//! forever. Each digest folds every voxel of the layout's box.
use super::tests_grid::plan_scaled;
use super::*;
use engine_core::VoxelScalePreset;

fn digest(version: u32, seed: u32, scale: VoxelScalePreset) -> u64 {
    let (c, _) = plan_scaled(seed, scale);
    let c = WorldSampleConfig { generator_version: version, ..c };
    let plan = crate::undercrown::plan::Plan::for_config(&c);
    let Some(l) = plan.v2.as_deref() else { return 0 };
    let v = c.voxel_meters();
    let (lo, hi) = ((l.min / v).floor().as_ivec3(), (l.max / v).ceil().as_ivec3());
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                let m = (IVec3::new(x, y, z).as_vec3() + Vec3::splat(0.5)) * v;
                let b = l.sample(&c, m, Mech::default()).map_or(0xffff, |b| b as u64);
                h = (h ^ b).wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    h
}

#[test]
fn the_shipped_undercrown_never_changes() {
    let got = [digest(32, 424242, VoxelScalePreset::OneMeter), digest(35, 7, VoxelScalePreset::OneMeter)];
    let want: [u64; 2] = [12982446922351527824, 10515500220958884070];
    assert_eq!(got, want, "a shipped generator's Undercrown changed: gate the change on a new version");
}
