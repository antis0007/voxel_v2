//! Bounded public ephemeris forecasts. Cached presentation is never authority
//! for a live uplink or a paid receiver action.
use super::{DayNightCycle, calendar, megastructures::{self, Kind}};
use crate::world::World;
use glam::{IVec3, Vec3};

#[derive(Clone)]
pub struct Forecast {
    pub kind: Kind,
    pub opens: f64,
    pub closes: f64,
    pub peak_degrees: f32,
}
pub(super) struct Cache {
    seed: u32,
    observer: IVec3,
    bucket: u64,
    origin: f64,
    minute: usize,
    entries: Vec<Forecast>,
    active: [Option<Forecast>;4],
    done: [bool;4],
}
impl Cache {
    fn complete(&self)->bool {self.minute>1440 || self.done.iter().all(|v|*v)}
}
pub fn pending(world:&World)->bool {
    world.orbital.pass_cache.borrow().as_ref().is_none_or(|c|!c.complete())
}
pub fn forecast(world: &World, day: &DayNightCycle, viewer: Vec3) -> Vec<Forecast> {
    let now=calendar(day);
    // A 128 m chart cell is small relative to the orbital shells, but lets a
    // walking operator finish a forecast instead of continually restarting it.
    let observer=(viewer/128.0).floor().as_ivec3();
    let bucket=(now*288.0).floor() as u64;
    let mut slot=world.orbital.pass_cache.borrow_mut();
    let valid=slot.as_ref().is_some_and(|c|c.seed==world.seed && c.observer==observer
        && (!c.complete() || (c.bucket==bucket && c.entries.iter().all(|e|e.closes>now))));
    if !valid {
        *slot=Some(Cache {seed:world.seed,observer,bucket,origin:now,minute:0,entries:Vec::with_capacity(4),
            active:std::array::from_fn(|_|None),done:[false;4]});
    }
    let cache=slot.as_mut().unwrap();
    let kinds=[Kind::Foundry,Kind::TwinHabitat,Kind::MirrorArray,Kind::DeadRelay];
    // Bound both sample count and CPU time. Let an in-progress chart finish even
    // if its refresh bucket changes, so slow frames cannot starve completion.
    let started=std::time::Instant::now();
    for sample in 0..32 {
        if sample>0 && started.elapsed()>=std::time::Duration::from_micros(250) {break;}
        if cache.complete() {break;}
        let at=cache.origin+cache.minute as f64/1440.0;
        cache.minute+=1;
        let mut clock=*day;
        clock.set_day_index(at.floor() as u32);
        clock.set_time_of_day(at.fract() as f32);
        let contacts=megastructures::geometric_contacts(world.seed,&clock,viewer);
        for (i,kind) in kinds.iter().enumerate() {
            if cache.done[i] {continue;}
            let visible=contacts.iter().find(|c|c.site.kind==*kind && c.elevation_deg>=15.0);
            if let Some(c)=visible {
                let event=cache.active[i].get_or_insert(Forecast {kind:*kind,opens:at,closes:at,peak_degrees:0.0});
                event.closes=at;
                event.peak_degrees=event.peak_degrees.max(c.elevation_deg);
            } else if let Some(mut event)=cache.active[i].take() {
                event.closes=at;cache.entries.push(event);cache.done[i]=true;
            }
        }
    }
    if cache.complete() {
        for event in &mut cache.active {if let Some(e)=event.take() {cache.entries.push(e);}}
    }
    cache.entries.sort_by(|a,b|a.opens.total_cmp(&b.opens));
    cache.entries.clone()
}
