//! Thin industrial fittings on the editable ground exchange. Walkways, walls,
//! stairs and cover are owned by worldgen::meridian_terminal, not these meshes.
use super::{Model,Quat,Vec3,architecture::{part,strut},palette::*};
use crate::world::World;
use engine_worldgen::meridian_terminal::{PASSENGER_GUIDE_SIDE_M, PASSENGER_SHAFT_HALF_M};

const LEGACY_DETAIL_RANGE_M: f32 = 1000.0;

/// Keep the tower silhouette in the same visibility group as its freight braid.
pub(super) fn distant_landmark(mesh: &mut Model, base: Vec3, viewer: Vec3, monumental: bool) {
    let distance_squared = base.distance_squared(viewer);
    if monumental {
        if distance_squared > super::ground_monument::DETAIL_RANGE_M.powi(2) {
            super::ground_monument::distant_landmark(mesh, base);
        }
    } else if distance_squared > LEGACY_DETAIL_RANGE_M.powi(2) {
        for side in [-1.0, 1.0] {
            for z in [-34.5, 1.5, 37.5] {
                let root = base + Vec3::new(side * 36.0, 0.0, z);
                let height = if side > 0.0 && z < -12.0 { 31.0 } else { 52.0 };
                strut(mesh, root, root + Vec3::Y * height, 2.0, STEEL);
            }
        }
        for z in [1.5, 37.5] {
            strut(mesh, base + Vec3::new(-36.0, 48.0, z),
                base + Vec3::new(36.0, 48.0, z), 1.6, STEEL);
        }
    }
}

pub(super) fn build(mesh:&mut Model,world:&World,viewer:Vec3) {
    let cfg=world.sample_config();
    if !engine_worldgen::meridian_terminal::enabled(&cfg) {return;}
    let range = if cfg.generator_version >= engine_worldgen::meridian_terminal::monument::VERSION {
        super::ground_monument::DETAIL_RANGE_M
    } else { LEGACY_DETAIL_RANGE_M };
    // Horizontal distance is a lower bound on the existing 3D detail cutoff.
    // Do not survey an invisible terminal's terrain when crossing planet faces.
    if (viewer.x - 1100.0).hypot(viewer.z + 900.0) > range { return; }
    let p=Vec3::new(1100.0,engine_worldgen::meridian_terminal::floor(&cfg),-900.0);
    let distance = p.distance_squared(viewer);
    if distance < 120.0 * 120.0 {
        // Inset marker lights lead from the main approach to the flush berth.
        for x in [-2.8, 2.8] {
            for z in [5.0, 9.0, 13.0, 17.0, 21.0] {
                mesh.cuboid(p + Vec3::new(x, 0.025, z), Vec3::new(0.14, 0.045, 1.5),
                    Quat::IDENTITY, [0.25, 0.88, 0.64], 0.42);
            }
            mesh.cuboid(p + Vec3::new(x, 0.025, 0.0), Vec3::new(0.14, 0.045, 4.0),
                Quat::IDENTITY, [0.25, 0.88, 0.64], 0.42);
        }
    }
    if cfg.generator_version>=engine_worldgen::meridian_terminal::monument::VERSION {
        super::ground_monument::build(mesh,world,viewer);return;
    }
    if distance > LEGACY_DETAIL_RANGE_M.powi(2) {return;}
    for side in [-1.0,1.0] {
        for bay in 0..5 {
            let z=-34.5+bay as f32*18.0;
            let damaged=side>0.0 && z< -12.0;
            let height=if damaged {29.0+(z+42.0)*0.45} else {51.0};
            // Flanged piers, recessed vertical conduits and diagonal knee
            // braces give the concrete structure an exposed industrial skeleton.
            for dx in [-1.0,1.0] {
                strut(mesh,p+Vec3::new(side*36.0,4.0,z+dx),
                    p+Vec3::new(side*36.0,height,z+dx),0.18,STEEL);
            }
            for y in (8..height as i32).step_by(8) {
                strut(mesh,p+Vec3::new(side*36.0,y as f32,z-1.2),
                    p+Vec3::new(side*36.0,y as f32+5.0,z+1.2),0.12,COPPER);
            }
            strut(mesh,p+Vec3::new(side*36.0,30.0_f32.min(height-8.0),z),
                p+Vec3::new(side*24.0,(height-5.0).max(18.0),z),0.45,STEEL);
            if side<0.0 {
                // Leave the same well in the roof fittings as in the voxel roof.
                if z.abs() < PASSENGER_GUIDE_SIDE_M + 0.5 {
                    let edge = PASSENGER_SHAFT_HALF_M + 0.5;
                    roof_truss(mesh, p, z, -36.0, -edge);
                    roof_truss(mesh, p, z, edge, 36.0);
                } else {
                    roof_truss(mesh, p, z, -36.0, if z < -12.0 { -12.0 } else { 36.0 });
                }
            }
            // Railings flank the galleries; stair entrances and ruined gaps
            // stay open. They are thin visual fittings, not collision barriers.
            if z<5.0 && !damaged {
                for y in [10.4,11.2] {
                    strut(mesh,p+Vec3::new(side*20.3,y,z-7.0),p+Vec3::new(side*20.3,y,z+7.0),0.06,COPPER);
                }
                for dz in [-7.0,0.0,7.0] {strut(mesh,p+Vec3::new(side*20.3,10.0,z+dz),p+Vec3::new(side*20.3,11.2,z+dz),0.07,STEEL);}
            }
            if !damaged {
                let casing=p+Vec3::new(side*32.8,5.0,z);
                part(mesh,casing,Vec3::new(0.5,6.0,2.2),Quat::IDENTITY,GRAPHITE,1.0);
                part(mesh,casing-Vec3::X*side*0.3,Vec3::new(0.12,4.4,1.5),Quat::IDENTITY,STEEL,0.5);
            }
        }
    }
    // The hydraulic tanks flank the passenger guide.
    for side in [-1.0,1.0] {
        let x = PASSENGER_SHAFT_HALF_M + 1.15 + 0.75;
        super::detail::vessel(mesh,p+Vec3::new(side*x,2.0,-3.0),4.0,1.15,
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),COPPER);
    }
    // The copper trunk links the restore console to the freight socket.
    let z = super::FREIGHT_SOCKET_Z_M;
    strut(mesh,p+Vec3::new(-12.0,0.15,10.0),p+Vec3::new(-12.0,0.15,z),0.12,COPPER);
    strut(mesh,p+Vec3::new(-12.0,0.15,z),p+Vec3::new(-6.0,0.15,z),0.12,COPPER);
}

fn roof_truss(mesh: &mut Model, p: Vec3, z: f32, start: f32, end: f32) {
    for y in [48.2, 52.2] {
        strut(mesh, p + Vec3::new(start, y, z), p + Vec3::new(end, y, z), 0.36, STEEL);
    }
    let mut x = start;
    while x < end {
        let next = (x + 12.0).min(end);
        let a = p + Vec3::new(x, 48.2, z);
        let b = p + Vec3::new(next, 52.2, z);
        strut(mesh, a, b, 0.30, STEEL);
        strut(mesh, b, p + Vec3::new(next, 48.2, z), 0.25, COPPER);
        x = next;
    }
}
