//! Persistent freight momentum and small-strain elastic modes. No camera or
//! renderer owns this state. One fixed-step update serves loaded and remote hooks.
use super::{DayNightCycle, skyhook::{self, Hook, Size}, megastructures::{MU, PLANET_RADIUS_M}};
use glam::{DVec3, Vec3};
use std::f64::consts::{PI, TAU};

pub const IRON_PER_CRATE: u32 = 8;
pub const IRON_KG: f64 = 20.0;
pub const LINE_NODES: usize = 17;
const YOUNG_PA: f64 = 80.0e9;
const FIBRE_KG_M3: f64 = 1600.0;
const STRAIN_LIMIT: f64 = 0.025;
const STOCK_LIMIT: u32 = 100_000;

fn slot(size: Size)->usize {match size {Size::Courier=>0,Size::Talon=>1,Size::Ark=>2}}
fn width(size: Size)->f64 {match size {Size::Courier=>2.0,Size::Talon=>8.0,Size::Ark=>30.0}}
fn hub_mass(size: Size)->f64 {match size {Size::Courier=>4.0e6,Size::Talon=>4.0e7,Size::Ark=>4.0e8}}
fn area(size: Size)->f64 {width(size).powi(2)/1000.0}
fn dir(a:f64)->DVec3 {DVec3::new(a.sin(),a.cos(),0.0)}
fn ease(t:f64)->f64 {let t=t.clamp(0.0,1.0);t*t*t*(t*(t*6.0-15.0)+10.0)}

#[derive(Clone,Copy,Default)]
struct Mode {q:f64,v:f64}
impl Mode {
    // Exact damped oscillator update under constant force over this substep.
    // Material wave speed sets frequency; oscillation is not a decorative sine.
    fn step(&mut self, force:f64, stiffness:f64, mass:f64, damping:f64, dt:f64, initial:bool) {
        let target=force/stiffness;
        if initial {self.q=target;self.v=0.0;return;}
        let omega=(stiffness/mass).sqrt();
        let decay=damping*omega;
        let wd=omega*(1.0-damping*damping).sqrt();
        let a=self.q-target;
        let b=(self.v+decay*a)/wd;
        let (s,c)=(wd*dt).rem_euclid(TAU).sin_cos();
        let e=(-decay*dt).exp();
        self.q=target+e*(a*c+b*s);
        self.v=e*((-decay*a+wd*b)*c+(-decay*b-wd*a)*s);
    }
}

#[derive(Clone,Copy,Default)]
struct Elastic {axial:[Mode;3], lateral:[Mode;2], energy_j:f64}
impl Elastic {
    fn offset(&self,s:f64)->(f64,f64) {
        let sum=|modes:&[Mode]| modes.iter().enumerate().map(|(j,m)|m.q*((j as f64+0.5)*PI*s).sin()).sum();
        (sum(&self.axial),sum(&self.lateral))
    }
    fn strain(&self,length:f64)->f64 {
        (0..=16).map(|i| {
            let s=i as f64/16.0;
            let axial=self.axial.iter().enumerate().map(|(j,m)| {
                let k=(j as f64+0.5)*PI;
                m.q*k/length*(k*s).cos()
            }).sum::<f64>();
            let lateral=self.lateral.iter().enumerate().map(|(j,m)| {
                let k=(j as f64+0.5)*PI;m.q*k/length*(k*s).cos()
            }).sum::<f64>();
            (axial+0.5*lateral*lateral).abs()
        }).fold(0.0,f64::max)
    }
    fn step(&mut self,hook:Hook,hub:DVec3,t:f64,payload:f64,dt:f64,initial:bool) {
        let length=hook.tether_m;
        let axis=(hook.tip(t)-hook.hub(t)).normalize();
        let side=DVec3::new(axis.y,-axis.x,0.0);
        let lambda=area(hook.size)*FIBRE_KG_M3;
        let gravity=|p:DVec3| -p*(MU/p.length().powi(3));
        let g0=gravity(hub);
        let acceleration=|s:f64| {
            let x=axis*(length*s);
            x*(hook.spin*hook.spin)+gravity(hub+x)-g0
        };
        let mean_tension=hook.design_tension_n(0.5,payload,lambda).max(1.0);
        self.energy_j=0.0;
        for (lateral,modes) in [(false,&mut self.axial[..]),(true,&mut self.lateral[..])] {
            for (j,m) in modes.iter_mut().enumerate() {
                let k=(j as f64+0.5)*PI;
                let component=if lateral {side} else {axis};
                let mut force=payload*acceleration(1.0).dot(component)*k.sin();
                for i in 0..12 {
                    let s=(i as f64+0.5)/12.0;
                    force+=lambda*length/12.0*acceleration(s).dot(component)*(k*s).sin();
                }
                let mass=lambda*length*0.5+payload;
                let stiffness=if lateral {mean_tension} else {YOUNG_PA*area(hook.size)}*k*k/(2.0*length);
                m.step(force,stiffness,mass,if lateral {0.08} else {0.04},dt,initial);
                self.energy_j+=0.5*(mass*m.v*m.v+stiffness*m.q*m.q);
            }
        }
    }
}

