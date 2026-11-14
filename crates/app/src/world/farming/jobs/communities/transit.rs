//! Travellers retain their worker body and pack. Arrival, never elapsed time,
//! permits exchange or transfer to a new settlement.
use super::*;
use super::super::{B, Task, known};
use crate::world::travel::civilization as regional;
use engine_worldgen::civilization as geo;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(in crate::world::farming::jobs) enum Purpose {
    Settle,
    Migrate,
    Survey,
    Exchange { payment: ItemStack, purchase: ItemStack },
    Raid,
    Campaign,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::world::farming::jobs) struct Mission {
    pub source: u32,
    pub target: u32,
    pub purpose: Purpose,
    pub cargo: Vec<ItemStack>,
    origin: [f32; 3],
    goal: [f32; 3],
    pub waypoint: [i32; 3],
    pub returning: bool,
    pub arrived: bool,
    next_meal: u64,
    rest_until: u64,
    blocked: u8,
    pub next_action: u64,
    #[serde(default)]
    incident_reported: bool,
    #[serde(default)]
    route_height: u8,
}

impl Mission {
    pub(in crate::world::farming::jobs) fn valid(&self) -> bool {
        (self.source != self.target || matches!(self.purpose,Purpose::Survey))
            && [self.source, self.target].into_iter().all(|id| geo::original_settlement(id) == Some(id))
            && self.route_height <= 12
            && self.cargo.len() <= 12 && self.cargo.iter().all(|s| (1..=256).contains(&s.count))
            && self.cargo.iter().enumerate().all(|(i,s)| !self.cargo[..i].iter().any(|p| p.item == s.item))
            && self.origin.iter().chain(&self.goal).all(|n| n.is_finite() && n.abs() < 1.0e7)
            && self.waypoint.iter().all(|n| n.unsigned_abs() < 1 << 28)
            && match self.purpose {
                Purpose::Exchange { payment, purchase } => payment.item != purchase.item
                    && (1..=64).contains(&payment.count) && (1..=64).contains(&purchase.count),
                _ => true,
            }
    }

    fn destination(&self) -> Vec3 { if self.returning { self.origin.into() } else { self.goal.into() } }

    fn next_waypoint(&mut self, feet: Vec3, scale: f32) {
        let direction = self.destination() - feet;
        let flat = Vec3::new(direction.x, 0.0, direction.z);
        let mut step = flat.clamp_length_max(12.0);
        // A short deterministic local detour, still validated by collision navigation.
        if self.blocked > 1 {
            let angle: f32 = if self.blocked % 2 == 0 { 0.65 } else { -0.65 };
            let (s, c) = angle.sin_cos();
            step = Vec3::new(step.x * c - step.z * s, 0.0, step.x * s + step.z * c);
        }
        self.waypoint = ((feet + step) / scale).floor().as_ivec3().to_array();
    }

    pub(in crate::world::farming::jobs) fn return_to_source(&mut self, feet: Vec3, scale: f32) {
        self.returning = true;
        self.arrived = false;
        self.blocked = 0;
        self.next_waypoint(feet, scale);
    }
}

pub(in crate::world::farming::jobs) fn walk(w: &mut World, feet: Vec3, j: &mut Jobs) -> bool {
    let Some(mut mission) = j.mission.take() else { return false; };
    walk_inner(w, feet, j, &mut mission);
    j.mission = Some(mission);
    true
}

