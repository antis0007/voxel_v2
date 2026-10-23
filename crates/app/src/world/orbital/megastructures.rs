//! Deterministic two-body circular orbits above a compressed spherical datum.
//! The voxel world is a local surface chart; f64 observer frames project celestial
//! positions into its tangent sky. No aircraft-style translation or camera cards.
use glam::{DQuat,DVec3,Mat3,Quat,Vec3};
use crate::world::{hash_u32,DayNightCycle};
/// The celestial layer models a planet SKY_SCALE times smaller than Earth.
/// Orbits, tethers and altitudes shrink with it while hardware keeps its
/// authored size, so it reads SKY_SCALE times larger at the same sky angles.
pub const SKY_SCALE:f64=4.0;
pub const PLANET_RADIUS_M:f64=6_371_000.0/SKY_SCALE;
/// One world day on the orbital clock. The planet turns once per world day, so
/// the sun, planet spin and eclipses share one frame.
pub const DAY_SECONDS:f64=86400.0;
/// Laps per world day of the reference shell (the foundry's). Real low orbit is
/// about 15, which on the accelerated day gives three-second passes; 3 keeps a
/// pass watchable while staying clear of ground-synchronous creep.
pub const ORBITS_PER_DAY:f64=3.0;
const REFERENCE_RADIUS_M:f64=PLANET_RADIUS_M+420_000.0/SKY_SCALE;
const REFERENCE_RATE:f64=std::f64::consts::TAU*ORBITS_PER_DAY/DAY_SECONDS;
/// Gravitational parameter tuned so the reference shell meets ORBITS_PER_DAY;
/// other altitudes follow Kepler from it.
pub const MU:f64=REFERENCE_RATE*REFERENCE_RATE*REFERENCE_RADIUS_M*REFERENCE_RADIUS_M*REFERENCE_RADIUS_M;
/// Width of the soft eclipse edge, metres.
pub const PENUMBRA_M:f64=12_000.0/SKY_SCALE;
/// Coverage for supported atmospheric crowns only, not celestial objects.
pub const RANGE_M:f32=20_000.0;

#[derive(Clone,Copy,Debug)]
pub enum Kind { Foundry,TwinHabitat }
impl Kind {
    pub fn name(self)->&'static str {match self {Self::Foundry=>"Loom orbital foundry",Self::TwinHabitat=>"Continuum twin habitat"}}
    pub fn description(self)->&'static str {match self {
        Self::Foundry=>"64 km orbital fabrication truss · sealed industrial districts",
        Self::TwinHabitat=>"Paired 36 km pressure cylinders · 4.8 km diameter · counter-rotating living drums",
    }}
    /// Overhead sweep on the default day (480 s, orbital clock x180): the foundry
    /// at 110 km crosses the sky at ~34 deg/s, the habitat at 130 km at ~28.
    /// Shells above ~200 km read as static from the ground; the smooth clock in
    /// main.rs (`lead_seconds`) is what keeps this speed from looking like jumps.
    pub fn altitude_m(self)->f64 {(match self {Self::Foundry=>440_000.0,Self::TwinHabitat=>520_000.0})/SKY_SCALE}
    pub fn scale(self)->f32 {10.0}
    pub fn bound_m(self)->f64 {match self {Self::Foundry=>40_000.0,Self::TwinHabitat=>24_000.0}}
}
#[derive(Clone,Copy,Debug)]
pub struct Site {pub key:u32,pub kind:Kind,pub position:Vec3,pub orientation:Quat,pub sunlight:f32}
#[derive(Clone,Copy)]
pub struct Contact {pub site:Site,pub distance_m:f32,pub bearing:u32,pub elevation_deg:f32,pub period_seconds:f64}

/// The shared orbital clock in world seconds. Skyhooks use the same clock and
/// planet spin, so every orbital object agrees on where the ground and the sun are.
pub fn seconds(day:&DayNightCycle)->f64 {super::skyhook::clock(day)}
fn planet_spin(time:f64)->DQuat {DQuat::from_rotation_z(-(super::skyhook::EARTH_RATE*time).rem_euclid(std::f64::consts::TAU))}
fn unit(key:u32)->f64 {(hash_u32(key)>>8) as f64/16777216.0}

