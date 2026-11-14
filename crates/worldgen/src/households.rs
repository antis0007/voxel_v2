//! Seeded surface households. Stable IDs also own resident stock and absences.
use crate::{BlockId as B, WorldSampleConfig};
use glam::Vec3;
pub const VERSION: u32 = 79;
const SPAN: f32 = 256.0;
const BASE: u32 = 1024;
mod settlement;

/// Shared district centre; terrain and building planners never call each other here.
pub(crate) fn district_center(x: f32, z: f32) -> (f32, f32) {
    settlement::hub((x / SPAN).floor() as i32, (z / SPAN).floor() as i32)
}

#[derive(Clone, Copy)]
pub struct Household { pub id: u32, pub home: Vec3 }

pub fn role(id: u32) -> Option<u32> {
    if (3..7).contains(&id) { Some(id-3) }
    else if (BASE..BASE+256*256*4).contains(&id) { Some((id-BASE)%4) }
    else { crate::civilization::role(id) }
}

fn horizontal(cfg: &WorldSampleConfig, x: i32, z: i32) -> (u32, f32, f32) {
    let h = crate::hash_u32(cfg.seed ^ (x as u32).wrapping_mul(0x9e3779b9) ^ (z as u32).wrapping_mul(0x85ebca6b));
    if cfg.generator_version >= 92 {
        let hub = settlement::hub(x, z);
        let side = |c: i32| if c.rem_euclid(2) == 0 { -1.0 } else { 1.0 };
        return (BASE + ((x+128) as u32*256+(z+128) as u32)*4 + h%4,
            hub.0 + side(x) * (22.0 + (h%19) as f32),
            hub.1 + side(z) * (22.0 + ((h>>8)%19) as f32));
    }
    (BASE + ((x+128) as u32*256+(z+128) as u32)*4 + h%4,
        (x as f32+0.5)*SPAN + (h%97) as f32-48.0,
        (z as f32+0.5)*SPAN + ((h>>8)%97) as f32-48.0)
}

fn site(cfg: &WorldSampleConfig, x: i32, z: i32) -> Option<Household> {
    type Entry=(WorldSampleConfig,i32,i32,Option<Household>);
    thread_local!{static CACHE:std::cell::RefCell<Vec<Entry>>=const{std::cell::RefCell::new(Vec::new())};}
    if let Some(found)=CACHE.with(|c|c.borrow().iter().find(|e|e.0==*cfg && e.1==x && e.2==z).map(|e|e.3)){return found;}
    let found=site_uncached(cfg,x,z);
    CACHE.with(|c|{let mut c=c.borrow_mut();if c.len()>=64{c.remove(0);}c.push((*cfg,x,z,found));});
    found
}
fn site_uncached(cfg: &WorldSampleConfig, x: i32, z: i32) -> Option<Household> {
    if cfg.generator_version < VERSION || !crate::fernwake::active(cfg)
        || !(-128..128).contains(&x) || !(-128..128).contains(&z) { return None; }
    let (id,wx,wz) = horizontal(cfg,x,z);
    if wx.hypot(wz)<80.0 || crate::valley::reserves(cfg,wx,wz)
        || crate::amberroot::reserves(cfg,wx,wz) || crate::sites::surface_reserved(cfg,wx,wz) { return None; }
    let mut low = f32::MAX; let mut high = f32::MIN;
    for (dx,dz) in [(0.0,0.0),(-6.0,-6.0),(-6.0,6.0),(6.0,-6.0),(6.0,6.0)] {
        let col = crate::sample_column(cfg,((wx+dx)/cfg.voxel_meters()).floor() as i32,((wz+dz)/cfg.voxel_meters()).floor() as i32);
        let y = cfg.world_y(col.height+1);
        low=low.min(y); high=high.max(y);
    }
    if low <= crate::WATER_LEVEL_METERS+1.0 || high-low>3.0 { return None; }
    if cfg.generator_version >= 92 && !settlement::connected(cfg, x, z, wx, wz, high) { return None; }
    Some(Household { id, home:Vec3::new(wx,high.ceil()+0.02,wz) })
}

