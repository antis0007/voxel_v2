//! The anchor is the landmark. All secondary roofs remain below ten metres.
use super::{Model,Quat,Vec3,architecture::{part,strut},palette::*};
use super::super::posed;
use crate::world::World;
use engine_worldgen::meridian_terminal::{PASSENGER_GUIDE_SIDE_M, PASSENGER_SHAFT_HALF_M};
pub(super) const DETAIL_RANGE_M: f32 = 2600.0;

pub(super) fn distant_landmark(mesh: &mut Model, base: Vec3) {
    let shaft_corner = PASSENGER_SHAFT_HALF_M.hypot(PASSENGER_GUIDE_SIDE_M);
    for sector in 0..6 {
        let a = sector as f32 * std::f32::consts::FRAC_PI_3;
        let radial = Vec3::new(a.cos(), 0.0, a.sin());
        let buttress = |y: f32| {
            base + radial * (48.0 - 37.0 * (y / 136.0).powf(0.62)) + Vec3::Y * y
        };
        for (lo, hi) in [(0.0, 34.0), (34.0, 68.0), (68.0, 102.0), (102.0, 136.0)] {
            strut(mesh, buttress(lo), buttress(hi), 5.0, CERAMIC);
        }
        let shaft = |y| base + radial * (18.0 - y * 0.043) + Vec3::Y * y;
        let radius = 3.2_f32.min(18.0 - 254.4 * 0.043 - shaft_corner);
        strut(mesh, shaft(24.0), shaft(254.4), radius, STEEL);
    }
}

pub(super) fn build(mesh:&mut Model,world:&World,viewer:Vec3) {
    let cfg=world.sample_config();
    let p=Vec3::new(1100.0,engine_worldgen::meridian_terminal::floor(&cfg),-900.0);
    if (p-viewer).length_squared() > DETAIL_RANGE_M.powi(2) {return;}
    // The monument is rigid: nothing in it depends on the viewer beyond the
    // near-detail cutoff, and only the dock crate on delivered stock. Building
    // its ~2k parts every frame cost ~1.4 ms, so each variant is built once
    // around the origin and posed at the terminal.
    let near=(p-viewer).length_squared()<=700.0_f32.powi(2);
    let stock=near && world.orbital.dynamics.delivered_iron>0;
    let key=posed::key(&[0x6d6f_6e75,u32::from(near),u32::from(stock)]);
    let range=posed::intern(key,|| {
        let mut local=Model::new(1.0);
        build_local(&mut local,near,stock);
        local.vertices
    });
    posed::place(mesh,range,Quat::IDENTITY,p);
    // The direct build left the scene on the strut finish; later orbital
    // parts were authored against that.
    mesh.set_surface_kind(41.0);
}

