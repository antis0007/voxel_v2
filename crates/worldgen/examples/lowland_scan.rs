//! Temporary diagnostic (gate 296 review): find warm, wet lowland river reaches.
//! args: version seed cx cz half step
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, planetary};

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let cfg = WorldSampleConfig { planet: None, seed: a[1].parse()?, generator_version: a[0].parse()?,
        world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter };
    let (cx, cz, half, step): (f64, f64, f64, f64) = (a[2].parse()?, a[3].parse()?, a[4].parse()?, a[5].parse()?);
    let n = (2.0 * half / step) as i64;
    for j in 0..n { for i in 0..n {
        let x = cx - half + i as f64 * step; let z = cz - half + j as f64 * step;
        let g = planetary::ground(&cfg, x as f32, z as f32, 4.0);
        if g.riparian < 0.2 && g.delta < 0.2 { continue; }
        if !(-2.0..=25.0).contains(&g.top_m) { continue; }
        let c = planetary::climate_at(&cfg, planetary::direction(&cfg, x, z));
        if c.winter_c < 2.0 { continue; }
        println!("{x:.0} {z:.0} top {:.1} water {:?} rip {:.2} delta {:.2} mud {:.2} winter {:.1} wet {:.2} slope {:.2}",
            g.top_m, g.water_m, g.riparian, g.delta, g.delta_mud, c.winter_c, c.wet, g.slope);
    }}
    Ok(())
}