fn walk_inner(w: &mut World, feet: Vec3, j: &mut Jobs, mission: &mut Mission) {
    let now = w.farming.clock;
    let scale = w.voxel_scale.meters();
    if now >= mission.next_meal {
        let meals=now.saturating_sub(mission.next_meal)/24_000+1;
        let available=mission.cargo.iter().filter(|s|s.item==MEAL).map(|s|u64::from(s.count)).sum::<u64>();
        let consumed=meals.min(available) as u32;
        if consumed>0 && custody::pack_take(&mut mission.cargo, ItemStack::new(MEAL, consumed)) {
            super::super::routine::ate(&mut j.routine, now);
        }
        if u64::from(consumed)<meals && !mission.returning {
            mission.return_to_source(feet, scale);
            j.reset_task();
        }
        mission.next_meal = mission.next_meal.saturating_add(meals.saturating_mul(24_000));
    }
    let previous=j.routine.needs_at;
    let elapsed = now.saturating_sub(previous);
    j.routine.needs_at = now;
    j.routine.fatigue = j.routine.fatigue.saturating_add(elapsed);
    let rested=now.min(mission.rest_until).saturating_sub(previous.min(mission.rest_until));
    j.routine.fatigue=j.routine.fatigue.saturating_sub(rested.saturating_mul(12));
    if now < mission.rest_until {
        j.path.clear(); j.status = "Resting";
        super::super::activity::wait(w,j,(feet/scale).floor().as_ivec3(),mission.rest_until.min(mission.next_meal));
        return;
    }
    if j.routine.fatigue > 24_000 {
        mission.rest_until = now.saturating_add(1200);
        j.path.clear(); j.status = "Resting";
        return;
    }
    let arrival_delta = feet - mission.destination();
    let arrived = if matches!(mission.purpose,Purpose::Survey) && !mission.returning {
        arrival_delta.x * arrival_delta.x + arrival_delta.z * arrival_delta.z <= 6.25
    } else { arrival_delta.length_squared() <= 6.25 };
    if arrived {
        mission.arrived = true;
        j.path.clear(); j.task = None; j.status = "Arrived";
        return;
    }
    while j.path.front().is_some_and(|p| (feet - *p).length_squared() < 0.09) {
        j.path.pop_front();
    }
    if !j.path.is_empty() && j.path.front()
        .is_some_and(|p| super::super::stand(w, (*p / scale).floor().as_ivec3(), scale)) {
        j.stalled = if j.last_walk.is_some_and(|p| feet.distance_squared(p) < 0.0025) {
            j.stalled.saturating_add(1)
        } else { 0 };
        j.last_walk = Some(feet);
        if j.stalled < 8 {
            j.status = if mission.returning { "Returning" }
                else if matches!(mission.purpose,Purpose::Survey) { "Scouting" } else { "Travelling" };
            return;
        }
        j.stalled = 0;
        mission.blocked = mission.blocked.saturating_add(1);
        if mission.blocked >= 8 && !mission.returning { mission.return_to_source(feet, scale); }
        else { mission.next_waypoint(feet, scale); }
        j.retry_at = now.saturating_add(PULSE * u64::from(mission.blocked.max(1)));
    }
    j.path.clear();
    if now < j.retry_at { j.status = "Route Blocked"; return; }
    let waypoint = IVec3::from(mission.waypoint);
    if super::super::metres(waypoint, scale).distance_squared(feet) < 4.0 {
        mission.blocked = 0;
        mission.next_waypoint(feet, scale);
        j.task = Some(Task::Routine(mission.waypoint.into()));
        return; // Source preparation admits the next corridor on the following wake.
    }
    let mut found = false;
    // One height candidate and a resumable search per wake, not thirteen full searches.
    for dy in [mission.route_height as i32] {
        let offset = if dy % 2 == 0 { dy / 2 } else { -(dy + 1) / 2 };
        let at = waypoint + IVec3::Y * offset;
        if known(w,at).is_none() || known(w,at-IVec3::Y).is_none() {
            super::super::residency::prepare_cells(w,vec![at,at-IVec3::Y,at+IVec3::Y*(2.0/scale).ceil() as i32]);
            j.status="Awaiting terrain";return;
        }
        if !super::super::stand(w, at, scale) { continue; }
        let task = Task::Routine(at);
        let route = match super::super::navigation::route(w,j,feet,task,768,24.0) {
            super::super::navigation::Route::Ready(path)=>path,
            super::super::navigation::Route::Pending=>{j.task=Some(task);j.status="Finding route";return;}
            super::super::navigation::Route::Blocked=>VecDeque::new(),
        };
        if !route.is_empty() {
            j.path = route; j.task = Some(task); found = true;
            break;
        }
    }
    if !found {
        mission.route_height = (mission.route_height+1)%13;
        if mission.route_height != 0 { j.status="Finding footing"; return; }
        mission.blocked = mission.blocked.saturating_add(1);
        j.retry_at = now.saturating_add(PULSE * u64::from(mission.blocked.min(8)));
        if mission.blocked >= 8 && !mission.returning { mission.return_to_source(feet, scale); }
        else { mission.next_waypoint(feet, scale); }
        j.task = Some(Task::Routine(mission.waypoint.into()));
    }
    j.status = if found { "Travelling" } else { "Route Blocked" };
}

