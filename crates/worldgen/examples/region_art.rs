//! Locate a river-led regional composition for repeatable native art review.
//! Prints actual solve data and camera poses; this does not render or alter a world.
//! cargo run --offline --profile release-fast -p engine_worldgen --example region_art -- 74
use engine_core::VoxelScalePreset;
use engine_worldgen::{geo, WorldSampleConfig, WorldType};

fn main() {
    let seed = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(74);
    let cfg = WorldSampleConfig {
    planet: None, seed, generator_version: geo::corridor::LOD_VERSION,
        world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter };
    let p = geo::planet_for(&cfg);
    // A substantial inland reach, close enough to spawn for one regional visit.
    let i = (0..p.height.len()).filter(|&i| {
        let [x,z] = p.node[i];
        p.land[i] && p.receiver[i] as usize != i && p.lake[i] == f32::MIN
            && (24.0..400.0).contains(&p.height[i]) && x.hypot(z) < 6000.0
            && p.discharge[i] > 2_500_000.0
    }).max_by(|&a,&b| p.discharge[a].total_cmp(&p.discharge[b]))
        .expect("no inland main stem in this seed; choose another seed");
    let [x,z] = p.node[i];
    let [rx,rz] = p.node[p.receiver[i] as usize];
    let (dx,dz) = (rx-x,rz-z);
    let length = dx.hypot(dz).max(1.0);
    let (dx,dz) = (dx/length,dz/length);
    println!("seed={seed} reach={x:.1},{z:.1} downstream={rx:.1},{rz:.1} area_km2={:.1}", p.discharge[i]/1e6);
    // View down the river, with its valley entering from the lower foreground.
    let yaw = dx.atan2(-dz);
    for (name,back,clearance,pitch) in [
        ("high",900.0,1050.0,-0.67), ("medium",420.0,360.0,-0.50),
        ("corridor",140.0,95.0,-0.28), ("ground",36.0,3.0,-0.08),
    ] {
        let (cx,cz) = (x-dx*back-dz*45.0,z-dz*back+dx*45.0);
        println!("{name} camera={cx:.2},{cz:.2},{clearance:.2},{yaw:.5},{pitch:.5}");
    }
    println!("valley camera={:.2},{:.2},230,{:.5},-0.55", x-dz*450.0,z+dx*450.0,dz.atan2(dx));
    println!("transition camera={:.2},{:.2},36,{:.5},-0.15", x-dz*110.0,z+dx*110.0,yaw);
    // Report the sampler's fields for inspecting how LOD retains this location.
    for version in [137, geo::corridor::LOD_VERSION] {
        let c = WorldSampleConfig { generator_version:version, ..cfg };
        for footprint in [1.0,32.0,33.0,64.0,128.0,256.0,512.0] {
            let g = geo::ground(&c,x,z,footprint);
            println!("v{version} footprint={footprint} height={:.2} moisture={:.3} sediment={:.3} riparian={:.3}",
                g.top_m,g.moisture,g.sediment,g.riparian);
        }
    }
    if std::env::args().any(|a| a == "--woodland-survey") {
        woodland_survey(cfg);
    }
}

// Sampler report for upland woodland at matched physical positions. This is
// source-field evidence, never a substitute for native imagery or traversal.
fn woodland_survey(cfg: WorldSampleConfig) {
    let p = geo::planet_for(&cfg);
    let i = (0..p.height.len()).filter(|&i| {
        let [x,z] = p.node[i];
        p.land[i] && p.lake[i] == f32::MIN && (145.0..360.0).contains(&p.height[i])
            && x.hypot(z) < 6000.0
    }).max_by(|&a,&b| p.wet[a].total_cmp(&p.wet[b])).expect("no upland survey site");
    let [cx,cz] = p.node[i];
    println!("upland seed={} centre={cx:.1},{cz:.1} solved_height={:.1}", cfg.seed,p.height[i]);
    println!("upland camera={:.2},{:.2},180,0,-0.55",cx,cz+280.0);
    for version in [geo::corridor::LOD_VERSION,engine_worldgen::landscape::WOODLAND_GENERATOR] {
        for scale in [VoxelScalePreset::OneMeter,VoxelScalePreset::HalfMeter,VoxelScalePreset::QuarterMeter] {
            let c = WorldSampleConfig {generator_version:version,voxel_scale:scale,..cfg};
            for footprint in [8.0,64.0,256.0] {
                let (mut count,mut occupied,mut density) = (0,0,0.0f32);
                for dz in -6..=6 { for dx in -6..=6 {
                    // Snap once to whole metres so all three voxel grids agree.
                    let (mx,mz) = (cx.round()+dx as f32*24.0,cz.round()+dz as f32*24.0);
                    let (x,z) = ((mx/c.voxel_meters()) as i32,(mz/c.voxel_meters()) as i32);
                    if let Some(column) = engine_worldgen::basin::cover_column(&c,x,z,footprint) {
                        count += 1;
                        density += column.vegetation_density;
                        if column.vegetation_density > 0.15 { occupied += 1; }
                    }
                }}
                println!("upland v{version} voxel_m={} footprint={footprint} samples={count} wooded={occupied} mean_density={:.5}",c.voxel_meters(),density/count.max(1) as f32);
            }
        }
    }
}
