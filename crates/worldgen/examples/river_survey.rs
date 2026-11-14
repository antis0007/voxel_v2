//! Local generated-channel diagnostic, independent of rendering and saves.
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, geo, terrain_height, sample_block};
use glam::{IVec3, Vec2};
fn main() {
    let cfg = WorldSampleConfig {
    planet: None, seed: 2738958700, generator_version: 78,
        world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter };
    let mut sources = geo::river_sources(&cfg, Vec2::ZERO, 32_000.0);
    sources.sort_by(|a,b| (a.position_m.x*a.position_m.x+a.position_m.z*a.position_m.z)
        .total_cmp(&(b.position_m.x*b.position_m.x+b.position_m.z*b.position_m.z)));
    let origin=sources[0].position_m;
    let p=geo::planet_for(&cfg);
    let mut i=p.node.iter().enumerate().min_by(|(_,a),(_,b)| {
        let d=|v:&[f32;2]| (v[0]-origin.x).powi(2)+(v[1]-origin.z).powi(2);
        d(a).total_cmp(&d(b))
    }).unwrap().0;
    for link in 0..4 {
        let r=p.receiver[i] as usize;
        println!("link={link} from={:?} head={} to={:?} head={}",p.node[i],p.height[i],p.node[r],p.height[r]);
        for t in 0..=16 {
            let f=t as f32/16.0;
            let x=p.node[i][0]+(p.node[r][0]-p.node[i][0])*f;
            let z=p.node[i][1]+(p.node[r][1]-p.node[i][1])*f;
            let g=geo::ground(&cfg,x,z,1.0);
            let bed=terrain_height(&cfg,x.floor() as i32,z.floor() as i32);
            let block=g.water_m.map(|h|sample_block(&cfg,IVec3::new(x.floor() as i32,h.ceil() as i32-1,z.floor() as i32)));
            println!("  x={x:.1} z={z:.1} ground={:.2} bed={bed} water={:?} top_block={block:?}",g.top_m,g.water_m);
        }
        i=r;
    }
}
