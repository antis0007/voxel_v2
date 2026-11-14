//! Finite communication knowledge and paid equipment upgrades. A dish is a
//! registered, editable ground installation; UI availability is never authority.
use super::{Channel, DayNightCycle, Site};
use crate::{player::Player, world::{BlockId as B, World, inventory::Item}};
use glam::{IVec2, IVec3, Vec3};

pub const NAMES: [&str; 6] = ["No radio", "Crystal receiver", "Valve transceiver",
    "Superheterodyne set", "Directional dish station", "Resonant phased array"];
pub const LESSONS: [&str; 3] = ["Carrier waves and public beacons", "Modulation and acknowledgement codes", "Antenna gain and directional tracking"];
pub const COSTS: [&[(u8,u32)]; 6] = [&[], &[(6,2),(5,1)], &[(6,4),(8,2)],
    &[(6,4),(8,4),(5,1)], &[(8,8),(6,6),(15,2)], &[(15,4),(12,2),(5,2)]];
pub const COST_TEXT: [&str; 6] = ["", "2 copper + 1 crystal", "4 copper + 2 iron",
    "4 copper + 4 iron + 1 crystal", "8 iron + 6 copper + 2 steel",
    "4 steel + 2 ancient circuits + 2 crystals"];

#[derive(Default)]
pub struct State { pub level: u32, pub knowledge: u32, pub dish: Option<IVec3>, pub target: Option<IVec2> }
#[derive(Clone, Copy)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub enum Action { Upgrade, Listen, Handshake, BuildDish, RepairDish, Aim(IVec2), TrackRelay }
#[derive(Clone, Copy, Default)]
pub struct Link { pub receive: bool, pub transmit: bool, pub quality: f32, pub directional: bool }
#[derive(Clone, Default)]
pub struct View {
    pub level: u32, pub knowledge: u32, pub status: &'static str,
    pub can_upgrade: bool, pub can_listen: bool, pub can_handshake: bool,
    pub can_build: bool, pub can_repair: bool, pub near_dish: bool,
    pub links: Vec<(IVec2,Link)>,
}
impl View {
    pub fn link(&self, cell: IVec2) -> Link {
        self.links.iter().find(|(c,_)| *c == cell).map(|(_,l)| *l).unwrap_or_default()
    }
}
fn affordable(w: &World, cost: &[(u8,u32)]) -> bool {
    cost.iter().all(|&(i,n)| w.inventory.count_item(Item::Resource(i)) >= n)
}
fn pay(w: &mut World, cost: &[(u8,u32)]) {
    for &(i,n) in cost { let paid=w.inventory.remove_item(Item::Resource(i),n); debug_assert!(paid); }
    w.sync_from_inventory();
}
fn known(w: &World, p: IVec3) -> Option<B> {
    w.chunks.get(&crate::world::chunk_coord_from_world_block(p))
        .map(|c| c.blocks[crate::world::ChunkData::local_index(p.x,p.y,p.z)])
}
pub fn near(w: &World, p: &Player) -> bool {
    w.orbital.radio.dish.is_some_and(|a| ((a.as_vec3()+Vec3::splat(0.5))*w.voxel_scale.meters()
        -p.eye(w.voxel_scale)*w.voxel_scale.meters()).length() <= 8.0)
}
pub fn intact(w: &World) -> bool {
    w.orbital.radio.dish.is_some_and(|a| known(w,a)==Some(B::Workbench)
        && known(w,a-IVec3::Y).is_some_and(|b|b.is_solid()))
}
fn open_sky(w: &World) -> bool {
    let Some(a)=w.orbital.radio.dish else {return false};
    let h=w.sample_config().blocks_for_height(12.0);
    // This bounded local clearance check never reads a region from disk.
    (1..=h).all(|y| known(w,a+IVec3::Y*y).is_some_and(|b|b.is_empty()))
}
/// Orbital survey uses a calibrated dish, not the portable set's 6 km service
/// range. The backend rechecks hardware, clearance and weather for every order.
pub fn orbital_uplink(w:&World,day:&DayNightCycle,p:&Player)->bool {
    let r=&w.orbital.radio;
    let weather=day.cloud_controls();
    let loss=(weather.cumulonimbus*weather.storm_thickness*1.3).clamp(0.0,1.0);
    r.level>=4 && r.knowledge&4!=0 && r.target.is_none() && near(w,p) && intact(w) && open_sky(w)
        && 1.0-loss*if r.level>=5 {0.08} else {0.7}>=0.4
}
pub fn orbital_target(w:&World,day:&DayNightCycle)->Option<Vec3> {
    let r=&w.orbital.radio;
    if r.level<4 || r.knowledge&4==0 || r.target.is_some() {return None;}
    let viewer=(r.dish?.as_vec3()+Vec3::splat(0.5))*w.voxel_scale.meters();
    super::megastructures::contacts(w.seed,day,viewer).into_iter()
        .find(|c|c.site.kind==super::megastructures::Kind::DeadRelay && c.elevation_deg>=15.0)
        .map(|c|c.site.position)
}
pub fn link(w: &World, day: &DayNightCycle, p: &Player, site: Site) -> Link {
    let r=&w.orbital.radio;
    if r.level==0 {return Link::default()}
    let directional=r.level>=4 && r.target==Some(site.cell) && near(w,p) && intact(w) && open_sky(w);
    let level=if directional {r.level} else {r.level.min(3)};
    let at=if directional {(r.dish.unwrap().as_vec3()+Vec3::splat(0.5))*w.voxel_scale.meters()}
        else {p.pos*w.voxel_scale.meters()};
    let range=[0.0,1500.0,3000.0,6000.0,6000.0,6000.0][level as usize];
    let tx=[0.0,0.0,1500.0,3000.0,6000.0,6000.0][level as usize];
    let distance=Vec3::new(site.x-at.x,0.0,site.z-at.z).length();
    let weather=day.cloud_controls();
    let static_loss=(weather.cumulonimbus*weather.storm_thickness*1.3).clamp(0.0,1.0)
        * [0.0,0.85,0.70,0.35,0.40,0.08][level as usize];
    let quality=(1.0-distance/range*0.45-static_loss).clamp(0.0,1.0);
    Link {receive: distance<=range && quality>=0.20, transmit: level>=2 && distance<=tx && quality>=0.40,
        quality, directional}
}
fn prerequisite(r: &State) -> bool {
    match r.level {0=>true,1=>r.knowledge&1!=0,2=>r.knowledge&2!=0,4=>r.knowledge&4!=0,_=>false}
}
pub fn view(w: &World, day: &DayNightCycle, p: &Player) -> View {
    let r=&w.orbital.radio;
    let local=near(w,p);
    let links:Vec<_>=super::contacts(w.seed,day,&w.orbital,p.pos*w.voxel_scale.meters())
        .iter().map(|c|(c.service.site.cell,link(w,day,p,c.service.site))).collect();
    View {level:r.level,knowledge:r.knowledge,
        status:if r.level==0 {"Build a receiver, then listen to the public service band."}
            else if r.level>=4 && !intact(w) {"Dish console unavailable; knowledge retained. Portable set remains usable."}
            else if r.level>=4 && local && !open_sky(w) {"Dish needs twelve metres of clear overhead space."}
            else if r.level>=4 && !local {"Operate within 8 m of the dish for directional gain. Portable set active."}
            else if r.level>=4 && r.knowledge&4!=0 && r.target.is_none() {"Tracking derelict relay above 15 degrees."}
            else {"Public service band · private messages remain encrypted"},
        can_upgrade:r.level<5 && prerequisite(r) && affordable(w,COSTS[(r.level+1).min(5) as usize])
            && (r.level!=4 || local && intact(w)),
        can_listen:links.iter().any(|(_,l)|l.receive),
        can_handshake:links.iter().any(|(_,l)|l.transmit),
        can_build:r.level==3 && r.dish.is_none() && affordable(w,COSTS[4]),
        can_repair:r.level>=4 && local && r.dish.is_some_and(|a|known(w,a)==Some(B::Air)
            && known(w,a-IVec3::Y).is_some_and(|b|b.is_solid())) && affordable(w,&[(8,2)]),
        near_dish:local, links}
}
fn deployment(w: &World,p: &Player) -> Result<IVec3,&'static str> {
    let scale=w.voxel_scale.meters();
    let probe=p.pos+p.flat_forward()*(4.0/scale)+Vec3::Y*(3.0/scale);
    let Some((ground,_))=crate::world::raycast_block(w,probe,Vec3::NEG_Y,8.0/scale) else {return Err("No supported dish site four metres ahead.")};
    let a=ground+IVec3::Y;
    let r=w.sample_config().blocks_for_height(1.5);
    let h=w.sample_config().blocks_for_height(6.0);
    if a.y+h>=w.sample_config().max_world_y() {return Err("Dish exceeds the world height limit.")}
    for z in -r..=r {for x in -r..=r {
        let at=a+IVec3::new(x,0,z);
        if !w.get(at-IVec3::Y).is_solid() {return Err("Dish requires a flat, dry foundation.")}
        if w.factory.nodes.contains_key(&crate::world::factory::anchor(at,crate::world::factory::step(w))) {
            return Err("Dish footprint overlaps factory machinery.")
        }
        for y in 0..h {if !w.get(at+IVec3::Y*y).is_empty() {return Err("Clear a dry dish site with six metres of headroom.")}}
    }}
    Ok(a)
}
pub fn action(w: &mut World, day: &DayNightCycle, p: &Player, action: Action) -> Vec<IVec3> {
    let mut changed=Vec::new();
    let v=view(w,day,p);
    let message=match action {
        Action::TrackRelay if w.orbital.radio.level>=4 && w.orbital.radio.knowledge&4!=0
            && near(w,p) && intact(w) => {
            w.orbital.radio.target=None;
            "Tracking derelict relay above 15 degrees. Select a contact to switch targets."
        }
        Action::Upgrade if v.can_upgrade => {
            let next=(v.level+1) as usize; pay(w,COSTS[next]); w.orbital.radio.level+=1;
            "Radio upgraded."
        }
        Action::Listen if v.can_listen => {w.orbital.radio.knowledge|=1;
            "Beacon decoded. Timetables unlocked."}
        Action::Handshake if v.can_handshake => {
            w.orbital.radio.knowledge|=2;
            if v.links.iter().any(|(_,l)|l.directional && l.transmit) {w.orbital.radio.knowledge|=4;}
            "Connected. Modulation learned. Aim a dish to learn tracking."
        }
        Action::Aim(cell) if v.near_dish && intact(w) && super::site(w.seed,cell).is_some()
            && v.links.iter().any(|(c,_)| *c==cell) => {
            w.orbital.radio.target=Some(cell); "Dish aimed. Connect to calibrate tracking."
        }
        Action::BuildDish if v.can_build => {
            let a=match deployment(w,p) {Ok(a)=>a,Err(e)=>{w.expedition.say(e);return changed}};
            let r=w.sample_config().blocks_for_height(1.5);
            let mut edits=Vec::new();
            for z in -r..=r {for x in -r..=r {let at=a+IVec3::new(x,0,z);
                edits.push((at,if at==a {B::Workbench} else {B::StoneBricks}));}}
            let chunks:std::collections::HashSet<_>=edits.iter().map(|(p,_)|crate::world::chunk_coord_from_world_block(*p)).collect();
            if chunks.into_iter().any(|c|w.resident_edit(c).is_err()) {
                w.expedition.say("Dish terrain unavailable; no materials charged.");return changed;
            }
            w.set_many(&edits);
            if !edits.iter().all(|&(at,b)|w.get(at)==b) {w.expedition.say("Dish placement refused; no materials charged.");return changed;}
            pay(w,COSTS[4]); w.orbital.radio.dish=Some(a); w.orbital.radio.level=4;
            changed.extend(edits.into_iter().map(|(p,_)|p));
            "Dish station built. Point it at a corridor and establish a directional handshake."
        }
        Action::RepairDish if v.can_repair => {
            let a=w.orbital.radio.dish.unwrap(); w.set(a,B::Workbench);
            if w.get(a)==B::Workbench {pay(w,&[(8,2)]);changed.push(a);}
            "Dish console restored; research and upgrades retained."
        }
        _ => "Unavailable. Check requirements.",
    };
    w.expedition.say(message); changed
}
pub fn request(w: &mut World,day: &DayNightCycle,p: &Player,cell:IVec2,channel:Channel) -> &'static str {
    if matches!(channel,Channel::Radio) {
        let Some(site)=super::site(w.seed,cell) else {return "No registered relay at that address."};
        if !link(w,day,p,site).transmit {return "Radio uplink unavailable: improve equipment, approach the corridor, or use an aligned dish."}
    } else if w.adventure.wand==0 {return "A resonant return requires an attuned wand."}
    w.orbital.request(w.seed,day,p.pos*w.voxel_scale.meters(),cell,channel)
}
impl State {
    pub fn encode(&self,bytes:&mut Vec<u8>) {
        let mut push=|n:u32|bytes.extend_from_slice(&n.to_le_bytes());
        push(self.level);push(self.knowledge);push(u32::from(self.dish.is_some()));
        if let Some(a)=self.dish {for n in a.to_array(){push(n as u32)}}
        push(u32::from(self.target.is_some()));
        if let Some(t)=self.target {push(t.x as u32);push(t.y as u32);}
    }
    pub fn decode(read:&mut impl FnMut()->anyhow::Result<u32>)->anyhow::Result<Self> {
        let level=read()?;let knowledge=read()?;let flag=read()?;
        anyhow::ensure!(level<=5 && knowledge<=7 && flag<=1,"Invalid radio progression");
        let dish=if flag==1 {Some(IVec3::new(read()? as i32,read()? as i32,read()? as i32))} else {None};
        let aimed=read()?;anyhow::ensure!(aimed<=1,"Invalid dish tracking flag");
        let target=if aimed==1 {Some(IVec2::new(read()? as i32,read()? as i32))} else {None};
        anyhow::ensure!((level>=4)==dish.is_some() && (target.is_none() || dish.is_some())
            && target.is_none_or(|t|t.x.unsigned_abs()<=1_000_000 && t.y.unsigned_abs()<=1_000_000)
            && (level<2 || knowledge&1!=0) && (level<3 || knowledge&2!=0) && (level<5 || knowledge&4!=0)
            && dish.is_none_or(|a| a.x.unsigned_abs()<=1_024_000 && a.z.unsigned_abs()<=1_024_000 && a.y.unsigned_abs()<=65536),"Invalid radio dish");
        Ok(Self {level,knowledge,dish,target})
    }
}
