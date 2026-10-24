//! Generators 32-35 have shipped: their complexes must sample bit-identically
//! forever. Each digest folds every voxel of a spread of complexes.
use super::*;

fn digest(version: u32, seed: u32, scale: VoxelScalePreset) -> u64 {
    let c = WorldSampleConfig { generator_version: version, ..cfg(seed, scale) };
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for (cell, d) in complexes(&c, 4) {
        h = (h ^ (cell.0 as u32 as u64) << 32 ^ cell.1 as u32 as u64).wrapping_mul(0x100_0000_01b3);
        for b in Grid::new(&c, &d).blocks {
            h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

#[test]
fn shipped_complexes_never_change() {
    let got = [
        digest(32, 424242, VoxelScalePreset::OneMeter),
        digest(35, 424242, VoxelScalePreset::OneMeter),
        digest(35, 7, VoxelScalePreset::HalfMeter),
    ];
    let want: [u64; 3] = [15793008056200210599, 15793008056200210599, 16689061336243203535];
    assert_eq!(got, want, "a shipped generator's complexes changed: gate the change on a new version");
}
