//! Versioned production-history regression checks.
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, planetary::{self, DriftFrame}};
use std::sync::{Arc, OnceLock};

fn config(version: u32, scale: VoxelScalePreset) -> WorldSampleConfig {
    WorldSampleConfig { seed: 74, generator_version: version, world_type: WorldType::Planet,
        planet: None, voxel_scale: scale }
}

fn legacy() -> &'static Vec<Arc<DriftFrame>> {
    static VALUE: OnceLock<Vec<Arc<DriftFrame>>> = OnceLock::new();
    VALUE.get_or_init(|| planetary::review::history::frames(&config(283, VoxelScalePreset::OneMeter)))
}

fn fine() -> &'static Vec<Arc<DriftFrame>> {
    static VALUE: OnceLock<Vec<Arc<DriftFrame>>> = OnceLock::new();
    VALUE.get_or_init(|| planetary::review::history::frames(&config(planetary::FINE_HISTORY_GENERATOR, VoxelScalePreset::OneMeter)))
}

fn digest(frames: &[Arc<DriftFrame>]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for frame in frames {
        for t in &frame.texels {
            for v in [t.height_m,t.sediment_m,t.runoff_m2,t.continental_km,t.ocean_age_myr,t.weathered_myr] {
                hash = (hash ^ u64::from(v.to_bits())).wrapping_mul(0x100000001b3);
            }
        }
    }
    hash
}

#[test]
fn legacy_history_retains_seed_74_checksum() {
    assert_eq!(legacy().len(), 49);
    assert_eq!(digest(legacy()), 0x0ab48154e3a78698);
}

#[test]
fn fine_history_is_bounded_conservative_and_chronological() {
    let frames = fine();
    assert_eq!(frames.len(), 241);
    assert_eq!(frames[0].myr, 0);
    let mut bytes = 0;
    let mut fractional_events = 0;
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(usize::from(f.myr), i);
        assert_eq!(f.n, 29);
        assert_eq!(f.texels.len(), 6*f.n*f.n);
        assert!(f.events.len() <= 4);
        bytes += f.texels.len() * std::mem::size_of::<planetary::DriftTexel>();
        let b = f.budget.expect("physical-history budget");
        assert!(b.sediment_error_m3.abs() < 1.0);
        assert!(b.continental_error_km3.abs() < 1e-3);
        for t in &f.texels {
            assert!(t.height_m.is_finite() && t.runoff_m2.is_finite());
            assert!(t.sediment_m >= -1e-5 && t.continental_km >= -1e-5);
        }
        for e in &f.events {
            assert!(e.myr.is_finite() && e.metres.is_finite());
            assert!(e.myr >= f64::from(f.myr.saturating_sub(1)) && e.myr <= f64::from(f.myr));
            fractional_events += usize::from((e.myr-e.myr.round()).abs() > 1e-6);
        }
        assert!(f.events.windows(2).all(|p| p[0].myr <= p[1].myr));
    }
    assert!(bytes < 90*1024*1024);
    assert!(fractional_events > 100);
}

#[test]
fn fine_history_repeats_at_quarter_metre_voxels() {
    let repeated = planetary::review::history::frames(&config(planetary::FINE_HISTORY_GENERATOR,
        VoxelScalePreset::QuarterMeter));
    assert_eq!(digest(fine()), digest(&repeated));
    for (a,b) in fine().iter().zip(&repeated) {
        assert_eq!(a.events.len(),b.events.len());
        for (a,b) in a.events.iter().zip(&b.events) {
            assert_eq!((a.myr,a.direction,a.metres,a.eruption),(b.myr,b.direction,b.metres,b.eruption));
        }
    }
}

#[test]
fn compact_preview_preserves_shared_face_edges() {
    use engine_core::cube_sphere::Face;
    use std::collections::HashMap;
    for frame in fine() {
        let side = frame.n - 1;
        let mut seen = HashMap::new();
        for face in Face::ALL {
            let basis = face.basis();
            for z in 0..=side {
                for x in 0..=side {
                    if x != 0 && z != 0 && x != side && z != side { continue; }
                    let key = basis.n * side as i32 + basis.u * (2*x as i32-side as i32)
                        + basis.v * (2*z as i32-side as i32);
                    let t = frame.texels[(face as usize*frame.n+z)*frame.n+x];
                    let values = [t.height_m.to_bits(),t.sediment_m.to_bits(),t.runoff_m2.to_bits(),
                        t.ocean_age_myr.to_bits(),t.continental_km.to_bits()];
                    if let Some(previous) = seen.insert(key.to_array(),values) { assert_eq!(previous,values); }
                }
            }
        }
    }
}
