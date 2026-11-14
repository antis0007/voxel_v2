//! Locate reproducible review cameras in real generated terrain (no scene edits).
//! cargo run --offline -p engine_worldgen --example painterly_locations
use engine_worldgen::{WorldSampleConfig, WorldType, WATER_LEVEL_METERS, terrain_height, tree_visual};
use engine_core::VoxelScalePreset;
use std::f32::consts::{FRAC_PI_2, PI};

fn main() {
    let version = std::env::args().nth(1).and_then(|v| v.parse().ok()).unwrap_or(49);
    let seed = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(2738958700);
    let cfg = WorldSampleConfig {
    planet: None, seed, generator_version: version,
        world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter };
    let mut castles = engine_worldgen::sites::horizon_landmarks(&cfg, (0,0), 4200.0);
    castles.retain(|(_,kind,_,_)| *kind == 3);
    castles.sort_by(|a,b| a.0.length_squared().total_cmp(&b.0.length_squared()));
    for (p,_,turn,_) in castles.into_iter().take(2) {
        println!("castle centre={p:?} turn={turn} camera={},{},24,0,-0.40",
            p.x,p.z+40.0);
        let rx = (p.x / 192.0).round() as i32;
        let rz = (p.z / 192.0).round() as i32;
        let site_seed = seed.wrapping_add((rx as u32).wrapping_mul(73856093))
            .wrapping_add((rz as u32).wrapping_mul(19349663));
        if let Some(b) = engine_worldgen::architecture::site_building_version(3,site_seed,false,version) {
            let mut changes = 0;
            for x in -20..20 { for z in -14..14 { for y in 0..3 {
                let q = [x as f32 + 0.5,y as f32 + 0.5,z as f32 + 0.5].into();
                if b.sample_sited(q,1.0,0.0,48) != b.sample_sited(q,1.0,0.0,version) {
                    changes += 1;
                }
            }}}
            println!("castle courtyard changed_voxels={changes}");
        }
    }
    'shore: for ring in 1i32..=30 {
        for ix in -ring..=ring { for iz in -ring..=ring {
            if ix.abs().max(iz.abs()) != ring { continue; }
            let (x,z) = (ix * 32, iz * 32);
            let h = terrain_height(&cfg, x, z) as f32;
            if !(WATER_LEVEL_METERS + 1.0..WATER_LEVEL_METERS + 4.0).contains(&h) { continue; }
            for (dx,dz,yaw) in [(0,-32,0.0),(-32,0,FRAC_PI_2),(0,32,PI),(32,0,-FRAC_PI_2)] {
                if terrain_height(&cfg,x+dx,z+dz) as f32 <= WATER_LEVEL_METERS - 2.0 {
                    println!("shore camera={x},{z},1.7,{yaw},-0.12 surface_y={h}");
                    break 'shore;
                }
            }
        }}
    }
    'tree: for ring in 0i32..=20 {
        for x in -ring..=ring { for z in -ring..=ring {
            if x.abs().max(z.abs()) != ring { continue; }
            if let Some(t) = tree_visual(&cfg,x,z)
                && !t.conifer && t.radius > 9.0 && t.height < 13.0 {
                    println!("old tree cell={x},{z} base={:?} radius={} camera={},{},1.7,0,0.18",
                        t.base,t.radius,t.base.x,t.base.z+15.0);
                    break 'tree;
                }
        }}
    }
}
