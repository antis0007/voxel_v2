//! Headless diagnostic of the gate307 relief component, not full-world QA.
#[path = "planet_lab/png.rs"] mod png;
use engine_worldgen::planetary::erratics::height;
use engine_core::cube_sphere::atlas;
use glam::DVec3;
use std::{path::Path, time::Instant};
fn main() -> anyhow::Result<()> {
    let root = std::env::args().nth(1).unwrap_or_else(|| "artifacts/erratics-307".into());
    std::fs::create_dir_all(&root)?;
    let seed = 2738958700;
    // Same physical patch crosses the +X/+Z chart boundary at x=0.
    let up = DVec3::new(1.0, 0.0, 1.0).normalize();
    let right = DVec3::new(1.0, 0.0, -1.0).normalize();
    let dir = |x: f64, z: f64| (up * atlas::RADIUS_M + right * x + DVec3::Y * z).normalize();
    let n = 512_usize;
    let step = 0.5;
    let mut values = Vec::with_capacity(n*n);
    let start = Instant::now();
    for z in 0..n { for x in 0..n {
        values.push(height(seed, dir((x as f64-n as f64/2.0)*step, (z as f64-n as f64/2.0)*step), 1.0, 1.0));
    } }
    let micros = start.elapsed().as_secs_f64()*1e6/(n*n) as f64;
    let mut rgb = vec![0_u8; n*n*3];
    let mut max_slope = 0.0_f64;
    let mut max_h = 0.0_f64;
    let mut covered = 0;
    let mut hash = 0xcbf29ce484222325_u64;
    for z in 0..n { for x in 0..n {
        let h = values[z*n+x]; max_h=max_h.max(h); if h>0.01 {covered+=1;}
        hash=(hash^h.to_bits()).wrapping_mul(0x100000001b3);
        let dx=(values[z*n+(x+1).min(n-1)]-values[z*n+x.saturating_sub(1)])/(2.0*step);
        let dz=(values[(z+1).min(n-1)*n+x]-values[z.saturating_sub(1)*n+x])/(2.0*step);
        max_slope=max_slope.max(dx.hypot(dz));
        let normal=DVec3::new(-dx,1.0,-dz).normalize();
        let light=0.22+0.78*normal.dot(DVec3::new(-0.6,0.7,-0.3).normalize()).max(0.0);
        let base=if h>0.02 {[0.48,0.46,0.43]}else{[0.27,0.32,0.20]};
        let c=png::encode(base.map(|v| (v*light) as f32),1.0);
        rgb[(z*n+x)*3..(z*n+x)*3+3].copy_from_slice(&c);
    } }
    png::write(&Path::new(&root).join("component-seam-patch.png"),n,n,&rgb)?;
    let mut max_edge_delta=0.0_f64; let mut edge_covered=0;
    for i in -100000..100000 {
        let z=i as f64*0.25;
        let a=height(seed,dir(-0.00001,z),1.0,1.0);
        let b=height(seed,dir(0.00001,z),1.0,1.0);
        if a>0.01 || b>0.01 {edge_covered+=1;}
        max_edge_delta=max_edge_delta.max((a-b).abs());
    }
    let report=format!("component_only=true\nseed={seed}\nsamples={}\nquery_us={micros:.4}\nheight_max_m={max_h:.4}\nslope_max={max_slope:.4}\ncovered_samples={covered}\nhash={hash:016x}\nchart_edge_samples=200000\nchart_edge_covered={edge_covered}\nchart_edge_max_delta_20micrometres={max_edge_delta:.9}\n",n*n);
    std::fs::write(Path::new(&root).join("component-metrics.txt"),&report)?;
    print!("{report}"); Ok(())
}