#[derive(Clone,Copy,Default)]
struct Body {radius:f64,radial_v:f64,angle:f64,delta_l:f64,elastic:Elastic}
impl Body {
    fn h(&self,hook:Hook)->f64 {(MU*hook.hub_radius_m).sqrt()+self.delta_l/hub_mass(hook.size)}
    /// Exact planar Kepler drift between cargo momentum transfers. Radius and
    /// radial velocity remain continuous when a transfer changes angular momentum.
    fn drift(&mut self,hook:Hook,dt:f64) {
        let h=self.h(hook);
        let ec=h*h/(MU*self.radius)-1.0;
        let es=self.radial_v*h/MU;
        let e=ec.hypot(es);
        if e<1.0e-10 {self.angle=(self.angle+h/(self.radius*self.radius)*dt).rem_euclid(TAU);return;}
        let a=h*h/(MU*(1.0-e*e));
        let f=es.atan2(ec);
        let root=(1.0-e*e).sqrt();
        let e0=(root*f.sin()).atan2(e+f.cos());
        let mean=(e0-e*e0.sin()+(MU/a.powi(3)).sqrt()*dt).rem_euclid(TAU);
        let mut anomaly=mean;
        for _ in 0..10 {anomaly-=(anomaly-e*anomaly.sin()-mean)/(1.0-e*anomaly.cos());}
        let next=(root*anomaly.sin()).atan2(anomaly.cos()-e);
        self.radius=a*(1.0-e*anomaly.cos());
        self.radial_v=(MU*a).sqrt()/self.radius*e*anomaly.sin();
        self.angle=(self.angle+next-f).rem_euclid(TAU);
    }
}

#[derive(Clone,Copy)]
pub struct Shipment {pub up:bool,pub cusp_s:f64,impulse:f64,applied:f64,handoff:DVec3}
impl Shipment {
    pub fn start(self,hook:Hook)->f64 {self.cusp_s-hook.turn_period()*if self.up {0.3} else {0.75}}
    pub fn end(self,hook:Hook)->f64 {self.cusp_s+hook.turn_period()*if self.up {0.75} else {0.3}}
    fn fraction(self,hook:Hook,t:f64)->f64 {
        // Ground-side lift draws energy from the atmospheric service. The hook
        // lends orbital momentum only after capture; descending mass repays it.
        let start=if self.up {self.cusp_s} else {self.start(hook)};
        let end=if self.up {self.end(hook)} else {self.cusp_s};
        ease((t-start)/(end-start))
    }
}

#[derive(Default)]
pub struct State {
    /// Disposable native review fixture owns time explicitly. Never serialized.
    pub(crate) manual_clock:bool,
    initialized:bool,seed:u32,epoch:f64,last_calendar:f64,bodies:[Body;3],elevator:[Mode;2],
    pub orbital_iron:u32,pub delivered_iron:u32,pub lifted_kg:u64,pub lowered_kg:u64,
    stock_momentum:f64,import_momentum:f64,pub shipment:Option<Shipment>,
}