// The surface chart uses east=X, north=-Z, altitude=Y at its origin.
struct Observer {position:DVec3,east:DVec3,up:DVec3,south:DVec3}
impl Observer {
    fn new(viewer:Vec3,time:f64)->Self {
        let lat=-(viewer.z as f64)/PLANET_RADIUS_M;
        let lon=viewer.x as f64/PLANET_RADIUS_M;
        let up=DVec3::new(lon.sin()*lat.cos(),lon.cos()*lat.cos(),-lat.sin());
        let east=DVec3::new(lon.cos(),-lon.sin(),0.0);
        let south=DVec3::new(lon.sin()*lat.sin(),lon.cos()*lat.sin(),lat.cos());
        let rotation=planet_spin(time);
        Self {position:rotation*up*(PLANET_RADIUS_M+viewer.y as f64),east:rotation*east,up:rotation*up,south:rotation*south}
    }
    fn local(&self,v:DVec3)->Vec3 {Vec3::new(v.dot(self.east) as f32,v.dot(self.up) as f32,v.dot(self.south) as f32)}
}

pub fn contacts(seed:u32,day:&DayNightCycle,viewer:Vec3)->Vec<Contact> {
    let time=seconds(day);
    let observer=Observer::new(viewer,time);
    // Lighting gives the local sun direction. Rotate the same world-origin
    // direction into inertial space for the planet's cylindrical eclipse test.
    let spin=planet_spin(time);
    let sun=spin*day.lighting().sun_dir.as_dvec3().normalize();
    let mut result=Vec::with_capacity(2);
    for (kind,salt) in [(Kind::Foundry,0x10a1_u32),(Kind::TwinHabitat,0x10a2)] {
        // One of each, as the lore has it ("the Loom", "the Continuum"). Three
        // members spaced member/3 day apart were a whole lap apart at
        // ORBITS_PER_DAY = 3, so they all hung in the same patch of sky.
        for member in 0..1_u32 {
            let key=hash_u32(seed^salt^member.wrapping_mul(0x9e3779b9));
            let epoch=(1.45+unit(key)*0.02+member as f64/3.0)*DAY_SECONDS;
            let radius=PLANET_RADIUS_M+kind.altitude_m();
            let rate=(MU/radius.powi(3)).sqrt();
            let angle=((time-epoch)*rate).rem_euclid(std::f64::consts::TAU);
            let inclination=(32.0+unit(key^7)*28.0).to_radians();
            let epoch_frame=DQuat::from_rotation_z(-(epoch/DAY_SECONDS).rem_euclid(1.0)*std::f64::consts::TAU);
            let u=epoch_frame*DVec3::Y;
            let v=epoch_frame*DVec3::new(inclination.cos(),0.0,-inclination.sin());
            let radial=u*angle.cos()+v*angle.sin();
            let tangent=-u*angle.sin()+v*angle.cos();
            let position=radial*radius;
            let relative=observer.local(position-observer.position);
            let distance=relative.length();
            let elevation=(relative.y/distance).clamp(-1.0,1.0).asin();
            // Keep bodies intersecting the limb; the fragment ray/sphere test
            // clips the hidden part instead of popping the entire object.
            let dip=(PLANET_RADIUS_M/(PLANET_RADIUS_M+viewer.y.max(0.0) as f64)).clamp(0.0,1.0).acos();
            if (elevation as f64) < -dip-(kind.bound_m()/distance as f64).asin() {continue;}
            let behind=position.dot(sun);
            let shadow_distance=(position-sun*behind).length();
            let sunlight=if behind<0.0 {((shadow_distance-PLANET_RADIUS_M)/PENUMBRA_M+0.5).clamp(0.0,1.0)} else {1.0};
            let x=observer.local(tangent);let y=observer.local(radial);let z=x.cross(y).normalize();
            result.push(Contact {site:Site {key,kind,position:viewer+relative,
                orientation:Quat::from_mat3(&Mat3::from_cols(x,y,z)),sunlight:sunlight as f32},
                distance_m:distance,bearing:relative.x.atan2(-relative.z).to_degrees().rem_euclid(360.0).round() as u32%360,
                elevation_deg:elevation.to_degrees(),period_seconds:std::f64::consts::TAU/rate});
        }
    }
    result.sort_by(|a,b|a.distance_m.total_cmp(&b.distance_m));result
}
