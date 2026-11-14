use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, planetary};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let version = args.get(1).and_then(|s|s.parse().ok()).unwrap_or(285);
    let seed = args.get(2).and_then(|s|s.parse().ok()).unwrap_or(74);
    let cfg = WorldSampleConfig { seed, generator_version: version, world_type: WorldType::Planet,
        planet: None, voxel_scale: VoxelScalePreset::OneMeter };
    let start = std::time::Instant::now();
    let frames = planetary::review::history::frames(&cfg);
    let elapsed = start.elapsed().as_secs_f64();
    let mut hash = 0xcbf29ce484222325_u64;
    let mut bytes = 0;
    let mut events = 0;
    let mut max_sediment_error = 0.0_f64;
    let mut max_continent_error = 0.0_f64;
    let mut min_gap = f64::INFINITY;
    let mut max_gap = 0;
    let mut invalid = 0;
    for (i, frame) in frames.iter().enumerate() {
        bytes += frame.texels.len() * std::mem::size_of::<planetary::DriftTexel>();
        events += frame.events.len();
        if i > 0 { max_gap = max_gap.max(frame.myr - frames[i-1].myr); }
        for texel in &frame.texels {
            for v in [texel.height_m,texel.sediment_m,texel.runoff_m2,texel.continental_km,
                texel.ocean_age_myr,texel.weathered_myr] {
                hash = (hash ^ u64::from(v.to_bits())).wrapping_mul(0x100000001b3);
                invalid += usize::from(!v.is_finite());
            }
            invalid += usize::from(texel.sediment_m < -1e-5 || texel.continental_km < -1e-5);
        }
        if let Some(b) = frame.budget {
            max_sediment_error = max_sediment_error.max(b.sediment_error_m3.abs());
            max_continent_error = max_continent_error.max(b.continental_error_km3.abs());
        }
        for e in &frame.events {
            invalid += usize::from(!e.myr.is_finite() || !e.metres.is_finite() ||
                e.myr > f64::from(frame.myr) + 1e-10 ||
                (i > 0 && e.myr < f64::from(frames[i-1].myr) - 1e-10));
            min_gap = min_gap.min((e.myr - e.myr.round()).abs());
        }
    }
    println!("version={version} seed={seed} seconds={elapsed:.4} frames={} face_n={} max_gap_myr={max_gap} bytes={bytes} events={events} invalid={invalid} max_sediment_error_m3={max_sediment_error:.6} max_continent_error_km3={max_continent_error:.9} hash={hash:016x}",frames.len(),frames.first().map_or(0,|f|f.n));
    assert_eq!(invalid,0);
    assert_eq!(frames.last().map(|f|f.myr),Some(240));
    assert!(max_sediment_error < 1.0);
    assert!(max_continent_error < 1e-3);
    if version == planetary::FINE_HISTORY_GENERATOR {
        assert_eq!(frames.len(),241);
        assert_eq!(max_gap,1);
        assert!(bytes < 90*1024*1024);
    }
}