impl State {
    pub fn advance(&mut self,seed:u32,day:&DayNightCycle) {
        let now=skyhook::clock(day);
        self.last_calendar=now;
        self.advance_to(seed,day,now);
    }
    pub fn time_seconds(&self)->Option<f64> {self.initialized.then_some(self.epoch)}
    pub fn tick(&mut self,seed:u32,day:&DayNightCycle,dt:f32) {
        if self.manual_clock {return;}
        if !self.initialized {self.advance(seed,day);}
        let calendar=skyhook::clock(day);
        let now=if (calendar-self.last_calendar).abs()>600.0 {calendar.max(self.epoch)}
            else {self.epoch+dt as f64*super::megastructures::DAY_SECONDS/crate::world::DEFAULT_DAY_LENGTH_SECONDS as f64};
        self.last_calendar=calendar;
        self.advance_to(seed,day,now);
    }
    fn advance_to(&mut self,seed:u32,day:&DayNightCycle,now:f64) {
        let hooks=skyhook::hooks(seed);
        if !self.initialized {
            self.seed=seed;self.epoch=now;self.initialized=true;
            for (b,h) in self.bodies.iter_mut().zip(hooks) {
                let p=h.hub(now);b.radius=h.hub_radius_m;b.angle=p.x.atan2(p.y);
                b.elastic.step(h,p,now,0.0,0.0,true);
            }
            return;
        }
        // Calendar rewinds never refund cargo or replay a completed transfer.
        if now<=self.epoch {return;}
        let old=self.epoch;
        let mut breaks=vec![old,now];
        if let Some(s)=self.shipment {
            for t in [s.start(hooks[1]),s.cusp_s,s.end(hooks[1])] {if t>old && t<now {breaks.push(t);}}
        }
        breaks.sort_by(f64::total_cmp);
        for window in breaks.windows(2) {
            for sub in 0..4 {
                let dt=(window[1]-window[0])/4.0;
                let t=window[0]+(sub+1) as f64*dt;
                let mut delta=0.0;
                let mut payload=0.0;
                if let Some(s)=self.shipment.as_mut() {
                    let fraction=s.fraction(hooks[1],t);
                    delta=s.impulse*(fraction-s.applied)*if s.up {-1.0} else {1.0};
                    s.applied=fraction;
                    if (s.up && t>=s.cusp_s && t<=s.end(hooks[1]))
                        || (!s.up && t>=s.start(hooks[1]) && t<=s.cusp_s) {payload=IRON_PER_CRATE as f64*IRON_KG;}
                }
                for (i,(b,h)) in self.bodies.iter_mut().zip(hooks).enumerate() {
                    b.drift(h,dt*0.5);
                    if i==1 {b.delta_l+=delta;}
                    b.drift(h,dt*0.5);
                    b.elastic.step(h,dir(b.angle)*b.radius,t,if i==1 {payload} else {0.0},dt,false);
                }
                // A single atmospheric reference string responds to actual wind
                // drag. Rendering scales its anchored bow for each span's length.
                let wind=day.cloud_controls();
                let speed=wind.wind_speed as f64*30.0;
                let angle=(wind.wind_dir_angle as f64).to_radians();
                let air_density=1.225*8000.0/18_000.0*(1.0-(-18_000.0_f64/8000.0).exp());
                let drag=0.5*air_density*1.1*2.0*speed*speed;
                let stiffness=80.0e6*PI*PI/(2.0*18_000.0);
                for (m,direction) in self.elevator.iter_mut().zip([angle.cos(),angle.sin()]) {
                    m.step(drag*18_000.0*2.0/PI*direction,stiffness,
                        FIBRE_KG_M3*0.1*18_000.0/2.0,0.15,dt,false);
                }
            }
            self.epoch=window[1];
            if let Some(s)=self.shipment {
                if window[0]<s.cusp_s && window[1]>=s.cusp_s {
                    let handoff=skyhook::fixed(self.point(hooks[1],s.cusp_s,1.0),s.cusp_s);
                    self.shipment.as_mut().unwrap().handoff=handoff;
                }
            }
        }
        if let Some(s)=self.shipment.filter(|s|now>=s.end(hooks[1])) {
            let mass=(IRON_PER_CRATE as f64*IRON_KG) as u64;
            if s.up {self.orbital_iron+=IRON_PER_CRATE;self.stock_momentum+=s.impulse;self.lifted_kg+=mass;}
            else {self.delivered_iron+=IRON_PER_CRATE;self.lowered_kg+=mass;}
            self.shipment=None;
        }
        self.epoch=now;
    }

