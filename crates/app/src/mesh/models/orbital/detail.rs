//! Reusable pressure vessels and open load frames, authored in local metres.
use super::{Model,Quat,Vec3,architecture::{part,strut},palette::{STEEL,GRAPHITE,COPPER}};
pub(super) fn vessel(mesh:&mut Model,c:Vec3,length:f32,radius:f32,q:Quat,color:[f32;3]) {
    let mark=mesh.mark();let h=length*0.5;
    super::pressure::hull(mesh,c,&[(-h,0.0),(-h*0.98,radius*0.46),(-h*0.84,radius*0.9),
        (-h*0.67,radius),(h*0.67,radius),(h*0.84,radius*0.9),(h*0.98,radius*0.46),(h,0.0)],32,color,0.0);
    for x in [-0.65,0.0,0.65] {
        let at=c+Vec3::X*h*x;
        super::pressure::hull(mesh,at,&[(-radius*0.06,radius*0.99),(-radius*0.06,radius*1.055),
            (radius*0.06,radius*1.055),(radius*0.06,radius*0.99)],24,STEEL,0.5);
    }
    for side in [-1.0,1.0] {
        let a=c+Vec3::new(-h*0.72,radius*0.62,side*radius*0.86);
        let b=c+Vec3::new(h*0.72,radius*0.62,side*radius*0.86);
        strut(mesh,a,b,radius*0.045,STEEL);
        for x in [-0.65,0.0,0.65] {
            strut(mesh,c+Vec3::new(h*x,radius*0.35,side*radius*0.92),
                c+Vec3::new(h*x,radius*0.62,side*radius*0.86),radius*0.035,COPPER);
        }
    }
    let hatch=c+Vec3::Y*radius;
    part(mesh,hatch,Vec3::new(length*0.26,radius*0.12,radius*0.82),Quat::IDENTITY,GRAPHITE,0.5);
    part(mesh,hatch+Vec3::Y*radius*0.075,Vec3::new(length*0.21,radius*0.05,radius*0.68),Quat::IDENTITY,STEEL,0.5);
    mesh.rotate_since(mark,c,q);
}