fn available(w: &World, c: &Community, id: u32) -> bool {
    let Some(p) = crew::view(&c.jobs, id) else { return false; };
    !p.mission && p.carry.is_none() && !p.batch && p.shopping.is_none()
        && w.travel.following != Some(id) && !w.travel.guard_posts.contains_key(&id)
        && !w.travel.politics.mobilized(id)
        && p.pos.is_some_and(|at| c.jobs.stockpile.is_some_and(|pile|
            Vec3::from(at).distance_squared(super::super::Task::Store(pile.into()).point(w.voxel_scale.meters())) <= 9.0))
}

fn launch(w: &mut World, source: u32, target: u32, goal: Vec3, purpose: Purpose, cargo: Vec<ItemStack>) -> bool {
    let Some(c) = w.farming.communities.sites.get(&source) else { return false; };
    if c.jobs.production.is_none() || crew::ids(&c.jobs).any(|id| crew::view(&c.jobs,id).is_some_and(|p| p.mission)) { return false; }
    let candidate = crew::ids(&c.jobs).filter(|&id| available(w,c,id)).max_by_key(|&id| {
        match purpose {
            Purpose::Raid => c.people.get(&id).map_or(0, |p| u32::from(p.grievance)),
            _ => engine_core::hash::hash_u32(id ^ target),
        }
    });
    let Some(id) = candidate else { return false; };
    let pile = IVec3::from(c.jobs.stockpile.unwrap());
    let origin = super::super::Task::Store(pile).point(w.voxel_scale.meters());
    let feet = Vec3::from(crew::view(&c.jobs,id).unwrap().pos.unwrap());
    let mut mission = Mission { source, target, purpose, cargo, origin: origin.to_array(), goal: goal.to_array(),
        waypoint: [0;3], returning: false, arrived: false, next_meal: w.farming.clock.saturating_add(24_000),
        rest_until: 0, blocked: 0, next_action: 0, incident_reported: false, route_height: 0 };
    mission.next_waypoint(feet, w.voxel_scale.meters());
    if !mission.valid() || !custody::exchange(w, pile, &mission.cargo, &[]) { return false; }
    let c = w.farming.communities.sites.get_mut(&source).unwrap();
    if matches!(mission.purpose,Purpose::Survey) {
        c.development.scout_routes = c.development.scout_routes.wrapping_add(1);
    }
    crew::set_mission(&mut c.jobs, id, mission);
    c.development.last_expansion = w.farming.clock;
    w.farming.communities.schedule(source, id, w.farming.clock.saturating_add(1));
    true
}

