//! Explicit disposable native acceptance fixture. Review commands invoke the
//! production receiver actions and stage inventory, calendar, checkpoint/reload
//! and invalid duplicate requests. Never active during ordinary play.
use crate::{player::Player, world::{self, BlockId as B, DayNightCycle, World,
    inventory::{Item, ItemStack, SurvivalInventory}, orbital::receiver::{self, Action}, storage::SaveService}};
use glam::{IVec3, Vec3};
use std::{collections::HashSet, path::PathBuf};
mod caves;

pub struct Review {
    root: PathBuf,
    last: String,
    original: Option<SurvivalInventory>,
    full: Option<SurvivalInventory>,
    expected: Option<(u32, u32, u32, Option<IVec3>)>,
    radio_expected: Option<Vec<u8>>,
    orbit_review: Option<(u32,f64)>,
}
impl Review {
    fn record_orbital_pose(day: &DayNightCycle, p: &Player) {
        println!("[orbital-pose] days={:.12} position={:.6},{:.6},{:.6} yaw={:.8} pitch={:.8}",
            world::orbital::megastructures::seconds(day)/world::orbital::megastructures::DAY_SECONDS,
            p.pos.x,p.pos.y,p.pos.z,p.yaw,p.pitch);
    }
    pub fn from_env(benchmark: bool) -> Option<Self> {
        if !benchmark || std::env::var("VOXEL_RECEIVER_REVIEW").as_deref() != Ok("1") { return None; }
        let root = PathBuf::from(std::env::var_os("VOXEL_CAPTURE_DIR")?);
        if std::env::var_os("VOXEL_SAVE_DIR").map(PathBuf::from) != Some(root.join("disposable-saves")) { return None; }
        std::fs::create_dir_all(&root).ok()?;
        world::storage::set_autosave_enabled(false);
        Some(Self { root, last: String::new(), original: None, full: None, expected: None, radio_expected: None, orbit_review:None })
    }
    pub fn install(&mut self, w: &mut World, p: &mut Player, day: &mut DayNightCycle) {
        let mut edits = Vec::new();
        if !engine_worldgen::meridian_terminal::enabled(&w.sample_config()) {
        for z in -908..=-891 { for x in 1093..=1107 { for y in 96..=103 {
            edits.push((IVec3::new(x,y,z), if y == 96 { B::StoneBricks } else { B::Air }));
        }}}
        }
        w.set_many(&edits);
        w.inventory = SurvivalInventory::default();
        for (i,n) in [(8,16),(6,16),(5,2)] { w.inventory.try_insert(ItemStack::new(Item::Resource(i),n)); }
        w.sync_from_inventory();
        p.pos=Vec3::new(1100.5,97.0,-895.5); p.precise_pos=p.pos.as_dvec3(); p.vel=Vec3::ZERO;
        p.yaw=0.0; p.pitch = -0.24; p.flying=true;
        if engine_worldgen::meridian_terminal::enabled(&w.sample_config()) {
            p.pos=engine_worldgen::meridian_terminal::console(&w.sample_config()).as_vec3()+Vec3::new(0.5,0.0,4.5);
            p.precise_pos=p.pos.as_dvec3();
        }
        day.set_day_index(1); day.set_time_of_day(0.12); day.set_day_length_seconds(600.0);
        w.orbital.dynamics.manual_clock=true;
        w.orbital.dynamics.advance(w.seed,day);
        self.report(w, "fixture");
    }
    fn signature(w: &World) -> (u32,u32,u32,Option<IVec3>) {
        (w.orbital.receiver.stage, w.inventory.count_item(Item::Resource(15)),
            w.inventory.count_item(Item::Resource(12)), w.orbital.receiver.anchor)
    }
    fn report(&self, w: &World, label: &str) {
        let s=Self::signature(w);
        println!("[receiver-review] {label} stage={} steel={} circuits={} anchor={:?} copper={} crystal={} iron={}",
            s.0,s.1,s.2,s.3,w.inventory.count_item(Item::Resource(6)),
            w.inventory.count_item(Item::Resource(5)),w.inventory.count_item(Item::Resource(8)));
        println!("[radio-review] {label} level={} knowledge={} dish={:?} target={:?}",w.orbital.radio.level,
            w.orbital.radio.knowledge,w.orbital.radio.dish,w.orbital.radio.target);
        let d=&w.orbital.dynamics;
        println!("[freight-review] {label} orbit={} ground={} lifted_kg={} lowered_kg={} capacity_kg={:.6} strain={:.9} elastic_j={:.6} momentum_error={:.9} pending={}",
            d.orbital_iron,d.delivered_iron,d.lifted_kg,d.lowered_kg,d.capacity_kg(w.seed),
            d.strain(w.seed),d.energy_j(),d.audit_error(),d.shipment.is_some());
    }
    pub fn frame(&mut self, w: &mut World, p: &mut Player, day: &mut DayNightCycle,
        saves: &mut SaveService, creative: &mut crate::creative::CreativeState,
        dirty: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> (bool,Option<String>) {
        // Unlike ordinary benchmarks, this explicitly exercises the real save worker.
        saves.tick(Some(w),p,day);
        // Frozen visual reviews must not retain an unrelated equip tutorial
        // across every composition capture. Production messages are unchanged.
        if w.generator_version>=65 {w.expedition.message_seconds=0.0;}
        let text=std::fs::read_to_string(self.root.join("command.txt")).unwrap_or_default();
        let text=text.trim();
        if text.is_empty() || text==self.last { return (false,None); }
        let command=text.split_whitespace().nth(1).unwrap_or("");
        if command=="reload" && saves.busy() { return (false,None); }
        self.last=text.to_owned();
        let mut reloaded=false;
        let mut changes=Vec::new();
        match command {
            "cenote" | "cenote-night" | "cenote-entry" => {
                caves::cenote(w,p,command);
                day.set_time_of_day(if command == "cenote-night" { 0.0 } else { 0.5 });
                creative.show_palette=false;
            }
            "cave-dry" => {
                caves::inspect(w,p);
                creative.show_palette=false;
            }
            "terminal-ground" | "terminal-court" | "terminal-gallery" | "terminal-dusk" | "terminal-dock" => {
                let cfg=w.sample_config();
                let base=Vec3::new(1100.0,engine_worldgen::meridian_terminal::floor(&cfg),-900.0);
                let (eye,target)=match command {
                    "terminal-ground" if cfg.generator_version>=65 =>(Vec3::new(-12.0,0.05,170.0),Vec3::new(0.0,108.0,0.0)),
                    "terminal-dock"=>(Vec3::new(65.0,0.05,42.0),Vec3::new(44.0,4.5,8.0)),
                    "terminal-ground"=>(Vec3::new(-28.0,1.7,66.0),Vec3::new(0.0,27.0,-7.0)),
                    "terminal-gallery" if cfg.generator_version>=65 =>(Vec3::new(-20.0,0.05,37.0),Vec3::new(-34.0,2.5,26.0)),
                    "terminal-gallery"=>(Vec3::new(-26.0,11.7,6.0),Vec3::new(-27.0,13.0,-24.0)),
                    _ if cfg.generator_version>=65 =>(Vec3::new(0.0,0.05,62.0),Vec3::new(0.0,24.0,0.0)),
                    _=>(Vec3::new(-13.0,1.7,27.0),Vec3::new(0.0,21.0,-10.0)),
                };
                p.pos=(base+eye)/cfg.voxel_meters();p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
                let d=target-eye;p.yaw=d.x.atan2(-d.z);p.pitch=d.y.atan2(Vec3::new(d.x,0.0,d.z).length());
                day.set_time_of_day(if command=="terminal-dusk" {0.70} else {0.36});
                creative.show_palette=false;
                println!("[terminal-review] {text} console={:?} solid_console={} floor={}",
                    engine_worldgen::meridian_terminal::console(&cfg),w.get(engine_worldgen::meridian_terminal::console(&cfg))==B::Workbench,base.y);
                self.report(w,text);
            }
            "terminal-reclaim" => {
                let cfg=w.sample_config();let a=engine_worldgen::meridian_terminal::console(&cfg);
                p.pos=a.as_vec3()+Vec3::new(0.5,0.0,3.5);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
                changes.extend(receiver::action(w,day,p,Action::Deploy));
                println!("[terminal-review] reclaimed={} intact={} workbench={} smelter={}",
                    w.orbital.receiver.anchor==Some(a),receiver::intact(w),
                    w.get(engine_worldgen::meridian_terminal::workbench(&cfg))==B::Workbench,
                    w.get(engine_worldgen::meridian_terminal::smelter(&cfg))==B::Smelter);
                self.report(w,text);
            }
            "freight-ready" => {
                let at=w.orbital.receiver.due()+0.00001;
                day.set_day_index(at.floor() as u32);day.set_time_of_day(at.fract() as f32);
                p.pos=w.orbital.receiver.anchor.map(|a|a.as_vec3()+Vec3::new(0.5,0.0,3.5))
                    .unwrap_or(Vec3::new(1100.5,97.0,-895.5));p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
                changes.extend(receiver::action(w,day,p,Action::Collect));self.report(w,text);
            }
            "freight-lift" | "freight-lower" | "freight-collect" => {
                p.pos=w.orbital.receiver.anchor.map(|a|a.as_vec3()+Vec3::new(0.5,0.0,3.5))
                    .unwrap_or(Vec3::new(1100.5,97.0,-895.5));p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
                let request=match command {"freight-lift"=>Action::LiftIron,"freight-lower"=>Action::LowerIron,_=>Action::CollectIron};
                changes.extend(receiver::action(w,day,p,request));self.report(w,text);
            }
            "freight-start" | "freight-catch" | "freight-mid" | "freight-finish" => {
                if let Some(s)=w.orbital.dynamics.shipment {
                    let hook=world::orbital::skyhook::hooks(w.seed)[1];
                    let seconds=match command {
                        "freight-start"=>s.start(hook)+0.1,
                        "freight-catch"=>s.cusp_s+0.1,
                        "freight-mid"=>if s.up {s.cusp_s+hook.turn_period()*0.625}
                            else {s.start(hook)+hook.turn_period()*0.375},
                        _=>s.end(hook)+0.1,
                    };
                    let at=seconds/world::orbital::megastructures::DAY_SECONDS;
                    day.set_day_index(at.floor() as u32);day.set_time_of_day(at.fract() as f32);
                    w.orbital.dynamics.advance(w.seed,day);
                }
                self.report(w,text);
            }
            "freight-close" => {
                let cfg=w.sample_config();
                let hook=world::orbital::skyhook::hooks(w.seed)[1];
                let x=(hook.port_lon*world::orbital::megastructures::PLANET_RADIUS_M) as f32;
                let port=Vec3::new(x,cfg.world_y(world::terrain_height(&cfg,(x*cfg.inv_voxel_meters()).floor() as i32,0)),0.0);
                let contact=world::orbital::skyhook::physical_contacts(w.seed,day,p.pos,&w.orbital.dynamics)[1];
                let at=w.orbital.dynamics.cargo(&contact,||port,world::orbital::skyhook::clock(day),p.pos).unwrap_or(contact.tip);
                p.pos=at+Vec3::new(200.0,-80.0,180.0);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
                let contact=world::orbital::skyhook::physical_contacts(w.seed,day,p.pos,&w.orbital.dynamics)[1];
                let target=w.orbital.dynamics.cargo(&contact,||port,world::orbital::skyhook::clock(day),p.pos).unwrap_or(contact.tip);
                let ray=target-p.pos;p.yaw=ray.x.atan2(-ray.z);p.pitch=ray.y.atan2(Vec3::new(ray.x,0.0,ray.z).length());
                creative.show_palette=false;Self::record_orbital_pose(day,p);self.report(w,text);
            }
            "freight-report" => {w.orbital.dynamics.advance(w.seed,day);self.report(w,text);}
            "radio-supplies" => {
                for i in [5,6,8,12,15] {w.inventory.try_insert(ItemStack::new(Item::Resource(i),64));}
                w.sync_from_inventory();self.report(w,text);
            }
            "radio-upgrade" | "radio-listen" | "radio-handshake" | "radio-build" | "radio-aim" | "radio-repair" => {
                use world::orbital::radio::{self,Action as R};
                let request=match command {"radio-upgrade"=>R::Upgrade,"radio-listen"=>R::Listen,
                    "radio-handshake"=>R::Handshake,"radio-build"=>R::BuildDish,"radio-aim"=>R::Aim(glam::IVec2::ZERO),_=>R::RepairDish};
                changes.extend(radio::action(w,day,p,request));self.report(w,text);
            }
            "radio-guard" => {
                use world::orbital::radio::{self,Action as R};
                let mut before=Vec::new();w.orbital.encode(&mut before,world::storage::SAVE_VERSION);
                let pack=w.inventory.clone();
                let request=if w.orbital.radio.level==0 {R::BuildDish} else {R::Upgrade};
                changes.extend(radio::action(w,day,p,request));
                if w.orbital.radio.level==0 {let _=radio::request(w,day,p,glam::IVec2::ZERO,world::orbital::Channel::Radio);}
                let mut after=Vec::new();w.orbital.encode(&mut after,world::storage::SAVE_VERSION);
                println!("[radio-review] guard_unchanged={}",before==after && pack.hotbar==w.inventory.hotbar && pack.pack==w.inventory.pack);
                self.report(w,text);
            }
            "radio-look" => {
                if let Some(a)=w.orbital.radio.dish {
                    p.pos=a.as_vec3()+Vec3::new(4.5,1.0,5.5);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
                    p.yaw=(-4.0f32).atan2(5.0);p.pitch=0.05;creative.show_palette=false;
                }
                self.report(w,text);
            }
            "radio-link-check" => {
                use world::orbital::radio;
                let site=world::orbital::site(w.seed,glam::IVec2::ZERO).unwrap();
                let mut clear=DayNightCycle::new(0.5,600.0);clear.set_weather_preset(world::WeatherPreset::Clear);
                let mut storm=DayNightCycle::new(0.5,600.0);storm.set_weather_preset(world::WeatherPreset::Stormy);
                let a=radio::link(w,&clear,p,site);let b=radio::link(w,&storm,p,site);
                let old=p.pos;p.pos+=Vec3::X*20000.0;let far=radio::link(w,&clear,p,site);p.pos=old;
                println!("[radio-review] weather_and_range={} clear={:.3} storm={:.3} directional={}",
                    b.quality<a.quality && !far.transmit && !far.receive,a.quality,b.quality,a.directional);
                if let Some(anchor)=w.orbital.radio.dish {
                    let block=anchor+IVec3::Y*3;let original=w.get(block);w.set(block,B::Stone);
                    let blocked=radio::link(w,&clear,p,site);w.set(block,original);changes.push(block);
                    println!("[radio-review] overhead_blocks_directional={}",a.directional && !blocked.directional);
                }
                self.report(w,text);
            }
            "radio-break" => {
                if let Some(a)=w.orbital.radio.dish {w.set(a,B::Air);changes.push(a);}
                self.report(w,text);
            }
            "radio-return" => {
                let answer=world::orbital::radio::request(w,day,p,glam::IVec2::ZERO,world::orbital::Channel::Radio);
                println!("[radio-review] uplink_ack={answer}");self.report(w,text);
            }
            "focus" => {
                println!("[receiver-review] grounded_receiver_focus={}",receiver::focused(w,p));
                self.report(w,text);
            }
            "deploy" | "order" | "collect" => {
                let action=match command { "deploy"=>Action::Deploy, "order"=>Action::Order, _=>Action::Collect };
                changes.extend(receiver::action(w,day,p,action));
                self.report(w,text);
            }
            "report" => self.report(w,text),
            "full-partial" | "full-second" => {
                if self.original.is_none() { self.original=Some(w.inventory.clone()); }
                for slot in w.inventory.hotbar.iter_mut().chain(w.inventory.pack.iter_mut()) {
                    *slot=Some(ItemStack::new(Item::Block(B::Stone),64));
                }
                w.inventory.cursor=None; w.inventory.craft_grid.fill(None);
                w.inventory.hotbar[0]=Some(ItemStack::new(Item::Resource(15),if command=="full-partial" {60} else {56}));
                w.sync_from_inventory(); self.full=Some(w.inventory.clone()); self.report(w,text);
            }
            "check-full" => {
                let unchanged=self.full.as_ref().is_some_and(|i| i.hotbar==w.inventory.hotbar
                    && i.pack==w.inventory.pack && i.cursor==w.inventory.cursor && i.craft_grid==w.inventory.craft_grid);
                println!("[receiver-review] full_inventory_atomic={} stage={}",unchanged && w.orbital.receiver.stage==1,w.orbital.receiver.stage);
                self.report(w,text);
            }
            "free" => {
                if let Some(i)=self.original.take() { w.inventory=i; w.sync_from_inventory(); }
                self.report(w,text);
            }
            "flight" => {
                let at=w.orbital.receiver.due()-0.012;
                day.set_day_index(at.floor() as u32); day.set_time_of_day(at.fract() as f32);
                day.set_day_length_seconds(600.0);
                p.pos=Vec3::new(1100.5,110.0,-820.0); p.precise_pos=p.pos.as_dvec3(); p.vel=Vec3::ZERO;
                p.pitch=0.75; creative.show_palette=false;
                self.report(w,text);
            }
            "orbital-flight" => {
                // Visual review only: begin a real scheduled descent above the
                // origin corridor, with a fixed ground camera and normal clock.
                let site = world::orbital::contacts(w.seed,day,&w.orbital,Vec3::new(1100.0,0.0,-900.0))
                    .into_iter().find(|c| c.service.site.cell == glam::IVec2::ZERO).unwrap().service;
                let at = site.handoff_day - 0.11 + 0.22 * 0.17;
                day.set_day_index(at.floor() as u32); day.set_time_of_day(at.fract() as f32);
                day.set_day_length_seconds(600.0);
                let height=world::orbital::hold_height(&w.sample_config(),&w.orbital,site.site);
                let heading=(site.site.key>>8) as f32/16777216.0*std::f32::consts::TAU;
                // A fixed surface camera perpendicular to the sweep exposes
                // lateral travel; looking along its plane hides the whole arc.
                let offset=Vec3::new(-heading.sin(),0.0,heading.cos())*3000.0;
                p.pos=Vec3::new(site.site.x,height-300.0,site.site.z)+offset;
                p.precise_pos=p.pos.as_dvec3(); p.vel=Vec3::ZERO;
                p.yaw=(-offset.x).atan2(offset.z); p.pitch=(2000.0_f32).atan2(3000.0);
                creative.show_palette=false;
                self.report(w,text);
            }
            "megastructure-day" | "megastructure-night" => {
                day.set_day_index(1);
                day.set_time_of_day(if command=="megastructure-night" {0.965} else {0.45});
                day.set_day_length_seconds(86400.0);
                self.report(w,text);
            }
            "orbit-before" | "orbit-after" | "orbit-hidden" => {
                if let Some((key,peak))=self.orbit_review {
                    let offset=if command=="orbit-before" {-45.0} else if command=="orbit-after" {45.0} else {3000.0};
                    let at=peak+offset/86400.0;
                    day.set_day_index(at.floor() as u32);day.set_time_of_day(at.fract() as f32);
                    let contacts=world::orbital::megastructures::contacts(w.seed,day,p.pos);
                    if let Some(c)=contacts.iter().find(|c|c.site.key==key) {
                        println!("[orbit-review] shot={command} key={key:08X} seconds={:.3} elevation={:.3} slant_m={:.1} altitude_m={:.1} period_s={:.3} sunlight={:.3} position={:?}",
                            world::orbital::megastructures::seconds(day),c.elevation_deg,c.distance_m,c.site.kind.altitude_m(),c.period_seconds,c.site.sunlight,c.site.position);
                    } else {println!("[orbit-review] shot={command} key={key:08X} below_planet_horizon=true");}
                }
                self.report(w,text);
            }
            "skyhook-pod" | "skyhook-cusp" | "skyhook-detail" | "skyhook-close" => {
                // Talon rotovator caught mid-lift: park the clock halfway up the pod's climb.
                use world::orbital::skyhook;
                let hook=skyhook::hooks(w.seed)[1];
                let now=skyhook::clock(day);
                let lift=hook.turn_period()*0.3;
                let frac=if matches!(command,"skyhook-detail"|"skyhook-close") {0.0} else if command=="skyhook-cusp" {0.04} else {0.5};
                let mut wait=hook.next_port_cusp(now)-lift*frac;
                if wait<0.0 {wait+=hook.pass_period();}
                // Walk forward whole passes until the cusp falls in daylight.
                let mut at=(now+wait)/world::orbital::megastructures::DAY_SECONDS;
                for _ in 0..60 {
                    let mut probe=*day;probe.set_day_index(at.floor() as u32);probe.set_time_of_day(at.fract() as f32);
                    if probe.lighting().sun_dir.y>0.5 {break;}
                    at+=hook.pass_period()/world::orbital::megastructures::DAY_SECONDS;
                }
                day.set_day_index(at.floor() as u32);day.set_time_of_day(at.fract() as f32);
                let cfg=w.sample_config();
                let x=(hook.port_lon*world::orbital::megastructures::PLANET_RADIUS_M) as f32;
                let ground=cfg.world_y(world::terrain_height(&cfg,(x*cfg.inv_voxel_meters()).floor() as i32,0)).max(cfg.world_y(cfg.water_level()));
                let port=Vec3::new(x,ground,0.0);
                p.pos=Vec3::new(x+250.0,ground+3.0,250.0);
                let contact=skyhook::contacts(w.seed,day,p.pos)[1];
                if command=="skyhook-detail" {
                    p.pos=contact.tip+Vec3::new(1800.0,-800.0,1600.0);
                } else if command=="skyhook-close" {
                    // Rendering draws the elastic line; its tip departs from the nominal rail.
                    let tip=skyhook::physical_contacts(w.seed,day,p.pos,&w.orbital.dynamics)[1].tip;
                    p.pos=tip+Vec3::new(240.0,-110.0,200.0);
                }
                let contact=skyhook::contacts(w.seed,day,p.pos)[1];
                let tip=contact.tip;
                let target=if command=="skyhook-close" {skyhook::physical_contacts(w.seed,day,p.pos,&w.orbital.dynamics)[1].tip}
                    else if command=="skyhook-detail" {tip}
                    else {contact.pod(port).unwrap_or(tip)};
                p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
                let d=target-p.pos;
                p.yaw=d.x.atan2(-d.z);p.pitch=d.y.atan2(Vec3::new(d.x,0.0,d.z).length());
                creative.show_palette=false;
                let pod=skyhook::contacts(w.seed,day,p.pos)[1].pod(port);
                println!("[skyhook-review] port={port:?} tip={tip:?} wait_s={wait:.1} to_next={:.1} lift={lift:.1} pod={pod:?}",hook.next_port_cusp(skyhook::clock(day)));
                Self::record_orbital_pose(day,p);
                self.report(w,text);
            }
            "scale-100" | "scale-85" | "scale-70" | "scale-50" => {
                // Change the render resolution mid-session, as automatic quality does.
                if let Some(percent)=command.strip_prefix("scale-").and_then(|n|n.parse::<u32>().ok()) {
                    crate::render::set_scene_scale_percent(percent);
                }
                self.report(w,text);
            }
            "meteor" => {
                // Park the clock a moment before a streak's peak and look at its head.
                use world::orbital::{meteors,skyhook};
                let cfg=w.sample_config();
                let (x,z)=(1400.0_f32,-1200.0_f32);
                let ground=cfg.world_y(world::terrain_height(&cfg,(x*cfg.inv_voxel_meters()).floor() as i32,(z*cfg.inv_voxel_meters()).floor() as i32)).max(cfg.world_y(cfg.water_level()));
                p.pos=Vec3::new(x,ground+3.0,z);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
                let now=skyhook::clock(day)*480.0/86400.0;
                let mut found=None;
                for k in 0..3840 {
                    let r=now+k as f64*0.25;
                    let mut probe=*day;probe.set_day_index((r/480.0).floor() as u32);probe.set_time_of_day((r/480.0).fract() as f32);
                    if let Some(s)=meteors::streaks(w.seed,&probe,p.pos).into_iter().find(|s|s.fade>0.95) {found=Some((r,s));break;}
                }
                if let Some((r,s))=found {
                    let at=(r-0.25)/480.0;
                    day.set_day_index(at.floor() as u32);day.set_time_of_day(at.fract() as f32);
                    let d=s.head-p.pos;
                    p.yaw=d.x.atan2(-d.z);p.pitch=d.y.atan2(Vec3::new(d.x,0.0,d.z).length());
                    println!("[meteor-review] peak_in_real_s=0.25 head={:?}",s.head);
                } else {println!("[meteor-review] no streak found in two days");}
                creative.show_palette=false;
                self.report(w,text);
            }
            "corridor-far" | "corridor-pop-near" | "corridor-pop-far"
            | "freight-pop-near" | "freight-pop-far" | "courier-pop-far" => {
                // Ground-level view of a corridor. The pop pairs straddle the 5-6 km veil
                // and the 6 km sight range; corridor-far stays at 9 km. Freight and courier
                // sites lie east of the viewer, so their poses sit on the west side.
                day.set_day_index(1);day.set_time_of_day(0.45);
                let cfg=w.sample_config();
                let (cell,reach)=match command {
                    "corridor-pop-near"=>(glam::IVec2::ZERO,5500.0),
                    "corridor-pop-far"=>(glam::IVec2::ZERO,6600.0),
                    "freight-pop-near"=>(glam::IVec2::new(3,-1),-5500.0),
                    "freight-pop-far"=>(glam::IVec2::new(3,-1),-6600.0),
                    "courier-pop-far"=>(glam::IVec2::new(3,0),-6600.0),
                    _=>(glam::IVec2::ZERO,9000.0),
                };
                let site=world::orbital::site(w.seed,cell).unwrap();
                let (x,z)=(site.x+reach,site.z);
                let ground=cfg.world_y(world::terrain_height(&cfg,(x*cfg.inv_voxel_meters()).floor() as i32,(z*cfg.inv_voxel_meters()).floor() as i32)).max(cfg.world_y(cfg.water_level()));
                p.pos=Vec3::new(x,ground+3.0,z);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
                let target=Vec3::new(site.x,world::orbital::hold_height(&cfg,&w.orbital,site)+9000.0,site.z);
                let d=target-p.pos;
                p.yaw=d.x.atan2(-d.z);p.pitch=d.y.atan2(Vec3::new(d.x,0.0,d.z).length());
                creative.show_palette=false;
                self.report(w,text);
            }
            "base-courier-near" | "base-freight-near" => {
                // Ground-level view 60 m west of a corridor footing, aimed at the plinth.
                day.set_day_index(1);day.set_time_of_day(0.45);
                let cfg=w.sample_config();
                let cell=if command=="base-courier-near" {glam::IVec2::new(3,0)} else {glam::IVec2::new(3,-1)};
                let site=world::orbital::site(w.seed,cell).unwrap();
                let footing=world::orbital::footing_height(&cfg,&w.orbital,site);
                let (x,z)=(site.x-60.0,site.z);
                let ground=cfg.world_y(world::terrain_height(&cfg,(x*cfg.inv_voxel_meters()).floor() as i32,(z*cfg.inv_voxel_meters()).floor() as i32)).max(cfg.world_y(cfg.water_level()));
                p.pos=Vec3::new(x,ground+2.0,z);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
                let target=Vec3::new(site.x,footing+10.0,site.z);
                let d=target-p.pos;
                p.yaw=d.x.atan2(-d.z);p.pitch=d.y.atan2(Vec3::new(d.x,0.0,d.z).length());
                creative.show_palette=false;
                self.report(w,text);
            }
            "megastructure-foundry" | "megastructure-habitat" | "megastructure-mirror" | "megastructure-relay" | "megastructure-crown"
            | "megastructure-foundry-detail" | "megastructure-habitat-detail" | "megastructure-mirror-detail" => {
                let cfg=w.sample_config();
                let (x,z)=if command=="megastructure-habitat" {(-2500.0_f32,-1400.0_f32)} else {(1400.0_f32,-1200.0_f32)};
                let height=cfg.world_y(world::terrain_height(&cfg,(x*cfg.inv_voxel_meters()).floor() as i32,(z*cfg.inv_voxel_meters()).floor() as i32));
                p.pos=Vec3::new(x,height.max(cfg.world_y(cfg.water_level()))+3.0,z);
                let mut target=if command=="megastructure-crown" {
                    let site=world::orbital::site(w.seed,glam::IVec2::ZERO).unwrap();
                    world::orbital::crown_position(site,world::orbital::hold_height(&cfg,&w.orbital,site))
                } else {
                    use world::orbital::megastructures::Kind;
                    let want=match command {"megastructure-habitat" | "megastructure-habitat-detail"=>Kind::TwinHabitat,"megastructure-mirror" | "megastructure-mirror-detail"=>Kind::MirrorArray,"megastructure-relay"=>Kind::DeadRelay,_=>Kind::Foundry};
                    let night=day.lighting().sun_dir.y<0.0;
                    let mut best:Option<(f64,world::orbital::megastructures::Contact,f32)>=None;
                    let start=(world::orbital::megastructures::seconds(day)
                        /world::orbital::megastructures::DAY_SECONDS).floor();
                    for minute in 0..1440 {
                        let at=start+minute as f64/1440.0;
                        let mut clock=*day;clock.set_day_index(at.floor() as u32);clock.set_time_of_day(at.fract() as f32);
                        if (clock.lighting().sun_dir.y<0.0)!=night {continue;}
                        for c in world::orbital::megastructures::contacts(w.seed,&clock,p.pos) {
                            if std::mem::discriminant(&c.site.kind)!=std::mem::discriminant(&want) {continue;}
                            let score=c.elevation_deg+c.site.sunlight*35.0;
                            if best.as_ref().is_none_or(|(_,_,s)|score>*s) {best=Some((at,c,score));}
                        }
                    }
                    let Some((at,c,_))=best else {
                        println!("[orbit-review] no qualifying pass in the requested day; camera unchanged");
                        self.report(w,text);
                        return (false,None);
                    };
                    day.set_day_index(at.floor() as u32);day.set_time_of_day(at.fract() as f32);
                    self.orbit_review=Some((c.site.key,at));
                    println!("[orbit-review] shot=peak key={:08X} seconds={:.3} elevation={:.3} slant_m={:.1} altitude_m={:.1} period_s={:.3} sunlight={:.3}",
                        c.site.key,world::orbital::megastructures::seconds(day),c.elevation_deg,c.distance_m,c.site.kind.altitude_m(),c.period_seconds,c.site.sunlight);
                    c.site.position
                };
                let detail=command.ends_with("-detail");
                if detail {
                    use world::orbital::megastructures::{self,Kind};
                    let want=if command.contains("mirror") {Kind::MirrorArray}
                        else if command.contains("habitat") {Kind::TwinHabitat} else {Kind::Foundry};
                    // Separate inspection view: true hardware scale, nearby
                    // camera. Reproject after relocation into its tangent frame.
                    p.pos=target+Vec3::new(0.7,0.35,0.8).normalize()*want.bound_m() as f32*2.8;
                    if let Some(c)=megastructures::contacts(w.seed,day,p.pos).into_iter().find(|c|c.site.kind==want) {
                        target=c.site.position;
                    }
                }
                p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;
                let d=target-p.pos;
                p.yaw=d.x.atan2(-d.z);p.pitch=d.y.atan2(Vec3::new(d.x,0.0,d.z).length());
                creative.show_palette=false;
                println!("[megastructure-review] shot={command} ground_camera={} pos={:?} target={target:?} time={:.4} indirect={}",!detail,p.pos,day.time_of_day(),crate::render::indirect_lighting_enabled());
                Self::record_orbital_pose(day,p);
                self.report(w,text);
            }
            "orbital-night" => {
                day.set_day_index(1); day.set_time_of_day(0.83);
                let answer=w.orbital.request(w.seed,day,Vec3::new(1100.0,97.0,-900.0),glam::IVec2::ZERO,world::orbital::Channel::Radio);
                println!("[receiver-review] night_booking={answer}");self.report(w,text);
            }
            "orbital-ground" | "orbital-anchorage" | "orbital-anchorage-sweep" | "orbital-rim-detail" | "orbital-detail" => {
                let contact = world::orbital::contacts(w.seed,day,&w.orbital,Vec3::new(1100.0,0.0,-900.0))
                    .into_iter().find(|c| c.service.site.cell == glam::IVec2::ZERO).unwrap();
                let site = contact.service.site;
                let at = contact.service.handoff_day
                    - if command == "orbital-anchorage-sweep" {0.066} else {0.0};
                day.set_day_index(at.floor() as u32); day.set_time_of_day(at.fract() as f32);
                day.set_day_length_seconds(600.0);
                let height = world::orbital::hold_height(&w.sample_config(),&w.orbital,site);
                let rim=command=="orbital-rim-detail";
                if rim {day.set_time_of_day(0.36);}
                let upper = command.starts_with("orbital-anchorage") || rim;
                let target = if upper {
                    Vec3::new(site.x+650.0,height+world::orbital::UPPER_ANCHORAGE_OFFSET_M,site.z+260.0)
                } else { Vec3::new(site.x,height,site.z) }+if rim {Vec3::X*site.class.upper_ring_radius_m()} else {Vec3::ZERO};
                let offset = if rim {Vec3::new(65.0,-22.0,80.0)}
                    else if command == "orbital-ground" {Vec3::new(-450.0,-320.0,450.0)}
                    else if upper { Vec3::new(-550.0,-250.0,650.0)*(site.class.upper_ring_radius_m()/210.0).max(1.0) }
                    else { Vec3::new(-55.0,-35.0,65.0) };
                p.pos=target+offset; p.precise_pos=p.pos.as_dvec3(); p.vel=Vec3::ZERO;
                p.yaw=(-offset.x).atan2(offset.z); p.pitch=(-offset.y).atan2(Vec3::new(offset.x,0.0,offset.z).length());
                creative.show_palette=false;
                println!("[orbital-review] shot={command} time={:.4} storm={:.3} indirect={}",
                    day.time_of_day(),day.cloud_controls().cumulonimbus,crate::render::indirect_lighting_enabled());
                self.report(w,text);
            }
            "look-console" => {
                p.pos=Vec3::new(1100.5,97.0,-895.5); p.precise_pos=p.pos.as_dvec3(); p.vel=Vec3::ZERO;
                p.pitch = -0.24; creative.show_palette=true;
                self.report(w,text);
            }
            "hide" => { creative.show_palette=false; }
            "show" => { creative.show_palette=true; }
            "save" => {
                self.expected=Some(Self::signature(w));let mut radio=Vec::new();w.orbital.encode(&mut radio,world::storage::SAVE_VERSION);self.radio_expected=Some(radio);
                saves.request(true); self.report(w,text);
            }
            "reload" => {
                if saves.failed { println!("[receiver-review] checkpoint_failed={:?}",saves.status); }
                else { match world::storage::load_review_checkpoint() {
                    Ok(saved) => {
                        *w=saved.world; *p=saved.player; *day=saved.day_night; reloaded=true;
                        w.orbital.dynamics.manual_clock=true;
                        println!("[receiver-review] reload_exact={} version={}",self.expected==Some(Self::signature(w)),world::storage::SAVE_VERSION);
                        let mut radio=Vec::new();w.orbital.encode(&mut radio,world::storage::SAVE_VERSION);
                        println!("[radio-review] progression_reload_exact={}",self.radio_expected.as_ref()==Some(&radio));
                        self.report(w,text);
                    }
                    Err(e) => println!("[receiver-review] reload_error={e:#}"),
                }}
            }
            "duplicates" => {
                let before=Self::signature(w);
                let inventory=w.inventory.clone();
                for _ in 0..3 {
                    changes.extend(receiver::action(w,day,p,Action::Collect));
                    changes.extend(receiver::action(w,day,p,Action::Order));
                    changes.extend(receiver::action(w,day,p,Action::Deploy));
                }
                println!("[receiver-review] duplicate_requests_rejected={}",before.0==2 && before==Self::signature(w)
                    && inventory.hotbar==w.inventory.hotbar && inventory.pack==w.inventory.pack);
                self.report(w,text);
            }
            "restore" => {
                let before=Self::signature(w);
                let iron=w.inventory.count_item(Item::Resource(8));
                if let Some(a)=w.orbital.receiver.anchor {
                    w.set(a,B::Air); changes.push(a);
                    changes.extend(receiver::action(w,day,p,Action::Restore));
                    changes.extend(receiver::action(w,day,p,Action::Collect));
                    println!("[receiver-review] restore_preserves_completion={}",receiver::intact(w) && before.0==2
                        && before==Self::signature(w) && w.inventory.count_item(Item::Resource(8))==iron.saturating_sub(2));
                }
                self.report(w,text);
            }
            "lift-pose" => {
                let cfg=w.sample_config();let a=engine_worldgen::meridian_terminal::console(&cfg);
                p.pos=a.as_vec3()+Vec3::new(0.5,0.0,3.5);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;p.flying=false;
                p.yaw=0.35;p.pitch=-0.12;creative.show_palette=false;
                println!("[lift-review] posed=true");
            }
            "lift-board" => {
                let cfg=w.sample_config();let a=engine_worldgen::meridian_terminal::console(&cfg);
                if w.orbital.receiver.anchor.is_none() { changes.extend(receiver::action(w,day,p,Action::Deploy)); }
                p.pos=a.as_vec3()+Vec3::new(0.5,0.0,3.5);p.precise_pos=p.pos.as_dvec3();p.vel=Vec3::ZERO;p.flying=false;
                changes.extend(receiver::action(w,day,p,Action::Board));
                // Face the guide-side window and the panel from the far corner.
                p.yaw=0.35;p.pitch=-0.12;
                creative.show_palette=false;
                let v=world::orbital::express::view(w,p,day);
                println!("[lift-review] boarded={} route_m={:.1} rated_g={:.3} rated_mps={:.1}",v.riding,v.route_m,v.rated_g,v.rated_mps);
            }
            "lift-time" => {
                let seconds:f64=text.split_whitespace().nth(2).and_then(|t|t.parse().ok()).unwrap_or(0.0);
                if let Some(r)=w.orbital.express.ride.as_mut() {
                    r.ticks=(seconds/crate::simulation::STEP_SECONDS as f64).round() as u64;
                    let t=r.telemetry();
                    println!("[lift-review] trip_s={seconds:.1} phase={} speed_mps={:.1} drive_g={:+.3} cabin_g={:.3} altitude_m={:.1} arrival_s={:.1} cabin={:?}",
                        t.phase.label(),t.speed_mps,t.drive_g,t.cabin_g,t.altitude_m,t.arrival_s,r.cabin_m());
                } else { println!("[lift-review] no_ride"); }
            }
            "lift-descend" => {
                let ok=world::orbital::express::descend(w);
                println!("[lift-review] descend={ok}");
            }
            "lift-walk" => {
                // Places the passenger in the lift frame: cabin or deck by position.
                let x:f32=text.split_whitespace().nth(2).and_then(|t|t.parse().ok()).unwrap_or(0.0);
                let z:f32=text.split_whitespace().nth(3).and_then(|t|t.parse().ok()).unwrap_or(0.0);
                if let Some(r)=w.orbital.express.ride.as_mut() {
                    r.local_m=Vec3::new(x,0.0,z);r.local_vel=Vec3::ZERO;
                    r.deck=x.abs()>world::orbital::express::HALF_WIDTH_M||z.abs()>world::orbital::express::HALF_WIDTH_M;
                    if r.deck {
                        p.precise_pos=(r.cabin_m()+r.local_m.as_dvec3())/w.voxel_scale.meters() as f64;
                        p.pos=p.precise_pos.as_vec3();p.vel=Vec3::ZERO;
                    }
                    println!("[lift-review] walked=({x:.1},{z:.1}) deck={} hub_m={:.1}",r.deck,r.hub_m);
                } else { println!("[lift-review] no_ride"); }
            }
            "lift-outside" => {
                // Hover beside the cabin, outside the frame, to see the shoe on the braid.
                if let Some(r)=w.orbital.express.ride.as_mut() {
                    r.deck=true;r.local_m=Vec3::new(-11.0,1.5,9.0);r.local_vel=Vec3::ZERO;
                    p.precise_pos=(r.cabin_m()+r.local_m.as_dvec3())/w.voxel_scale.meters() as f64;
                    p.pos=p.precise_pos.as_vec3();p.vel=Vec3::ZERO;p.flying=true;
                    p.yaw=0.88;p.pitch=-0.08;
                    println!("[lift-review] outside=true cabin={:?}",r.cabin_m());
                } else { println!("[lift-review] no_ride"); }
            }
            "lift-look" => {
                let yaw:f32=text.split_whitespace().nth(2).and_then(|t|t.parse().ok()).unwrap_or(0.0);
                let pitch:f32=text.split_whitespace().nth(3).and_then(|t|t.parse().ok()).unwrap_or(0.0);
                p.yaw=yaw;p.pitch=pitch;
            }
            "habitat-status" => {
                let total=w.orbital.habitats.habitats.len();
                let ready=w.orbital.habitats.habitats.iter().filter(|h|h.ready&&h.gas.iter().all(|g|g[0]>=0.0)).count();
                println!("[habitat-review] total={total} ready={ready} pressure={:?} origin={:?}",
                    world::orbital::habitat::readings(w,p),w.orbital.habitats.habitats.first().map(|h|h.origin));
            }
            "capture" => { return (false,Some(format!("{}.png",text.split_whitespace().next().unwrap_or("review")))); }
            _ => println!("[receiver-review] unknown_command={text}"),
        }
        for cell in changes { dirty.insert(cell); world::mark_chunk_and_neighbors_dirty(chunks,cell); }
        (reloaded,None)
    }
}
