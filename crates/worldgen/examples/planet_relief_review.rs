//! Fixed physical windows, terrain slopes and reproducible native bookmarks.
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, planetary};
use glam::{DVec2, DVec3};
use engine_core::cube_sphere::{Projection, atlas};
use std::io::{BufWriter, Write};

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let [mode, version, seed] = args.as_slice() && mode == "--bathymetry" {
        let cfg = WorldSampleConfig {
            planet: None, seed: seed.parse()?, generator_version: version.parse()?,
            world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter,
        };
        anyhow::ensure!(planetary::active(&cfg), "requires spherical Planet terrain");
        println!("seed={} version={} {:?}", cfg.seed, cfg.generator_version,
            planetary::review::bathymetry_audit(&cfg));
        return Ok(());
    }
    anyhow::ensure!(args.len() >= 3, "planet_relief_review <version> <seed> <output-prefix> [side=500] [spacing=8] [origin-x origin-z across-x across-z]");
    let version = args[0].parse()?;
    let seed = args[1].parse()?;
    let cfg = WorldSampleConfig { planet: None, seed, generator_version: version,
        world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter };
    let side: usize = args.get(3).map_or(Ok(500), |s| s.parse())?;
    let spacing: f64 = args.get(4).map_or(Ok(8.0), |s| s.parse())?;
    anyhow::ensure!((8..=1000).contains(&side) && spacing.is_finite() && spacing > 0.0, "invalid grid");
    let mut frame_values = match seed {
        74 => [343580.71852828487,20453.256840943533,0.9996345010145292,0.02703450353591003],
        731 => [220133.883213782,15433.079563446368,-0.015519576267836456,-0.9998795641238333],
        _ => [343584.0,16000.0,1.0,0.0],
    };
    if args.len()>5 {
        anyhow::ensure!(args.len()==9,"provide all four frame coordinates");
        for (v,arg) in frame_values.iter_mut().zip(&args[5..9]) { *v=arg.parse::<f64>()?; }
    }
    let [x,z,ax,az]=frame_values;
    anyhow::ensure!(frame_values.iter().all(|v|v.is_finite()) && (ax.hypot(az)-1.0).abs()<1e-6,"invalid frame");
    let frame = planetary::review::catchment::Frame { origin:DVec2::new(x,z),
        across:DVec2::new(ax,az), downstream:DVec2::new(az,-ax) };
    println!("seed={seed} version={version} side={side} spacing_m={spacing} origin={:?} across={:?} downstream={:?}",frame.origin,frame.across,frame.downstream);
    let start = std::time::Instant::now();
    let mut samples = Vec::with_capacity(side*side);
    let mut hash = 0xcbf29ce484222325_u64;
    for z in 0..side { for x in 0..side {
        let p = frame.chart((x as f64+0.5)*spacing,(z as f64+0.5)*spacing);
        let g = planetary::ground(&cfg,p.x as f32,p.y as f32,spacing as f32);
        hash = (hash ^ u64::from(g.top_m.to_bits())).wrapping_mul(0x100000001b3);
        samples.push(g);
    }}
    let sample_ms = start.elapsed().as_secs_f64()*1000.0;
    for (suffix, field) in [
        ("height", samples.iter().map(|g|g.top_m).collect::<Vec<_>>()),
        ("slope", samples.iter().map(|g|g.slope).collect()),
        ("water", samples.iter().map(|g|g.water_m.unwrap_or(f32::NAN)).collect()),
    ] {
        let mut out=BufWriter::new(std::fs::File::create(format!("{}-{suffix}.f32",args[2]))?);
        for value in field { out.write_all(&value.to_le_bytes())?; }
        out.flush()?;
    }
    let mut errors=Vec::new();
    let mut false_rock=0;
    for z in 1..side-1 { for x in 1..side-1 {
        let i=z*side+x;
        let p = frame.chart((x as f64+0.5)*spacing,(z as f64+0.5)*spacing);
        let (face,local) = atlas::split(DVec3::new(p.x,0.0,p.y));
        let (east,north) = Projection::Tangent.tangents(face,
            local.x/atlas::HALF_M,local.z/atlas::HALF_M);
        let axis = |v: DVec2| (east*v.x+north*v.y)*(atlas::RADIUS_M/atlas::HALF_M);
        let (u,v) = (axis(frame.across),axis(frame.downstream));
        let du=f64::from(samples[i+1].top_m-samples[i-1].top_m)/(2.0*spacing);
        let dv=f64::from(samples[i+side].top_m-samples[i-side].top_m)/(2.0*spacing);
        let actual=((v.length_squared()*du*du-2.0*u.dot(v)*du*dv+u.length_squared()*dv*dv)
            /(u.length_squared()*v.length_squared()-u.dot(v).powi(2))).max(0.0).sqrt() as f32;
        errors.push((actual-samples[i].slope).abs());
        false_rock+=usize::from(actual<0.3 && samples[i].slope>0.65);
    }}
    errors.sort_by(f32::total_cmp);
    println!("sample_ms={sample_ms:.2} hash={hash:016x} slope_mae={} slope_error_p95={} false_rock_samples={false_rock}",
        errors.iter().sum::<f32>()/errors.len() as f32,errors[errors.len()*95/100]);
    let mut max_spatial=0.0_f32;
    let mut max_lod=0.0_f32;
    let mut scale_hashes=Vec::new();
    for scale in [VoxelScalePreset::OneMeter,VoxelScalePreset::HalfMeter,VoxelScalePreset::QuarterMeter] {
        let scaled=WorldSampleConfig { voxel_scale:scale,..cfg };
        let mut h=0xcbf29ce484222325_u64;
        for z in 0..8 { for x in 0..8 {
            let p=frame.chart(192.0+x as f64*448.0,192.0+z as f64*448.0);
            let (x,z)=(p.x as f32,p.y as f32);
            let a=planetary::ground(&scaled,x,z,8.0).top_m;
            h=(h^u64::from(a.to_bits())).wrapping_mul(0x100000001b3);
            max_spatial=max_spatial.max((planetary::ground(&scaled,x-0.0625,z,8.0).top_m
                -planetary::ground(&scaled,x+0.0625,z,8.0).top_m).abs());
            let low = planetary::ground(&scaled,x,z,31.999);
            let high = planetary::ground(&scaled,x,z,32.001);
            let jump = (low.top_m - high.top_m).abs();
            if jump > max_lod && jump > 0.02 {
                println!("lod_jump x={x} z={z} metres={jump} heights={:?} water={:?} slopes={:?} riparian={:?}",
                    [low.top_m, high.top_m], [low.water_m, high.water_m], [low.slope, high.slope], [low.riparian, high.riparian]);
            }
            max_lod=max_lod.max(jump);
        }}
        scale_hashes.push(h);
        println!("scale={scale:?} physical_hash={h:016x}");
    }
    anyhow::ensure!(scale_hashes.iter().all(|h|*h==scale_hashes[0]),"scale changed physical terrain");
    println!("spatial_0_125m_step_max={max_spatial} footprint32_step_max={max_lod}");
    Ok(())
}