/// Called outside the borrowed executor, so both towns have one accessible owner.
pub(super) fn policy(w: &mut World, source: u32) {
    let Some(c) = w.farming.communities.sites.get(&source) else { return; };
    if c.stage != 2 || w.farming.clock.saturating_sub(c.development.last_expansion) < 2400 { return; }
    let alive = c.people.values().filter(|p| p.died_at.is_none() && !p.enlisted).count();
    let raid_eligible = c.development.shortages >= 12
        && c.people.values().any(|p| p.died_at.is_none() && p.grievance >= 12);
    let seeking_home = c.development.shortages >= 4;
    let needs = c.development.needs.clone();
    if alive == 0 { return; }
    if crew::ids(&c.jobs).any(|id| crew::mission(&c.jobs,id).is_some()) { return; }
    let Some(pile) = stockpile(w, source) else { return; };
    if !custody::ready(w, pile) { return; }
    let stock = w.block_data.chest(pile);
    let bread = production::count(&stock, MEAL);
    let cfg = w.sample_config();
    let Some(from) = geo::settlement_by_id(&cfg,source) else { return; };
    if !geo::prepare_settlements_near(&cfg,from.center,512.0) { return; }
    let nearby = geo::settlements_near(&cfg,from.center,512.0);
    let neighbors: Vec<_> = nearby.iter().copied().filter(|site| site.id != source
        && w.farming.communities.sites.get(&site.id).is_some_and(|c|c.stage==2)).take(16).collect();
    // Migration moves an existing individual into a completed, currently vacant
    // home. Several journeys may compete; arrival rechecks availability.
    if seeking_home && alive >= 2 && bread >= 2 {
        for target in &neighbors {
            if regional::politics::at_war(w,source,target.id) { continue; }
            let Some(c) = w.farming.communities.sites.get(&target.id) else { continue; };
            if c.development.vacancies.is_empty() || c.people.len() >= crew::CAPACITY { continue; }
            let Some(other) = stockpile(w,target.id).filter(|&p| custody::ready(w,p)) else { continue; };
            if production::count(&w.block_data.chest(other),MEAL) < (c.people.len() as u32 + 1) * 4 { continue; }
            if launch(w,source,target.id,super::super::Task::Store(other).point(w.voxel_scale.meters()),
                Purpose::Migrate,vec![ItemStack::new(MEAL,2)]) { return; }
        }
    }
    if bread < alive as u32 * 4 && bread >= 2 {
        for target in &neighbors {
            if regional::politics::at_war(w, source, target.id) { continue; }
            let Some(other) = stockpile(w,target.id).filter(|&p| custody::ready(w,p)) else { continue; };
            let other_stock = w.block_data.chest(other);
            let target_people = population(w,target.id).unwrap_or(0) as u32;
            if production::count(&other_stock,MEAL) < target_people * 4 + 12 { continue; }
            for (item, amount) in [(Item::Block(B::Log),6), (TIMBER,24), (Item::Block(B::Cobblestone),24)] {
                if production::count(&stock,item) < amount + 8 { continue; }
                let payment = ItemStack::new(item,amount);
                if launch(w,source,target.id,super::super::Task::Store(other).point(w.voxel_scale.meters()),
                    Purpose::Exchange { payment, purchase: ItemStack::new(MEAL,12) },
                    vec![payment,ItemStack::new(MEAL,2)]) { return; }
            }
        }
    }
    if bread >= 2 {
        // Trade intermediate goods as well as meals. Each town offers only stock
        // above its own published target, and the destination rechecks on arrival.
        for target in &neighbors {
            if regional::politics::at_war(w,source,target.id) { continue; }
            let Some(other) = stockpile(w,target.id).filter(|&p| custody::ready(w,p)) else { continue; };
            let other_stock = w.block_data.chest(other);
            let other_needs = w.farming.communities.sites[&target.id].development.needs.clone();
            for wanted in needs.iter().filter(|s| production::count(&stock,s.item) < s.count).take(8) {
                let reserve = other_needs.iter().find(|s|s.item==wanted.item).map_or(8,|s|s.count.div_ceil(2))
                    .max(if wanted.item == MEAL { population(w,target.id).unwrap_or(0) as u32 * 4 } else { 0 });
                let amount = production::count(&other_stock,wanted.item).saturating_sub(reserve)
                    .min(wanted.count.saturating_sub(production::count(&stock,wanted.item))).min(12);
                if amount == 0 { continue; }
                for payment in stock.iter().flatten().filter(|s|s.item!=wanted.item && s.item!=MEAL) {
                    let own_reserve = needs.iter().find(|s|s.item==payment.item).map_or(8,|s|s.count.div_ceil(2));
                    let offered = amount.saturating_mul(value(wanted.item)).div_ceil(value(payment.item));
                    if offered == 0 || offered > 64 || production::count(&stock,payment.item) < own_reserve+offered { continue; }
                    let buy = ItemStack::new(wanted.item,amount);
                    let pay = ItemStack::new(payment.item,offered);
                    if launch(w,source,target.id,super::super::Task::Store(other).point(w.voxel_scale.meters()),
                        Purpose::Exchange {payment:pay,purchase:buy},vec![pay,ItemStack::new(MEAL,2)]) { return; }
                }
            }
        }
    }
    // A sustained shortage can produce a particular resident's theft expedition.
    if raid_eligible {
        for target in &neighbors {
            let Some(other) = stockpile(w,target.id).filter(|&p| custody::ready(w,p)) else { continue; };
            if production::count(&w.block_data.chest(other),MEAL) < 8 { continue; }
            if launch(w,source,target.id,super::super::Task::Store(other).point(w.voxel_scale.meters()),
                Purpose::Raid,Vec::new()) { return; }
        }
    }
    if alive >= 4 && bread >= development::provisions(alive) {
      for target in nearby {
        if target.id == source || target.history.households > 0
            || w.travel.settlements.contains_key(&target.id)
            || w.farming.communities.sites.contains_key(&target.id)
            || !matches!(target.history.livelihood,geo::Livelihood::Floodplain | geo::Livelihood::Pasture)
            || !geo::linked(&cfg,from,target)
            || w.farming.communities.sites.len() >= MAX_SITES { continue; }
        let goods = vec![ItemStack::new(Item::Block(B::Chest),1), ItemStack::new(Item::Block(B::Workbench),1),
            ItemStack::new(MEAL,12),ItemStack::new(Item::Block(B::WheatCrop),12),ItemStack::new(TIMBER,48)];
        let pile = ((target.center + Vec3::new(0.0,0.0,2.0)) / w.voxel_scale.meters()).floor().as_ivec3();
        if launch(w,source,target.id,super::super::Task::Store(pile).point(w.voxel_scale.meters()),Purpose::Settle,goods) { return; }
      }
    }
    if bread >= alive as u32 * 2 + 2 {
        let n = w.farming.communities.sites[&source].development.scout_routes;
        let angle = (n % 12) as f32 * std::f32::consts::TAU / 12.0;
        let radius = 28.0 + (n / 12 % 3) as f32 * 6.0;
        let goal = from.center + Vec3::new(angle.cos() * radius,0.0,angle.sin() * radius);
        launch(w,source,source,goal,Purpose::Survey,vec![ItemStack::new(MEAL,2)]);
    }
}

