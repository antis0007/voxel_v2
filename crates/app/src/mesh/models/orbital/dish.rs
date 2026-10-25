use super::{Model, World, DayNightCycle, Quat, Vec3, part, strut, orbital};

pub(super) fn build(mesh:&mut Model,w:&World,day:&DayNightCycle,viewer:Vec3) {
    let r=&w.orbital.radio;
    let Some(a)=r.dish else {return};
    let p=(a.as_vec3()+Vec3::splat(0.5))*w.voxel_scale.meters();
    if (p-viewer).length_squared()>256.0*256.0 || !orbital::radio::intact(w) {return}
    let mark=mesh.mark();
    let alloy=[0.27,0.33,0.37]; let ceramic=[0.76,0.73,0.63];
    part(mesh,p+Vec3::Y*0.53,Vec3::new(1.8,0.16,1.8),Quat::IDENTITY,alloy,0.5);
    strut(mesh,p+Vec3::Y*0.5,p+Vec3::Y*2.0,0.16,alloy);
    let pivot=p+Vec3::Y*2.0;
    let bowl=mesh.mark();
    for z in -3i32..=3 {for x in -3i32..=3 {
        if x*x+z*z>10 {continue}
        let at=pivot+Vec3::new(x as f32*0.40,(x*x+z*z) as f32*0.035,z as f32*0.40);
        let normal=Vec3::new(-x as f32*0.175,1.0,-z as f32*0.175).normalize();
        part(mesh,at,Vec3::new(0.42,0.08,0.42),Quat::from_rotation_arc(Vec3::Y,normal),ceramic,0.0);
    }}
    strut(mesh,pivot,pivot+Vec3::Y*1.0,0.035,alloy);
    for i in 0..3 {
        let angle=i as f32*std::f32::consts::TAU/3.0;
        strut(mesh,pivot+Vec3::new(angle.cos()*1.2,0.32,angle.sin()*1.2),pivot+Vec3::Y*0.95,0.025,alloy);
    }
    part(mesh,pivot+Vec3::Y*0.92,Vec3::new(0.25,0.20,0.25),Quat::IDENTITY,[0.62,0.43,0.21],0.5);
    let color=if r.level>=5 {[0.36,0.75,0.68]} else {[0.80,0.48,0.19]};
    for i in 0..8 {
        let angle=i as f32*std::f32::consts::TAU/8.0;
        let at=pivot+Vec3::new(angle.cos()*1.28,0.36,angle.sin()*1.28);
        mesh.cuboid(at,Vec3::splat(if r.level>=5 {0.09} else {0.05}),Quat::IDENTITY,color,0.6);
    }
    if let Some(target)=orbital::radio::orbital_target(w,day) {
        mesh.rotate_since(bowl,pivot,Quat::from_rotation_arc(Vec3::Y,(target-pivot).normalize()));
    } else if let Some(site)=r.target.and_then(|cell|orbital::site(w.seed,cell)) {
        let height=orbital::hold_height(&w.sample_config(),&w.orbital,site);
        let target=Vec3::new(site.x+650.0,height+orbital::UPPER_ANCHORAGE_OFFSET_M,site.z+260.0);
        mesh.rotate_since(bowl,pivot,Quat::from_rotation_arc(Vec3::Y,(target-pivot).normalize()));
    }
    for v in &mut mesh.vertices[mark..] {if v.surface[2]!=62.0 {v.surface[1]=2.0;}}
}
