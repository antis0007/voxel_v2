//! Cave-enabled voxel/material and footprint audit for dormant erratic deposits.
use engine_core::{VoxelScalePreset, cube_sphere::atlas};
use engine_worldgen::{WorldSampleConfig, WorldType, planetary, sample_block, sample_block_in_column};
use glam::{DVec3, IVec3};
use engine_content::BlockId;
use std::{io::Write, path::PathBuf, time::Instant};
fn main() -> anyhow::Result<()> {
    let out=PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "artifacts/erratics-308-20261114".into()));
    std::fs::create_dir_all(&out)?;
    planetary::configure_geology_cache(out.join("geology-cache"));
    let cfg=WorldSampleConfig {planet:None,seed:2738958700,generator_version:308,
        world_type:WorldType::Planet,voxel_scale:VoxelScalePreset::OneMeter};
    let mut log=std::fs::File::create(out.join(std::env::args().nth(2).unwrap_or_else(|| "audit.txt".into())))?;
    writeln!(log,"seed={} gen={} history_env={:?}",cfg.seed,cfg.generator_version,std::env::var("VOXEL_HISTORY_MYR"))?;log.flush()?;
    let mut warm=(131161,-23094); let mut checked=0;
    'search: for i in 0..12000 {
        let y=1.0-2.0*(i as f64+0.5)/12000.0;let angle=i as f64*2.399963229728653;
        let dir=DVec3::new((1.0-y*y).sqrt()*angle.cos(),y,(1.0-y*y).sqrt()*angle.sin());
        let Some(probe)=planetary::review::lab::probe(&cfg,dir) else {continue};
        if probe.former_ice<0.3 || probe.annual_c<5.0 || probe.solved_m<planetary::review::lab::SEA_M+15.0 {continue;}
        checked+=1; if checked>100 {break;}
        let at=atlas::from_body(dir*atlas::RADIUS_M);
        for dz in -16..16 {for dx in -16..16 {
            let x=at.x as i32+dx*4;let z=at.z as i32+dz*4;
            let d=atlas::to_body(DVec3::new(x as f64,0.0,z as f64)).normalize();
            if planetary::erratics::height(cfg.seed,d,probe.former_ice,1.0)<1.0 {continue;}
            let c=planetary::column(&cfg,x,z,1.0);
            if c.deposit_m>=1.0 {warm=(x,z);writeln!(log,"selected_deposit x={x} z={z} coarse_sites_checked={checked} depth={}",c.deposit_m)?;log.flush()?;break 'search;}
        }}
    }
    for (label,xx,zz) in [("former_ice",warm.0,warm.1),("unglaciated",111525,17776)] {
        let dir=atlas::to_body(DVec3::new(xx as f64,0.0,zz as f64)).normalize();
        writeln!(log,"{label} x={xx} z={zz} probe={:?}",planetary::review::lab::probe(&cfg,dir))?;log.flush()?;
        let mut hits=Vec::new(); let mut window_hash=0xcbf29ce484222325_u64;
        let start=Instant::now();
        for z in -32..32 {for x in -32..32 {
            let x=xx+x*2;let z=zz+z*2;
            let c=planetary::column(&cfg,x,z,1.0);
            window_hash=(window_hash ^ c.height as u64 ^ c.deposit_m.to_bits() as u64).wrapping_mul(0x100000001b3);
            if c.deposit_m>0.0 {hits.push((x,z,c));}
        }}
        writeln!(log,"column_scan_samples=4096 scan_us_per_column={} deposit_columns={} hash={window_hash:016x}",start.elapsed().as_secs_f64()*1e6/4096.0,hits.len())?;log.flush()?;
        let mut material_failures=0;let mut substrate_failures=0;let mut cells=0;
        for &(x,z,c) in &hits {
            let n=cfg.blocks_for_span(c.deposit_m);
            let bed=engine_worldgen::ColumnSample {height:c.height-n,surface:c.deposit_bed_surface,deposit_m:0.0,..c};
            for depth in 0..n {
                let p=IVec3::new(x,c.height-depth,z);
                let actual=sample_block(&cfg,p);
                cells+=1;
                if actual!=BlockId::Stone {material_failures+=1;}
            }
            for depth in 0..4 {
                let p=IVec3::new(x,bed.height-depth,z);
                if sample_block(&cfg,p)!=sample_block_in_column(&cfg,p,bed) {substrate_failures+=1;}
            }
        }
        writeln!(log,"cave_enabled_deposit_cells={cells} nonstone={material_failures} buried_substrate_mismatches={substrate_failures}")?;
        for &(x,z,c) in hits.iter().take(4) {
            let blocks:Vec<_>=(0..8).map(|d|sample_block(&cfg,IVec3::new(x,c.height-d,z))).collect();
            writeln!(log,"x={x} z={z} depth={} bedsurface={:?} surface={:?} down8={blocks:?}",c.deposit_m,c.deposit_bed_surface,c.surface)?;
            for fp in [1.0,2.0,4.0,6.0,8.0,10.0,12.0,16.0] {
                let (c,water,g)=planetary::orbital_surface(&cfg,x,z,fp);
                writeln!(log,"footprint={fp} top={} deposit={} water={water:?}",g.top_m,c.deposit_m)?;
            }
        }
        let t=Instant::now();
        for _ in 0..3 {for z in 0..32 {for x in 0..32 {std::hint::black_box(planetary::column(&cfg,xx+x,zz+z,1.0));}}}
        writeln!(log,"warm_column_us={} samples=3072",t.elapsed().as_secs_f64()*1e6/3072.0)?;log.flush()?;
    }
    writeln!(log,"complete=true")?;Ok(())
}