fn value(item: Item) -> u32 {
    match item {
        Item::Tool(_) | Item::Weapon(_) | Item::Armor(..) => 16,
        Item::Resource(_) | Item::Block(B::Log) => 4,
        Item::Food(_) => 2,
        _ => 1,
    }
}

/// The pack is the sole owner of expedition provisions. An inherited position
/// transfers an old campaign's already paid supply; it never debits it again.
pub(crate) fn muster(w: &mut World, source: u32, target: u32, id: u32, supply: u8,
    inherited: Option<Vec3>) -> bool {
    let (Some(pile),Some(destination))=(stockpile(w,source),stockpile(w,target)) else {return false;};
    let Some(c)=w.farming.communities.sites.get(&source) else {return false;};
    let Some(view)=crew::view(&c.jobs,id) else {return false;};
    if view.mission || view.carry.is_some() || view.batch || view.shopping.is_some() {return false;}
    let origin=super::super::Task::Store(pile).point(w.voxel_scale.meters());
    let Some(feet)=inherited.or_else(||crew::physical(w,&c.jobs,id)) else {return false;};
    if inherited.is_none() && feet.distance_squared(origin)>9.0 {return false;}
    let mut cargo=Vec::new();
    if supply>0 {cargo.push(ItemStack::new(MEAL,u32::from(supply)));}
    if inherited.is_none() {cargo.push(ItemStack::new(TIMBER,4));}
    let mut mission=Mission {source,target,purpose:Purpose::Campaign,cargo,
        origin:origin.to_array(),goal:super::super::Task::Store(destination).point(w.voxel_scale.meters()).to_array(),
        waypoint:[0;3],returning:false,arrived:false,next_meal:w.farming.clock.saturating_add(24_000),
        rest_until:0,blocked:0,next_action:0,incident_reported:false,route_height:0};
    mission.next_waypoint(feet,w.voxel_scale.meters());
    if !mission.valid() || (inherited.is_none() && !custody::exchange(w,pile,&mission.cargo,&[])) {return false;}
    let c=w.farming.communities.sites.get_mut(&source).unwrap();
    if inherited.is_some() {crew::set_position(&mut c.jobs,id,feet,w.farming.clock);}
    crew::set_mission(&mut c.jobs,id,mission);
    w.farming.communities.schedule(source,id,w.farming.clock.saturating_add(1));
    true
}

pub(crate) fn campaign_state(w: &World, id: u32) -> Option<(bool,bool,u32)> {
    let j=super::super::resident_state(w,id)?;
    let m=crew::mission(j,id).filter(|m|matches!(m.purpose,Purpose::Campaign))?;
    Some((m.arrived,m.returning,m.cargo.iter().filter(|s|s.item==MEAL).map(|s|s.count).sum()))
}

pub(crate) fn recall(w: &mut World, source: u32, id: u32) {
    interrupt_activity(w,id);
    let Some(c)=w.farming.communities.sites.get(&source) else {return;};
    let Some(m)=crew::mission(&c.jobs,id).filter(|m|!m.returning).cloned() else {return;};
    let Some(feet)=crew::physical(w,&c.jobs,id) else {return;};
    return_pack(w,source,id,m,feet);
}

pub(crate) enum Siege { Waiting, Contact, Breach, Returning }