pub fn home(cfg: &WorldSampleConfig, id: u32) -> Option<Vec3> {
    if !(BASE..BASE+256*256*4).contains(&id) { return crate::civilization::home(cfg, id); }
    let n=(id-BASE)/4;
    site(cfg,(n/256) as i32-128,(n%256) as i32-128).filter(|s|s.id==id).map(|s|s.home)
}

pub fn nearby(cfg: &WorldSampleConfig, p: Vec3) -> Vec<Household> {
    if cfg.generator_version < VERSION { return Vec::new(); }
    let x=(p.x/SPAN).floor() as i32; let z=(p.z/SPAN).floor() as i32;
    let mut out=Vec::new();
    crate::civilization::nearby(cfg, p, &mut out);
    if !crate::fernwake::active(cfg) { return out; }
    for dz in -1..=1 { for dx in -1..=1 {
        if let Some(s)=site(cfg,x+dx,z+dz) { if s.home.distance_squared(p)<100.0*100.0 {out.push(s);} }
    }}
    out
}

pub fn refuge(cfg:&WorldSampleConfig,p:Vec3)->bool {
    if cfg.generator_version<VERSION{return false;}
    let x=(p.x/SPAN).floor() as i32;let z=(p.z/SPAN).floor() as i32;
    let (_,wx,wz)=horizontal(cfg,x.clamp(-128,127),z.clamp(-128,127));
    if (p.x-wx).abs()>9.0 || (p.z-wz).abs()>9.0{return false;}
    site(cfg,x,z).is_some_and(|s|(-1.0..7.0).contains(&(p.y-s.home.y)))
}

pub fn sample(cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    if cfg.generator_version < VERSION || !crate::fernwake::active(cfg) {return None;}
    let x=(m.x/SPAN).floor() as i32; let z=(m.z/SPAN).floor() as i32;
    if !(-128..128).contains(&x)||!(-128..128).contains(&z){return None;}
    if cfg.generator_version >= 92 {
        if let Some(block) = settlement::lane(cfg, m, x, z) { return Some(block); }
    }
    let (_,wx,wz)=horizontal(cfg,x,z);
    if (m.x-wx).abs()>7.0 || (m.z-wz).abs()>7.0 {return None;}
    let s=site(cfg,x,z)?;
    let mut p=m-(s.home-Vec3::Y*0.02);
    // Both rows open onto the shared lane, rather than all facing south.
    if cfg.generator_version >= 92 && z.rem_euclid(2) == 0 { p.x = -p.x; p.z = -p.z; }
    if !(-4.0..7.0).contains(&p.y) {return None;}
    if p.y<0.0 {return Some(if p.y < -1.0 {B::Dirt} else {B::Grass});}
    let role=role(s.id)?;
    // Open grove, low sprite household, and two full-sized workshop shelters.
    if role==2 {
        if p.x.abs()>4.0 && p.z.abs()>4.0 && p.y<4.0 {return Some(B::Log);}
        if p.x.abs()>3.0 && p.z.abs()>3.0 && (4.0..6.0).contains(&p.y) {return Some(B::Leaves);}
    } else {
        let h=if role==3 {2.0} else {4.0};
        let w=if role==3 {2.5} else {4.5};
        if p.x.abs()<w && p.z.abs()<w {
            if (h..h+1.0).contains(&p.y) {return Some(B::Planks);}
            let wall=p.x.abs()>w-1.0 || p.z.abs()>w-1.0;
            let door=p.x.abs()<1.5 && p.z<0.0;
            let window=p.y>=1.0 && p.y<2.0 && p.z.abs()<1.0;
            if wall && !door && !window && p.y<h {return Some(B::Log);}
        }
    }
    if (p.x-5.0).abs()<0.5 && p.z.abs()<0.5 {
        if p.y<1.0{return Some(B::StoneBricks);}
        if p.y<2.0{return Some(B::Torch);}
    }
    if p.z>5.0 && p.x.abs()<2.0 && p.y<1.0 {return Some(B::FungalSoil);}
    Some(B::Air)
}
