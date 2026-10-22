//! Upper-atmosphere anchorages and rigid, locally textured orbital parts.
//! These are distant scene models, not enterable voxel habitats.
use super::{Model, Class, Quat, Vec3};
use crate::world::DayNightCycle;

pub(super) fn part(mesh: &mut Model, at: Vec3, size: Vec3, rotation: Quat,
    color: [f32; 3], finish: f32) {
    mesh.material_box(at, size, rotation, color, 62.0, finish);
}

pub(super) fn strut(mesh: &mut Model, a: Vec3, b: Vec3, radius: f32, color: [f32; 3]) {
    let mark = mesh.mark();
    mesh.set_surface_kind(62.0);
    mesh.beam(a, b, radius, color, 0.0);
    mesh.bake_model_uv_since(mark, a, 62.0);
    for v in &mut mesh.vertices[mark..] { v.surface[3] = 0.5; }
    mesh.set_surface_kind(41.0);
}

/// Local-frame anchorage geometry. Shape depends only on class and site key;
/// per frame only the ring spin and the suspension alignment change.
/// Positions are metres relative to the anchorage centre, before spin/heading.
struct CachedAnchorage {
    fixed: Vec<crate::mesh::Vertex3>,
    ring: Vec<crate::mesh::Vertex3>,
}

thread_local! {
    static ANCHORAGE_CACHE: std::cell::RefCell<std::collections::HashMap<(u8, u32), std::rc::Rc<CachedAnchorage>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

fn class_tag(class: Class) -> u8 {
    match class { Class::Courier => 0, Class::Freight => 1, Class::Conservatory => 2 }
}

/// Rebuilding a station's thousands of parts every frame cost ~12 ms of CPU.
/// The geometry is built once per site in its local frame and then posed by a
/// single rigid transform, so the vertices match the direct build.
pub(super) fn anchorage(mesh: &mut Model, at: Vec3, axis: Vec3, class: Class, key: u32, day: &DayNightCycle) {
    let cached = ANCHORAGE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 64 { cache.clear(); }
        cache.entry((class_tag(class), key)).or_insert_with(|| {
            // Unit scale: cached positions are plain metres.
            let mut local = Model::new(1.0);
            let ring = build_anchorage_local(&mut local, class, key);
            let ring_vertices = local.vertices[ring.clone()].to_vec();
            let mut fixed = local.vertices[..ring.start].to_vec();
            fixed.extend_from_slice(&local.vertices[ring.end..]);
            std::rc::Rc::new(CachedAnchorage { fixed, ring: ring_vertices })
        }).clone()
    });
    let heading=(key>>8) as f32/16777216.0*std::f32::consts::TAU;
    let align=Quat::from_rotation_arc(Vec3::Y,axis)*Quat::from_rotation_y(heading);
    // The ring rotates about the keel bearing. No camera-relative pose.
    let spin=(day.time_of_day() as f64*std::f64::consts::TAU*8.0).rem_euclid(std::f64::consts::TAU) as f32;
    let ring_rotation=align*Quat::from_rotation_y(spin);
    let scale=mesh.scale;
    mesh.vertices.reserve(cached.fixed.len()+cached.ring.len());
    for (source, rotation) in [(&cached.fixed, align), (&cached.ring, ring_rotation)] {
        mesh.vertices.extend(source.iter().map(|v| {
            let mut out=*v;
            out.pos=((at+rotation*Vec3::from_array(v.pos))/scale).to_array();
            out.normal=(rotation*Vec3::from_array(v.normal)).to_array();
            out
        }));
    }
}

