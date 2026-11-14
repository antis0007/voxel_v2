use super::{codec, key};
use super::super::{Grid, Key, Stage};
use crate::{WorldSampleConfig, WorldType};
use engine_core::VoxelScalePreset;

fn fixture(n: usize, spacing: f64, base: bool) -> Stage {
    let values = |salt: usize| (0..n*n).map(|i| match (i + salt) % 4 {
        0 => -0.0, 1 => 0.0, 2 => (i as f32) * 0.03125 - 9.0,
        _ => f32::from_bits(1 + i as u32),
    }).collect();
    Stage { grid: Grid { n, cell_m: spacing, min_m: [-128.0, 320.0] },
        rock_delta: values(0), soil: values(1), base: base.then(|| values(2)) }
}

fn same_stage(a: &Stage, b: &Stage) {
    let Stage { grid: ag, rock_delta: ar, soil: asoil, base: ab } = a;
    let Stage { grid: bg, rock_delta: br, soil: bsoil, base: bb } = b;
    assert!(codec::same_grid(ag, bg));
    let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(ar), bits(br));
    assert_eq!(bits(asoil), bits(bsoil));
    assert_eq!(ab.as_ref().map(|v| bits(v)), bb.as_ref().map(|v| bits(v)));
}

#[test]
fn stage_codec_roundtrips_all_levels_and_signed_zero() {
    for (n, spacing) in [(22,32.0), (44,16.0), (88,8.0), (176,4.0)] {
        for base in [false, true] {
            let original = fixture(n, spacing, base);
            let bytes = codec::encode(&original).unwrap();
            let restored = codec::decode(&bytes, &original.grid, base).unwrap();
            same_stage(&original, &restored);
            assert_eq!(restored.rock_delta[0].to_bits(), (-0.0_f32).to_bits());
        }
    }
}

#[test]
fn stage_codec_rejects_malformed_and_wrong_identity() {
    let mut original = fixture(22, 32.0, true);
    let bytes = codec::encode(&original).unwrap();
    let bad = |b: &[u8]| assert!(codec::decode(b, &original.grid, true).is_err());
    for end in [0, 7, 8, 35, 36, 37, 40, bytes.len()-1] { bad(&bytes[..end]); }
    let mut v = bytes.clone(); v.push(0); bad(&v);
    let mut v = bytes.clone(); v[0] ^= 1; bad(&v);
    let mut v = bytes.clone(); v[8..12].copy_from_slice(&u32::MAX.to_le_bytes()); bad(&v);
    let mut v = bytes.clone(); v[12..20].copy_from_slice(&f64::NAN.to_bits().to_le_bytes()); bad(&v);
    let mut v = bytes.clone(); v[36] = 2; bad(&v);
    let mut v = bytes.clone(); v[37..41].copy_from_slice(&u32::MAX.to_le_bytes()); bad(&v);
    for nonfinite in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut v = bytes.clone(); v[41..45].copy_from_slice(&nonfinite.to_bits().to_le_bytes()); bad(&v);
    }
    bad(&vec![0; codec::MAX_BYTES + 1]);
    assert!(codec::decode(&bytes, &original.grid, false).is_err());
    let mut wrong = original.grid; wrong.min_m[0] += 448.0;
    assert!(codec::decode(&bytes, &wrong, true).is_err());
    original.rock_delta[0] = f32::NAN;
    assert!(codec::encode(&original).is_err());
    original.rock_delta[0] = 0.0; original.soil.pop();
    assert!(codec::encode(&original).is_err());
}

fn cfg() -> WorldSampleConfig {
    WorldSampleConfig { generator_version: 287, seed: 74, world_type: WorldType::Planet,
        voxel_scale: VoxelScalePreset::OneMeter, planet: Some(engine_voxel::PlanetSettings::default()) }
}
fn tile() -> Key { Key { face: 0, x: -3, z: 7 } }
fn request(world: &[u8], query: &[u8], checksum: u64, parent: Option<&Stage>) -> Vec<u8> {
    key::from_identities(world, query, checksum, tile(), 32.0, 8, false, parent).unwrap().bytes
}

