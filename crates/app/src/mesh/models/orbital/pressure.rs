//! Closed surfaces of revolution, with continuous normals and part-local UVs.
//! Pressure vessels have curved ends; struts and cladding remain discrete parts.
use super::{Model, Vec3};

/// Revolve an (axial position, radius) profile about X. Profile runs left to
/// right, starting and ending on the axis. Facets share analytic surface normals.
pub(super) fn hull(mesh: &mut Model, centre: Vec3, profile: &[(f32, f32)], sectors: usize,
    color: [f32; 3], finish: f32) {
    let mark = mesh.mark();
    let radius=profile.iter().map(|p|p.1).fold(0.0,f32::max);
    mesh.set_surface_kind(62.0);
    let normal = |j: usize, angle: f32| {
        if j == 0 { return -Vec3::X; }
        if j == profile.len()-1 { return Vec3::X; }
        let a = profile[j.saturating_sub(1)];
        let b = profile[(j + 1).min(profile.len() - 1)];
        Vec3::new(a.1 - b.1, (b.0-a.0)*angle.cos(), (b.0-a.0)*angle.sin()).normalize_or_zero()
    };
    for j in 0..profile.len()-1 {
        for i in 0..sectors {
            let a = i as f32 * std::f32::consts::TAU / sectors as f32;
            let b = (i+1) as f32 * std::f32::consts::TAU / sectors as f32;
            let point = |k: usize, angle: f32| centre + Vec3::new(profile[k].0,
                profile[k].1*angle.cos(), profile[k].1*angle.sin());
            for corners in [[(j,a),(j,b),(j+1,b)],[(j,a),(j+1,b),(j+1,a)]] {
                // A pole emits a single triangle, avoiding zero-area geometry.
                if corners[0].0 == corners[1].0 && profile[corners[0].0].1 == 0.0
                    || corners[1].0 == corners[2].0 && profile[corners[1].0].1 == 0.0 { continue; }
                let start = mesh.mark();
                mesh.triangle(corners.map(|(k,t)| point(k,t)), normal(j,a), color, 0.0);
                // triangle() may reverse winding. Recover the normal from the
                // emitted position rather than pairing it with the input order.
                for v in &mut mesh.vertices[start..] {
                    let local = Vec3::from_array(v.pos)*mesh.scale-centre;
                    let k = if (local.x-profile[j].0).abs() <= (local.x-profile[j+1].0).abs() {j} else {j+1};
                    v.normal = normal(k,local.z.atan2(local.y)).to_array();
                    let mut angle=local.z.atan2(local.y).rem_euclid(std::f32::consts::TAU);
                    if a>std::f32::consts::PI && angle<0.1 {angle+=std::f32::consts::TAU;}
                    v.surface[0]=local.x;
                    v.surface[1]=angle*radius;
                }
            }
        }
    }
    for v in &mut mesh.vertices[mark..] { v.surface[3] = finish; }
    mesh.set_surface_kind(41.0);
}

/// Continuous toroidal pressure rim with smooth radial and tube normals.
pub(super) fn rim(mesh: &mut Model, radius: f32, tube: f32, sectors: usize, color: [f32; 3]) {
    mesh.set_surface_kind(62.0);
    for i in 0..sectors {
        for j in 0..8 {
            let sample = |u: usize, v: usize| {
                let a = u as f32 * std::f32::consts::TAU / sectors as f32;
                let b = v as f32 * std::f32::consts::TAU / 8.0;
                let radial = Vec3::new(a.cos(),0.0,a.sin());
                let n = radial*b.cos()+Vec3::Y*b.sin();
                (radial*radius+n*tube,n)
            };
            for corners in [[(i,j),(i+1,j),(i+1,j+1)],[(i,j),(i+1,j+1),(i,j+1)]] {
                let values = corners.map(|(u,v)| sample(u,v));
                let start = mesh.mark();
                // triangle() can reverse winding; assign normals from positions
                // afterwards rather than assuming its output vertex order.
                mesh.triangle(values.map(|(p,_)| p),values[0].1,color,0.0);
                for v in &mut mesh.vertices[start..] {
                    let p = Vec3::from_array(v.pos)*mesh.scale;
                    let radial = Vec3::new(p.x,0.0,p.z).normalize_or_zero();
                    v.normal = (p-radial*radius).normalize_or_zero().to_array();
                    let mut u=p.z.atan2(p.x).rem_euclid(std::f32::consts::TAU);
                    let mut w=p.y.atan2(Vec3::new(p.x,0.0,p.z).length()-radius).rem_euclid(std::f32::consts::TAU);
                    if i>sectors/2 && u<0.1 {u+=std::f32::consts::TAU;}
                    if j>4 && w<0.1 {w+=std::f32::consts::TAU;}
                    v.surface[0]=u*radius;v.surface[1]=w*tube;
                }
            }
        }
    }
    mesh.set_surface_kind(41.0);
}