pub(crate) fn siege(w: &mut World, source: u32, id: u32, target: u32) -> Siege {
    let Some(c)=w.farming.communities.sites.get(&source) else {return Siege::Waiting;};
    let Some(mut mission)=crew::mission(&c.jobs,id).cloned() else {return Siege::Waiting;};
    let Some(feet)=crew::physical(w,&c.jobs,id) else {return Siege::Waiting;};
    let Some(pile)=stockpile(w,target).filter(|&p|custody::ready(w,p)) else {return Siege::Waiting;};
    if !mission.arrived || mission.returning
        || feet.distance_squared(super::super::Task::Store(pile).point(w.voxel_scale.meters()))>9.0 {return Siege::Waiting;}
    let defender=w.farming.communities.sites.get(&target).and_then(|c|
        crew::ids(&c.jobs).filter(|&other|other!=id).find_map(|other|
            crew::physical(w,&c.jobs,other).filter(|&at|at.distance_squared(feet)<=9.0
                && contact_clear(w,feet,at)).map(|at|(other,at))));
    if let Some((defender,at))=defender {
        for (person,position,damage) in [(id,feet,2.0),(defender,at,1.0)] {
            let health=vehicle::villager_actor(w,person).map(|e|e.health)
                .or_else(||crew::body(w,person).map(|b|b.health())).unwrap_or(0.0);
            let next=(health-damage).max(0.0);
            set_health(w,person,next);
            if let Some(actor)=w.adventure.enemies.iter_mut().find(|e|vehicle::villager_id(e)==Some(person)) {actor.health=next;}
            if next==0.0 {super::super::worker_fell(w,person,position);}
        }
        return Siege::Contact;
    }
    if !custody::pack_take(&mut mission.cargo,ItemStack::new(TIMBER,1)) {
        return_pack(w,source,id,mission,feet);
        return Siege::Returning;
    }
    if let Some(c)=w.farming.communities.sites.get_mut(&source) {crew::set_mission(&mut c.jobs,id,mission);}
    Siege::Breach
}

fn return_pack(w: &mut World, source: u32, id: u32, mut mission: Mission, feet: Vec3) {
    mission.return_to_source(feet,w.voxel_scale.meters());
    if let Some(c) = w.farming.communities.sites.get_mut(&source) {
        crew::set_mission(&mut c.jobs,id,mission);
    }
}

pub(super) fn arrive(w: &mut World, source: u32, id: u32) {
    let Some(c) = w.farming.communities.sites.get(&source) else { return; };
    let Some(mission) = crew::mission(&c.jobs,id).filter(|m| m.arrived).cloned() else { return; };
    let Some(feet) = crew::view(&c.jobs,id).and_then(|p| p.pos).map(Vec3::from) else { return; };
    if mission.returning {
        let Some(pile) = stockpile(w,source) else { return; };
        if feet.distance_squared(super::super::Task::Store(pile).point(w.voxel_scale.meters())) > 9.0
            || !custody::exchange(w,pile,&[],&mission.cargo) { return; }
        if let Some(c) = w.farming.communities.sites.get_mut(&source) { crew::end_mission(&mut c.jobs,id); }
        return;
    }
    match mission.purpose {
        Purpose::Exchange { payment, purchase } => {
            let Some(pile) = stockpile(w,mission.target) else { return_pack(w,source,id,mission,feet); return; };
            let mut pack = mission.cargo.clone();
            let reserve = w.farming.communities.sites.get(&mission.target).map_or(8,|c|
                c.development.reserve(purchase.item).max(if purchase.item == MEAL { c.people.len() as u32 * 4 } else { 0 }));
            if feet.distance_squared(super::super::Task::Store(pile).point(w.voxel_scale.meters())) <= 9.0
                && !regional::politics::at_war(w,source,mission.target)
                && production::count(&w.block_data.chest(pile),purchase.item) >= reserve.saturating_add(purchase.count)
                && custody::pack_take(&mut pack,payment) && custody::exchange(w,pile,&[purchase],&[payment]) {
                custody::pack_add(&mut pack,purchase);
                regional::politics::trade_completed(w,source,mission.target);
                let mut returned = mission;
                returned.cargo = pack;
                return_pack(w,source,id,returned,feet);
            } else { return_pack(w,source,id,mission,feet); }
        }
        Purpose::Settle => settle(w,source,id,mission,feet),
        Purpose::Migrate => migrate(w,source,id,mission,feet),
        Purpose::Campaign => {}, // The political owner resolves contact at the actual destination.
        Purpose::Survey => {
            let observed = with_site(w,source,|w,city| {
                let Some(pile) = w.farming.jobs.stockpile else { return false; };
                development::observe(w,city,pile.into(),feet)
            }).unwrap_or(false);
            if observed { return_pack(w,source,id,mission,feet); }
        },
        Purpose::Raid => raid(w,source,id,mission,feet),
    }
}

