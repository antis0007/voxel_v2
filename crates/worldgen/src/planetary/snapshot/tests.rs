use super::*;
use engine_core::VoxelScalePreset;
use crate::planetary::{DriftTexel, HistoryBudget, HistoryEvent};

fn cfg() -> WorldSampleConfig {
    let mut planet = engine_voxel::PlanetSettings::default();
    planet.values[0] = 37; planet.values[4] = 61;
    planet.civilization = engine_voxel::CivilizationMode::Wilderness;
    WorldSampleConfig { generator_version: 287, seed: 731, world_type: WorldType::Planet,
        voxel_scale: VoxelScalePreset::OneMeter, planet: Some(planet) }
}

fn frame(index: usize) -> DriftFrame {
    let myr = index as u16 * 3;
    let texels = (0..TEXELS).map(|i| {
        let x = (i % 19) as f32 * 0.01;
        DriftTexel { height_m: x - 10.0, stress: [x, x * 0.2, x * 0.3],
            continental_km: x * 28.0, ocean_age_myr: x * 100.0, sediment_m: x * 0.4,
            runoff_m2: x * 12.0, lineage: (i % 19) as u16, climate: Some([x, x - 5.0]),
            winter_c: Some(-0.0), rupture_m: -x, eruption_m: x * 2.0, weathered_myr: x * 30.0,
            plate: (i % 19) as u16 }
    }).collect();
    DriftFrame { duration_myr: 720, myr, n: 29, texels, sea_m: -0.0,
        annual_range_c: Some([-20.0, 30.0]),
        budget: Some(HistoryBudget { continental_km3: 1.0, continental_error_km3: -0.0,
            continental_eroded_km3: 2.0, continental_created_km3: 3.0, eroded_m3: 4.0,
            sediment_m3: 5.0, sediment_error_m3: -0.25, alongshore_m3: 6.0, plates: 19 }),
        events: if index == 0 { Vec::new() } else {
            vec![HistoryEvent { myr: f64::from(myr) - 0.25, direction: [1.0, -0.0, 0.0],
                metres: -2.25, eruption: index % 2 == 0 }]
        },
        motion: (0..19).map(|k| [k as f32 * 0.001, -0.002, 0.0]).collect() }
}

fn same_frame(a: &DriftFrame, b: &DriftFrame) {
    assert_eq!((a.duration_myr,a.myr,a.n), (b.duration_myr,b.myr,b.n));
    assert_eq!(a.sea_m.to_bits(), b.sea_m.to_bits());
    assert_eq!(a.annual_range_c.unwrap().map(f32::to_bits), b.annual_range_c.unwrap().map(f32::to_bits));
    let budget = |b: HistoryBudget| [b.continental_km3,b.continental_error_km3,
        b.continental_eroded_km3,b.continental_created_km3,b.eroded_m3,b.sediment_m3,
        b.sediment_error_m3,b.alongshore_m3].map(f64::to_bits);
    assert_eq!(budget(a.budget.unwrap()), budget(b.budget.unwrap()));
    assert_eq!(a.budget.unwrap().plates, b.budget.unwrap().plates);
    let texel = |t: &DriftTexel| [t.height_m,t.stress[0],t.stress[1],t.stress[2],
        t.continental_km,t.ocean_age_myr,t.sediment_m,t.runoff_m2,t.climate.unwrap()[0],
        t.climate.unwrap()[1],t.winter_c.unwrap(),t.rupture_m,t.eruption_m,t.weathered_myr].map(f32::to_bits);
    assert_eq!(a.texels.len(), b.texels.len());
    for (a,b) in a.texels.iter().zip(&b.texels) {
        assert_eq!(texel(a),texel(b)); assert_eq!(a.lineage,b.lineage);
    }
    assert_eq!(a.events.len(), b.events.len());
    for (a,b) in a.events.iter().zip(&b.events) {
        assert_eq!(a.myr.to_bits(),b.myr.to_bits());
        assert_eq!(a.direction.map(f32::to_bits),b.direction.map(f32::to_bits));
        assert_eq!(a.metres.to_bits(),b.metres.to_bits()); assert_eq!(a.eruption,b.eruption);
    }
}

