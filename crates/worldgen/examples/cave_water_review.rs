//! Prints natural cave water counts without loading or modifying saves.
use engine_worldgen::{WorldSampleConfig, WorldType};
use engine_core::VoxelScalePreset;
use engine_content::BlockId;
use glam::IVec3;

fn main() {
    for seed in [7,74,512] {
        let cfg=WorldSampleConfig{
    planet: None,generator_version:73,seed,world_type:WorldType::Hills,
            voxel_scale:VoxelScalePreset::OneMeter};
        let mut cells=0;let mut wet=[0usize;2];
        for (index,version) in [72,73].into_iter().enumerate() {
        let mut cfg=cfg;cfg.generator_version=version;
        for z in (-256..=256).step_by(16) {for x in (-256..=256).step_by(16) {
            let column=engine_worldgen::sample_column(&cfg,x,z);
            for y in cfg.min_world_y()+2..column.height-12 {
                let at=IVec3::new(x,y,z);
                if !engine_worldgen::cave_carves(&cfg,at,&column){continue;}
                if index==1 {cells+=1;}
                wet[index]+=usize::from(engine_worldgen::sample_block_in_column(&cfg,at,column)==BlockId::Water);
            }
        }}}
        println!("seed={seed} cave_cells={cells} water_v72={} water_v73={}",wet[0],wet[1]);
    }
}