fn settle(w: &mut World, source: u32, id: u32, mission: Mission, feet: Vec3) {
    let cfg = w.sample_config();
    let Some(site) = geo::settlement_by_id(&cfg,mission.target) else { return_pack(w,source,id,mission,feet); return; };
    if w.farming.communities.sites.contains_key(&site.id) || w.travel.settlements.contains_key(&site.id)
        || w.farming.communities.sites.len() >= MAX_SITES
        || w.travel.settlements.len() >= regional::MAX_SETTLEMENTS || site.history.households != 0 {
        return_pack(w,source,id,mission,feet); return;
    }
    let pile = ((site.center + Vec3::new(0.0,0.0,2.0))/w.voxel_scale.meters()).floor().as_ivec3();
    if !super::super::residency::prepare_cells(w,vec![pile,pile-IVec3::Y,pile+IVec3::X]) { return; }
    if known(w,pile) != Some(B::Air) || known(w,pile+IVec3::X) != Some(B::Air)
        || !known(w,pile-IVec3::Y).is_some_and(B::is_solid)
        || crate::world::farming::base::overlaps(w,pile,pile+IVec3::X) {
        return_pack(w,source,id,mission,feet); return;
    }
    let mut target = founding::plan(w,site);
    let origin = w.farming.communities.sites.get_mut(&source).unwrap();
    let Some(mut person) = origin.people.remove(&id) else { return; };
    let Some(mut worker) = crew::take_worker(&mut origin.jobs,id) else {
        origin.people.insert(id,person); return;
    };
    origin.jobs.homes.remove(&id);
    if person.home.entry != person.home.destination && origin.development.vacancies.len() < 4 {
        origin.development.vacancies.push(person.home.clone());
    }
    restaff(&mut origin.jobs,&w.travel);
    let at = (feet / w.voxel_scale.meters()).floor().as_ivec3().to_array();
    person.home = Home { destination: at, entry: at };
    target.jobs.homes.insert(id,person.home.clone());
    target.people.insert(id,person);
    worker.mission = None;
    worker.reset();
    crew::put_worker(&mut target.jobs,worker);
    target.starter = Some(mission.cargo);
    w.farming.communities.cancel(source,id);
    w.farming.communities.owners.insert(id,site.id);
    w.farming.communities.sites.insert(site.id,target);
    w.farming.communities.schedule(site.id,FOUNDING,w.farming.clock.saturating_add(1));
    regional::found_community(w,source,site,id);
}

