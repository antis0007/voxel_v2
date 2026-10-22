//! Sealed aerial infrastructure, authored in metres. Sparse seed placement and
//! calendar-driven mechanisms are independent of terrain/entity residency.
use super::{Model, Quat, Vec3, architecture::{part,strut}};
use crate::world::{World,DayNightCycle,orbital::megastructures::{self as catalog,Kind}};

const SHELL:[f32;3]=[0.67,0.69,0.65];
const METAL:[f32;3]=[0.22,0.29,0.32];
const DARK:[f32;3]=[0.065,0.11,0.15];
const COPPER:[f32;3]=[0.43,0.25,0.13];

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
            Kind::Foundry=>foundry(mesh,c.site.position,day,c.site.key),
            Kind::TwinHabitat=>habitat(mesh,c.site.position,day,c.site.key),
        }
        mesh.scale_since(mark,c.site.position,Vec3::splat(c.site.kind.scale()));
        mesh.rotate_since(mark,c.site.position,c.site.orientation);
        long_range_since(mesh,mark);
        for vertex in &mut mesh.vertices[mark..] {
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
        for side in [-1.0,1.0] {
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
    // 0.3 g at the real 2.4 km drum radius. Counter-rotation cancels the
    // nominal angular momenta instead of spinning the whole docking spine.
    let spin=(catalog::seconds(day)*(0.3_f64*9.81/2400.0).sqrt()).rem_euclid(std::f64::consts::TAU) as f32;
    for side in [-1.0,1.0] {
        let c=p+Vec3::Z*side*620.0;
        for x in [-1900.0,1900.0] {
            strut(mesh,p+Vec3::X*x,c+Vec3::X*x,22.0,METAL);
            part(mesh,c+Vec3::X*x,Vec3::new(90.0,150.0,150.0),Quat::IDENTITY,SHELL,0.0);
        }
        let drum=mesh.mark();
        let sectors=16;
        let step=std::f32::consts::TAU/sectors as f32;
        for i in 0..sectors {
            let a=i as f32*step;
            let radial=Vec3::new(0.0,a.cos(),a.sin());
            let q=Quat::from_rotation_x(a);
            part(mesh,c+radial*232.0,Vec3::new(3600.0,12.0,2.0*232.0*(step*0.5).tan()),q,DARK,0.5);
            for bay in 0..6 {
                let x=-1500.0+bay as f32*600.0;
                let at=c+Vec3::X*x+radial*240.0;
                part(mesh,at,Vec3::new(585.0,10.0,2.0*240.0*(step*0.5).tan()),q,
                    if i%4==0 {DARK} else {SHELL},if i%4==0 {1.0} else {0.0});
                if (i+bay+key as usize)%3==0 {
                    mesh.cuboid(at+radial*6.0,Vec3::new(450.0,0.2,0.5),q,[0.79,0.61,0.32],0.65);
                }
            }
            let next=Vec3::new(0.0,(a+step).cos(),(a+step).sin());
            for end in [-1.0,1.0] {
                let cap=c+Vec3::X*end*1800.0;
                let cap_mark=mesh.mark();
                mesh.set_surface_kind(62.0);
                mesh.triangle([cap,cap+radial*240.0,cap+next*240.0],Vec3::X*end,SHELL,0.0);
                mesh.bake_model_uv_since(cap_mark,cap,62.0);
                mesh.set_surface_kind(41.0);
            }
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
        }
    }}
}