/// The monument in metres around its floor centre. `near` adds the anchor
/// chamber, cargo dock and control wing; `stock` the delivered crate.
fn build_local(mesh:&mut Model,near:bool,stock:bool) {
    let p=Vec3::ZERO;
    let shaft_corner = PASSENGER_SHAFT_HALF_M.hypot(PASSENGER_GUIDE_SIDE_M);
    for sector in 0..6 {
        let a=sector as f32*std::f32::consts::FRAC_PI_3;
        let radial=Vec3::new(a.cos(),0.0,a.sin());
        // Massive curved load paths terminate in broad, buried anchor sockets.
        for bay in 0..16 {
            let sample=|i:usize| {let t=i as f32/16.0;p+radial*(48.0-37.0*t.powf(0.62))+Vec3::Y*136.0*t};
            let lo=sample(bay);let hi=sample(bay+1);let t=(bay as f32+0.5)/16.0;
            let axis=(hi-lo).normalize();let tangent=Vec3::new(-a.sin(),0.0,a.cos());
            let q=Quat::from_mat3(&glam::Mat3::from_cols(tangent,axis,tangent.cross(axis)));
            // Project the radial section onto the sloping member's local frame.
            // This covers the voxel load core without exposed staircase edges.
            let depth=(16.0-5.0*t)/axis.y.max(0.3);
            let shell=std::array::from_fn(|i|(STEEL[i]+CERAMIC[i])*0.5);
            part(mesh,(lo+hi)*0.5,Vec3::new(14.0-4.0*t,(hi-lo).length()+0.4,depth),q,shell,0.0);
            if bay%4==0 {
                part(mesh,(lo+hi)*0.5,Vec3::new(14.5-4.0*t,1.2,depth+0.5),q,STEEL,0.5);
            }
        }
        part(mesh,p+radial*48.0+Vec3::Y*3.0,Vec3::new(20.0,6.0,18.0),Quat::from_rotation_y(-a),GRAPHITE,0.5);
        for bay in 0..12 {
            let y0=24.0+bay as f32*19.2;let y1=y0+19.2;
            let lo=p+radial*(18.0-y0*0.043)+Vec3::Y*y0;
            let hi=p+radial*(18.0-y1*0.043)+Vec3::Y*y1;
            let radius = 3.2_f32.min(18.0 - y1 * 0.043 - shaft_corner);
            strut(mesh,lo,hi,radius,STEEL);
            // Ceramic blade panels and recessed thermal seams follow the shaft,
            // rather than competing with it as horizontal concrete bridges.
            let c=(lo+hi)*0.5+radial*2.8;
            part(mesh,c,Vec3::new(5.6,15.8,0.8),Quat::from_rotation_y(-a+std::f32::consts::FRAC_PI_2),CERAMIC,0.0);
        }
    }
    for y in [24.0,64.0,104.0,144.0,184.0,224.0,250.0] {
        let mark=mesh.mark();
        super::pressure::rim(mesh,18.0-y*0.043+2.5,1.35,48,STEEL);
        for v in &mut mesh.vertices[mark..] {v.pos=(Vec3::from_array(v.pos)+Vec3::Y*y/mesh.scale+p/mesh.scale).to_array();}
    }
    if !near {return;}
    // Broad annular anchor chamber. Open front portal reveals the guide well.
    for side in [-1.0,1.0] {
        super::detail::vessel(mesh,p+Vec3::new(side*7.0,7.0,0.0),32.0,2.3,
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),COPPER);
        part(mesh,p+Vec3::new(side*7.2,5.0,22.4),Vec3::new(1.1,10.0,1.0),Quat::IDENTITY,STEEL,0.5);
    }
    part(mesh,p+Vec3::new(0.0,11.2,22.4),Vec3::new(15.5,1.2,1.0),Quat::IDENTITY,STEEL,0.5);
    // Low, recognizable cargo dock: ground rails, parked service climber and
    // a loading arch that remains far below the anchor's silhouette.
    for x in [38.0,50.0] {
        strut(mesh,p+Vec3::new(x,0.16,-8.0),p+Vec3::new(x,0.16,37.0),0.12,STEEL);
        part(mesh,p+Vec3::new(x,5.0,27.0),Vec3::new(1.2,10.0,1.2),Quat::IDENTITY,STEEL,0.5);
    }
    part(mesh,p+Vec3::new(44.0,10.0,27.0),Vec3::new(13.0,1.3,2.0),Quat::IDENTITY,STEEL,0.5);
    super::detail::vessel(mesh,p+Vec3::new(44.0,3.7,8.0),10.0,3.2,
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),CARGO);
    for side in [-1.0,1.0] {
        part(mesh,p+Vec3::new(44.0+side*3.4,3.7,8.0),Vec3::new(0.7,6.0,8.0),Quat::IDENTITY,STEEL,0.5);
        for z in [5.0,11.0] {
            super::pressure::hull(mesh,p+Vec3::new(44.0+side*4.0,2.0,z),&[
                (-0.3,0.0),(-0.3,0.9),(0.3,0.9),(0.3,0.0)],16,STEEL,0.5);
        }
    }
    // Delivered stock alone creates filled crates. An empty dock never suggests
    // a continuous, unaccounted cargo flow.
    if stock {
        part(mesh,p+Vec3::new(35.0,1.0,19.0),Vec3::new(2.2,2.0,2.4),Quat::IDENTITY,CARGO,0.0);
    }
    // The control wing's canopy frames its actual receiver console, off-axis.
    for z in [22.0,30.0] {
        part(mesh,p+Vec3::new(-25.0,3.2,z),Vec3::new(0.35,6.4,0.35),Quat::IDENTITY,STEEL,0.5);
    }
    part(mesh,p+Vec3::new(-28.0,6.6,26.0),Vec3::new(8.0,0.4,10.0),Quat::IDENTITY,STEEL,0.5);
}
