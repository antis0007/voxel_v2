//! Sealed aerial infrastructure, authored in metres. Sparse seed placement and
//! calendar-driven mechanisms are independent of terrain/entity residency.
use super::{Model, Quat, Vec3, architecture::{part,strut}};
use crate::world::{World,DayNightCycle,orbital::megastructures::{self as catalog,Kind}};

use super::palette::{CERAMIC as SHELL,STEEL as METAL,GRAPHITE as DARK,COPPER};

pub(super) fn long_range_since(mesh:&mut Model,mark:usize) {
    for v in &mut mesh.vertices[mark..] {
        if v.surface[2]==62.0 {v.surface[2]=63.0;}
        else {v.surface[1]=3.0;}
    }
}
pub(super) fn build(mesh:&mut Model,world:&World,day:&DayNightCycle,viewer:Vec3) {
    for c in catalog::contacts(world.seed,day,viewer) {
        let mark=mesh.mark();
        match c.site.kind {
            Kind::Foundry=>foundry(mesh,Vec3::ZERO,day,c.site.key),
            Kind::TwinHabitat=>habitat(mesh,Vec3::ZERO,day,c.site.key),
            Kind::MirrorArray=>mirror_array_tracking(mesh,Vec3::ZERO,day,c.site.key,
                c.site.orientation.inverse()*super::observer_sun(day,viewer)),
            Kind::DeadRelay=>dead_relay(mesh,Vec3::ZERO,day,c.site.key),
        }
        mesh.scale_since(mark,Vec3::ZERO,Vec3::splat(c.site.kind.scale()));
        mesh.rotate_since(mark,Vec3::ZERO,c.site.orientation);
        long_range_since(mesh,mark);
        for vertex in &mut mesh.vertices[mark..] {
            // Author UVs and small fittings near the local origin, before the
            // final world translation loses precision at orbital distances.
            vertex.pos=(Vec3::from_array(vertex.pos)+c.site.position/mesh.scale).to_array();
            // Positive AO tag is private to celestial meshes; negative values
            // belong to terrain residency. The fragment path reads eclipse.
            vertex.ao_strength=4.0+c.site.sunlight;
            if vertex.surface[2]==63.0 {
                vertex.surface[0]*=c.site.kind.scale();
                vertex.surface[1]*=c.site.kind.scale();
            }
        }
    }
}
fn lamp(mesh:&mut Model,at:Vec3,size:Vec3,color:[f32;3]) {
    mesh.cuboid(at,size,Quat::IDENTITY,color,0.8);
}
fn foundry(mesh:&mut Model,p:Vec3,day:&DayNightCycle,key:u32) {
    // Two arch ribs in separate planes carry a 6.4 km factory bridge.
    // The open web and hanging bays provide scale against clouds and mountains.
    let node=|i:usize,z:f32| {
        let u=i as f32/16.0;
        p+Vec3::new(-3200.0+6400.0*u,(u*std::f32::consts::PI).sin()*1800.0,z)
    };
    for z in [-240.0,240.0] {
        strut(mesh,p+Vec3::new(-3200.0,0.0,z),p+Vec3::new(3200.0,0.0,z),24.0,METAL);
        for i in 0..16 {
            let a=node(i,z);let b=node(i+1,z);
            strut(mesh,a,b,26.0,SHELL);
            strut(mesh,a,p+Vec3::new(b.x-p.x,0.0,z),12.0,METAL);
            strut(mesh,a,p+Vec3::new(a.x-p.x,0.0,z),8.0,COPPER);
            if i%2==0 {strut(mesh,a,node(i,-z),12.0,METAL);}
        }
    }
    // Eight sealed industrial districts, each comparable to a large shipyard.
    for bay in 0..8 {
        let x=-2800.0+bay as f32*800.0;
        let c=p+Vec3::new(x,-170.0,0.0);
        part(mesh,c,Vec3::new(650.0,220.0,520.0),Quat::IDENTITY,DARK,0.5);
        part(mesh,c+Vec3::Y*125.0,Vec3::new(700.0,35.0,600.0),Quat::IDENTITY,SHELL,0.0);
        if matches!(bay, 2 | 5) {
            // The processing bays have tall heat exchangers and a shared header.
            for dx in [-125.0, 0.0, 125.0] {
                let base = c + Vec3::new(dx, 142.0, 0.0);
                part(mesh, base + Vec3::Y * 140.0, Vec3::new(82.0, 280.0, 170.0), Quat::IDENTITY, DARK, 0.5);
                for y in [50.0, 125.0, 200.0, 275.0] {
                    part(mesh, base + Vec3::Y * y, Vec3::new(108.0, 16.0, 195.0), Quat::IDENTITY, SHELL, 0.5);
                }
            }
            strut(mesh, c + Vec3::new(-160.0, 375.0, 100.0),
                c + Vec3::new(160.0, 375.0, 100.0), 17.0, COPPER);
        } else if matches!(bay, 0 | 7) {
            // Open freight berths on the outer bays read through in silhouette.
            let out = if bay == 0 { -1.0 } else { 1.0 };
            for z in [-235.0, 235.0] {
                let a = c + Vec3::new(out * 220.0, 140.0, z);
                let b = c + Vec3::new(out * 450.0, 140.0, z);
                strut(mesh, a, b, 14.0, METAL);
                strut(mesh, a + Vec3::Y * 130.0, b, 11.0, COPPER);
                strut(mesh, a, a + Vec3::Y * 130.0, 17.0, SHELL);
            }
            part(mesh, c + Vec3::new(out * 430.0, 140.0, 0.0),
                Vec3::new(40.0, 28.0, 490.0), Quat::IDENTITY, METAL, 0.5);
        }
        // Raised ribs, service housings and recessed radiator slots break the
        // broad slabs into functional districts with readable depth at range.
        for rib in 0..5 {
            let dx=-260.0+rib as f32*130.0;
            part(mesh,c+Vec3::new(dx,148.0,0.0),Vec3::new(12.0,12.0,568.0),Quat::IDENTITY,METAL,0.5);
            if rib%2==0 {
                part(mesh,c+Vec3::new(dx,178.0,75.0),Vec3::new(80.0,52.0,150.0),Quat::IDENTITY,[0.44,0.47,0.45],0.0);
            }
        }
        for side in [-1.0,1.0] {
            for panel in 0..6 {
                let dx=-250.0+panel as f32*100.0;
                part(mesh,c+Vec3::new(dx,20.0,side*264.0),Vec3::new(82.0,124.0,7.0),Quat::IDENTITY,[0.38,0.42,0.41],0.0);
                part(mesh,c+Vec3::new(dx,-70.0,side*268.0),Vec3::new(68.0,26.0,10.0),Quat::IDENTITY,DARK,1.0);
            }
            strut(mesh,p+Vec3::new(x-290.0,0.0,side*270.0),c+Vec3::new(-290.0,-120.0,side*270.0),12.0,METAL);
            strut(mesh,p+Vec3::new(x+290.0,0.0,side*270.0),c+Vec3::new(290.0,-120.0,side*270.0),12.0,METAL);
            for deck in 0..4 {
                // Actual three-metre observation strips, not scaled-up windows.
                lamp(mesh,c+Vec3::new(0.0,-55.0+deck as f32*30.0,side*261.0),
                    Vec3::new(440.0,0.3,0.15),[0.84,0.61,0.31]);
            }
            part(mesh,c+Vec3::new(0.0,-125.0,side*145.0),Vec3::new(560.0,18.0,160.0),Quat::IDENTITY,DARK,1.0);
        }
    }
    // Four levitation pylons: visible load path into the buried field coils.
    for x in [-2850.0,-950.0,950.0,2850.0] {
        let bottom=p+Vec3::new(x,-1150.0,0.0);
        for z in [-200.0,200.0] {
            strut(mesh,p+Vec3::new(x,0.0,z),bottom+Vec3::Z*z*0.25,24.0,METAL);
        }
        part(mesh,bottom,Vec3::new(280.0,90.0,280.0),Quat::IDENTITY,SHELL,0.0);
        for side in [-1.0,1.0] {
            lamp(mesh,bottom+Vec3::new(side*141.0,0.0,0.0),Vec3::new(2.0,20.0,100.0),[0.27,0.59,0.62]);
        }
    }
    // One slow maintenance gantry, continuously wrapping along a closed rail.
    // Sinusoidal travel eases at the ends; no pulsing beams or camera effects.
    let phase=(catalog::seconds(day)*std::f64::consts::TAU/30_000.0).rem_euclid(std::f64::consts::TAU) as f32+(key&255) as f32*0.02;
    let x=phase.sin()*2700.0;
    part(mesh,p+Vec3::new(x,90.0,0.0),Vec3::new(180.0,70.0,650.0),Quat::IDENTITY,[0.55,0.36,0.15],0.5);
}
fn habitat(mesh:&mut Model,p:Vec3,day:&DayNightCycle,key:u32) {
    // Stationary central docking spine isolates the two counter-rotating drums.
    part(mesh,p,Vec3::new(4100.0,90.0,100.0),Quat::IDENTITY,METAL,0.5);
    // 0.3 g at the catalog's rendered drum radius. Counter-rotation cancels the
    // nominal angular momenta instead of spinning the whole docking spine.
    let radius=240.0*Kind::TwinHabitat.scale() as f64;
    let spin=(catalog::seconds(day)*(0.3_f64*9.81/radius).sqrt()).rem_euclid(std::f64::consts::TAU) as f32;
    for side in [-1.0,1.0] {
        let c=p+Vec3::Z*side*620.0;
        for x in [-1900.0,1900.0] {
            strut(mesh,p+Vec3::X*x,c+Vec3::X*x,22.0,METAL);
            part(mesh,c+Vec3::X*x,Vec3::new(90.0,150.0,150.0),Quat::IDENTITY,SHELL,0.0);
        }
        let drum=mesh.mark();
        super::pressure::hull(mesh,c,&[
            (-1910.0,0.0),(-1900.0,90.0),(-1870.0,170.0),(-1830.0,220.0),
            (-1780.0,240.0),(1780.0,240.0),(1830.0,220.0),(1870.0,170.0),
            (1900.0,90.0),(1910.0,0.0)],64,SHELL,0.0);
        // External standoff ribs cast their own silhouettes above the pressure
        // skin. Separated service spines replace a smooth, featureless drum.
        for rib in 0..12 {
            let angle=rib as f32*std::f32::consts::TAU/12.0;
            let radial=Vec3::new(0.0,angle.cos(),angle.sin());
            for bay in 0..6 {
                let x=-1500.0+bay as f32*600.0;
                let a=c+Vec3::X*(x-260.0)+radial*270.0;
                let b=c+Vec3::X*(x+260.0)+radial*270.0;
                strut(mesh,a,b,6.0,METAL);
                for end in [-1.0,1.0] {
                    strut(mesh,c+Vec3::X*(x+end*260.0)+radial*240.0,
                        c+Vec3::X*(x+end*260.0)+radial*270.0,4.0,COPPER);
                }
                if rib%3==0 && bay%2==0 {
                    super::detail::vessel(mesh,c+Vec3::X*x+radial*290.0,180.0,34.0,
                        Quat::from_rotation_x(angle),METAL);
                }
            }
        }
        let sectors=64;
        let step=std::f32::consts::TAU/sectors as f32;
        for i in 0..sectors {
            let a=i as f32*step;
            let radial=Vec3::new(0.0,a.cos(),a.sin());
            let q=Quat::from_rotation_x(a);
            for bay in 0..6 {
                let x=-1500.0+bay as f32*600.0;
                let at=c+Vec3::X*x+radial*240.0;
                if i%8==0 {
                    part(mesh,at+radial*2.0,Vec3::new(550.0,3.0,14.0),q,DARK,0.5);
                }
                if (i+bay + key as usize).is_multiple_of(3) {
                    mesh.cuboid(at+radial*6.0,Vec3::new(450.0,0.2,0.5),q,[0.79,0.61,0.32],0.65);
                }
            }
            let next=Vec3::new(0.0,(a+step).cos(),(a+step).sin());
            for x in [-1800.0,-600.0,600.0,1800.0] {
                strut(mesh,c+Vec3::X*x+radial*255.0,c+Vec3::X*x+next*255.0,9.0,METAL);
                if x.abs()>1700.0 {strut(mesh,c+Vec3::X*(x*1.055),c+Vec3::X*x+radial*240.0,6.0,COPPER);}
            }
        }
        mesh.rotate_since(drum,c,Quat::from_rotation_x(side*spin));
    }
    // Outboard heat rejection banks stay fixed to the non-rotating spine.
    for x in [-1.0,1.0] {for z in [-1.0,1.0] {
        let at=p+Vec3::new(x*1800.0,500.0,z*600.0);
        strut(mesh,p+Vec3::X*x*1850.0,at,15.0,METAL);
        for panel in 0..4 {
            part(mesh,at+Vec3::Z*z*panel as f32*180.0,Vec3::new(500.0,8.0,155.0),
                Quat::from_rotation_z(x*0.22),DARK,1.0);
            for fin in 0..6 {
                part(mesh,at+Vec3::new(-210.0+fin as f32*84.0,9.0,z*panel as f32*180.0),
                    Vec3::new(7.0,6.0,148.0),Quat::from_rotation_z(x*0.22),METAL,0.5);
            }
        }
    }}
}