fn migrate(w: &mut World, source: u32, id: u32, mission: Mission, feet: Vec3) {
    let Some(target) = w.farming.communities.sites.get(&mission.target) else {
        return_pack(w,source,id,mission,feet); return;
    };
    let vacancy = target.development.vacancies.last().cloned();
    if target.people.len() >= crew::CAPACITY || target.stage != 2 || vacancy.is_none()
        || regional::politics::at_war(w,source,mission.target) {
        return_pack(w,source,id,mission,feet); return;
    }
    let home = vacancy.unwrap();
    let Some(pile) = stockpile(w,mission.target) else { return; };
    let mut cells = vec![pile];
    let head = (1.8 / w.voxel_scale.meters()).ceil() as i32;
    for at in [IVec3::from(home.entry),IVec3::from(home.destination)] {
        cells.extend([at-IVec3::Y,at,at+IVec3::Y*head]);
    }
    if feet.distance_squared(super::super::Task::Store(pile).point(w.voxel_scale.meters())) > 9.0
        || !super::super::residency::prepare_cells(w,cells) { return; }
    if !super::super::stand(w,home.entry.into(),w.voxel_scale.meters())
        || !super::super::stand(w,home.destination.into(),w.voxel_scale.meters()) {
        return_pack(w,source,id,mission,feet); return;
    }
    // Preflight the only fallible stock mutation before transferring either owner.
    let transferable = w.farming.communities.sites.get(&source).is_some_and(|c|
        c.people.contains_key(&id) && crew::view(&c.jobs,id).is_some());
    if !transferable || !custody::exchange(w,pile,&[],&mission.cargo) { return; }
    let origin = w.farming.communities.sites.get_mut(&source).unwrap();
    let mut person = origin.people.remove(&id).expect("preflighted migrant");
    let mut worker = crew::take_worker(&mut origin.jobs,id).expect("preflighted worker");
    origin.jobs.homes.remove(&id);
    if person.home.entry != person.home.destination && origin.development.vacancies.len() < 4 {
        origin.development.vacancies.push(person.home.clone());
    }
    restaff(&mut origin.jobs,&w.travel);
    worker.mission = None;
    worker.reset();
    person.home = home.clone();
    person.grievance /= 2;
    let target = w.farming.communities.sites.get_mut(&mission.target).unwrap();
    target.development.vacancies.pop();
    target.people.insert(id,person);
    target.jobs.homes.insert(id,home);
    crew::put_worker(&mut target.jobs,worker);
    restaff(&mut target.jobs,&w.travel);
    w.farming.communities.cancel(source,id);
    w.farming.communities.owners.insert(id,mission.target);
    w.farming.communities.schedule(mission.target,id,w.farming.clock.saturating_add(1));
    regional::moved_resident(w,source,mission.target);
}

fn raid(w: &mut World, source: u32, id: u32, mut mission: Mission, feet: Vec3) {
    let Some(pile) = stockpile(w,mission.target) else { return_pack(w,source,id,mission,feet); return; };
    if feet.distance_squared(super::super::Task::Store(pile).point(w.voxel_scale.meters())) > 9.0 { return; }
    if w.farming.clock < mission.next_action { return; }
    let defender = w.farming.communities.sites.get(&mission.target).and_then(|c|
        crew::ids(&c.jobs).find(|&other| crew::view(&c.jobs,other).and_then(|p| p.pos)
            .is_some_and(|p| Vec3::from(p).distance_squared(feet) <= 9.0
                && contact_clear(w,feet,Vec3::from(p)))));
    if let Some(defender) = defender {
        // These are individual nonlethal scuffles; lethal campaigns keep their
        // existing combat owner. Damage is written to the same physical body.
        let healthy = scuffle(w,id,2.0);
        scuffle(w,defender,1.0);
        if !mission.incident_reported {
            regional::politics::raid_recorded(w,source,mission.target);
            mission.incident_reported = true;
        }
        if !healthy { return_pack(w,source,id,mission,feet); return; }
        mission.next_action = w.farming.clock.saturating_add(240);
        if let Some(c) = w.farming.communities.sites.get_mut(&source) { crew::set_mission(&mut c.jobs,id,mission); }
        return;
    }
    let available = production::count(&w.block_data.chest(pile),MEAL).min(8);
    if available > 0 && custody::exchange(w,pile,&[ItemStack::new(MEAL,available)],&[]) {
        custody::pack_add(&mut mission.cargo,ItemStack::new(MEAL,available));
        if !mission.incident_reported { regional::politics::raid_recorded(w,source,mission.target); }
    }
    return_pack(w,source,id,mission,feet);
}

fn contact_clear(w: &World, a: Vec3, b: Vec3) -> bool {
    let a = a + Vec3::Y;
    let b = b + Vec3::Y;
    let steps = (a.distance(b) / (w.voxel_scale.meters()*0.5)).ceil().max(1.0) as usize;
    (0..=steps).all(|i| {
        let p = (a.lerp(b,i as f32/steps as f32) / w.voxel_scale.meters()).floor().as_ivec3();
        w.resident_block(p).is_some_and(|block| !block.is_solid())
    })
}

fn scuffle(w: &mut World, id: u32, damage: f32) -> bool {
    let Some(health) = vehicle::villager_actor(w,id).map(|e|e.health)
        .or_else(|| crew::body(w,id).map(|b|b.health())) else { return false; };
    if health <= 0.0 { return false; }
    let next = (health - damage).max(health.min(2.0));
    set_health(w,id,next);
    if let Some(actor) = w.adventure.enemies.iter_mut().find(|e|vehicle::villager_id(e)==Some(id)) {
        actor.health = next;
        vehicle::scuffle_reaction(actor);
    }
    next > 4.0
}