    fn reference_impulse(hook:Hook)->f64 {
        (MU*hook.hub_radius_m).sqrt()-skyhook::EARTH_RATE*(hook.hub_radius_m-hook.tether_m).powi(2)
    }
    pub fn capacity_kg(&self,seed:u32)->f64 {
        let h=skyhook::hooks(seed)[1];
        let floor=h.hub_radius_m-h.size.cusp_altitude_m()*0.2;
        let reserve=hub_mass(h.size)*((MU*h.hub_radius_m).sqrt()-(MU*floor).sqrt())+self.bodies[1].delta_l;
        let held=self.shipment.filter(|s|s.up).map_or(0.0,|s|s.impulse*(1.0-s.applied));
        ((reserve-held)/Self::reference_impulse(h)).max(0.0)
    }
    pub fn strain(&self,seed:u32)->f64 {self.bodies[1].elastic.strain(skyhook::hooks(seed)[1].tether_m)}
    pub fn energy_j(&self)->f64 {self.bodies.iter().map(|b|b.elastic.energy_j).sum()}
    pub fn audit_error(&self)->f64 {
        let in_transit=self.shipment.map_or(0.0,|s|s.impulse*if s.up {s.applied} else {1.0-s.applied});
        self.bodies[1].delta_l+self.stock_momentum+in_transit-self.import_momentum
    }
    pub fn can_lift(&self,seed:u32)->bool {
        self.shipment.is_none() && self.orbital_iron<=STOCK_LIMIT-IRON_PER_CRATE
            && self.capacity_kg(seed)>=IRON_PER_CRATE as f64*IRON_KG && self.strain(seed)<STRAIN_LIMIT
    }
    pub fn can_lower(&self)->bool {
        self.shipment.is_none() && self.orbital_iron>=IRON_PER_CRATE && self.delivered_iron<=STOCK_LIMIT-IRON_PER_CRATE
    }
    pub fn queue(&mut self,seed:u32,day:&DayNightCycle,up:bool)->bool {
        self.advance(seed,day);
        if (up && !self.can_lift(seed)) || (!up && !self.can_lower()) {return false;}
        let hook=skyhook::hooks(seed)[1];
        let now=self.epoch;
        let lead=hook.turn_period()*if up {0.3} else {0.75};
        let mut cusp=now+hook.next_port_cusp(now);
        if cusp-now<lead {cusp+=hook.pass_period();}
        let impulse=if up {Self::reference_impulse(hook)*IRON_PER_CRATE as f64*IRON_KG}
            else if self.orbital_iron==IRON_PER_CRATE {self.stock_momentum}
            else {self.stock_momentum*(IRON_PER_CRATE as f64/self.orbital_iron as f64)};
        if !up {self.orbital_iron-=IRON_PER_CRATE;self.stock_momentum-=impulse;}
        let handoff=skyhook::fixed(self.point(hook,cusp,1.0),cusp);
        self.shipment=Some(Shipment {up,cusp_s:cusp,impulse,applied:0.0,handoff});
        true
    }
    /// Finite receiver contracts import cargo from the existing orbital commons.
    /// Credit occurs when the sealed shipment is actually unloaded, once only.
    pub fn credit_import(&mut self,seed:u32,day:&DayNightCycle,mass_kg:f64) {
        self.advance(seed,day);
        let impulse=Self::reference_impulse(skyhook::hooks(seed)[1])*mass_kg;
        self.bodies[1].delta_l+=impulse;self.import_momentum+=impulse;
        self.lowered_kg+=mass_kg.round() as u64;
    }