fn unit(key:u32,i:u32)->f32 {(crate::world::hash_u32(key^i.wrapping_mul(0x9e3779b9))>>8) as f32/16777216.0}
/// A ring of sun mirrors on spokes. Most petals still hold their cant; a few are
/// stuck at the wrong angle and a few are gone, leaving bare spoke stubs.
#[cfg(test)]
fn mirror_array(mesh:&mut Model,p:Vec3,day:&DayNightCycle,key:u32) {
    mirror_array_tracking(mesh,p,day,key,day.lighting().sun_dir.normalize());
}

fn mirror_array_tracking(mesh:&mut Model,p:Vec3,day:&DayNightCycle,key:u32,sun:Vec3) {
    const PETALS:u32=18;
    let ring=mesh.mark();
    part(mesh,p,Vec3::new(260.0,120.0,260.0),Quat::IDENTITY,METAL,0.5);
    let spin=(catalog::seconds(day)*std::f64::consts::TAU/60_000.0).rem_euclid(std::f64::consts::TAU) as f32;
    let sun=Quat::from_rotation_y(-spin)*sun;
    // The receiver stays above the mirror plane so reflected rays clear the hub.
    let focus=p+Vec3::Y*750.0;
    strut(mesh,p,focus,18.0,METAL);
    part(mesh,focus,Vec3::new(100.0,160.0,100.0),Quat::IDENTITY,COPPER,0.5);
    for side in [-1.0,1.0] {
        strut(mesh,p+Vec3::X*side*180.0,focus-Vec3::Y*120.0,7.0,METAL);
    }
    let step=std::f32::consts::TAU/PETALS as f32;
    let dir=|i:u32|{let a=i as f32*step;Vec3::new(a.cos(),0.0,a.sin())};
    for i in 0..PETALS {
        let a=i as f32*step;
        let radial=dir(i);
        strut(mesh,p+radial*60.0,p+radial*1200.0,24.0,METAL);
        strut(mesh,p+radial*700.0,p+dir(i+1)*700.0,16.0,METAL);
        let roll=(i+key)%7;
        if roll==0 {
            // Snapped petal: only a bent stub of its frame remains.
            strut(mesh,p+radial*1200.0,p+radial*1300.0+Vec3::Y*70.0,18.0,COPPER);
            continue;
        }
        let leaf=mesh.mark();
        let centre=p+radial*1260.0;
        let to_receiver=(focus-centre).normalize();
        // Reflection normal bisects the directions toward the sun and receiver.
        // Compensate the ring's spin before solving, then pose the whole ring.
        let normal=(sun+to_receiver).try_normalize().unwrap_or(Vec3::Y);
        let tangent=Vec3::new(-a.sin(),0.0,a.cos());
        let projected=radial-normal*radial.dot(normal);
        let x=projected.try_normalize().unwrap_or(tangent);
        let tracking=Quat::from_mat3(&glam::Mat3::from_cols(x,normal,x.cross(normal)));
        let q=if (i+key).is_multiple_of(5) {
            // Failed bearings retain their seeded mechanical jam angle.
            Quat::from_rotation_y(-a)*Quat::from_rotation_z(0.9+unit(key,i)*0.5)
        } else {tracking};
        for panel in 0..8 {
            let x=-341.25+panel as f32*97.5;
            let sag=x*x/6000.0;
            let tilt=(x/3000.0).atan();
            part(mesh,centre+Vec3::new(x,sag,0.0),Vec3::new(95.0,4.0,404.0),
                Quat::from_rotation_z(tilt),[0.58,0.65,0.69],0.65);
        }
        for z in [-210.0,210.0] {
            for panel in 0..8 {
                let x0=-390.0+panel as f32*97.5;
                let x1=x0+97.5;
                strut(mesh,centre+Vec3::new(x0,x0*x0/6000.0,z),
                    centre+Vec3::new(x1,x1*x1/6000.0,z),5.0,SHELL);
            }
        }
        strut(mesh,centre-Vec3::Z*210.0,centre+Vec3::Z*210.0,7.0,METAL);
        mesh.rotate_since(leaf,centre,q);
    }
    // One slow turn per few minutes; the whole ring wheels, the hub stays fixed.
    mesh.rotate_since(ring,p,Quat::from_rotation_y(spin));
}
/// A dead relay tumbling end over end: one solar wing intact, the other snapped
/// with its panels drifting off, the dish cracked and canted. No lamps.
fn dead_relay(mesh:&mut Model,p:Vec3,day:&DayNightCycle,key:u32) {
    const WEATHERED:[f32;3]=[0.30,0.27,0.24];
    const RUST:[f32;3]=[0.38,0.22,0.12];
    let body=mesh.mark();
    strut(mesh,p-Vec3::X*350.0,p+Vec3::X*350.0,30.0,WEATHERED);
    part(mesh,p,Vec3::new(200.0,150.0,150.0),Quat::IDENTITY,WEATHERED,0.0);
    // Intact wing: arm and five panels.
    strut(mesh,p+Vec3::new(90.0,0.0,0.0),p+Vec3::new(90.0,0.0,420.0),16.0,METAL);
    for j in 0..5 {
        part(mesh,p+Vec3::new(90.0,0.0,70.0+j as f32*75.0),Vec3::new(130.0,10.0,66.0),Quat::IDENTITY,DARK,1.0);
    }
    // Snapped wing: a short stump, then panels adrift at odd angles.
    strut(mesh,p+Vec3::new(-90.0,0.0,0.0),p+Vec3::new(-90.0,10.0,-130.0),18.0,RUST);
    for j in 0..4 {
        let at=p+Vec3::new(-90.0+(unit(key,10+j)-0.5)*220.0,(unit(key,20+j)-0.5)*180.0,-200.0-j as f32*110.0);
        let q=Quat::from_euler(glam::EulerRot::XYZ,unit(key,30+j)*3.0,unit(key,40+j)*3.0,unit(key,50+j)*3.0);
        part(mesh,at,Vec3::new(130.0,10.0,66.0),q,DARK,1.0);
    }
    // Dish panels tore off in a sector, exposing ribs and the skewed receiver.
    let dish = p + Vec3::new(355.0, 25.0, 0.0);
    let pose = Quat::from_rotation_z(0.55);
    let rim = |i: u32| {
        let angle = i as f32 * std::f32::consts::TAU / 12.0;
        dish + pose * Vec3::new(angle.cos() * 95.0, 35.0, angle.sin() * 95.0)
    };
    let missing = key % 12;
    for i in 0..12 {
        let a = rim(i);
        let b = rim(i + 1);
        strut(mesh, dish, a, 4.5, WEATHERED);
        if (i + 12 - missing) % 12 >= 3 {
            strut(mesh, a, b, 5.0, RUST);
            strut(mesh, dish.lerp(a, 0.5), dish.lerp(b, 0.5), 7.0, WEATHERED);
        }
    }
    let receiver = dish + pose * Vec3::new(15.0, 80.0, -8.0);
    for i in [1, 5, 9] {
        strut(mesh, rim(i), receiver, 3.0, METAL);
    }
    part(mesh, receiver, Vec3::new(22.0, 35.0, 22.0), pose, RUST, 0.0);
    // A few loose fragments.
    for j in 0..6 {
        let at=p+Vec3::new((unit(key,60+j)-0.5)*900.0,(unit(key,70+j)-0.5)*300.0,(unit(key,80+j)-0.5)*500.0);
        part(mesh,at,Vec3::splat(12.0+unit(key,90+j)*26.0),Quat::from_rotation_y(unit(key,100+j)*3.0),WEATHERED,0.0);
    }
    // Slow tumble about an axis fixed by the site key.
    let axis=Vec3::new(0.35+unit(key,1),0.2+unit(key,2),0.6+unit(key,3)).normalize();
    let angle=(catalog::seconds(day)*std::f64::consts::TAU/40_000.0).rem_euclid(std::f64::consts::TAU) as f32;
    mesh.rotate_since(body,p,Quat::from_axis_angle(axis,angle));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent(mesh:&Model,centre:Vec3)->f32 {
        mesh.vertices.iter().map(|v|(Vec3::from_array(v.pos)-centre).length()).fold(0.0,f32::max)
    }

    #[test]
    fn mirror_array_and_dead_relay_emit_bounded_geometry() {
        let day=DayNightCycle::default();
        let c=Vec3::new(10.0,20.0,30.0);
        let mut mirror=Model::new(1.0);
        mirror_array(&mut mirror,c,&day,7);
        assert!(mirror.vertices.len()>1_000,"{} vertices",mirror.vertices.len());
        // Authored ring: 1.2 km spokes plus petals out to ~1.7 km.
        assert!(extent(&mirror,c)<1_800.0,"{}",extent(&mirror,c));
        let mut relay=Model::new(1.0);
        dead_relay(&mut relay,c,&day,7);
        assert!(relay.vertices.len()>500,"{} vertices",relay.vertices.len());
        // Truss, wings, dish and drifting fragments stay within a kilometre.
        assert!(extent(&relay,c)<1_000.0,"{}",extent(&relay,c));
    }

    #[test]
    fn mirror_array_turns_with_the_orbital_clock() {
        let c=Vec3::ZERO;
        let mut a=Model::new(1.0);
        mirror_array(&mut a,c,&DayNightCycle::default(),7);
        let mut later=DayNightCycle::default();
        later.advance(20.0);
        let mut b=Model::new(1.0);
        mirror_array(&mut b,c,&later,7);
        assert_eq!(a.vertices.len(),b.vertices.len());
        assert!(a.vertices.iter().zip(&b.vertices).any(|(p,q)|p.pos!=q.pos),"ring did not move");
    }
}