#[test]
fn stage_key_binds_both_complete_configs_and_shelf() {
    let cfg = cfg();
    let identity = |c: &WorldSampleConfig, shelf| crate::planetary::geology_cache::identity(c, shelf).unwrap();
    let base = identity(&cfg, true);
    let expected = request(&base, &base, 100, None);
    let mut variants = Vec::new();
    let mut c = cfg; c.seed ^= 1; variants.push(c);
    for scale in [VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut c = cfg; c.voxel_scale = scale; variants.push(c);
    }
    let mut c = cfg; c.planet = None; variants.push(c);
    for i in 0..cfg.planet.unwrap().values.len() {
        let mut c = cfg; c.planet.as_mut().unwrap().values[i] += 1; variants.push(c);
    }
    let mut c = cfg; c.planet.as_mut().unwrap().civilization = engine_voxel::CivilizationMode::Wilderness;
    variants.push(c);
    for c in variants {
        let changed = identity(&c, true);
        assert_ne!(request(&changed, &base, 100, None), expected, "construction {c:?}");
        assert_ne!(request(&base, &changed, 100, None), expected, "query {c:?}");
    }
    let other_shelf = identity(&cfg, false);
    assert_ne!(request(&other_shelf, &base, 100, None), expected);
    assert_ne!(request(&base, &other_shelf, 100, None), expected);
    for version in [278, 279, 280, 283, 286, 288] {
        let mut c = cfg; c.generator_version = version;
        assert!(crate::planetary::geology_cache::identity(&c, true).is_none());
        assert!(key::request(&base, 100, true, &c, tile(), 32.0, 8, false, None).is_none());
    }
    for kind in [WorldType::Superflat, WorldType::Hills, WorldType::Ridged,
        WorldType::Nether, WorldType::Manyfold, WorldType::Hall] {
        let mut c = cfg; c.world_type = kind;
        assert!(crate::planetary::geology_cache::identity(&c, true).is_none());
    }
    let mut c = cfg; c.planet.as_mut().unwrap().values[0] = 101;
    assert!(crate::planetary::geology_cache::identity(&c, true).is_none());
}

#[test]
fn stage_key_binds_final_world_request_and_complete_parent() {
    let base = request(b"world-executable-schema", b"query-executable-schema", 100, None);
    assert_ne!(request(b"changed-world-executable-schema", b"query-executable-schema", 100, None), base);
    assert_ne!(request(b"world-executable-schema", b"changed-query-executable-schema", 100, None), base);
    assert_ne!(request(b"world-executable-schema", b"query-executable-schema", 101, None), base);
    for k in [Key { face: 1, ..tile() }, Key { x: 4, ..tile() }, Key { z: -8, ..tile() }] {
        let changed = key::from_identities(b"world-executable-schema", b"query-executable-schema",
            100, k, 32.0, 8, false, None).unwrap();
        assert_ne!(changed.bytes, base);
    }
    for (spacing, passes, distant) in [(16.0,7,false), (8.0,8,false), (4.0,5,false), (32.0,8,true)] {
        assert_ne!(key::from_identities(b"world-executable-schema", b"query-executable-schema",
            100, tile(), spacing, passes, distant, None).unwrap().bytes, base);
    }
    assert!(key::from_identities(b"w", b"q", 1, tile(), 32.0, 7, false, None).is_none());
    assert!(key::from_identities(b"w", b"q", 1, Key { face: 6, ..tile() }, 32.0, 8, false, None).is_none());
    let parent = fixture(22, 32.0, true);
    let expected = request(b"w", b"q", 1, Some(&parent));
    assert_ne!(request(b"w", b"q", 1, None), expected);
    for change in 0..7 {
        let mut p = fixture(22, 32.0, true);
        match change {
            0 => p.rock_delta[0] = 0.0, // -0 -> +0 must change the key
            1 => p.soil[0] = 1.0,
            2 => p.base.as_mut().unwrap()[0] = 1.0,
            3 => p.base = None,
            4 => p.grid.min_m[0] += 448.0,
            5 => p.grid.min_m[1] += 448.0,
            _ => p = fixture(44, 16.0, true),
        }
        assert_ne!(request(b"w", b"q", 1, Some(&p)), expected);
    }
}
