//! Actual solved-world screening for dormant erratic307. No production edits.
use engine_core::{VoxelScalePreset, cube_sphere::atlas};
use engine_worldgen::{WorldSampleConfig, WorldType, planetary, sample_block_in_column};
use glam::{DVec3, IVec3};
use std::{io::Write, time::Instant};
fn main() -> anyhow::Result<()> {
    let cfg = WorldSampleConfig { planet:None, seed:2738958700, generator_version:307,
        world_type:WorldType::Planet, voxel_scale:VoxelScalePreset::OneMeter };
    let out=std::env::args().nth(1).unwrap_or_else(|| "artifacts/erratics-307-20261114/world-audit.txt".into());
    let mut log=std::fs::File::create(out)?;
    writeln!(log,"seed={} generator={} history_env={:?}",cfg.seed,cfg.generator_version,std::env::var("VOXEL_HISTORY_MYR"))?; log.flush()?;
    let mut warm=None; let mut barren=None;
    for i in 0..12000 {
        let y=1.0-2.0*(i as f64+0.5)/12000.0;
        let a=i as f64*2.399963229728653;
        let d=DVec3::new((1.0-y*y).sqrt()*a.cos(),y,(1.0-y*y).sqrt()*a.sin());
        let Some(p)=planetary::review::lab::probe(&cfg,d) else {continue};
        if p.solved_m<=planetary::review::lab::SEA_M+15.0 {continue;}
        if warm.is_none() && p.former_ice>0.30 && p.annual_c>5.0 {warm=Some((d,p));}
        if barren.is_none() && p.former_ice==0.0 && p.annual_c>5.0 {barren=Some((d,p));}
        if warm.is_some() && barren.is_some(){break;}
    }
    for (label,site) in [("warm_former_ice",warm),("unglaciated",barren)] {
        let Some((d,p))=site else {writeln!(log,"{label}: no qualifying coarse site in12000 directions")?;continue};
        let at=atlas::from_body(d*atlas::RADIUS_M);
        writeln!(log,"{label}: x={} z={} former_ice={} annual_c={} solved_m={}",at.x,at.z,p.former_ice,p.annual_c,p.solved_m)?;log.flush()?;
        let mut hits=Vec::new();
        for z in -32..32 {for x in -32..32 {
            let xx=at.x as i32+x*2;let zz=at.z as i32+z*2;
            let dir=atlas::to_body(DVec3::new(xx as f64,0.0,zz as f64)).normalize();
            let ice=planetary::review::lab::probe(&cfg,dir).map_or(0.0,|p|p.former_ice);
            let h=planetary::erratics::height(cfg.seed,dir,ice,1.0);
            if h>0.8 {hits.push((xx,zz,h));}
        }}
        writeln!(log,"component_candidates={}",hits.len())?;
        for &(x,z,raw_h) in hits.iter().take(6) {
            let (c,water,g)=planetary::orbital_surface(&cfg,x,z,1.0);
            let blocks:Vec<_>=(0..8).map(|depth|sample_block_in_column(&cfg,IVec3::new(x,c.height-depth,z),c)).collect();
            writeln!(log,"candidate x={x} z={z} raw_relief={raw_h} top={} voxel_y={} surface={:?} filler={:?} water={water:?} down8={blocks:?}",g.top_m,c.height,c.surface,c.filler)?;
            for footprint in [1.0,2.0,4.0,8.0,12.0,16.0] {
                let (_,_,g)=planetary::orbital_surface(&cfg,x,z,footprint);
                writeln!(log,"footprint={footprint} top={}",g.top_m)?;
            }
        }
        // Warm the actual column path, then measure a small repeat window.
        for z in 0..32 {for x in 0..32 {std::hint::black_box(planetary::column(&cfg,at.x as i32+x,at.z as i32+z,1.0));}}
        let t=Instant::now();
        for _ in 0..3 {for z in 0..32 {for x in 0..32 {std::hint::black_box(planetary::column(&cfg,at.x as i32+x,at.z as i32+z,1.0));}}}
        writeln!(log,"warm_column_us={} samples=3072",t.elapsed().as_secs_f64()*1e6/3072.0)?;log.flush()?;
    }
    writeln!(log,"complete=true")?; Ok(())
}
