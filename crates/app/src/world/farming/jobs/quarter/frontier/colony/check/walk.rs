//! Walk the paid public frontage into all six physically completed houses.
use super::*;

pub(super) fn check(w:&World,p:&mut Player)->Result<()> {
    let base=IVec3::from(w.farming.jobs.quarter.as_ref().context("Missing city")?.origin).as_vec3();
    p.pos=base+Vec3::new(10.5,2.0,14.5);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;p.flying=false;
    let mut points=Vec::new();
    for (door,z) in [(5.5,-17.0),(5.5,-6.0),(5.5,5.0),(15.5,-13.0),(15.5,-2.0),(15.5,9.0)] {
        points.extend([(10.5,z-0.5),(door,z-0.5),(door,z+2.5),(door,z-0.5),(10.5,z-0.5)]
            .into_iter().map(|(x,z)|base+Vec3::new(x,2.0,z)));
    }
    let distance=super::super::super::super::walk::route(w,p,points)?;
    ensure!(!p.flying&&p.pos.y>=base.y+1.9,"Grounded house access failed");
    println!("[city-colony-check] PASS six interiors on paid connected streets distance_m={distance:.2}");
    Ok(())
}
