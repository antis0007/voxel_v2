//! Suspended service vessel: exposed load paths, shielded cargo and folding fins.
use super::{Model, Quat, Vec3, architecture::{part, strut}};
use crate::world::{DayNightCycle, orbital::{Class, Service}};

pub(super) fn build(mesh: &mut Model, p: Vec3, axis: Vec3, service: Service, day: &DayNightCycle, signal: [f32;3]) {
    let w = service.site.class.width();
    let metal = [0.24,0.30,0.34];
    let shell = [0.79,0.77,0.68];
    let copper = [0.45,0.25,0.12];
    let dark = [0.065,0.10,0.14];
    let mark = mesh.mark();
    let t = service.lowering;
    let align = ((t-0.35)/0.61).clamp(0.0,1.0);
    let align = align.powi(3)*(align*(align*6.0-15.0)+10.0);
    let bank = (std::f32::consts::PI*t).sin().powi(2);
    let heading = (service.site.key>>8) as f32/16777216.0*std::f32::consts::TAU;
    let sense = if service.site.key&1==0 {1.0} else {-1.0};
    // Narrow central pressure hull; the cargo outriggers leave a clear void
    // beneath the suspension tower, rather than filling a rectangular slab.
    part(mesh,p,Vec3::new(w*0.22,w*0.15,w*0.83),Quat::IDENTITY,dark,0.5);
    for end in [-1.0,1.0] {
        part(mesh,p+Vec3::new(0.0,w*0.015,end*w*0.34),
            Vec3::new(w*0.19,w*0.14,w*0.23),Quat::from_rotation_x(end*0.16),shell,0.0);
    }
    // Raised, sloped bridge with a recessed panoramic observation band.
    part(mesh,p+Vec3::new(0.0,w*0.13,-w*0.18),
        Vec3::new(w*0.20,w*0.15,w*0.22),Quat::from_rotation_x(-0.18),shell,0.0);
    mesh.cuboid(p+Vec3::new(0.0,w*0.14,-w*0.295),
        Vec3::new(w*0.145,w*0.028,w*0.009),Quat::from_rotation_x(-0.18),[0.36,0.62,0.64],0.45);
    for side in [-1.0,1.0] {
        let cargo=p+Vec3::new(side*w*0.32,-w*0.09,w*0.02);
        // Three individual pressure canisters, with thermal shields and
        // exposed gaps between ribs. Lamps belong to the recessed service rail.
        for bay in -1..=1 {
            let c=cargo+Vec3::Z*bay as f32*w*0.19;
            part(mesh,c,Vec3::new(w*0.18,w*0.18,w*0.16),Quat::IDENTITY,dark,0.5);
            part(mesh,c+Vec3::Y*w*0.09,Vec3::new(w*0.22,w*0.025,w*0.17),
                Quat::from_rotation_z(-side*0.15),shell,0.0);
            for z in [-1.0,1.0] {
                part(mesh,c+Vec3::Z*z*w*0.08,Vec3::new(w*0.195,w*0.19,w*0.012),Quat::IDENTITY,metal,0.5);
            }
            mesh.cuboid(c+Vec3::new(side*w*0.099,0.0,0.0),
                Vec3::new(w*0.009,w*0.025,w*0.09),Quat::IDENTITY,[0.88,0.63,0.30],0.8);
        }
        // Two open triangular load frames carry the pods up to the gimbal.
        for z in [-0.29,0.29] {
            let low=p+Vec3::new(side*w*0.43,0.0,z*w);
            let high=p+Vec3::new(0.0,w*0.65,z*w*0.18);
            strut(mesh,low,high,w*0.013,metal);
            strut(mesh,low,p+Vec3::new(0.0,w*0.14,z*w),w*0.012,metal);
            strut(mesh,low.lerp(high,0.5),p+Vec3::new(0.0,w*0.14,z*w),w*0.007,copper);
        }
        // Long, separated radiator feathers; each has a real hinge and spine.
        let hinge=p+Vec3::new(side*w*0.43,w*0.025,w*0.06);
        let wing=mesh.mark();
        let count=if matches!(service.site.class,Class::Conservatory) {4} else {3};
        for feather in 0..count {
            let z=(feather as f32-(count-1) as f32*0.5)*w*0.135;
            let length=w*(0.49-0.055*(feather as f32-(count-1) as f32*0.5).abs());
            let c=hinge+Vec3::new(side*length*0.5,0.0,z);
            part(mesh,c,Vec3::new(length,w*0.012,w*0.105),Quat::IDENTITY,[0.065,0.12,0.18],1.0);
            strut(mesh,hinge+Vec3::Z*z,hinge+Vec3::new(side*length,0.0,z),w*0.0055,metal);
            for rib in 1..5 {
                part(mesh,hinge+Vec3::new(side*length*rib as f32/5.0,w*0.009,z),
                    Vec3::new(w*0.006,w*0.007,w*0.102),Quat::IDENTITY,[0.37,0.42,0.43],0.5);
            }
        }
        mesh.rotate_since(wing,hinge,Quat::from_rotation_z(side*(0.20+(1.0-align)*0.95)));
        mesh.cuboid(p+Vec3::new(side*w*0.43,w*0.025,-w*0.30),
            Vec3::splat(w*0.018),Quat::IDENTITY,signal,1.0);
    }
    // Split ceramic gimbal cheeks and a dark bearing, kept mechanically
    // separate from the fixed cable collar above the rotating vessel.
    let joint=p+Vec3::Y*w*0.71;
    for side in [-1.0,1.0] {
        part(mesh,joint+Vec3::X*side*w*0.055-Vec3::Y*w*0.05,
            Vec3::new(w*0.035,w*0.16,w*0.12),Quat::IDENTITY,shell,0.0);
    }
    strut(mesh,joint-Vec3::X*w*0.07,joint+Vec3::X*w*0.07,w*0.026,copper);
    let line_frame=Quat::from_rotation_arc(Vec3::Y,axis);
    let rotation=line_frame*Quat::from_rotation_y(heading+sense*2.6*(1.0-align))
        *Quat::from_rotation_z(sense*bank*0.25+service.suspension_angle(day))
        *Quat::from_rotation_x(bank*0.12);
    mesh.rotate_since(mark,joint,rotation);
    let collar=mesh.mark();
    strut(mesh,joint,joint+Vec3::Y*w*0.16,w*0.025,metal);
    part(mesh,joint+Vec3::Y*w*0.10,Vec3::new(w*0.14,w*0.035,w*0.14),Quat::IDENTITY,shell,0.0);
    mesh.rotate_since(collar,joint,line_frame);
}