/// Builds the anchorage around the origin with no spin or heading. Returns the
/// vertex range of the rotating pressure rim.
fn build_anchorage_local(mesh: &mut Model, class: Class, key: u32) -> std::ops::Range<usize> {
    let at=Vec3::ZERO;
    let mut ring_range=0..0;
    let r=class.upper_ring_radius_m();
    let shell=[0.79,0.77,0.68];
    let alloy=[0.25,0.31,0.35];
    let dark=[0.065,0.12,0.18];
    let copper=[0.45,0.25,0.12];
    // An open, tapered keel carries the suspension load through the bearing.
    // Wide gaps between diagonal members are visible against the sky.
    for quadrant in 0..4 {
        let a=quadrant as f32*std::f32::consts::FRAC_PI_2;
        let radial=Vec3::new(a.cos(),0.0,a.sin());
        for bay in 0..5 {
            let y=-0.65+bay as f32*0.25;
            let lo=at+Vec3::Y*y*r+radial*r*(0.065+0.035*(1.0-y.abs()));
            let hi=at+Vec3::Y*(y+0.25)*r+radial*r*0.09;
            strut(mesh,lo,hi,r*0.012,alloy);
            let next=Vec3::new((a+std::f32::consts::FRAC_PI_2).cos(),0.0,(a+std::f32::consts::FRAC_PI_2).sin());
            strut(mesh,lo,at+Vec3::Y*(y+0.25)*r+next*r*0.09,r*0.006,copper);
        }
    }
    for y in [-0.65,-0.40,0.10,0.60] {
        part(mesh,at+Vec3::Y*y*r,Vec3::new(r*0.24,r*0.045,r*0.24),Quat::IDENTITY,shell,0.0);
    }
    part(mesh,at+Vec3::Y*r*0.32,Vec3::new(r*0.20,r*0.30,r*0.20),Quat::IDENTITY,dark,0.5);
    strut(mesh,at+Vec3::Y*r*0.62,at+Vec3::Y*r*1.05,r*0.008,alloy);
    // Continuous segmented pressure rim, separate inner/outer truss rails,
    // paired spokes and recessed windows: readable mass and negative space.
    if !matches!(class,Class::Courier) {
        let ring=mesh.mark();
        let sectors=if matches!(class,Class::Conservatory) {96} else {48};
        // A larger radius adds circumference, not giant windows or decks.
        let tube=r.min(210.0);
        let step=std::f32::consts::TAU/sectors as f32;
        for i in 0..sectors {
            let a=(i as f32+0.5)*step;
            let radial=Vec3::new(a.cos(),0.0,a.sin());
            let q=Quat::from_rotation_y(-a);
            let c=at+radial*r;
            let length=2.0*r*(step*0.5).tan()*1.005;
            part(mesh,c,Vec3::new(tube*0.11,tube*0.095,length),q,shell,0.0);
            // A dark equatorial window belt is subdivided by real mullions.
            for window in [-1.0,0.0,1.0] {
                let tangent=Vec3::new(-a.sin(),0.0,a.cos());
                let wc=c+radial*tube*0.056+tangent*window*length*0.29;
                mesh.cuboid(wc,Vec3::new(tube*0.004,tube*0.022,length*0.21),q,
                    if (i+key as usize)%5==0 {[0.24,0.39,0.41]} else {[0.85,0.64,0.34]},0.65);
            }
            let a0=i as f32*step;
            let a1=(i+1) as f32*step;
            let v0=Vec3::new(a0.cos(),0.0,a0.sin());
            let v1=Vec3::new(a1.cos(),0.0,a1.sin());
            for y in [-0.075,0.075] {
                strut(mesh,at+v0*r*0.91+Vec3::Y*y*r,at+v1*r*0.91+Vec3::Y*y*r,r*0.008,alloy);
            }
            strut(mesh,at+v0*r*0.91-Vec3::Y*r*0.075,at+v1*r*0.91+Vec3::Y*r*0.075,r*0.0045,copper);
            if i%8==0 {
                for y in [-1.0,1.0] {
                    strut(mesh,at+Vec3::Y*y*r*0.24,c+Vec3::Y*y*r*0.045,r*0.009,alloy);
                }
                part(mesh,c+Vec3::Y*r*0.075,Vec3::new(r*0.15,r*0.06,r*0.25),q,shell,0.0);
            }
        }
        ring_range=ring..mesh.mark();
    }
    // Offset docking arms and feathered thermal banks make the bus asymmetric
    // in profile, while keeping radiators clear of the inhabited pressure rim.
    for side in [-1.0,1.0] {
        let root=at+Vec3::new(0.0,r*0.48,0.0);
        let tip=root+Vec3::new(side*r*1.25,r*0.20,0.0);
        strut(mesh,root,tip,r*0.018,alloy);
        strut(mesh,at+Vec3::Y*r*0.12,tip,r*0.01,alloy);
        for feather in 0..5 {
            let x=side*r*(0.40+feather as f32*0.20);
            let hinge=root+Vec3::new(x,x.abs()*0.16,0.0);
            for z in [-1.0,1.0] {
                let q=Quat::from_rotation_x(z*0.20);
                part(mesh,hinge+Vec3::new(0.0,r*0.055,z*r*0.30),
                    Vec3::new(r*0.16,r*0.012,r*0.52),q,dark,1.0);
                strut(mesh,hinge,hinge+Vec3::new(0.0,r*0.11,z*r*0.56),r*0.0045,alloy);
            }
        }
        let dock=at+Vec3::new(side*r*0.34,-r*0.35,0.0);
        strut(mesh,at-Vec3::Y*r*0.35,dock,r*0.022,alloy);
        part(mesh,dock,Vec3::new(r*0.12,r*0.15,r*0.12),Quat::IDENTITY,shell,0.0);
        mesh.cuboid(dock+Vec3::new(0.0,0.0,-r*0.062),Vec3::new(r*0.065,r*0.07,r*0.008),
            Quat::IDENTITY,[0.88,0.63,0.30],0.8);
    }
    // The pressure rim's spin axis, keel and suspension terminal share a single
    // frame. `anchorage` aligns AFTER local spin/heading so the bearing follows
    // the actual tension line throughout the sweep, rather than precessing
    // about world Y.
    ring_range
}
