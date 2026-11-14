//! Fixed-seed terrain metrics and native-camera bookmarks; never opens saves.
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, geo};
use std::io::Write;

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    anyhow::ensure!(args.len() >= 6, "mountain_review <planet|hills|ridged> <version> <seed> <x_m> <z_m> <output.csv>");
    let kind = match args[0].as_str() {
        "planet" => WorldType::Planet,
        "hills" => WorldType::Hills,
        "ridged" => WorldType::Ridged,
        _ => anyhow::bail!("unknown world type"),
    };
    let cfg = WorldSampleConfig { world_type: kind, generator_version: args[1].parse()?,
        seed: args[2].parse()?, voxel_scale: VoxelScalePreset::OneMeter, planet: None };
    let (cx, cz): (f32, f32) = (args[3].parse()?, args[4].parse()?);
    let started = std::time::Instant::now();
    const N: usize = 251;
    const STEP: f32 = 16.0;
    let at = |i: usize| (cx + (i % N) as f32 * STEP - 2000.0, cz + (i / N) as f32 * STEP - 2000.0);
    let ground: Vec<_> = (0..N*N).map(|i| { let (x,z)=at(i); geo::ground(&cfg,x,z,1.0) }).collect();
    let mut peaks = Vec::new();
    let mut steep = 0;
    let mut curvature = Vec::new();
    for z in 4..N-4 { for x in 4..N-4 {
        let i=z*N+x;
        let h=ground[i].top_m;
        let slope=(ground[i+1].top_m-ground[i-1].top_m).hypot(ground[i+N].top_m-ground[i-N].top_m)/(2.0*STEP);
        steep += usize::from(slope>1.0);
        let mean=(ground[i-4].top_m+ground[i+4].top_m+ground[i-4*N].top_m+ground[i+4*N].top_m)*0.25;
        curvature.push((h-mean).abs());
        if h>50.0 && [-1,1,-(N as isize),N as isize].iter().all(|d|ground[(i as isize+d)as usize].top_m<h) {
            peaks.push((h-mean,i));
        }
    }}
    peaks.sort_unstable_by(|a,b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    curvature.sort_unstable_by(f32::total_cmp);
    let mut hash=0xcbf29ce484222325u64;
    for g in &ground { hash=(hash^u64::from(g.top_m.to_bits())).wrapping_mul(0x100000001b3); }
    let mut out=std::io::BufWriter::new(std::fs::File::create(&args[5])?);
    writeln!(out,"x_m,z_m,height_m,slope,water_m,starting_height_m,eroded_height_m,hillslope_net_m,fluvial_incision_m,fracture_damage,wet_contributing_area_m2")?;
    let mut channels = Vec::new();
    for (i,g) in ground.iter().enumerate() { let (x,z)=at(i);
        write!(out,"{x},{z},{},{},{}",g.top_m,g.slope,g.water_m.unwrap_or(f32::NAN))?;
        if let Some(h) = geo::history::sample(&cfg, f64::from(x), f64::from(z)) {
            writeln!(out,",{},{},{},{},{},{}",h.starting_height_m,h.eroded_height_m,h.hillslope_net_m,h.fluvial_incision_m,h.fracture_damage,h.contributing_area_m2)?;
            if h.current_solved_height_m > 25.0 { channels.push((h.fluvial_incision_m,i)); }
        } else { writeln!(out,",,,,,,")?; }
    }
    out.flush()?;
    println!("world={kind:?} generator={} seed={} samples={} sample_ms={:.2} hash={hash:016x} peaks={} steep45_pct={:.3} curvature64_p95_m={:.3}",cfg.generator_version,cfg.seed,ground.len(),started.elapsed().as_secs_f64()*1000.0,peaks.len(),100.0*steep as f64/((N-8)*(N-8))as f64,curvature[curvature.len()*95/100]);
    let planet = geo::planet_for(&cfg);
    let mut raw_steep = 0;
    let mut raw_count = 0;
    let mut raw_curvature = Vec::new();
    for i in 0..planet.height.len() {
        if planet.grid.on_border(i) || planet.height[i] < 60.0 { continue; }
        let n = planet.grid.n;
        let slope = (planet.height[i+1]-planet.height[i-1]).hypot(planet.height[i+n]-planet.height[i-n]) / (2.0 * planet.grid.cell_m as f32);
        raw_count += 1;
        raw_steep += usize::from(slope > 1.0);
        let [x,z] = planet.grid.at(i);
        let mean = [[x-64.0,z],[x+64.0,z],[x,z-64.0],[x,z+64.0]].map(|[x,z]|planet.grid.bilinear(&planet.height,x,z)).iter().sum::<f64>() * 0.25;
        raw_curvature.push((planet.height[i] as f64-mean).abs());
    }
    raw_curvature.sort_by(f64::total_cmp);
    println!("solve_checksum={:016x} mountain_cells={raw_count} solved_steep45_pct={:.3} solved_curvature64_p95_m={:.3}",planet.checksum,100.0*raw_steep as f64/raw_count.max(1) as f64,raw_curvature[raw_curvature.len()*95/100]);
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let scaled = WorldSampleConfig { voxel_scale: scale, ..cfg };
        let mut scale_hash = 0xcbf29ce484222325u64;
        let mut max_boundary_step = 0.0_f32;
        let mut max_spatial_step = 0.0_f32;
        for z in -8..=8 { for x in -8..=8 {
            let (x,z) = (cx + x as f32 * 128.0, cz + z as f32 * 128.0);
            let h = geo::ground(&scaled,x,z,1.0).top_m;
            scale_hash = (scale_hash^u64::from(h.to_bits())).wrapping_mul(0x100000001b3);
            let a = geo::ground(&scaled,x,z,31.999).top_m;
            let b = geo::ground(&scaled,x,z,32.001).top_m;
            max_boundary_step = max_boundary_step.max((a-b).abs());
            max_spatial_step = max_spatial_step.max((geo::ground(&scaled,x-0.001,z,1.0).top_m-geo::ground(&scaled,x+0.001,z,1.0).top_m).abs());
        }}
        println!("scale={scale:?} physical_hash={scale_hash:016x} lod32_step_max_m={max_boundary_step:.6} spatial_boundary_step_max_m={max_spatial_step:.6}");
    }
    for (prominence,i) in peaks.iter().take(8) { let (x,z)=at(*i);
        let raw=planet.grid.bilinear(&planet.height,f64::from(x),f64::from(z));
        println!("summit x={x} z={z} height={} solved={raw:.2} prominence64={prominence:.2} camera={},{},-{},0,-0.15",ground[*i].top_m,x,z+300.0,500.0+ground[*i].top_m+25.0);
    }
    if let Some(budget) = geo::history::budget(&cfg) { println!("history={budget:?}"); }
    channels.sort_by(|a,b|b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut bookmarks: Vec<(f32,f32)> = Vec::new();
    for (incision,i) in channels {
        let (x,z) = at(i);
        if bookmarks.iter().any(|(a,b)| (x-a).hypot(z-b) < 250.0) { continue; }
        println!("incised_valley x={x} z={z} cut_m={incision:.2} height={} camera={},{},-{},0,-0.30",ground[i].top_m,x,z+220.0,500.0+ground[i].top_m+110.0);
        bookmarks.push((x,z));
        if bookmarks.len() == 5 { break; }
    }
    Ok(())
}