    fn point(&self,hook:Hook,t:f64,s:f64)->DVec3 {
        if !self.initialized {return hook.along(t,s);}
        let mut b=self.bodies[slot(hook.size)];b.drift(hook,t-self.epoch);
        let axis=(hook.tip(t)-hook.hub(t)).normalize();
        let side=DVec3::new(axis.y,-axis.x,0.0);
        let (axial,lateral)=b.elastic.offset(s);
        dir(b.angle)*b.radius+axis*(hook.tether_m*s+axial)+side*lateral
    }
    pub fn line(&self,hook:Hook,t:f64,viewer:Vec3)->[Vec3;LINE_NODES] {
        // Propagate a body once per line, not once for every tessellation vertex.
        let (hub,elastic)=if self.initialized {
            let mut b=self.bodies[slot(hook.size)];b.drift(hook,t-self.epoch);
            (dir(b.angle)*b.radius,b.elastic)
        } else {(hook.hub(t),Elastic::default())};
        let axis=(hook.tip(t)-hook.hub(t)).normalize();
        let side=DVec3::new(axis.y,-axis.x,0.0);
        std::array::from_fn(|i| {
            let s=i as f64/(LINE_NODES-1) as f64;let (a,l)=elastic.offset(s);
            skyhook::local(skyhook::fixed(hub+axis*(hook.tether_m*s+a)+side*l,t),viewer)
        })
    }
    pub fn tip_altitude(&self,hook:Hook,t:f64)->f32 {(self.point(hook,t,1.0).length()-PLANET_RADIUS_M) as f32}
    pub fn elevator_bow(&self)->Vec3 {Vec3::new(self.elevator[0].q as f32,0.0,self.elevator[1].q as f32)}
    pub fn cargo_axis(&self,contact:&skyhook::Contact,t:f64)->Vec3 {
        let Some(s)=self.shipment else {return Vec3::Y;};
        let direction=(contact.hub-contact.tip).normalize();
        let blend=if s.up {ease((t-s.start(contact.hook))/(s.cusp_s-s.start(contact.hook))*5.0-4.0)}
            else {1.0-ease((t-s.cusp_s)/(s.end(contact.hook)-s.cusp_s)*5.0)};
        Vec3::Y.lerp(direction,blend as f32).normalize()
    }
    // Terrain at the port is needed only during the actual ground handoff.
    // An empty manifest or a capsule on the tether must not trigger a survey.
    pub fn cargo(&self,contact:&skyhook::Contact,port:impl FnOnce()->Vec3,t:f64,viewer:Vec3)->Option<Vec3> {
        if contact.hook.size!=Size::Talon {return None;}
        let s=self.shipment?;let hook=contact.hook;
        if t<s.start(hook) || t>s.end(hook) {return None;}
        let along=|fraction:f64| {
            let f=(fraction.clamp(0.0,1.0)*(LINE_NODES-1) as f64) as f32;
            let i=(f.floor() as usize).min(LINE_NODES-2);
            contact.line[i].lerp(contact.line[i+1],(f-i as f32).clamp(0.0,1.0))
        };
        if s.up {
            if t<s.cusp_s {return Some(port().lerp(contact.cusp,ease((t-s.start(hook))/(hook.turn_period()*0.3)) as f32));}
            let haul=s.cusp_s+hook.turn_period()*0.5;
            Some(along(1.0-ease((t-haul)/(hook.turn_period()*0.25))))
        } else if t<=s.cusp_s {
            Some(along(ease((t-s.start(hook))/(hook.turn_period()*0.25))))
        } else {
            let tip=skyhook::local(s.handoff,viewer);
            Some(tip.lerp(port(),ease((t-s.cusp_s)/(hook.turn_period()*0.3)) as f32))
        }
    }