#[test]
fn completed_snapshot_roundtrip_preserves_all_state_and_rejects_malformed() {
    let cfg = cfg();
    let plates = Plates::snapshot_fixture(&cfg, false);
    let progress = Progress::default();
    for i in 0..FRAMES { progress.publish_drift(frame(i)); }
    let mut bytes = encode(&plates, &progress).unwrap();
    assert!(bytes.len() < codec::MAX_BYTES);
    let (restored, frames) = decode(&bytes, &cfg, false).unwrap();
    plates.snapshot_assert_equal(&restored);
    for (a,b) in progress.drift_frames().iter().zip(&frames) { same_frame(a,b); }
    let restored_progress = Progress::default();
    for frame in frames { restored_progress.publish_drift(frame); }
    assert_eq!(encode(&restored, &restored_progress).unwrap(), bytes);
    drop(restored); drop(restored_progress);

    assert!(decode(&bytes, &cfg, true).is_err());
    let mut changed = cfg; changed.seed += 1;
    assert!(decode(&bytes, &changed, false).is_err());
    changed = cfg; changed.planet.as_mut().unwrap().values[11] += 1;
    assert!(decode(&bytes, &changed, false).is_err());
    changed = cfg; changed.generator_version = 286;
    assert!(decode(&bytes, &changed, false).is_err());
    for end in [0, 7, 39, bytes.len() - 1] {
        assert!(decode(&bytes[..end], &cfg, false).is_err());
    }
    // Packed header is 39 bytes with a recipe. Plate metadata is 33 bytes.
    let count_at = 39 + 33;
    let saved = bytes[count_at..count_at + 4].to_vec();
    bytes[count_at..count_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode(&bytes, &cfg, false).is_err());
    bytes[count_at..count_at + 4].copy_from_slice(&saved);
    let scalar_at = 39 + 4;
    let saved = bytes[scalar_at..scalar_at + 8].to_vec();
    bytes[scalar_at..scalar_at + 8].copy_from_slice(&f64::NAN.to_bits().to_le_bytes());
    assert!(decode(&bytes, &cfg, false).is_err());
    bytes[scalar_at..scalar_at + 8].copy_from_slice(&saved);
    let flag_at = 39 + 4 + 24;
    let saved = bytes[flag_at]; bytes[flag_at] = 2;
    assert!(decode(&bytes, &cfg, false).is_err()); bytes[flag_at] = saved;
    let parent_at = count_at + 4 + 19 * 56 + 4 + 19 * 24 + 4 + 18 * 4;
    let saved = bytes[parent_at..parent_at + 4].to_vec();
    bytes[parent_at..parent_at + 4].copy_from_slice(&18u32.to_le_bytes());
    assert!(decode(&bytes, &cfg, false).is_err());
    bytes[parent_at..parent_at + 4].copy_from_slice(&saved);
    bytes.push(0);
    assert!(decode(&bytes, &cfg, false).is_err());
}

#[test]
fn snapshot_primitives_and_frame_validation_are_bounded() {
    let mut e = Encoder::new();
    for x in [0.0, -0.0, f64::from_bits(1), 1.0, f64::MAX] { e.f64(x).unwrap(); }
    let bytes = e.finish();
    let mut d = Decoder::new(&bytes).unwrap();
    for x in [0.0, -0.0, f64::from_bits(1), 1.0, f64::MAX] { assert_eq!(d.f64().unwrap().to_bits(), x.to_bits()); }
    d.end().unwrap();
    assert!(d.f64().is_err());
    assert!(Encoder::new().f64(f64::INFINITY).is_err());
    assert!(Encoder::new().f32(f32::NAN).is_err());
    assert!(Decoder::new(&[2]).unwrap().flag().is_err());
    assert!(Decoder::new(&u32::MAX.to_le_bytes()).unwrap().count(0, 40, 24).is_err());
    assert!(Decoder::new(&f64::NEG_INFINITY.to_bits().to_le_bytes()).unwrap().f64().is_err());
    let mut f = frame(0); let mut e = Encoder::new();
    frames::write(&mut e, &f, 0).unwrap();
    let mut bytes = e.finish();
    bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(frames::read(&mut Decoder::new(&bytes).unwrap(), 0).is_err());
    f.texels[0].climate = None;
    assert!(frames::write(&mut Encoder::new(), &f, 0).is_err());
    assert!(encode(&Plates::new(&cfg()), &Progress::default()).is_err());
}