    pub fn encode(&self,bytes:&mut Vec<u8>) {
        let mut put=|v:u32|bytes.extend_from_slice(&v.to_le_bytes());
        put(u32::from(self.initialized));put(self.seed);
        let mut float=|v:f64| {let b=v.to_bits();put(b as u32);put((b>>32) as u32);};
        float(self.epoch);
        float(self.last_calendar);
        for b in self.bodies {for v in [b.radius,b.radial_v,b.angle,b.delta_l] {float(v);}
            for m in b.elastic.axial.iter().chain(&b.elastic.lateral) {float(m.q);float(m.v);}
            float(b.elastic.energy_j);}
        for m in self.elevator {float(m.q);float(m.v);}
        for v in [self.stock_momentum,self.import_momentum] {float(v);}
        for v in [self.orbital_iron,self.delivered_iron] {put(v);}
        for v in [self.lifted_kg,self.lowered_kg] {put(v as u32);put((v>>32) as u32);}
        put(u32::from(self.shipment.is_some()));
        if let Some(s)=self.shipment {put(u32::from(s.up));
            for v in [s.cusp_s,s.impulse,s.applied,s.handoff.x,s.handoff.y,s.handoff.z] {
                let b=v.to_bits();put(b as u32);put((b>>32) as u32);
            }
        }
    }
    pub fn decode(read:&mut impl FnMut()->anyhow::Result<u32>)->anyhow::Result<Self> {
        fn number(read:&mut impl FnMut()->anyhow::Result<u32>,bound:f64)->anyhow::Result<f64> {
            let lo=read()? as u64;let hi=read()? as u64;
            let v=f64::from_bits(lo|(hi<<32));anyhow::ensure!(v.is_finite() && v.abs()<=bound,"Invalid orbital dynamics number");Ok(v)
        }
        let initialized=read()?;anyhow::ensure!(initialized<=1,"Invalid orbital dynamics flag");
        let seed=read()?;let epoch=number(read,1.0e15)?;anyhow::ensure!(epoch>=0.0,"Invalid orbital epoch");
        let last_calendar=number(read,1.0e15)?;
        let mut state=Self {initialized:initialized==1,seed,epoch,last_calendar,..Self::default()};
        for b in &mut state.bodies {
            b.radius=number(read,5.0e6)?;b.radial_v=number(read,10000.0)?;
            b.angle=number(read,TAU)?;b.delta_l=number(read,1.0e18)?;
            anyhow::ensure!(!state.initialized || b.radius>PLANET_RADIUS_M,"Invalid hook radius");
            for m in b.elastic.axial.iter_mut().chain(&mut b.elastic.lateral) {m.q=number(read,100_000.0)?;m.v=number(read,10_000.0)?;}
            b.elastic.energy_j=number(read,1.0e22)?;
        }
        for m in &mut state.elevator {m.q=number(read,10000.0)?;m.v=number(read,1000.0)?;}
        state.stock_momentum=number(read,1.0e18)?;state.import_momentum=number(read,1.0e18)?;
        state.orbital_iron=read()?;state.delivered_iron=read()?;
        anyhow::ensure!(state.orbital_iron<=STOCK_LIMIT && state.delivered_iron<=STOCK_LIMIT,"Invalid orbital stock");
        let mut counter=||->anyhow::Result<u64>{let lo=read()? as u64;let hi=read()? as u64;let n=lo|(hi<<32);anyhow::ensure!(n<=1_000_000_000_000,"Invalid freight total");Ok(n)};
        state.lifted_kg=counter()?;state.lowered_kg=counter()?;
        let present=read()?;anyhow::ensure!(present<=1,"Invalid shipment flag");
        if present==1 {
            let up=read()?;anyhow::ensure!(up<=1,"Invalid shipment direction");
            let cusp_s=number(read,1.0e15)?;let impulse=number(read,1.0e18)?;let applied=number(read,1.0)?;
            let handoff=DVec3::new(number(read,5.0e6)?,number(read,5.0e6)?,number(read,5.0e6)?);
            anyhow::ensure!(cusp_s>=0.0 && impulse>0.0 && applied>=0.0,"Invalid freight transfer");
            state.shipment=Some(Shipment {up:up==1,cusp_s,impulse,applied,handoff});
        }
        anyhow::ensure!(state.stock_momentum>=0.0 && state.import_momentum>=0.0,"Invalid momentum stock");
        if state.initialized {
            for (b,h) in state.bodies.iter().zip(skyhook::hooks(state.seed)) {
                let angular=b.h(h);
                let ec=angular*angular/(MU*b.radius)-1.0;
                let es=b.radial_v*angular/MU;
                anyhow::ensure!(angular>0.0 && ec.hypot(es)<0.25 && b.elastic.energy_j>=0.0,
                    "Invalid bound hook orbit");
            }
        }
        let scale=state.stock_momentum+state.import_momentum+state.bodies[1].delta_l.abs()+1.0;
        anyhow::ensure!(state.audit_error().abs()<=scale*1.0e-9,"Orbital momentum ledger mismatch");
        Ok(state)
    }
}
